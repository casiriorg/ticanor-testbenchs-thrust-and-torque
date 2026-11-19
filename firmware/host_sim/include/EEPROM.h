// In-memory EEPROM for the host build. When the environment variable
// TB_EEPROM_FILE is set, the image is loaded from and saved to that file so
// persistence across "reboots" can be tested.
#pragma once

#include <stdint.h>
#include <string.h>

class HostEeprom {
 public:
  HostEeprom();

  template <typename T>
  T& get(int address, T& value) {
    memcpy(&value, data_ + address, sizeof(T));
    return value;
  }

  template <typename T>
  const T& put(int address, const T& value) {
    memcpy(data_ + address, &value, sizeof(T));
    flush();
    return value;
  }

 private:
  void flush();
  uint8_t data_[1024];
};

extern HostEeprom EEPROM;
