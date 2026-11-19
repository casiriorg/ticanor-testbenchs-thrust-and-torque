//! Application services: the orchestration layer the UI talks to.

pub mod bench;

pub use bench::{Alarm, Bench, BenchConfig, BenchEvent, BenchSnapshot, EventSink};
