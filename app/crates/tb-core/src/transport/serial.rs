//! Serial port transport and USB port discovery.

use std::io::{self, Read, Write};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serialport::{SerialPort, SerialPortType};

use super::Transport;
use crate::config::defaults::{defaults, UsbId};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortRole {
    /// USB-serial bridge typical of the acquisition board (CH340, FT232...).
    AcquisitionBoard,
    /// USB identity of a known flight controller.
    FlightController,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortCandidate {
    pub port: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub product: Option<String>,
    pub role: PortRole,
}

fn matches(ids: &[UsbId], vid: u16, pid: u16) -> bool {
    ids.iter().any(|id| id.vid == vid && id.pid.map_or(true, |p| p == pid))
}

/// Lists serial ports and classifies them by USB vendor/product id. The id
/// tables live in `config/defaults.json` so new boards need no code change.
pub fn list_ports() -> Vec<PortCandidate> {
    let ids = &defaults().usb_ids;
    let ports = serialport::available_ports().unwrap_or_default();
    ports
        .into_iter()
        .map(|p| match p.port_type {
            SerialPortType::UsbPort(usb) => {
                let role = if matches(&ids.acquisition_board, usb.vid, usb.pid) {
                    PortRole::AcquisitionBoard
                } else if matches(&ids.flight_controller, usb.vid, usb.pid) {
                    PortRole::FlightController
                } else {
                    PortRole::Unknown
                };
                PortCandidate { port: p.port_name, vid: Some(usb.vid), pid: Some(usb.pid), product: usb.product, role }
            }
            _ => PortCandidate { port: p.port_name, vid: None, pid: None, product: None, role: PortRole::Unknown },
        })
        .collect()
}

pub struct SerialTransport {
    port: Box<dyn SerialPort>,
    name: String,
}

impl SerialTransport {
    /// Opens the port. On an Arduino Nano this toggles DTR, which resets the
    /// board: it starts disarmed and needs about two seconds for its
    /// bootloader before it answers. The session handshake waits for that.
    pub fn open(name: &str, baud: u32) -> Result<Self> {
        let port = serialport::new(name, baud).timeout(Duration::from_millis(5)).open()?;
        Ok(Self { port, name: name.to_string() })
    }
}

impl Transport for SerialTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(0),
            Err(e) => Err(e),
        }
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.port.write_all(data)?;
        self.port.flush()
    }

    fn label(&self) -> String {
        self.name.clone()
    }
}
