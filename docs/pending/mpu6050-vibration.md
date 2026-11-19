# MPU6050 accelerometers

**Goal.** Measure vibration at the motor mount and at the frame to compare propeller balance and detect resonances.
The load cells cannot do this: 80 SPS limits them to 40 Hz and rotors spin at 60-130 Hz.

**Hardware.** Two MPU6050 on I2C (A4/A5), addresses 0x68 and 0x69 (AD0 high), 400 kHz, bolted to metal.

**Firmware.**
- `sensors/Mpu6050Sensor.{h,cpp}` implementing `Sensor`; three channels per unit (codes 0x10-0x12).
- Use the chip's FIFO at 1 kHz and read it in bursts from `poll()`; never block on I2C. The `Wire` library is
  blocking, so either use short burst reads (<1 ms) or a non-blocking TWI driver.
- Send several samples per Data frame (the frame format already allows `n` samples) to cut overhead.
- Budget: 2 units × 3 axes × 1 kHz ≈ 20 kB/s at 500 kBd capacity of 50 kB/s. Measure `loop_max_us` and `tx_dropped`.
  If the Nano cannot keep up, drop to one unit or 500 Hz, or move to an RP2040/ESP32 (the protocol is unchanged).
- Parameters: range (±2/4/8/16 g), DLPF setting, enable per unit.

**PC.** Sensor catalogue entries exist. Add: spectrum up to 500 Hz, amplitude at 1× and 2× RPM (needs rpm-sensor),
comparison of 1× amplitude between recordings normalised by RPM².

**Acceptance.** A known imbalance (tape on one blade) raises the 1× peak measurably and repeatably.
