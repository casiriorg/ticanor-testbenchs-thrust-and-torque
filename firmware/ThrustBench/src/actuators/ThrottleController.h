// ESC throttle with the safety rules enforced on the device itself, so they
// hold even if the PC application hangs, crashes or the cable is pulled.
//
// State machine:
//   Disarmed --arm()--> Armed --disarm()--> Disarmed
//   Armed --host silent > watchdogMs--> Failsafe --disarm()--> Disarmed
//   any --estop()--> EStop --clearEstop()--> Disarmed
//
// Throttle is only ever non-zero in Armed. Leaving Armed forces it to zero
// immediately, bypassing the rise-rate limit.
#pragma once

#include <stdint.h>

#include "../config/ConfigStore.h"
#include "../hal/PwmOutput.h"
#include "../protocol/Protocol.h"
#include "../sensors/Sensor.h"

namespace tb {

class ThrottleController {
 public:
  ThrottleController(PwmOutput& pwm, const GlobalConfig& config) : pwm_(pwm), config_(config) {}

  void begin(uint32_t nowMs);

  // Call every loop pass. Runs the watchdog and the rise-rate limiter.
  void update(uint32_t nowMs);

  // Any valid frame from the host counts as a sign of life.
  void notifyHostAlive(uint32_t nowMs) { lastHostMs_ = nowMs; }

  bool arm(uint32_t nowMs);
  void disarm();
  void estop();
  bool clearEstop();

  // Requests a throttle (0..kThrottleFull). Rejected unless armed; values
  // above the configured ceiling are clamped. Returns false when rejected.
  bool setTarget(uint16_t throttle);

  // Applies changed PWM settings. Only legal while not armed.
  bool applyConfig();

  proto::ActuatorState state() const { return state_; }
  uint16_t throttle() const { return current_; }
  uint16_t pulseUs() const { return pwm_.pulseUs(); }
  uint8_t watchdogTrips() const { return watchdogTrips_; }

  // Emits the applied throttle as a data channel: every
  // kThrottleSamplePeriodMs while armed and once on every state change, so
  // recordings carry the command on the same clock as the load cells.
  bool pollSample(uint32_t nowMs, Sample& out);

 private:
  void writeOutput();
  void forceZero(proto::ActuatorState next);

  PwmOutput& pwm_;
  const GlobalConfig& config_;
  proto::ActuatorState state_ = proto::ActuatorState::Disarmed;
  uint16_t target_ = 0;
  uint16_t current_ = 0;
  uint32_t lastHostMs_ = 0;
  uint32_t lastUpdateMs_ = 0;
  uint32_t lastSampleMs_ = 0;
  uint32_t slewRemainder_ = 0;
  uint8_t watchdogTrips_ = 0;
  bool samplePending_ = true;
};

// Adapter that exposes the applied throttle through the Sensor interface.
class ThrottleChannel : public Sensor {
 public:
  explicit ThrottleChannel(ThrottleController& controller) : controller_(controller) {}

  void begin() override {}
  void applyConfig() override {}
  bool poll(Sample& out) override;
  void describe(ChannelInfo& info) const override;
  uint8_t channelId() const override { return channel::kThrottle; }

 private:
  ThrottleController& controller_;
};

}  // namespace tb
