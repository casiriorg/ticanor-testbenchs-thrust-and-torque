#include "Hx711.h"

namespace tb {

void Hx711::begin(uint8_t gain) {
  pinMode(doutPin_, INPUT);
  pinMode(sckPin_, OUTPUT);
  digitalWrite(sckPin_, LOW);
#if defined(__AVR__)
  // Cache the port registers: digitalWrite() costs several microseconds per
  // call, direct port access a fraction of one.
  sckPort_ = portOutputRegister(digitalPinToPort(sckPin_));
  sckMask_ = digitalPinToBitMask(sckPin_);
  doutPort_ = portInputRegister(digitalPinToPort(doutPin_));
  doutMask_ = digitalPinToBitMask(doutPin_);
#endif
  setGain(gain);
}

void Hx711::setGain(uint8_t gain) {
  // Total clock pulses per read select the next conversion:
  // 25 = channel A gain 128, 27 = channel A gain 64.
  extraPulses_ = (gain == 64) ? 3 : 1;
}

#if defined(__AVR__)

bool Hx711::ready() const { return (*doutPort_ & doutMask_) == 0; }

inline void Hx711::pulseHigh() const { *sckPort_ |= sckMask_; }
inline void Hx711::pulseLow() const { *sckPort_ &= static_cast<uint8_t>(~sckMask_); }
inline bool Hx711::sampleDout() const { return (*doutPort_ & doutMask_) != 0; }

#else  // host build (firmware/host_sim)

bool Hx711::ready() const { return digitalRead(doutPin_) == LOW; }

inline void Hx711::pulseHigh() const { digitalWrite(sckPin_, HIGH); }
inline void Hx711::pulseLow() const { digitalWrite(sckPin_, LOW); }
inline bool Hx711::sampleDout() const { return digitalRead(doutPin_) == HIGH; }

#endif

int32_t Hx711::read() {
  uint32_t value = 0;
  const uint8_t totalPulses = static_cast<uint8_t>(24 + extraPulses_);
  for (uint8_t i = 0; i < totalPulses; ++i) {
    // The HX711 powers down if SCK stays high for more than 60 us. Interrupts
    // are masked only for the high phase of each pulse (about 1 us), so an
    // interrupt can stretch the harmless low phase but never the high one.
    // Masking per bit instead of for the whole read keeps the serial receive
    // interrupt responsive at 500 kBd.
#if defined(__AVR__)
    const uint8_t sreg = SREG;
    cli();
    pulseHigh();
    __asm__ __volatile__("nop\n\tnop\n\tnop\n\tnop");  // DOUT settling, >0.1 us
    const bool bit = sampleDout();
    pulseLow();
    SREG = sreg;
#else
    pulseHigh();
    const bool bit = sampleDout();
    pulseLow();
#endif
    if (i < 24) {
      value = (value << 1) | (bit ? 1UL : 0UL);
    }
  }
  // Sign-extend the 24-bit two's complement result.
  if (value & 0x800000UL) {
    value |= 0xFF000000UL;
  }
  return static_cast<int32_t>(value);
}

}  // namespace tb
