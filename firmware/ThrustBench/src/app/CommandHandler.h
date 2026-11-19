// Decodes host commands and replies. Owns no hardware: it works through the
// references in DeviceContext, which keeps it testable on a PC.
#pragma once

#include <stdint.h>

#include "../actuators/ThrottleController.h"
#include "../config/ConfigStore.h"
#include "../protocol/FrameCodec.h"
#include "../sensors/Sensor.h"

namespace tb {

struct DeviceContext {
  proto::FrameWriter& writer;
  ConfigStore& config;
  ThrottleController& throttle;
  Sensor* const* sensors;
  uint8_t sensorCount;
  bool& streaming;
  // Re-applies configuration to hardware after a parameter changed.
  void (*applyConfig)();
};

class CommandHandler {
 public:
  explicit CommandHandler(const DeviceContext& context) : ctx_(context) {}

  void handle(uint8_t type, uint8_t seq, const uint8_t* payload, uint8_t len, uint32_t nowMs);

 private:
  void ack(uint8_t type, uint8_t seq);
  void nack(uint8_t type, uint8_t seq, proto::Err error);
  void sendInfo();
  void sendChannel(uint8_t index);
  void sendParam(uint8_t scope, uint8_t field, uint32_t rawValue);

  ChannelConfig* channelConfig(uint8_t channelId);
  bool readParam(uint8_t scope, uint8_t field, uint32_t& rawValue);
  proto::Err writeParam(uint8_t scope, uint8_t field, const uint8_t* value);
  proto::Err writeChannelParam(ChannelConfig& channel, uint8_t field, const uint8_t* value);
  proto::Err writeGlobalParam(uint8_t field, uint32_t value);

  DeviceContext ctx_;
};

}  // namespace tb
