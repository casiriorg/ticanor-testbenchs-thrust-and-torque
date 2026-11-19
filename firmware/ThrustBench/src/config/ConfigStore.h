// Run-time configuration and its EEPROM persistence.
#pragma once

#include <stdint.h>

namespace tb {

constexpr uint8_t kLoadCellCount = 2;

// Calibration and set-up of one load-cell channel.
// Engineering value = (raw - offset) / scale, negated when `invert` is set.
// The firmware always streams raw counts; this record is the authoritative
// copy of the calibration that the desktop app reads, applies and edits.
struct ChannelConfig {
  uint8_t sensorType;  // proto::SensorType
  uint8_t enabled;
  uint8_t gain;        // 128 or 64
  uint8_t invert;
  int32_t offset;
  float scale;
  uint32_t calTime;    // Unix time of last calibration, 0 = nominal values
};

struct GlobalConfig {
  uint16_t pwmMinUs;
  uint16_t pwmMaxUs;
  uint16_t pwmFreqHz;
  uint16_t watchdogMs;
  uint16_t throttleMax;
  uint16_t slewPerSec;
  uint8_t ratePin;
  uint8_t rateSps;
  uint8_t disarmedOutput;
  uint8_t reserved;
  uint32_t deviceUid;
};

struct DeviceConfig {
  ChannelConfig loadCells[kLoadCellCount];
  GlobalConfig global;
};

class ConfigStore {
 public:
  // Loads the EEPROM image, falling back to compiled defaults when it is
  // missing, from another layout version, or fails its CRC.
  void begin();

  DeviceConfig& data() { return config_; }
  const DeviceConfig& data() const { return config_; }

  // Writes the RAM configuration to EEPROM. Blocks for up to ~150 ms, so the
  // command handler refuses it while the actuator is armed.
  bool save();

  // Restores compiled defaults into RAM (keeps the device UID). Not saved.
  void loadDefaults();

  void markDirty() { dirty_ = true; }
  bool dirty() const { return dirty_; }
  bool defaultsLoadedAtBoot() const { return defaultsAtBoot_; }

 private:
  DeviceConfig config_;
  bool dirty_ = false;
  bool defaultsAtBoot_ = false;
};

}  // namespace tb
