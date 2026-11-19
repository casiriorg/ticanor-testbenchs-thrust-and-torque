// Sensor abstraction. To add a sensor (MPU6050, voltage divider, current
// sensor, RPM pickup): implement this interface in a new file under
// sensors/, give it a free channel id, and register one instance in
// app/DeviceApp.cpp. Nothing else in the firmware needs to change, and the
// desktop app discovers the new channel through the Channel descriptor.
#pragma once

#include <stdint.h>
#include <string.h>

#include "../protocol/Protocol.h"

namespace tb {

struct Sample {
  uint32_t timeUs;   // micros() when the measurement became available
  uint8_t channel;
  int32_t value;     // raw, uncalibrated
};

// Everything the host needs to interpret a channel (sent as Msg::Channel).
struct ChannelInfo {
  uint8_t id;
  uint8_t sensorType;
  uint8_t unit;
  uint8_t flags;
  uint16_t rateHz;
  uint8_t gain;
  int32_t offset;
  float scale;
  uint32_t calTime;
  char name[proto::kChannelNameLen];  // ASCII, zero padded
};

// Copies a channel name into the fixed-width, zero-padded wire field.
// Names of exactly kChannelNameLen characters are stored without terminator.
inline void setChannelName(ChannelInfo& info, const char* name) {
  size_t len = strlen(name);
  if (len > sizeof(info.name)) {
    len = sizeof(info.name);
  }
  memset(info.name, 0, sizeof(info.name));
  memcpy(info.name, name, len);
}

class Sensor {
 public:
  virtual void begin() = 0;

  // Re-reads the configuration after the host changed a parameter.
  virtual void applyConfig() = 0;

  // Must return immediately. Returns true when `out` holds a new sample.
  virtual bool poll(Sample& out) = 0;

  virtual void describe(ChannelInfo& info) const = 0;

  virtual uint8_t channelId() const = 0;
};

// Channel ids. Keep them stable: recordings reference them.
namespace channel {
constexpr uint8_t kThrust = 1;
constexpr uint8_t kTorque = 2;
constexpr uint8_t kThrottle = 16;
}  // namespace channel

}  // namespace tb
