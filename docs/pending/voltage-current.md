# Voltage and current

**Goal.** Electrical power and efficiency (g/W), battery sag.

**Hardware.** Resistive divider to A0 for 6S (25.2 V → under 5 V; e.g. 100 k / 22 k), current sensor to A1
(hall type, 50 A or more). Add an RC filter on each.

**Firmware.** `sensors/AdcSensor`: non-blocking ADC (start conversion, read on completion via the ADC interrupt or by
polling ADSC), oversample to 100 Hz. Sensor codes 0x20 and 0x21. Calibration uses the same offset/scale fields as
the load cells, so the existing parameter commands and UI work.

**PC.** Electrical power, g/W and overall efficiency (with RPM and torque) in the throttle curve and the comparison.
Optional cut-off limits for minimum voltage and maximum current in `SafetyLimits`.

**Acceptance.** Within 1 % of a bench multimeter at three points each.
