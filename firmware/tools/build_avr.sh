#!/usr/bin/env bash
# Builds the firmware for the Arduino Nano with plain avr-gcc, without the
# Arduino IDE. Useful for CI and for checking flash/RAM usage.
#
#   firmware/tools/build_avr.sh /path/to/ArduinoCore-avr [build-dir]
#
# Requires: avr-gcc, avr-libc and a checkout of
# https://github.com/arduino/ArduinoCore-avr
# Day-to-day development works just as well from the Arduino IDE: open
# firmware/ThrustBench/ThrustBench.ino and press Upload.
set -euo pipefail

CORE_ROOT="${1:?usage: build_avr.sh <ArduinoCore-avr> [build-dir]}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKETCH="$HERE/../ThrustBench"
BUILD="${2:-$HERE/../build/avr}"

CORE="$CORE_ROOT/cores/arduino"
VARIANT="$CORE_ROOT/variants/eightanaloginputs"
EEPROM="$CORE_ROOT/libraries/EEPROM/src"

COMMON=(-mmcu=atmega328p -DF_CPU=16000000L -DARDUINO=10819 -DARDUINO_AVR_NANO
        -DARDUINO_ARCH_AVR -Os -ffunction-sections -fdata-sections -flto -w
        -I"$CORE" -I"$VARIANT" -I"$EEPROM")
CXXFLAGS=(-std=gnu++11 -fno-exceptions -fno-threadsafe-statics -fpermissive)
# Our own sources are compiled with warnings on.
OWNFLAGS=(-Wall -Wextra -Wno-unused-parameter)

mkdir -p "$BUILD/core" "$BUILD/sketch"
objects=()

for src in "$CORE"/*.c; do
  obj="$BUILD/core/$(basename "$src").o"
  avr-gcc "${COMMON[@]}" -std=gnu11 -c "$src" -o "$obj"
  objects+=("$obj")
done
for src in "$CORE"/*.S; do
  obj="$BUILD/core/$(basename "$src").o"
  avr-gcc "${COMMON[@]}" -x assembler-with-cpp -c "$src" -o "$obj"
  objects+=("$obj")
done
for src in "$CORE"/*.cpp; do
  obj="$BUILD/core/$(basename "$src").o"
  avr-g++ "${COMMON[@]}" "${CXXFLAGS[@]}" -c "$src" -o "$obj"
  objects+=("$obj")
done

while IFS= read -r src; do
  obj="$BUILD/sketch/$(echo "${src#"$SKETCH"/}" | tr '/' '_').o"
  avr-g++ "${COMMON[@]}" "${CXXFLAGS[@]}" -Wno-error "${OWNFLAGS[@]}" -Wall -c "$src" -o "$obj" 2>&1 \
    | sed 's/^/  /'
  objects+=("$obj")
done < <(find "$SKETCH/src" -name '*.cpp' | sort)

# The .ino is plain C++ once Arduino.h is included.
avr-g++ "${COMMON[@]}" "${CXXFLAGS[@]}" -x c++ -include Arduino.h \
  -c "$SKETCH/ThrustBench.ino" -o "$BUILD/sketch/ThrustBench.ino.o"
objects+=("$BUILD/sketch/ThrustBench.ino.o")

avr-g++ "${COMMON[@]}" -Wl,--gc-sections "${objects[@]}" -o "$BUILD/ThrustBench.elf" -lm
avr-objcopy -O ihex -R .eeprom "$BUILD/ThrustBench.elf" "$BUILD/ThrustBench.hex"

echo "Built $BUILD/ThrustBench.hex"
# Nano limits: 30720 bytes of flash (with bootloader), 2048 bytes of RAM.
avr-size -C --mcu=atmega328p "$BUILD/ThrustBench.elf" | grep -E "Program|Data"
