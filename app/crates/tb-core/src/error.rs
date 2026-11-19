//! Error type shared by every layer of the core.

use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serial port error: {0}")]
    Serial(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("the device did not answer in time ({0})")]
    Timeout(&'static str),

    /// The device understood the command and refused it.
    #[error("the device rejected the command: {0}")]
    Rejected(DeviceError),

    #[error("no device is connected")]
    NotConnected,

    /// A safety rule on the PC side blocked the action.
    #[error("blocked by a safety rule: {0}")]
    Safety(String),

    #[error("recording error: {0}")]
    Recording(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("data format error: {0}")]
    Format(String),
}

impl Error {
    /// Stable identifier the UI uses to pick a translated message.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Serial(_) => "serial",
            Error::Protocol(_) => "protocol",
            Error::Timeout(_) => "timeout",
            Error::Rejected(_) => "rejected",
            Error::NotConnected => "not_connected",
            Error::Safety(_) => "safety",
            Error::Recording(_) => "recording",
            Error::Invalid(_) => "invalid",
            Error::Format(_) => "format",
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Format(e.to_string())
    }
}

impl From<serialport::Error> for Error {
    fn from(e: serialport::Error) -> Self {
        Error::Serial(e.to_string())
    }
}

/// Serialised as `{ code, message }` so the frontend can branch on `code`.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Error", 2)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

/// Error codes returned by the firmware in a NACK (see docs/protocol.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DeviceError {
    UnknownCommand,
    BadLength,
    BadValue,
    NotAllowed,
    StorageFailed,
    Other(u8),
}

impl From<u8> for DeviceError {
    fn from(code: u8) -> Self {
        match code {
            1 => DeviceError::UnknownCommand,
            2 => DeviceError::BadLength,
            3 => DeviceError::BadValue,
            4 => DeviceError::NotAllowed,
            5 => DeviceError::StorageFailed,
            other => DeviceError::Other(other),
        }
    }
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::UnknownCommand => write!(f, "unknown command"),
            DeviceError::BadLength => write!(f, "wrong payload length"),
            DeviceError::BadValue => write!(f, "value out of range"),
            DeviceError::NotAllowed => write!(f, "not allowed in the current state"),
            DeviceError::StorageFailed => write!(f, "EEPROM write failed"),
            DeviceError::Other(code) => write!(f, "error code {code}"),
        }
    }
}
