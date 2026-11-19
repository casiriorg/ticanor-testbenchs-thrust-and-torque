#include "ThrottleController.h"

#include <Arduino.h>

#include "../config/Defaults.h"

namespace tb {

using proto::ActuatorState;

void ThrottleController::begin(uint32_t nowMs) {
  lastHostMs_ = nowMs;
  lastUpdateMs_ = nowMs;
  lastSampleMs_ = nowMs;
  pwm_.begin(config_.pwmFreqHz);
  forceZero(ActuatorState::Disarmed);
}

void ThrottleController::writeOutput() {
  if (state_ != ActuatorState::Armed && config_.disarmedOutput == 1) {
    pwm_.disable();
    return;
  }
  const uint32_t span = static_cast<uint32_t>(config_.pwmMaxUs - config_.pwmMinUs);
  const uint16_t us = static_cast<uint16_t>(
      config_.pwmMinUs + (span * current_) / proto::kThrottleFull);
  pwm_.setPulseUs(us);
}

void ThrottleController::forceZero(ActuatorState next) {
  state_ = next;
  target_ = 0;
  current_ = 0;
  slewRemainder_ = 0;
  samplePending_ = true;
  writeOutput();
}

void ThrottleController::update(uint32_t nowMs) {
  const uint32_t elapsedMs = nowMs - lastUpdateMs_;
  if (elapsedMs == 0) {
    return;
  }
  lastUpdateMs_ = nowMs;
  if (state_ != ActuatorState::Armed) {
    return;
  }

  // Watchdog: the host must keep talking while the motor may be spinning.
  if (nowMs - lastHostMs_ > config_.watchdogMs) {
    if (watchdogTrips_ != 0xFF) {
      ++watchdogTrips_;
    }
    forceZero(ActuatorState::Failsafe);
    return;
  }

  if (current_ == target_) {
    return;
  }
  if (target_ < current_ || config_.slewPerSec == 0) {
    current_ = target_;  // reductions are never delayed
    slewRemainder_ = 0;
  } else {
    // Integer rise-rate limiter; the remainder carries sub-unit steps over.
    const uint32_t budget = static_cast<uint32_t>(config_.slewPerSec) * elapsedMs + slewRemainder_;
    const uint32_t step = budget / 1000UL;
    slewRemainder_ = budget % 1000UL;
    const uint32_t next = static_cast<uint32_t>(current_) + step;
    current_ = next >= target_ ? target_ : static_cast<uint16_t>(next);
  }
  writeOutput();
}

bool ThrottleController::arm(uint32_t nowMs) {
  if (state_ != ActuatorState::Disarmed) {
    return false;
  }
  lastHostMs_ = nowMs;
  forceZero(ActuatorState::Armed);
  return true;
}

void ThrottleController::disarm() {
  // An emergency stop stays latched until it is explicitly cleared.
  if (state_ == ActuatorState::EStop) {
    return;
  }
  forceZero(ActuatorState::Disarmed);
}

void ThrottleController::estop() { forceZero(ActuatorState::EStop); }

bool ThrottleController::clearEstop() {
  if (state_ != ActuatorState::EStop) {
    return false;
  }
  forceZero(ActuatorState::Disarmed);
  return true;
}

bool ThrottleController::setTarget(uint16_t throttle) {
  if (state_ != ActuatorState::Armed || throttle > proto::kThrottleFull) {
    return false;
  }
  target_ = throttle > config_.throttleMax ? config_.throttleMax : throttle;
  return true;
}

bool ThrottleController::applyConfig() {
  if (state_ == ActuatorState::Armed) {
    return false;
  }
  pwm_.setFrequency(config_.pwmFreqHz);
  writeOutput();
  return true;
}

bool ThrottleController::pollSample(uint32_t nowMs, Sample& out) {
  const bool periodic = state_ == ActuatorState::Armed &&
                        nowMs - lastSampleMs_ >= defaults::kThrottleSamplePeriodMs;
  if (!samplePending_ && !periodic) {
    return false;
  }
  samplePending_ = false;
  lastSampleMs_ = nowMs;
  out.timeUs = micros();
  out.channel = channel::kThrottle;
  out.value = current_;
  return true;
}

bool ThrottleChannel::poll(Sample& out) { return controller_.pollSample(millis(), out); }

void ThrottleChannel::describe(ChannelInfo& info) const {
  info.id = channel::kThrottle;
  info.sensorType = static_cast<uint8_t>(proto::SensorType::ThrottleCommand);
  info.unit = static_cast<uint8_t>(proto::Unit::ThrottleFraction);
  info.flags = proto::kChanEnabled | proto::kChanCalibrated;
  info.rateHz = static_cast<uint16_t>(1000 / defaults::kThrottleSamplePeriodMs);
  info.gain = 0;
  info.offset = 0;
  info.scale = static_cast<float>(proto::kThrottleFull);
  info.calTime = 0;
  setChannelName(info, "throttle");
}

}  // namespace tb
