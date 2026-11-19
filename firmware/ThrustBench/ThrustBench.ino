// ThrustBench firmware for Arduino Nano (ATmega328P, CH340).
//
// Reads two HX711 load-cell amplifiers (thrust and torque) without delays,
// streams raw samples over a framed binary protocol, stores calibration in
// EEPROM and drives one ESC with on-device safety (arming, watchdog,
// emergency stop).
//
// This file is intentionally empty of logic. Start reading at
// src/app/DeviceApp.cpp; the layout is:
//
//   src/config/     compiled defaults, pin map, EEPROM-backed configuration
//   src/hal/        hardware drivers (HX711, Timer1 PWM)
//   src/sensors/    Sensor interface and implementations
//   src/actuators/  ESC throttle controller and its safety state machine
//   src/protocol/   COBS framing, CRC, message identifiers
//   src/app/        command handling and the main loop
//
// Board: "Arduino Nano", Processor: "ATmega328P" (or "Old Bootloader").
// No external libraries are required.

#include "src/app/DeviceApp.h"

void setup() { tb::DeviceApp::setup(); }

void loop() { tb::DeviceApp::loop(); }
