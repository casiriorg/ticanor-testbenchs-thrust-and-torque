//! Tauri commands, grouped by concern. Each command is a one-line adapter to
//! the bench service: argument decoding in, serialised result out.
//!
//! Every command is declared `async` so Tauri runs it on its worker pool;
//! none of them may block the window's main thread.

pub mod actuator;
pub mod analysis;
pub mod connection;
pub mod device;
pub mod recording;
pub mod settings;

pub type CommandResult<T> = Result<T, tb_core::Error>;
