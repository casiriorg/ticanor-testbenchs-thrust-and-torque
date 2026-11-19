// Non-blocking HX711 driver.
//
// The usual Arduino libraries wait for the converter inside read(); this one
// never waits. Call ready() every loop pass and read() only when it returns
// true. A read takes roughly 100 microseconds.
#pragma once

#include <Arduino.h>

namespace tb {

class Hx711 {
 public:
  Hx711(uint8_t doutPin, uint8_t sckPin) : doutPin_(doutPin), sckPin_(sckPin) {}

  void begin(uint8_t gain);

  // Gain for channel A: 128 or 64. Takes effect on the conversion after the
  // next read(), because the HX711 latches it from the extra clock pulses.
  void setGain(uint8_t gain);

  // True when a new conversion is waiting (DOUT low).
  bool ready() const;

  // Clocks out one 24-bit conversion, sign-extended. Call only when ready().
  int32_t read();

 private:
  void pulseHigh() const;
  void pulseLow() const;
  bool sampleDout() const;

  uint8_t doutPin_;
  uint8_t sckPin_;
  uint8_t extraPulses_ = 1;
#if defined(__AVR__)
  volatile uint8_t* sckPort_ = nullptr;
  volatile uint8_t* doutPort_ = nullptr;
  uint8_t sckMask_ = 0;
  uint8_t doutMask_ = 0;
#endif
};

}  // namespace tb
