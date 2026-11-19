# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

ThrustBench is a test bench for drone motors/propellers. This repository (`app/`) is the desktop application:
a Rust core, a Tauri shell, and a React UI. It is its own git repository, nested inside a larger project folder
that is *also* a git repository (`../` = `thrustbench/`, with siblings `firmware/`, `docs/`, `analysis/`; these are
not tracked by this repo, but are read by it). The acquisition board firmware, protocol spec, safety design and
calibration docs live there and are the source of truth for several things in this crate:

- `docs/protocol.md`: wire protocol, mirrored in `crates/tb-core/src/protocol/ids.rs`. Change both together.
- `firmware/ThrustBench/src/protocol/Protocol.h`: the firmware side of the same identifiers.
- `firmware/ThrustBench/src/config/Defaults.h` and `crates/tb-core/src/config/defaults.json` must agree on defaults.
- `docs/safety.md`: the full safety layering (firmware and core); `src/safety.rs` only documents the core half.
- `docs/architecture.md`: the same diagram reproduced below, plus the "Extending" recipes for new sensors, drivers and boards.

## Commands

```
npm install
npm run dev            # UI only, in a browser, against mockBackend (no Rust/Tauri needed)
npm run app:dev         # full desktop app (tauri dev)
npm run app:build        # build the executable/installers (app/target/release/bundle)
npm run build           # tsc --noEmit && vite build
npm run typecheck       # tsc --noEmit only
```

Rust tests (from `app/`):

```
cargo test -p tb-core                              # unit + integration tests against the in-process simulator
cargo test -p tb-core --test bench_sim              # one integration test file
cargo test -p tb-core bench_sim::some_test_name      # one test by name
```

Firmware-in-the-loop test (needs the sibling `firmware/` checkout and a C++ toolchain):

```
../firmware/host_sim/build.sh /tmp/tb_host_sim
TB_HOST_SIM=/tmp/tb_host_sim cargo test -p tb-core --test firmware_sim
```

No hardware available? Use **Use simulator** in the running app, or generate a demo recording:

```
cargo run -p tb-core --example demo_recording -- <folder> "My motor"
```

## Architecture

```
 Arduino Nano                         PC
 ┌──────────────────────┐   USB    ┌─────────────────────────────────────────────┐
 │ sensors → protocol   │ ───────► │ tb-core (Rust)            React UI          │
 │ actuator ← commands  │ ◄─────── │ transport → session →     views/components  │
 │ config (EEPROM)      │  frames  │ services::bench ─events─► stores            │
 └──────────────────────┘          │        ▲ commands (Tauri controllers) ◄─────│
                                   └─────────────────────────────────────────────┘
```

The device streams raw counts and owns calibration (EEPROM); the PC converts, records, analyses and commands.

### `crates/tb-core`: no UI dependency, reusable, headless-testable

Layering (lower never depends on higher), from `crates/tb-core/src/lib.rs`:

```
services    bench orchestration: discovery, safety, recording, profiles
actuators   ways of commanding the ESC      recording   CSV + metadata
device      session with one board          analysis    statistics, spectra
transport   serial port, simulator          calibration least-squares fit
protocol    framing and messages            config      defaults, settings
domain      shared data types               error
```

**`services::bench::Bench`** is the one entry point the UI/Tauri layer uses. A background "tick" thread (50 Hz,
`crates/tb-core/src/services/bench.rs`) finds/connects the board, pumps samples, runs the safety supervisor, feeds
the recorder, plays throttle profiles, and publishes `BenchEvent` (`Snapshot` | `Samples` | `Alarm`). All mutable
state sits behind one `Mutex<Inner>` held only for short, non-blocking work. The two slow paths (connecting, since
the board takes seconds to boot, and emergency stop) are deliberately kept outside it (`estop_session` is a
separate handle so e-stop can never wait on anything).

**Safety is two independent layers**, each able to stop the motor without the other (see `../docs/safety.md`):
firmware (arming, watchdog, ceiling, rise limit, latched e-stop) and `src/safety.rs`'s `SafetySupervisor` on the
PC (arm preconditions, its own ceiling/rise-rate shaping, a dead-man timer keyed on `ui_alive()` heartbeats from
the window, and threshold trips on incoming samples). Both must be understood together when changing anything
throttle- or arming-related; the PC side is pure logic with injected `Instant`s, so it is unit tested without I/O.

**`ThrottleDriver` trait** (`actuators/mod.rs`) abstracts how the ESC is commanded: `NanoPwmDriver` (board PWM,
firmware owns the safety state machine), `MavlinkDriver` (experimental, through a PX4 flight controller),
`ManualDriver` (operator uses other equipment; app only records). `Bench` talks only to the trait.

**`Transport` trait** (`transport/mod.rs`) abstracts the byte link: `SerialTransport` vs the in-process `SimDevice`.
This is what lets the whole bench run and be tested with no hardware attached.

Errors (`error.rs`) are one `Error` enum for every layer, serialized as `{ code, message }`; `code` is what the UI
branches/translates on, `message` is the English fallback.

### `src-tauri`: thin shell, no logic

`main.rs` creates the `Bench` and registers Tauri commands; `state.rs`'s `WindowSink` forwards `BenchEvent`s to the
window; `controllers/*` are one-line adapters (decode args, call a `Bench` method, return the result) grouped by
concern (connection, actuator, device, recording, analysis, settings). Every command is `async` so Tauri runs it
off the main thread. If you're adding a capability, the Rust logic belongs in `tb-core`, not here.

### `src`: React UI

`services/backend.ts` is the *only* door the UI uses to reach the outside world; views/stores never call Tauri
directly. `isDesktop` picks `tauriBackend` (real IPC) or `mockBackend` (in-memory, used by `npm run dev` in a
plain browser) behind the same `Backend` interface, which is why UI work doesn't require the Rust toolchain.
`stores/benchStore.ts` (zustand) holds everything driven by `BenchEvent`s, subscribes once in `init()`, and runs
the UI heartbeat (`api.uiAlive()` every 300 ms, the other end of the core's dead-man timer, so don't remove or
slow it down casually). High-rate `samples` events bypass React state entirely into `stores/liveBuffer.ts`
for the live charts. `views/` are the four screens (Live/Monitor, Recordings, Device, Settings); `components/`
are shared widgets (safety bar, dialogs, charts).

Frontend types in `src/services/types.ts` mirror the `serde` output of the Rust `domain`/`services::bench` types
by hand. There is no codegen, so a field renamed on one side must be renamed on the other.

## Writing style

In comments, docs and commit/changelog text, don't use an em dash or double dash as punctuation. Use a period,
comma, colon, semicolon or parentheses instead, or rephrase the sentence.

## Changelog

Every code change (a bug fix, a feature, anything beyond answering a question) gets one entry in
`../docs/changelog/`, named `log_<two-word-description>_<YYYYMMDD-HHMM>.md` (e.g. `log_monitor-limits_20261118-2300.md`).
Each entry is a brief, per-feature-granular summary of what changed and why (not a diff dump); a few lines per
distinct piece of the change is enough.

### Extending (from `../docs/architecture.md`)

- **New sensor:** firmware `Sensor` class + channel id → sensor code in `Protocol.h`/`ids.rs` → entry in
  `defaults.json`'s `sensor_catalog` → optional unit handling in `src/utils/units.ts`. Recording, live charts and
  CSV all pick it up from the channel descriptor automatically.
- **New throttle mode:** implement `ThrottleDriver`, add a `DriverKind` variant, list it in `bench.rs::driver_options`.
- **New board:** implement `Transport` if it isn't a serial port; the protocol itself is board-independent.
