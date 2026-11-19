# Hardware

## Wiring (Arduino Nano, ATmega328P)

Pin map: `firmware/ThrustBench/src/config/BoardPins.h`.

| Signal | Nano pin | Notes |
|---|---|---|
| HX711 thrust DT / SCK | D4 / D5 | |
| HX711 torque DT / SCK | D6 / D7 | |
| ESC signal | D9 | must be D9 (Timer1 hardware PWM) |
| ESC ground | GND | signal ground must be shared with the Nano |
| reserved: RPM pickup | D2 | docs/pending |
| reserved: second ESC | D10 | docs/pending |
| reserved: voltage, current | A0, A1 | docs/pending |
| reserved: I2C (MPU6050) | A4, A5 | docs/pending |

Do **not** connect the ESC's BEC 5 V to the Nano while USB is plugged in (the AMPX 40A has no BEC).

Load cell to HX711: red E+, black E−, green A+, white A−. One HX711 per cell; channel B is unused.
If a value moves the wrong way, calibrate (the sign is detected) or tick "Inverted".

## HX711 rate

The modules must run at 80 samples per second: the RATE pin tied high (on most boards, cut the trace or move the
solder jumper). At the default 10 SPS the application still works but cannot follow steps and ramps.

If a module exposes RATE, wire it to a free digital pin and set that pin in Device → "HX711 RATE pin"; the rate can
then be switched from the application. 10 SPS is quieter and its filter rejects 50/60 Hz mains hum, which suits
calibration and drift tests. Without the pin, use the display's mains filter and smoothing instead.

## ESC

The MAD AMPX 40A (2-6S) takes a standard servo pulse (OneShot is not supported) and, per its manual, shuts down when
the signal is lost for more than 0.3 s. Defaults: 1000-2000 µs at 50 Hz. If the ESC was calibrated to another range,
set the pulse limits in Device, or recalibrate the ESC (docs/pending/esc-calibration.md).

## Battery disconnect

Add a way to cut battery power that can be reached without crossing the propeller plane; see `docs/safety.md`.

## USB

The Nano's CH340 needs its driver on Windows. The application finds the board by USB id (`defaults.json`,
`usb_ids`) and confirms it with a handshake. Use a short, good cable and keep it away from the motor wires.
