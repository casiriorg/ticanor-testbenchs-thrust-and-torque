// A strain-gauge load cell read through a dedicated HX711.
#pragma once

#include "../config/ConfigStore.h"
#include "../hal/Hx711.h"
#include "Sensor.h"

namespace tb {

class LoadCellSensor : public Sensor {
 public:
  LoadCellSensor(uint8_t channelId, const char* name, Hx711& adc, ChannelConfig& config,
                 const GlobalConfig& global)
      : id_(channelId), name_(name), adc_(adc), config_(config), global_(global) {}

  void begin() override;
  void applyConfig() override;
  bool poll(Sample& out) override;
  void describe(ChannelInfo& info) const override;
  uint8_t channelId() const override { return id_; }

 private:
  uint8_t id_;
  const char* name_;
  Hx711& adc_;
  ChannelConfig& config_;
  const GlobalConfig& global_;
};

}  // namespace tb
