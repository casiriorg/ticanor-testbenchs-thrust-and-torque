// Servo-style pulse output for an ESC on Timer1 / OC1A (Arduino pin D9).
//
// The pulse is generated entirely by timer hardware, so its width does not
// jitter when interrupts are masked (the HX711 driver does that briefly).
// The Arduino Servo library is interrupt driven and would jitter.
#pragma once

#include <Arduino.h>

namespace tb {

class PwmOutput {
 public:
  // Configures the timer for `freqHz` (50..490) and leaves the output off.
  void begin(uint16_t freqHz);

  // Reprograms the frame rate. The caller guarantees the motor is stopped.
  void setFrequency(uint16_t freqHz);

  // Outputs a repeating pulse of `us` microseconds. Glitch-free: the new
  // width is latched by the timer at the start of the next period.
  void setPulseUs(uint16_t us);

  // Stops pulsing and drives the pin low ("no signal").
  void disable();

  uint16_t pulseUs() const { return pulseUs_; }

 private:
  uint16_t pulseUs_ = 0;
  uint16_t periodUs_ = 20000;
};

}  // namespace tb
