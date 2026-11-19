# Calibration

Each load cell is calibrated with known masses. The application fits a straight line through several points and
stores the result on the board (EEPROM), with the date. Recordings keep the raw counts and the calibration used,
so data can be recalibrated later (`Channel.recalibrated` in the Python package).

## Before starting

- Set **local gravity** in Settings. Standard gravity (9.80665) is the default; at Popayán's latitude and altitude
  it is about 9.78 m/s², a 0.3 % difference.
- Let the electronics warm up for ten minutes; load cells drift with temperature.
- Disarm. Calibration and tare are refused while armed.

## Reference masses

Use anything stable (bags of nuts or sand, water bottles), weigh each on the precision scale and label it with its
measured mass. Suggested points:

| Channel | Masses |
|---|---|
| Thrust (10 kg cell) | 0, 500, 1000, 2000, 3000 g; add 5000 g when testing larger propellers |
| Torque | 0, 100, 200, 500 g on the calibration arm |

Up to 5 kg the precision scale is the reference. Above that, the 20 kg scale limits the accuracy of the calibration,
so note it in the test set-up.

## Geometry (Settings, or the Calibration panel)

Every distance is a parameter, because the mechanical design can change.

- **Force ratio** (thrust): force on the cell divided by the weight applied. 1.0 when the mass rests directly on the
  motor mount along the thrust axis. If you load through a lever or pulley, enter the lever ratio.
- **Calibration arm** (torque): distance in mm from the motor axis to where the mass hangs, measured perpendicular
  to the force. Reference torque = mass × gravity × arm.

The torque axis is vertical on a propeller-up bench, so the weight must be redirected: either lay the bench on its
side for calibration, or run a string from the arm over a low-friction pulley. Friction in the pulley shows up as
hysteresis; compare loading and unloading.

## Procedure (Device → Calibration)

1. Choose the channel.
2. Empty fixture: enter 0 g, **Capture point**. Keep the bench still for the capture time (3 s by default).
3. Place each mass, enter its value, capture. Points with the same mass replace each other.
4. Read the fit: *non-linearity* is the largest error as a percentage of the calibrated span, *noise* the spread of
   the raw reading during each capture.
5. **Apply and save to board**.

What to expect from a healthy set-up: R² above 0.9999 and non-linearity below about 0.1 %. Larger values usually
mean friction, a cable pulling on the moving part, a loose bolt or a mass that touches something.
If the reading falls with load, the channel is stored as inverted automatically.

## Checks worth doing once

- **Hysteresis:** capture the same masses going up and coming down; the difference is friction.
- **Repeatability:** remove and replace one mass five times.
- **Creep:** leave the largest mass for ten minutes and watch the live value.
- **Cross-talk:** load thrust and watch torque, and the reverse.

## Tare

Tare zeroes a channel at the current load; do it before each test, motor stopped. The application warns when the last
tare is older than the configured age. Tare changes only the offset, not the scale.

## Changing the load cell

Device → Load cells → pick the model and **Apply nominal values**. This writes an estimated scale and marks the channel
"nominal" until it is calibrated. Models are listed in `app/crates/tb-core/src/config/defaults.json`.
