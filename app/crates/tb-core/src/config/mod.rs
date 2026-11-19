//! Defaults (compiled in from `defaults.json`) and persisted user settings.

pub mod defaults;
pub mod settings;

pub use defaults::{defaults, Defaults, LoadCellProfile};
pub use settings::{AppSettings, ColumnNaming, SafetyLimits, SettingsStore};
