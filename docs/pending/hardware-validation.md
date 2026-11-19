# Hardware validation (do first)

Nothing in this repository has run on the physical bench. Work through this list in order, **without a propeller**
until step 6.

1. **Flash and connect.** The application should find the board, show firmware and three channels.
   If it does not: check the CH340 driver and the port list in the log.
2. **HX711 rate.** Record ten seconds and open it in Recordings: "Rate (Hz)" in Statistics should be about 80 per load cell. About 10 Hz means
   RATE is still tied low (docs/hardware.md).
3. **Signs and noise.** Press on each cell by hand: thrust and torque should move the expected way. Note the idle
   noise (standard deviation) for later comparison.
4. **Loop timing.** Status reports `loop_max_us`; it should stay well under 1000. `tx_dropped` must stay 0.
5. **ESC signal, no motor power.** With an oscilloscope or servo tester on D9: 1000 µs disarmed, pulse follows the
   slider when armed, returns to 1000 µs on STOP, on unplugging USB (within 0.3 s plus ESC time), and on closing the app.
6. **Motor without propeller.** Arm, 10 %, STOP. Then pull the USB cable at 10 %: the motor must stop.
   Kill the application process at 10 %: the motor must stop.
7. **Calibrate** both cells (docs/calibration.md); record R² and non-linearity.
8. **Resonance.** Tap the motor mount and look at the thrust spectrum; note the peak. Keep test RPM away from it.
9. **Propeller, low throttle, behind the cage.** Staircase to 30 %. Compare the curve with the manufacturer's table.

Record the results in this file; they become the bench's acceptance record.

Known software risks to watch during these steps:
- `SerialTransport` opens the port with a 5 ms read timeout; on Windows confirm reads do not stall.
- Automatic search tries the first CH340/FTDI port it sees. With several such devices plugged in, choose the port by
  hand (the manual port list is exposed by `list_ports`, but the UI only offers automatic search and the simulator).
