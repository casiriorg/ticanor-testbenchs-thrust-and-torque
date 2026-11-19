#include "LoadCellSensor.h"

#include <Arduino.h>

namespace tb {

void LoadCellSensor::begin() { adc_.begin(config_.gain); }

void LoadCellSensor::applyConfig() { adc_.setGain(config_.gain); }

bool LoadCellSensor::poll(Sample& out) {
  if (!adc_.ready()) {
    return false;
  }
  // Timestamp before clocking the data out, so the read time is not included.
  const uint32_t now = micros();
  // Always clock the conversion out, even when the channel is disabled:
  // otherwise DOUT stays low and the converter stops producing new data.
  const int32_t raw = adc_.read();
  if (!config_.enabled) {
    return false;
  }
  out.timeUs = now;
  out.channel = id_;
  out.value = raw;
  return true;
}

void LoadCellSensor::describe(ChannelInfo& info) const {
  info.id = id_;
  info.sensorType = config_.sensorType;
  info.unit = static_cast<uint8_t>(
      config_.sensorType == static_cast<uint8_t>(proto::SensorType::LoadCellTorque)
          ? proto::Unit::NewtonMetre
          : proto::Unit::Newton);
  info.flags = static_cast<uint8_t>((config_.enabled ? proto::kChanEnabled : 0) |
                                    (config_.invert ? proto::kChanInvert : 0) |
                                    (config_.calTime != 0 ? proto::kChanCalibrated : 0));
  info.rateHz = global_.rateSps;
  info.gain = config_.gain;
  info.offset = config_.offset;
  info.scale = config_.scale;
  info.calTime = config_.calTime;
  setChannelName(info, name_);
}

}  // namespace tb
