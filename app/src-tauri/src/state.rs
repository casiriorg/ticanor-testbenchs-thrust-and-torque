//! Shared application state and the bridge from core events to the window.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tb_core::services::bench::{Bench, BenchEvent, EventSink};

/// Name of the single event channel the frontend listens to.
pub const BENCH_EVENT: &str = "bench-event";

pub struct AppState {
    pub bench: Arc<Bench>,
}

/// Forwards bench events to every window.
pub struct WindowSink {
    handle: AppHandle,
}

impl WindowSink {
    pub fn new(handle: AppHandle) -> Self {
        Self { handle }
    }
}

impl EventSink for WindowSink {
    fn emit(&self, event: BenchEvent) {
        if let Err(e) = self.handle.emit(BENCH_EVENT, &event) {
            log::warn!("could not deliver an event to the window: {e}");
        }
    }
}
