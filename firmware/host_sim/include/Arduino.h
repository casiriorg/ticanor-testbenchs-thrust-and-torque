// Minimal Arduino API for building the real firmware sources on a PC.
//
// firmware/host_sim compiles the exact same .cpp files that run on the Nano
// against this header and a simulated bench (two HX711s, an ESC and a motor
// model). The result is a program that speaks the wire protocol over
// stdin/stdout, used by the desktop app's integration tests.
#pragma once

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define HIGH 1
#define LOW 0
#define INPUT 0
#define OUTPUT 1
#define A0 14
#define A6 20
#define A7 21

#define constrain(x, lo, hi) ((x) < (lo) ? (lo) : ((x) > (hi) ? (hi) : (x)))

uint32_t micros();
uint32_t millis();
void pinMode(uint8_t pin, uint8_t mode);
void digitalWrite(uint8_t pin, uint8_t value);
int digitalRead(uint8_t pin);
int analogRead(uint8_t pin);

class Print {
 public:
  virtual ~Print() {}
  virtual size_t write(const uint8_t* data, size_t len) = 0;
  virtual int availableForWrite() { return 0; }
};

// Serial port backed by stdin (non-blocking) and stdout.
class HostSerial : public Print {
 public:
  void begin(uint32_t baud);
  int available();
  int read();
  size_t write(const uint8_t* data, size_t len) override;
  int availableForWrite() override { return 63; }
  bool closed() const { return closed_; }

 private:
  void fill();
  uint8_t buf_[256];
  size_t head_ = 0;
  size_t tail_ = 0;
  bool closed_ = false;
};

extern HostSerial Serial;
