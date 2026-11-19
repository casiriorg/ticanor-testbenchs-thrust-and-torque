# ESC throttle-range calibration

**Goal.** Teach the ESC the pulse range the board sends, without a radio.

**Why it is not built yet.** The procedure sends full throttle at power-up. A mistake with a propeller mounted is
dangerous, so it needs its own guarded flow.

**Flow (AMPX 40A manual).** Signal at maximum → power the ESC → two beeps → signal to minimum within 3 s → confirmation tones.

**Design.**
- Firmware: a dedicated state `Calibrating`, entered only from Disarmed with a second magic value, that outputs
  `PwmMaxUs` regardless of the ceiling, times out to minimum after 10 s, and leaves on any command.
- UI: wizard with explicit steps and checkboxes: propeller removed, battery disconnected, then "connect the battery
  now", then "I heard two beeps".
- STOP ends it at any point.

**Acceptance.** After the wizard, 0 % does not spin the motor and the motor starts within the first few percent.
