// Compiled-in defaults. Every value here can be changed at run time from the
// desktop app (Device > Configuration) and persisted to EEPROM; this file only
// decides what a factory-fresh or "Load defaults" board looks like.
//
// The desktop app keeps the matching defaults in
// app/crates/tb-core/src/config/defaults.json.
#pragma once

#include <stdint.h>

namespace tb {
namespace defaults {

// ---- Serial link ----------------------------------------------------------
// 500 kBd is exact on a 16 MHz ATmega328P (0 % baud error) and leaves room for
// the 1 kHz accelerometer stream planned for later.
constexpr uint32_t kBaudRate = 500000;

// ---- Load cells -----------------------------------------------------------
// Both cells default to the 10 kg rectangular (bar) strain-gauge cell.
constexpr float kLoadCellCapacityKg = 10.0f;
constexpr float kLoadCellSensitivityMvPerV = 1.0f;  // rated output at capacity
constexpr uint8_t kHx711Gain = 128;                  // HX711 channel A
constexpr uint8_t kHx711RateSps = 80;                // module strapped for 80 SPS
constexpr uint8_t kHx711RatePin = 255;               // 255 = RATE not wired

// Distance from the motor axis to the point where the torque rod pushes the
// torque cell. Only used to derive the nominal torque scale below.
constexpr float kTorqueArmMm = 40.0f;

// Nominal scale derived from the HX711 transfer function:
//   counts = (mV/V) * 1e-3 * gain * 2^24
// At gain 128 a 1 mV/V cell at capacity gives about 2 147 484 counts.
// These are estimates so a new board shows plausible numbers; a real
// calibration with known weights replaces them (calTime becomes non-zero).
constexpr float kStandardGravity = 9.80665f;
constexpr float kCountsAtCapacity =
    kLoadCellSensitivityMvPerV * 1e-3f * static_cast<float>(kHx711Gain) * 16777216.0f;
constexpr float kThrustCountsPerNewton =
    kCountsAtCapacity / (kLoadCellCapacityKg * kStandardGravity);
constexpr float kTorqueCountsPerNewtonMetre =
    kThrustCountsPerNewton / (kTorqueArmMm * 1e-3f);

constexpr int32_t kLoadCellOffset = 0;
constexpr bool kLoadCellInvert = false;

// ---- ESC output -----------------------------------------------------------
constexpr uint16_t kPwmMinUs = 1000;
constexpr uint16_t kPwmMaxUs = 2000;
constexpr uint16_t kPwmFreqHz = 50;
constexpr uint8_t kDisarmedOutput = 0;  // 0 = hold minimum pulse, 1 = no signal

// ---- Safety ---------------------------------------------------------------
// Host silence longer than this while armed drops the throttle to zero.
constexpr uint16_t kWatchdogMs = 300;
// Hard ceiling on commanded throttle (0..10000 = 0..100 %). Starts
// conservative on purpose: raise it deliberately from the app.
constexpr uint16_t kThrottleMax = 6000;
// Maximum rise rate in throttle units per second (5000 = 50 %/s).
// Reductions are always immediate.
constexpr uint16_t kSlewPerSec = 5000;

// ---- Reporting ------------------------------------------------------------
constexpr uint16_t kStatusPeriodMs = 100;
constexpr uint16_t kThrottleSamplePeriodMs = 20;

}  // namespace defaults
}  // namespace tb
