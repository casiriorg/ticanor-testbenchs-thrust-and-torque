# Architecture

## Overview

```
 Arduino Nano                         PC
 ┌──────────────────────┐   USB    ┌─────────────────────────────────────────────┐
 │ sensors → protocol   │ ───────► │ tb-core (Rust)            React UI          │
 │ actuator ← commands  │ ◄─────── │ transport → session →     views/components  │
 │ config (EEPROM)      │  frames  │ services::bench ─events─► stores            │
 └──────────────────────┘          │        ▲ commands (Tauri controllers) ◄─────│
                                   └─────────────────────────────────────────────┘
```

The device streams raw counts and owns the calibration (EEPROM). The PC converts, records, analyses and commands.
The contract between them is `docs/protocol.md`.

## Firmware (`firmware/ThrustBench/src`)

| Folder | Role |
|---|---|
| `config/` | compiled defaults, pin map, EEPROM-backed configuration |
| `hal/` | hardware drivers: non-blocking HX711, Timer1 PWM |
| `sensors/` | `Sensor` interface and implementations |
| `actuators/` | throttle controller with the safety state machine |
| `protocol/` | COBS, CRC, frame reader/writer, identifiers |
| `app/` | command handler and the main loop (`DeviceApp.cpp`, the composition root) |

The loop never waits: it reads a bounded number of incoming bytes, updates the actuator, polls each sensor and
sends what is ready. A sensor is a class with `poll()`; adding one means a new file and one line in `DeviceApp.cpp`.

`firmware/host_sim` compiles the same sources for the PC against a simulated bench, for integration tests.

## Core (`app/crates/tb-core/src`), no UI dependency

| Module | Role |
|---|---|
| `protocol` | framing and typed messages (both directions) |
| `transport` | `Transport` trait; serial port and in-process simulator |
| `device` | session thread: decoding, request/reply, heartbeat, link supervision |
| `actuators` | `ThrottleDriver` trait: board PWM, MAVLink (experimental), manual |
| `safety` | PC-side rules as pure, tested logic |
| `recording` | recorder thread, CSV + metadata, library, recovery |
| `analysis` | statistics, spectrum, display filters, throttle curves |
| `calibration` | multi-point least-squares fit |
| `profile` | scripted throttle profiles |
| `config` | `defaults.json` and persisted settings |
| `services::bench` | orchestration: discovery, drivers, safety, recorder, profiles, events |

`Bench` is the only entry point the UI layer uses. It runs a 50 Hz tick thread and publishes `BenchEvent`s
(snapshot, samples, alarm).

## Desktop shell (`app/src-tauri`)

Thin by design: `main.rs` creates the `Bench`, `state.rs` forwards its events to the window, and
`controllers/` exposes its methods as Tauri commands grouped by concern. No logic lives here.

## UI (`app/src`)

| Folder | Role |
|---|---|
| `services/` | `backend.ts` is the only door to the core; `tauriBackend.ts` (real) and `mockBackend.ts` (browser) implement it |
| `stores/` | `benchStore` (state from events), `liveBuffer` (samples, outside React state) |
| `views/` | Live, Recordings, Device, Settings |
| `components/` | safety bar, dialogs, charts, forms |
| `utils/` | units, display filters, hooks |
| `i18n/` | translations |

## Data flow of a sample

HX711 ready → `LoadCellSensor::poll` → Data frame → session thread (unwrap time, apply calibration) →
bench tick (safety checks, recorder queue) → `samples` event → `liveBuffer` → charts.
The recorder writes on its own thread; a slow disk drops rows (counted) instead of delaying anything.

## Extending

- **New sensor:** firmware `Sensor` class + channel id; sensor code in `Protocol.h`/`ids.rs`; entry in
  `defaults.json` `sensor_catalog`; optional unit handling in `app/src/utils/units.ts`. Recording, live charts
  and CSV pick it up from the descriptor.
- **New throttle mode:** implement `ThrottleDriver`, add a `DriverKind`, list it in `bench.rs::driver_options`.
- **New board:** implement `Transport` if it is not a serial port; the protocol is board independent.
