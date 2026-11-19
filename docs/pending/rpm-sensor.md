# RPM sensor

**Goal.** Shaft speed, for thrust and torque coefficients and for order analysis.

**Hardware.** Reflective infrared pickup (TCRT5000 or similar) aimed at a mark on the motor bell, output to D2 (INT0).
Shield it from ambient light. Alternative: a hall sensor and a small magnet.

**Firmware.** `sensors/RpmSensor`: an interrupt stores the timestamp of each pulse; `poll()` reports the period of
the last revolution (or the mean over 50 ms) as RPM × 10. Parameters: pulses per revolution, minimum valid period
(debounce). Sensor code 0x30.

**PC.** Show RPM; add columns to the throttle curve: RPM, Ct and Cq (needs air density from temperature and altitude
in the test set-up), mechanical power = torque × angular speed.

**Acceptance.** Reading matches a handheld tachometer within 1 % from 1000 to 10000 RPM.
