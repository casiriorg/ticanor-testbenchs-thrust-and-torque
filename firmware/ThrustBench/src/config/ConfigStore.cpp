#include "ConfigStore.h"

#include <Arduino.h>
#include <EEPROM.h>

#include "../protocol/Crc16.h"
#include "../protocol/Protocol.h"
#include "Defaults.h"

namespace tb {
namespace {

constexpr uint16_t kMagic = 0x5442;  // "TB"
// Bump when DeviceConfig changes layout; older images are then discarded.
constexpr uint8_t kLayoutVersion = 1;
constexpr int kEepromBase = 0;

struct Header {
  uint16_t magic;
  uint8_t version;
  uint8_t size;
};

ChannelConfig defaultChannel(proto::SensorType type, float scale) {
  ChannelConfig c;
  c.sensorType = static_cast<uint8_t>(type);
  c.enabled = 1;
  c.gain = defaults::kHx711Gain;
  c.invert = defaults::kLoadCellInvert ? 1 : 0;
  c.offset = defaults::kLoadCellOffset;
  c.scale = scale;
  c.calTime = 0;
  return c;
}

// A per-board identifier so the app can tell two benches apart. Seeded from
// floating analog inputs and timing jitter; it only needs to be unlikely to
// collide, not cryptographically random.
uint32_t generateUid() {
  uint32_t x = micros();
  for (uint8_t i = 0; i < 16; ++i) {
    x = (x << 5) ^ (x >> 3) ^ static_cast<uint32_t>(analogRead(A6 + (i & 1)));
    x += micros();
  }
  return x != 0 ? x : 0x54420001UL;
}

uint16_t imageCrc(const Header& header, const DeviceConfig& config) {
  uint8_t buf[sizeof(Header) + sizeof(DeviceConfig)];
  memcpy(buf, &header, sizeof(Header));
  memcpy(buf + sizeof(Header), &config, sizeof(DeviceConfig));
  return proto::crc16(buf, sizeof(buf));
}

}  // namespace

void ConfigStore::loadDefaults() {
  const uint32_t uid = config_.global.deviceUid;
  config_.loadCells[0] =
      defaultChannel(proto::SensorType::LoadCellThrust, defaults::kThrustCountsPerNewton);
  config_.loadCells[1] =
      defaultChannel(proto::SensorType::LoadCellTorque, defaults::kTorqueCountsPerNewtonMetre);

  GlobalConfig& g = config_.global;
  g.pwmMinUs = defaults::kPwmMinUs;
  g.pwmMaxUs = defaults::kPwmMaxUs;
  g.pwmFreqHz = defaults::kPwmFreqHz;
  g.watchdogMs = defaults::kWatchdogMs;
  g.throttleMax = defaults::kThrottleMax;
  g.slewPerSec = defaults::kSlewPerSec;
  g.ratePin = defaults::kHx711RatePin;
  g.rateSps = defaults::kHx711RateSps;
  g.disarmedOutput = defaults::kDisarmedOutput;
  g.reserved = 0;
  g.deviceUid = uid;
  dirty_ = true;
}

void ConfigStore::begin() {
  Header header;
  EEPROM.get(kEepromBase, header);
  bool valid = header.magic == kMagic && header.version == kLayoutVersion &&
               header.size == sizeof(DeviceConfig);
  if (valid) {
    uint16_t stored;
    EEPROM.get(kEepromBase + static_cast<int>(sizeof(Header)), config_);
    EEPROM.get(kEepromBase + static_cast<int>(sizeof(Header) + sizeof(DeviceConfig)), stored);
    valid = stored == imageCrc(header, config_);
  }
  if (valid) {
    dirty_ = false;
    defaultsAtBoot_ = false;
    return;
  }
  // First boot or corrupted image: start from defaults with a fresh UID and
  // persist immediately so the UID is stable from now on.
  config_.global.deviceUid = generateUid();
  loadDefaults();
  defaultsAtBoot_ = true;
  save();
}

bool ConfigStore::save() {
  Header header;
  header.magic = kMagic;
  header.version = kLayoutVersion;
  header.size = sizeof(DeviceConfig);
  const uint16_t crc = imageCrc(header, config_);

  // EEPROM.put only rewrites bytes that changed, which preserves endurance.
  EEPROM.put(kEepromBase, header);
  EEPROM.put(kEepromBase + static_cast<int>(sizeof(Header)), config_);
  EEPROM.put(kEepromBase + static_cast<int>(sizeof(Header) + sizeof(DeviceConfig)), crc);

  // Read back to confirm the write.
  DeviceConfig check;
  EEPROM.get(kEepromBase + static_cast<int>(sizeof(Header)), check);
  if (memcmp(&check, &config_, sizeof(DeviceConfig)) != 0) {
    return false;
  }
  dirty_ = false;
  return true;
}

}  // namespace tb
