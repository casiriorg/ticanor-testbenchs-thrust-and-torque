//! Connection to the acquisition board.

pub mod session;

pub use session::{start_session, LinkStats, SessionEvent, SessionHandle};
