# Second motor (coaxial)

The design assumes one motor today but keeps the door open:

- Firmware: `PwmOutput` drives OC1A (D9); OC1B (D10) is free on the same timer. Make `ThrottleController` hold an
  array of outputs; `SetThrottle` gains an optional index byte (old one-argument form stays valid).
- Protocol: a second throttle channel (id 17) and, for a second measurement unit, channels 3 and 4.
- PC: `ThrottleDriver::set_throttle(index, value)`; profiles get a per-motor target or a ratio; the safety supervisor
  applies limits per output and stops all outputs together.
- UI: two sliders with a link option; curves per motor and combined.

Arming and emergency stop always act on all outputs.
