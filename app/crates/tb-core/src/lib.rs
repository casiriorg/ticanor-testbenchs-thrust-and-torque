//! ThrustBench core.
//!
//! Everything that is not user interface lives here, with no dependency on
//! Tauri, so it can be tested headless and reused from other front ends.
//!
//! Layering (lower layers never depend on higher ones):
//!
//! ```text
//! services    bench orchestration: discovery, safety, recording, profiles
//! actuators   ways of commanding the ESC      recording   CSV + metadata
//! device      session with one board          analysis    statistics, spectra
//! transport   serial port, simulator          calibration least-squares fit
//! protocol    framing and messages            config      defaults, settings
//! domain      shared data types               error
//! ```

pub mod actuators;
pub mod analysis;
pub mod calibration;
pub mod config;
pub mod device;
pub mod domain;
pub mod error;
pub mod profile;
pub mod protocol;
pub mod recording;
pub mod safety;
pub mod services;
pub mod transport;

pub use error::{Error, Result};
