#include "CommandHandler.h"

#include <Arduino.h>

#include "../config/BoardPins.h"
#include "Version.h"

namespace tb {

using proto::ChannelField;
using proto::Cmd;
using proto::Err;
using proto::GlobalField;
using proto::Msg;

namespace {

bool isArmed(const ThrottleController& throttle) {
  return throttle.state() == proto::ActuatorState::Armed;
}

// Pins that must never be reassigned as the HX711 RATE output.
bool isFreeDigitalPin(uint32_t pin) {
  if (pin < 2 || pin > 19) {
    return false;
  }
  return pin != pins::kThrustDout && pin != pins::kThrustSck && pin != pins::kTorqueDout &&
         pin != pins::kTorqueSck && pin != pins::kEscPwm;
}

}  // namespace

void CommandHandler::ack(uint8_t type, uint8_t seq) {
  const uint8_t payload[2] = {type, seq};
  ctx_.writer.send(Msg::Ack, payload, sizeof(payload), false);
}

void CommandHandler::nack(uint8_t type, uint8_t seq, Err error) {
  const uint8_t payload[3] = {type, seq, static_cast<uint8_t>(error)};
  ctx_.writer.send(Msg::Nack, payload, sizeof(payload), false);
}

void CommandHandler::sendInfo() {
  uint8_t p[12];
  p[0] = proto::kVersion;
  p[1] = version::kMajor;
  p[2] = version::kMinor;
  p[3] = version::kPatch;
  proto::put32(p + 4, ctx_.config.data().global.deviceUid);
  p[8] = ctx_.sensorCount;
  uint16_t caps = proto::kCapPwmActuator;
  if (ctx_.config.data().global.ratePin != 255) {
    caps |= proto::kCapRatePin;
  }
  proto::put16(p + 9, caps);
  p[11] = ctx_.config.defaultsLoadedAtBoot() ? 1 : 0;
  ctx_.writer.send(Msg::Info, p, sizeof(p), false);
}

void CommandHandler::sendChannel(uint8_t index) {
  ChannelInfo info;
  ctx_.sensors[index]->describe(info);
  uint8_t p[21 + proto::kChannelNameLen];
  p[0] = index;
  p[1] = ctx_.sensorCount;
  p[2] = info.id;
  p[3] = info.sensorType;
  p[4] = info.unit;
  p[5] = info.flags;
  proto::put16(p + 6, info.rateHz);
  p[8] = info.gain;
  proto::put32(p + 9, static_cast<uint32_t>(info.offset));
  proto::putFloat(p + 13, info.scale);
  proto::put32(p + 17, info.calTime);
  memcpy(p + 21, info.name, proto::kChannelNameLen);
  ctx_.writer.send(Msg::Channel, p, sizeof(p), false);
}

void CommandHandler::sendParam(uint8_t scope, uint8_t field, uint32_t rawValue) {
  uint8_t p[6];
  p[0] = scope;
  p[1] = field;
  proto::put32(p + 2, rawValue);
  ctx_.writer.send(Msg::Param, p, sizeof(p), false);
}

ChannelConfig* CommandHandler::channelConfig(uint8_t channelId) {
  DeviceConfig& config = ctx_.config.data();
  if (channelId == channel::kThrust) {
    return &config.loadCells[0];
  }
  if (channelId == channel::kTorque) {
    return &config.loadCells[1];
  }
  return nullptr;
}

bool CommandHandler::readParam(uint8_t scope, uint8_t field, uint32_t& raw) {
  if (scope == proto::kScopeGlobal) {
    const GlobalConfig& g = ctx_.config.data().global;
    switch (static_cast<GlobalField>(field)) {
      case GlobalField::PwmMinUs: raw = g.pwmMinUs; return true;
      case GlobalField::PwmMaxUs: raw = g.pwmMaxUs; return true;
      case GlobalField::PwmFreqHz: raw = g.pwmFreqHz; return true;
      case GlobalField::WatchdogMs: raw = g.watchdogMs; return true;
      case GlobalField::ThrottleMax: raw = g.throttleMax; return true;
      case GlobalField::SlewPerSec: raw = g.slewPerSec; return true;
      case GlobalField::RatePin: raw = g.ratePin; return true;
      case GlobalField::RateSps: raw = g.rateSps; return true;
      case GlobalField::DisarmedOutput: raw = g.disarmedOutput; return true;
      case GlobalField::DeviceUid: raw = g.deviceUid; return true;
    }
    return false;
  }
  const ChannelConfig* c = channelConfig(scope);
  if (c == nullptr) {
    return false;
  }
  switch (static_cast<ChannelField>(field)) {
    case ChannelField::Enabled: raw = c->enabled; return true;
    case ChannelField::Gain: raw = c->gain; return true;
    case ChannelField::Offset: raw = static_cast<uint32_t>(c->offset); return true;
    case ChannelField::Scale: memcpy(&raw, &c->scale, 4); return true;
    case ChannelField::Invert: raw = c->invert; return true;
    case ChannelField::CalTime: raw = c->calTime; return true;
    case ChannelField::SensorType: raw = c->sensorType; return true;
  }
  return false;
}

Err CommandHandler::writeChannelParam(ChannelConfig& c, uint8_t field, const uint8_t* value) {
  const uint32_t raw = proto::get32(value);
  switch (static_cast<ChannelField>(field)) {
    case ChannelField::Enabled:
      if (raw > 1) return Err::BadValue;
      c.enabled = static_cast<uint8_t>(raw);
      break;
    case ChannelField::Gain:
      if (raw != 128 && raw != 64) return Err::BadValue;
      c.gain = static_cast<uint8_t>(raw);
      break;
    case ChannelField::Offset:
      c.offset = static_cast<int32_t>(raw);
      break;
    case ChannelField::Scale: {
      const float scale = proto::getFloat(value);
      // Rejects zero, negatives, NaN and infinities in one comparison chain.
      if (!(scale > 1e-6f && scale < 1e12f)) return Err::BadValue;
      c.scale = scale;
      break;
    }
    case ChannelField::Invert:
      if (raw > 1) return Err::BadValue;
      c.invert = static_cast<uint8_t>(raw);
      break;
    case ChannelField::CalTime:
      c.calTime = raw;
      break;
    case ChannelField::SensorType:
      if (raw != static_cast<uint32_t>(proto::SensorType::LoadCellThrust) &&
          raw != static_cast<uint32_t>(proto::SensorType::LoadCellTorque) &&
          raw != static_cast<uint32_t>(proto::SensorType::LoadCellGeneric)) {
        return Err::BadValue;
      }
      c.sensorType = static_cast<uint8_t>(raw);
      break;
    default:
      return Err::BadValue;
  }
  return Err::None;
}

Err CommandHandler::writeGlobalParam(uint8_t field, uint32_t v) {
  // Output and safety settings must not move under a spinning motor.
  if (isArmed(ctx_.throttle)) {
    return Err::NotAllowed;
  }
  GlobalConfig& g = ctx_.config.data().global;
  switch (static_cast<GlobalField>(field)) {
    case GlobalField::PwmMinUs:
      if (v < 500 || v + 100 > g.pwmMaxUs) return Err::BadValue;
      g.pwmMinUs = static_cast<uint16_t>(v);
      break;
    case GlobalField::PwmMaxUs:
      if (v > 2500 || v < static_cast<uint32_t>(g.pwmMinUs) + 100) return Err::BadValue;
      g.pwmMaxUs = static_cast<uint16_t>(v);
      break;
    case GlobalField::PwmFreqHz:
      // The longest pulse must fit inside one period with margin.
      if (v < 50 || v > 490 || 1000000UL / v < static_cast<uint32_t>(g.pwmMaxUs) + 200) {
        return Err::BadValue;
      }
      g.pwmFreqHz = static_cast<uint16_t>(v);
      break;
    case GlobalField::WatchdogMs:
      if (v < 100 || v > 2000) return Err::BadValue;
      g.watchdogMs = static_cast<uint16_t>(v);
      break;
    case GlobalField::ThrottleMax:
      if (v > proto::kThrottleFull) return Err::BadValue;
      g.throttleMax = static_cast<uint16_t>(v);
      break;
    case GlobalField::SlewPerSec:
      if (v > 0xFFFF) return Err::BadValue;
      g.slewPerSec = static_cast<uint16_t>(v);
      break;
    case GlobalField::RatePin:
      if (v != 255 && !isFreeDigitalPin(v)) return Err::BadValue;
      g.ratePin = static_cast<uint8_t>(v);
      break;
    case GlobalField::RateSps:
      if (v != 10 && v != 80) return Err::BadValue;
      g.rateSps = static_cast<uint8_t>(v);
      break;
    case GlobalField::DisarmedOutput:
      if (v > 1) return Err::BadValue;
      g.disarmedOutput = static_cast<uint8_t>(v);
      break;
    case GlobalField::DeviceUid:
      return Err::NotAllowed;
    default:
      return Err::BadValue;
  }
  return Err::None;
}

Err CommandHandler::writeParam(uint8_t scope, uint8_t field, const uint8_t* value) {
  if (scope == proto::kScopeGlobal) {
    return writeGlobalParam(field, proto::get32(value));
  }
  ChannelConfig* c = channelConfig(scope);
  if (c == nullptr) {
    return Err::BadValue;
  }
  return writeChannelParam(*c, field, value);
}

void CommandHandler::handle(uint8_t type, uint8_t seq, const uint8_t* payload, uint8_t len,
                            uint32_t nowMs) {
  switch (static_cast<Cmd>(type)) {
    case Cmd::GetInfo:
      sendInfo();
      return;

    case Cmd::GetChannel:
      if (len != 1) return nack(type, seq, Err::BadLength);
      if (payload[0] >= ctx_.sensorCount) return nack(type, seq, Err::BadValue);
      sendChannel(payload[0]);
      return;

    case Cmd::SetStream:
      if (len != 1) return nack(type, seq, Err::BadLength);
      ctx_.streaming = payload[0] != 0;
      return ack(type, seq);

    case Cmd::GetParam: {
      if (len != 2) return nack(type, seq, Err::BadLength);
      uint32_t raw = 0;
      if (!readParam(payload[0], payload[1], raw)) return nack(type, seq, Err::BadValue);
      sendParam(payload[0], payload[1], raw);
      return;
    }

    case Cmd::SetParam: {
      if (len != 6) return nack(type, seq, Err::BadLength);
      const Err error = writeParam(payload[0], payload[1], payload + 2);
      if (error != Err::None) return nack(type, seq, error);
      ctx_.config.markDirty();
      ctx_.applyConfig();
      return ack(type, seq);
    }

    case Cmd::SaveConfig:
      // EEPROM writes block for milliseconds; never do that while armed.
      if (isArmed(ctx_.throttle)) return nack(type, seq, Err::NotAllowed);
      if (!ctx_.config.save()) return nack(type, seq, Err::StorageFailed);
      return ack(type, seq);

    case Cmd::LoadDefaults:
      if (isArmed(ctx_.throttle)) return nack(type, seq, Err::NotAllowed);
      ctx_.config.loadDefaults();
      ctx_.applyConfig();
      return ack(type, seq);

    case Cmd::Heartbeat:
      return;  // liveness was already recorded by the caller; no reply needed

    case Cmd::Arm:
      if (len != 2) return nack(type, seq, Err::BadLength);
      if (proto::get16(payload) != proto::kArmMagic) return nack(type, seq, Err::BadValue);
      if (!ctx_.throttle.arm(nowMs)) return nack(type, seq, Err::NotAllowed);
      return ack(type, seq);

    case Cmd::Disarm:
      ctx_.throttle.disarm();
      return ack(type, seq);

    case Cmd::SetThrottle:
      if (len != 2) return nack(type, seq, Err::BadLength);
      if (!ctx_.throttle.setTarget(proto::get16(payload))) {
        return nack(type, seq, Err::NotAllowed);
      }
      return ack(type, seq);

    case Cmd::EStop:
      ctx_.throttle.estop();
      return ack(type, seq);

    case Cmd::ClearEStop:
      if (len != 2) return nack(type, seq, Err::BadLength);
      if (proto::get16(payload) != proto::kArmMagic) return nack(type, seq, Err::BadValue);
      if (!ctx_.throttle.clearEstop()) return nack(type, seq, Err::NotAllowed);
      return ack(type, seq);
  }
  nack(type, seq, Err::UnknownCommand);
}

}  // namespace tb
