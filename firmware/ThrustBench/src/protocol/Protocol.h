// ThrustBench wire protocol, version 1.
//
// This header is the single source of truth for every identifier that travels
// on the wire. The desktop application mirrors it in
// app/crates/tb-core/src/protocol/ids.rs and the human-readable specification
// lives in docs/protocol.md. Change all three together.
#pragma once

#include <stdint.h>

namespace tb {
namespace proto {

constexpr uint8_t kVersion = 1;

// A frame body is: type(1) seq(1) payload(0..kMaxPayload) crc16(2).
// It is COBS encoded and terminated by a single 0x00 byte.
// kMaxBody is sized so an encoded frame always fits the 63 byte TX buffer of
// the ATmega328P HardwareSerial without blocking.
constexpr uint8_t kMaxBody = 48;
constexpr uint8_t kMaxPayload = kMaxBody - 4;
constexpr uint8_t kMaxEncoded = kMaxBody + 2;  // COBS overhead + delimiter

// Host -> device commands.
enum class Cmd : uint8_t {
  GetInfo = 0x01,
  GetChannel = 0x02,    // payload: index(u8)
  SetStream = 0x03,     // payload: enabled(u8)
  GetParam = 0x05,      // payload: scope(u8) field(u8)
  SetParam = 0x06,      // payload: scope(u8) field(u8) value(4 bytes LE)
  SaveConfig = 0x07,    // persist RAM configuration to EEPROM
  LoadDefaults = 0x08,  // restore compiled defaults into RAM (not saved)
  Heartbeat = 0x10,     // keeps the actuator watchdog alive
  Arm = 0x11,           // payload: magic(u16) == kArmMagic
  Disarm = 0x12,
  SetThrottle = 0x13,   // payload: throttle(u16), 0..kThrottleFull
  EStop = 0x14,         // latched; never rejected
  ClearEStop = 0x15,    // payload: magic(u16) == kArmMagic
};

// Device -> host messages.
enum class Msg : uint8_t {
  Info = 0x81,
  Channel = 0x82,
  Data = 0x83,
  Ack = 0x84,     // payload: cmd(u8) cmdSeq(u8)
  Nack = 0x85,    // payload: cmd(u8) cmdSeq(u8) error(u8)
  Status = 0x86,
  Param = 0x87,   // payload: scope(u8) field(u8) value(4 bytes LE)
};

enum class Err : uint8_t {
  None = 0,
  UnknownCommand = 1,
  BadLength = 2,
  BadValue = 3,
  NotAllowed = 4,   // e.g. changing PWM settings or saving while armed
  StorageFailed = 5,
};

// What a channel measures. The desktop app maps these codes to names, icons
// and default units, so new sensors only need a new code here.
enum class SensorType : uint8_t {
  LoadCellThrust = 0x01,
  LoadCellTorque = 0x02,
  LoadCellGeneric = 0x03,
  AccelX = 0x10,
  AccelY = 0x11,
  AccelZ = 0x12,
  Voltage = 0x20,
  Current = 0x21,
  Rpm = 0x30,
  ThrottleCommand = 0x40,
};

// Engineering unit obtained after applying a channel's calibration:
//   value = (raw - offset) / scale, negated when the invert flag is set.
enum class Unit : uint8_t {
  Counts = 0,
  Newton = 1,
  NewtonMetre = 2,
  Volt = 3,
  Ampere = 4,
  Rpm = 5,
  MetrePerSecondSquared = 6,
  ThrottleFraction = 7,  // raw 0..kThrottleFull maps to 0..1
};

// Channel flags (bitmask).
constexpr uint8_t kChanEnabled = 0x01;
constexpr uint8_t kChanInvert = 0x02;
constexpr uint8_t kChanCalibrated = 0x04;  // calTime != 0

// Parameter scopes: a channel id, or kScopeGlobal.
constexpr uint8_t kScopeGlobal = 0xFF;

enum class ChannelField : uint8_t {
  Enabled = 1,     // u32: 0 or 1
  Gain = 2,        // u32: 128 or 64 (HX711 channel A)
  Offset = 3,      // i32: raw counts at zero load (tare)
  Scale = 4,       // f32: raw counts per engineering unit
  Invert = 5,      // u32: 0 or 1
  CalTime = 6,     // u32: Unix time of the last calibration, 0 = never
  SensorType = 7,  // u32: SensorType code
};

enum class GlobalField : uint8_t {
  PwmMinUs = 1,        // u32: pulse width at 0 % throttle
  PwmMaxUs = 2,        // u32: pulse width at 100 % throttle
  PwmFreqHz = 3,       // u32: 50..490
  WatchdogMs = 4,      // u32: host silence that triggers the failsafe
  ThrottleMax = 5,     // u32: hard ceiling, 0..kThrottleFull
  SlewPerSec = 6,      // u32: max throttle rise per second, 0 = unlimited
  RatePin = 7,         // u32: Arduino pin wired to HX711 RATE, 255 = none
  RateSps = 8,         // u32: 10 or 80 (informational when RatePin == 255)
  DisarmedOutput = 9,  // u32: 0 = minimum pulse, 1 = no signal
  DeviceUid = 10,      // u32: read only
};

enum class ActuatorState : uint8_t {
  Disarmed = 0,
  Armed = 1,
  EStop = 2,     // latched by the host; needs ClearEStop
  Failsafe = 3,  // watchdog tripped; needs Disarm
};

// Status flags (bitmask).
constexpr uint8_t kStatusStreaming = 0x01;
constexpr uint8_t kStatusConfigDirty = 0x02;   // RAM differs from EEPROM
constexpr uint8_t kStatusDefaultsLoaded = 0x04;  // EEPROM was empty/corrupt at boot
constexpr uint8_t kStatusTxOverflow = 0x08;    // data frames were dropped

// Capability bits reported in Info.
constexpr uint16_t kCapPwmActuator = 0x0001;
constexpr uint16_t kCapRatePin = 0x0002;

constexpr uint16_t kArmMagic = 0xA55A;
constexpr uint16_t kThrottleFull = 10000;  // 100.00 %

constexpr uint8_t kChannelNameLen = 8;

}  // namespace proto
}  // namespace tb
