// Pin map for the Arduino Nano (ATmega328P). Wiring is fixed at compile time;
// everything else is run-time configuration.
#pragma once

#include <stdint.h>

namespace tb {
namespace pins {

// HX711 #1: thrust load cell.
constexpr uint8_t kThrustDout = 4;
constexpr uint8_t kThrustSck = 5;

// HX711 #2: torque load cell.
constexpr uint8_t kTorqueDout = 6;
constexpr uint8_t kTorqueSck = 7;

// ESC signal. Must stay on D9: it is the Timer1 OC1A hardware PWM output,
// which keeps the pulse width exact even while interrupts are masked.
constexpr uint8_t kEscPwm = 9;

// Reserved for the modules described in docs/pending/. Not used yet.
constexpr uint8_t kRpmSensor = 2;   // INT0, infrared RPM pickup
constexpr uint8_t kEscPwm2 = 10;    // Timer1 OC1B, second motor (coaxial)
constexpr uint8_t kVoltageSense = 14;  // A0, battery voltage divider
constexpr uint8_t kCurrentSense = 15;  // A1, current sensor
// A4/A5: I2C for the MPU6050 accelerometers.

}  // namespace pins
}  // namespace tb
