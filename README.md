# Ticanor Testbench

Test bench software for drone motors and propellers: an Arduino Nano firmware that reads two load cells
(thrust and torque) and drives an ESC, and a desktop application that shows, records, replays and compares the data.

```
firmware/   Arduino Nano firmware (.ino + modules) and a PC build of it for tests
app/        Desktop application: Rust core (crates/tb-core), Tauri shell (src-tauri), React UI (src)
analysis/   Python package and notebooks for deeper analysis of recordings
docs/       Protocol, architecture, safety, calibration, hardware; docs/pending = specified but not built
```

## Status

Built and tested without the physical bench:

- Firmware compiles for the Nano (about 9 kB flash, 474 B RAM) and passes integration tests when built for the PC.
- The core has 48 automated tests, including runs against the real firmware code.
- The desktop shell builds on Linux; the UI was exercised in a browser against a mock backend.

**Not yet done:** nothing has run on real hardware. The Windows installer builds and launches (v0.1.0).
Do the first hardware runs without a propeller. See `docs/safety.md`.

## Quick start

Firmware: open `firmware/ThrustBench/ThrustBench.ino` in the Arduino IDE, board "Arduino Nano",
processor "ATmega328P" (or "Old Bootloader"), Upload. No libraries needed. Wiring is in `docs/hardware.md`.

Application (needs Rust, Node 20+ and the Tauri prerequisites for your system):

**Windows:** the `x86_64-pc-windows-msvc` Rust toolchain needs the MSVC linker, which only comes with
Visual Studio. Install the "Desktop development with C++" workload, either through the Visual Studio
Installer or with:

```
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
```

If `cargo`/`tauri dev` fails with `link.exe failed: exit code 1` and the linker invocation shows
`link: extra operand ...` or `Try 'link --help'`, that `link.exe` is Git for Windows' coreutils tool
(`Program Files\Git\usr\bin\link.exe`, a hard-link utility, not a linker) shadowing the real one because
Git's `usr\bin` comes before the Visual Studio tools on `PATH`. Run `npm run app:dev` from a plain
PowerShell/cmd prompt (not Git Bash) once the Build Tools above are installed, or open a "Developer
Command Prompt for VS 2022" / "x64 Native Tools Command Prompt for VS 2022", where `vcvars` puts the
correct `link.exe` first.

```
cd app
npm install
npm run app:dev        # run the desktop application
npm run app:build      # build the executable and installers (output in app/target/release/bundle)
npm run dev            # UI only, in a browser, with a mock backend
```

No hardware? Press **Use simulator** in the application.

Tests:

```
cd app && cargo test -p tb-core
firmware/host_sim/build.sh /tmp/tb_host_sim
TB_HOST_SIM=/tmp/tb_host_sim cargo test -p tb-core --test firmware_sim
```

Analysis notebooks: see `analysis/README.md`.

## Where things are

| I want to... | Look at |
|---|---|
| change a default (load cell, limits, units) | `firmware/ThrustBench/src/config/Defaults.h`, `app/crates/tb-core/src/config/defaults.json` |
| add a sensor | `firmware/ThrustBench/src/sensors/Sensor.h`, `docs/protocol.md`, `docs/pending/` |
| understand a safety rule | `docs/safety.md` |
| calibrate | `docs/calibration.md` |
| add a language | `app/src/i18n/` |
| see what is planned | `docs/pending/README.md` |

Recordings are saved to `Documents/ThrustBench/<date>_<time>_<motor>_<propeller>/` as `data.csv`,
`session.json` and `setup.json`. The folder can be changed in Settings.

## License

MIT, see `LICENSE`. Dependencies are checked in `THIRD-PARTY-LICENSES.md`; the one case worth knowing
about is that `serialport` and a few of Tauri's own dependencies are MPL-2.0 (file-level copyleft, fine
to depend on as-is, see that file for what it would mean to modify one of them directly).
