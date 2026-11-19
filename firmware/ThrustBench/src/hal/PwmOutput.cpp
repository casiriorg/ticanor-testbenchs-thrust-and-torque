#include "PwmOutput.h"

#include "../config/BoardPins.h"

#if !defined(__AVR__)
// Host build: firmware/host_sim reads this to drive its motor model.
uint16_t g_hostPwmPulseUs = 0;
#endif

namespace tb {

void PwmOutput::begin(uint16_t freqHz) {
  pinMode(pins::kEscPwm, OUTPUT);
  digitalWrite(pins::kEscPwm, LOW);
  setFrequency(freqHz);
  disable();
}

void PwmOutput::setFrequency(uint16_t freqHz) {
  freqHz = constrain(freqHz, 50, 490);
  periodUs_ = static_cast<uint16_t>(1000000UL / freqHz);
#if defined(__AVR__)
  // Fast PWM, TOP = ICR1 (mode 14), prescaler 8: one tick = 0.5 us at 16 MHz.
  const uint16_t top = static_cast<uint16_t>((F_CPU / 8UL) / freqHz - 1UL);
  const uint8_t sreg = SREG;
  cli();
  TCCR1A = (TCCR1A & _BV(COM1A1)) | _BV(WGM11);
  TCCR1B = _BV(WGM13) | _BV(WGM12) | _BV(CS11);
  ICR1 = top;
  SREG = sreg;
#endif
}

void PwmOutput::setPulseUs(uint16_t us) {
  // Never let the pulse reach the period, or the output would stay high.
  const uint16_t limit = static_cast<uint16_t>(periodUs_ - 100);
  if (us > limit) {
    us = limit;
  }
  pulseUs_ = us;
#if defined(__AVR__)
  const uint16_t ticks = static_cast<uint16_t>((static_cast<uint32_t>(us) * (F_CPU / 8UL)) / 1000000UL);
  const uint8_t sreg = SREG;
  cli();
  OCR1A = ticks;             // double-buffered: applied at the next period
  TCCR1A |= _BV(COM1A1);     // connect OC1A (non-inverting)
  SREG = sreg;
#else
  g_hostPwmPulseUs = us;
#endif
}

void PwmOutput::disable() {
  pulseUs_ = 0;
#if defined(__AVR__)
  const uint8_t sreg = SREG;
  cli();
  TCCR1A &= static_cast<uint8_t>(~_BV(COM1A1));  // disconnect OC1A
  SREG = sreg;
  digitalWrite(pins::kEscPwm, LOW);
#else
  g_hostPwmPulseUs = 0;
#endif
}

}  // namespace tb
