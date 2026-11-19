# MAVLink throttle mode

**State.** `app/crates/tb-core/src/actuators/mavlink.rs` exists and is unit tested for framing only. It has never
talked to a flight controller. The UI labels it experimental.

**To validate (Pixhawk 6X, PX4, propeller removed).**
1. Confirm the USB id is recognised (`defaults.json` → `usb_ids.flight_controller`); add it if not.
2. Confirm PX4 accepts `MAV_CMD_ACTUATOR_TEST` (310) with param1 = value, param2 = timeout, param5 = output function,
   and that `ACTUATOR_OUTPUT_FUNCTION_MOTOR1` is 1 in the MAVLink dialect PX4 uses. Read the COMMAND_ACK (not parsed yet).
3. Confirm the maximum timeout PX4 allows and that the motor stops when the application is killed.
4. Confirm what PX4 needs beforehand: vehicle disarmed, safety switch, parameters.
5. Check which motor output maps to the ESC on the bench; make the motor number a setting.

**To build.**
- Parse COMMAND_ACK and surface rejections.
- Read `BATTERY_STATUS` (voltage, current from the PM-03D) and `ESC_STATUS` if available, and inject them as channels.
- The throttle recorded in this mode is the commanded value stamped with PC time (±25 ms); document or improve.
- Clock alignment between flight-controller telemetry and board samples.

**Note.** For comparing motors and propellers the board PWM mode is sufficient and better timed. This mode is for
checking behaviour with the real flight stack and ESC protocol.
