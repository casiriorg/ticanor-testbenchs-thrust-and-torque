// Simulated hardware behind the host Arduino API: two HX711 converters fed
// by a simple motor/propeller model that reacts to the ESC pulse width.
#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>

#include <chrono>
#include <cmath>
#include <random>

#include "Arduino.h"
#include "EEPROM.h"

#include "../ThrustBench/src/config/BoardPins.h"
#include "../ThrustBench/src/config/Defaults.h"

extern uint16_t g_hostPwmPulseUs;  // written by PwmOutput.cpp

namespace {

using Clock = std::chrono::steady_clock;
const Clock::time_point g_start = Clock::now();

uint64_t nowUs64() {
  return std::chrono::duration_cast<std::chrono::microseconds>(Clock::now() - g_start).count();
}

std::mt19937 g_rng(12345);

// First-order motor model: thrust and torque follow throttle squared.
struct MotorModel {
  double level = 0.0;  // 0..1, lagged throttle
  uint64_t lastUs = 0;

  void advance(uint64_t nowUs) {
    const double pulse = g_hostPwmPulseUs;
    double target = (pulse - 1000.0) / 1000.0;
    target = target < 0.0 ? 0.0 : (target > 1.0 ? 1.0 : target);
    const double dt = (nowUs - lastUs) * 1e-6;
    lastUs = nowUs;
    const double tau = 0.08;  // seconds
    level += (target - level) * (1.0 - std::exp(-dt / tau));
  }
  double thrustNewton() const { return 30.0 * level * level; }
  double torqueNewtonMetre() const { return 0.5 * level * level; }
};

MotorModel g_motor;

// Behavioural HX711: a conversion every 12.5 ms (80 SPS), DOUT low when
// ready, data shifted out MSB first on SCK rising edges.
class Hx711Model {
 public:
  Hx711Model(uint8_t dout, uint8_t sck, int32_t offset, bool torque)
      : dout_(dout), sck_(sck), offset_(offset), torque_(torque) {}

  uint8_t dout() const { return dout_; }
  uint8_t sck() const { return sck_; }

  bool ready(uint64_t nowUs) const { return pulses_ == 0 && nowUs >= nextReadyUs_; }

  void onSck(uint8_t level, uint64_t nowUs) {
    if (level == HIGH && sckLevel_ == LOW) {
      if (pulses_ == 0) {
        if (nowUs < nextReadyUs_) {
          sckLevel_ = level;
          return;  // gain-select pulses after a read, or a spurious clock
        }
        latch(nowUs);
      }
      ++pulses_;
      if (pulses_ >= 25) {
        pulses_ = 0;
        nextReadyUs_ = nowUs + 12500;
      }
    }
    sckLevel_ = level;
  }

  int readDout(uint64_t nowUs) const {
    if (pulses_ >= 1 && pulses_ <= 24) {
      return (value_ >> (24 - pulses_)) & 1U;
    }
    return ready(nowUs) ? LOW : HIGH;
  }

 private:
  void latch(uint64_t nowUs) {
    g_motor.advance(nowUs);
    const double t = nowUs * 1e-6;
    std::normal_distribution<double> noise(0.0, 40.0);
    double counts = offset_ + noise(g_rng);
    if (torque_) {
      counts += g_motor.torqueNewtonMetre() * tb::defaults::kTorqueCountsPerNewtonMetre;
    } else {
      // A small 17 Hz ripple makes spectra non-trivial.
      const double ripple = 0.05 * g_motor.level * std::sin(2.0 * M_PI * 17.0 * t);
      counts += (g_motor.thrustNewton() + ripple) * tb::defaults::kThrustCountsPerNewton;
    }
    value_ = static_cast<uint32_t>(static_cast<int32_t>(std::lround(counts))) & 0xFFFFFFU;
  }

  uint8_t dout_;
  uint8_t sck_;
  int32_t offset_;
  bool torque_;
  uint8_t sckLevel_ = LOW;
  uint8_t pulses_ = 0;
  uint32_t value_ = 0;
  uint64_t nextReadyUs_ = 20000;
};

Hx711Model g_adcs[] = {
    Hx711Model(tb::pins::kThrustDout, tb::pins::kThrustSck, 84210, false),
    Hx711Model(tb::pins::kTorqueDout, tb::pins::kTorqueSck, -35120, true),
};

}  // namespace

uint32_t micros() { return static_cast<uint32_t>(nowUs64()); }
uint32_t millis() { return static_cast<uint32_t>(nowUs64() / 1000); }

void pinMode(uint8_t, uint8_t) {}

void digitalWrite(uint8_t pin, uint8_t value) {
  const uint64_t now = nowUs64();
  for (Hx711Model& adc : g_adcs) {
    if (adc.sck() == pin) {
      adc.onSck(value, now);
    }
  }
}

int digitalRead(uint8_t pin) {
  const uint64_t now = nowUs64();
  for (const Hx711Model& adc : g_adcs) {
    if (adc.dout() == pin) {
      return adc.readDout(now);
    }
  }
  return LOW;
}

int analogRead(uint8_t) { return static_cast<int>(g_rng() % 1024); }

// ---- Serial ---------------------------------------------------------------

HostSerial Serial;

void HostSerial::begin(uint32_t) {
  const int flags = fcntl(STDIN_FILENO, F_GETFL, 0);
  fcntl(STDIN_FILENO, F_SETFL, flags | O_NONBLOCK);
}

void HostSerial::fill() {
  if (head_ != tail_ || closed_) {
    return;
  }
  const ssize_t n = ::read(STDIN_FILENO, buf_, sizeof(buf_));
  if (n > 0) {
    head_ = 0;
    tail_ = static_cast<size_t>(n);
  } else if (n == 0) {
    closed_ = true;  // host closed the pipe
  }
}

int HostSerial::available() {
  fill();
  return static_cast<int>(tail_ - head_);
}

int HostSerial::read() {
  fill();
  return head_ < tail_ ? buf_[head_++] : -1;
}

size_t HostSerial::write(const uint8_t* data, size_t len) {
  size_t done = 0;
  while (done < len) {
    const ssize_t n = ::write(STDOUT_FILENO, data + done, len - done);
    if (n <= 0) {
      closed_ = true;
      break;
    }
    done += static_cast<size_t>(n);
  }
  return done;
}

// ---- EEPROM ---------------------------------------------------------------

HostEeprom EEPROM;

HostEeprom::HostEeprom() {
  memset(data_, 0xFF, sizeof(data_));
  if (const char* path = getenv("TB_EEPROM_FILE")) {
    if (FILE* f = fopen(path, "rb")) {
      const size_t n = fread(data_, 1, sizeof(data_), f);
      (void)n;
      fclose(f);
    }
  }
}

void HostEeprom::flush() {
  if (const char* path = getenv("TB_EEPROM_FILE")) {
    if (FILE* f = fopen(path, "wb")) {
      fwrite(data_, 1, sizeof(data_), f);
      fclose(f);
    }
  }
}
