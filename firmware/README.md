# Firmware

Arduino Nano (ATmega328P) firmware. Open `ThrustBench/ThrustBench.ino` in the Arduino IDE and upload; no external
libraries are needed. Wiring: `docs/hardware.md`. Protocol: `docs/protocol.md`.

- Defaults you may want to change before the first upload: `ThrustBench/src/config/Defaults.h`. Everything in it can
  also be changed later from the application and saved to EEPROM.
- Pins: `ThrustBench/src/config/BoardPins.h`.
- Adding a sensor: implement `Sensor` (`src/sensors/Sensor.h`) and register it in `src/app/DeviceApp.cpp`.

Command-line build and size check: `tools/build_avr.sh <path to ArduinoCore-avr>`.

PC build for tests (`host_sim/`): the same sources compiled against a simulated bench; see `host_sim/build.sh`.
