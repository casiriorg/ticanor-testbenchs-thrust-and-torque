//! Wire protocol between the acquisition board and the PC (docs/protocol.md).

pub mod framing;
pub mod ids;
pub mod messages;

pub use framing::{encode_frame, Frame, FrameDecoder};
pub use messages::{Command, DeviceMessage};
