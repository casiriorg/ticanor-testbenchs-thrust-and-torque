# Analysis and library extras

- **Order analysis.** With RPM: amplitude at 1×, 2× and blade-pass frequency against speed; waterfall plot.
- **Efficiency.** g/W, propeller and motor efficiency once voltage, current and RPM exist.
- **Step response.** Rise time and overshoot from the built-in "Step response" profile.
- **Hysteresis.** Up and down ramps overlaid, from the "Slow ramp" profile.
- **Profile editor.** Profiles are JSON (`tb_core::profile::Profile`); the UI only offers the built-in ones.
  Add loading from and saving to `<data folder>/profiles/*.json` and a simple step editor.
- **Library management.** Rename, delete (to a trash folder), edit `setup.json` afterwards, export a selection.
- **Parts catalogue.** Saved motors, propellers and ESCs to pick from in the set-up form
  (today the form remembers only the last set-up).
- **Manual port selection.** `list_ports`/`connect_port` exist in the core and the controllers; add a port picker.
- **Python inside the application.** Kept out on purpose (packaging cost). If needed later, ship the notebooks'
  package as a sidecar executable and call it from a controller.
- **Recording format.** One sparse CSV works at today's rates. With 1 kHz accelerometers, write one file per rate
  group (`data_80hz.csv`, `data_1khz.csv`) or add a binary option.
