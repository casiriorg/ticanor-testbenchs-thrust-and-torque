# Closed-loop control (proposal)

**Goal.** Hold a thrust (later RPM) target instead of a throttle, to compare motors at equal thrust and to measure
endurance at constant load.

**Where the loop runs.** On the PC, in the bench tick (50 Hz), as a new source of setpoints next to the manual slider
and profiles. The firmware keeps receiving throttle only, so every existing limit still applies. Thrust at 80 SPS
supports a loop bandwidth of a few hertz, which is enough for holds; it is not for fast transients.

**Design.**
- `profile::Step` gains `kind: hold_thrust` with `target_n`. A `ThrustController` (PI with feed-forward from the last
  throttle curve, output clamped to the safety ceiling, anti-windup, rate limited by the supervisor) produces the setpoint.
- Abort conditions: thrust sensor not calibrated, tare too old, sample gap over 200 ms, error not converging in N seconds,
  output saturated at the ceiling for more than 2 s.
- UI: target field in thrust units, live error, clear indication that the loop is active.

**Risks.** A wrong sign or a stuck sensor drives the throttle to the ceiling. Mitigations: ceiling for closed loop
set separately and lower by default; sanity check that thrust rises when throttle rises during the first second.

**Acceptance.** Holds a target within 2 % for 60 s on the simulator and on the bench; every abort condition tested.
