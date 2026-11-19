//! Byte transports. The session only depends on the `Transport` trait, so a
//! real serial port, the in-process simulator and test doubles are
//! interchangeable.

pub mod serial;
pub mod sim;

use std::io;

pub trait Transport: Send {
    /// Reads whatever is available, waiting a few milliseconds at most.
    /// `Ok(0)` means "nothing yet"; an `Err` means the link is gone.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    fn write_all(&mut self, data: &[u8]) -> io::Result<()>;

    /// Human-readable description for logs and the UI (e.g. "COM5").
    fn label(&self) -> String;
}
