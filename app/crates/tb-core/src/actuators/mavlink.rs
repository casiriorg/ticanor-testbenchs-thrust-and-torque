//! EXPERIMENTAL: throttle through a PX4 flight controller using the MAVLink
//! actuator test command (`MAV_CMD_ACTUATOR_TEST`, id 310).
//!
//! Status: written from the MAVLink and PX4 documentation and unit tested
//! for framing only. It has NOT been run against a real flight controller.
//! Validate it with the propeller removed before trusting it; see
//! docs/pending/mavlink-driver.md for the open points.
//!
//! How it works: while "armed" the driver re-sends the actuator test command
//! five times a second with a one second timeout. If this application stops
//! (crash, cable pulled), PX4 leaves test mode on its own within that second.
//! PX4 only accepts the command while the vehicle itself is disarmed.

use std::io;
use std::time::{Duration, Instant};

use super::ThrottleDriver;
use crate::domain::{ActuatorState, DriverKind};
use crate::error::{Error, Result};
use crate::transport::serial::SerialTransport;
use crate::transport::Transport;

const MAV_CMD_ACTUATOR_TEST: u16 = 310;
/// `ACTUATOR_OUTPUT_FUNCTION_MOTOR1` in the MAVLink common dialect.
const OUTPUT_FUNCTION_MOTOR1: f32 = 1.0;
const TEST_TIMEOUT_S: f32 = 1.0;
const RESEND_PERIOD: Duration = Duration::from_millis(200);

const MSG_HEARTBEAT: u32 = 0;
const MSG_COMMAND_LONG: u32 = 76;
const CRC_EXTRA_HEARTBEAT: u8 = 50;
const CRC_EXTRA_COMMAND_LONG: u8 = 152;

/// Identity this application uses on the MAVLink network (a ground station).
const OWN_SYSTEM_ID: u8 = 255;
const OWN_COMPONENT_ID: u8 = 190;

/// CRC-16/MCRF4XX, the checksum MAVLink calls X.25.
fn crc_accumulate(crc: u16, byte: u8) -> u16 {
    let mut tmp = byte ^ (crc & 0xFF) as u8;
    tmp ^= tmp << 4;
    (crc >> 8) ^ ((tmp as u16) << 8) ^ ((tmp as u16) << 3) ^ ((tmp as u16) >> 4)
}

fn crc_of(data: &[u8], extra: u8) -> u16 {
    let crc = data.iter().fold(0xFFFF, |crc, &b| crc_accumulate(crc, b));
    crc_accumulate(crc, extra)
}

/// Encodes a MAVLink 2 frame (unsigned).
fn encode_v2(seq: u8, msg_id: u32, payload: &[u8], crc_extra: u8) -> Vec<u8> {
    // MAVLink 2 senders drop trailing zero bytes of the payload.
    let mut len = payload.len();
    while len > 1 && payload[len - 1] == 0 {
        len -= 1;
    }
    let mut frame = vec![0xFD, len as u8, 0, 0, seq, OWN_SYSTEM_ID, OWN_COMPONENT_ID];
    frame.extend_from_slice(&msg_id.to_le_bytes()[..3]);
    frame.extend_from_slice(&payload[..len]);
    let crc = crc_of(&frame[1..], crc_extra);
    frame.extend_from_slice(&crc.to_le_bytes());
    frame
}

fn command_long(seq: u8, target_system: u8, target_component: u8, command: u16, params: [f32; 7]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(33);
    for p in params {
        payload.extend_from_slice(&p.to_le_bytes());
    }
    payload.extend_from_slice(&command.to_le_bytes());
    payload.extend_from_slice(&[target_system, target_component, 0]);
    encode_v2(seq, MSG_COMMAND_LONG, &payload, CRC_EXTRA_COMMAND_LONG)
}

/// Looks for a CRC-valid HEARTBEAT (MAVLink 1 or 2) and returns the sender's
/// system and component ids.
fn find_heartbeat(data: &[u8]) -> Option<(u8, u8)> {
    for start in 0..data.len() {
        let (header_len, msg_id, sysid, compid) = match data[start] {
            0xFD if data.len() >= start + 10 => {
                let id = u32::from_le_bytes([data[start + 7], data[start + 8], data[start + 9], 0]);
                (10, id, data[start + 5], data[start + 6])
            }
            0xFE if data.len() >= start + 6 => (6, data[start + 5] as u32, data[start + 3], data[start + 4]),
            _ => continue,
        };
        let payload_len = data[start + 1] as usize;
        let end = start + header_len + payload_len;
        if msg_id != MSG_HEARTBEAT || data.len() < end + 2 {
            continue;
        }
        let crc = crc_of(&data[start + 1..end], CRC_EXTRA_HEARTBEAT);
        if crc.to_le_bytes() == [data[end], data[end + 1]] {
            return Some((sysid, compid));
        }
    }
    None
}

pub struct MavlinkDriver {
    transport: Box<dyn Transport>,
    target: (u8, u8),
    seq: u8,
    state: ActuatorState,
    setpoint: f32,
    last_sent: Instant,
}

impl MavlinkDriver {
    /// Opens the flight controller's port and waits for its heartbeat.
    pub fn connect(port: &str) -> Result<Self> {
        let transport = SerialTransport::open(port, 57_600)?;
        Self::with_transport(Box::new(transport), Duration::from_secs(3))
    }

    pub fn with_transport(mut transport: Box<dyn Transport>, wait: Duration) -> Result<Self> {
        let deadline = Instant::now() + wait;
        let mut seen = Vec::new();
        let mut buf = [0u8; 256];
        let target = loop {
            let n = transport.read(&mut buf)?;
            seen.extend_from_slice(&buf[..n]);
            if let Some(target) = find_heartbeat(&seen) {
                break target;
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout("no MAVLink heartbeat from the flight controller"));
            }
            if seen.len() > 8192 {
                seen.drain(..4096);
            }
        };
        Ok(Self { transport, target, seq: 0, state: ActuatorState::Disarmed, setpoint: 0.0, last_sent: Instant::now() })
    }

    fn send_test(&mut self, value: f32, timeout_s: f32) -> io::Result<()> {
        let params = [value, timeout_s, 0.0, 0.0, OUTPUT_FUNCTION_MOTOR1, 0.0, 0.0];
        let frame = command_long(self.seq, self.target.0, self.target.1, MAV_CMD_ACTUATOR_TEST, params);
        self.seq = self.seq.wrapping_add(1);
        self.last_sent = Instant::now();
        self.transport.write_all(&frame)
    }

    /// Stops the motor: zero output now, then release the test mode.
    fn stop_output(&mut self) -> io::Result<()> {
        self.setpoint = 0.0;
        self.send_test(0.0, 0.1)?;
        self.send_test(f32::NAN, 0.0)
    }
}

impl ThrottleDriver for MavlinkDriver {
    fn kind(&self) -> DriverKind {
        DriverKind::Mavlink
    }

    fn can_command(&self) -> bool {
        true
    }

    fn arm(&mut self) -> Result<()> {
        if self.state != ActuatorState::Disarmed {
            return Err(Error::Safety("clear the emergency stop before arming".into()));
        }
        self.setpoint = 0.0;
        self.send_test(0.0, TEST_TIMEOUT_S)?;
        self.state = ActuatorState::Armed;
        Ok(())
    }

    fn disarm(&mut self) -> Result<()> {
        self.stop_output()?;
        if self.state != ActuatorState::EStop {
            self.state = ActuatorState::Disarmed;
        }
        Ok(())
    }

    fn set_throttle(&mut self, fraction: f64) -> Result<()> {
        if self.state != ActuatorState::Armed {
            return Err(Error::Safety("not armed".into()));
        }
        self.setpoint = fraction.clamp(0.0, 1.0) as f32;
        self.send_test(self.setpoint, TEST_TIMEOUT_S)?;
        Ok(())
    }

    fn estop(&mut self) {
        self.state = ActuatorState::EStop;
        for _ in 0..3 {
            let _ = self.stop_output();
        }
    }

    fn clear_estop(&mut self) -> Result<()> {
        if self.state == ActuatorState::EStop {
            self.state = ActuatorState::Disarmed;
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<()> {
        // Drain whatever the flight controller sends so its buffers never fill.
        let mut buf = [0u8; 256];
        while self.transport.read(&mut buf)? == buf.len() {}
        if self.state == ActuatorState::Armed && self.last_sent.elapsed() >= RESEND_PERIOD {
            self.send_test(self.setpoint, TEST_TIMEOUT_S)?;
        }
        Ok(())
    }

    fn state(&self) -> Option<ActuatorState> {
        Some(self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn crc_matches_mcrf4xx_check_value() {
        let crc = b"123456789".iter().fold(0xFFFF, |crc, &b| crc_accumulate(crc, b));
        assert_eq!(crc, 0x6F91);
    }

    fn heartbeat_frame() -> Vec<u8> {
        // custom_mode(4) type autopilot base_mode system_status mavlink_version
        let payload = [0, 0, 0, 0, 2, 12, 81, 3, 3];
        let mut frame = encode_v2(7, MSG_HEARTBEAT, &payload, CRC_EXTRA_HEARTBEAT);
        frame[5] = 1; // pretend it came from system 1, component 1
        frame[6] = 1;
        let end = frame.len() - 2;
        let crc = crc_of(&frame[1..end], CRC_EXTRA_HEARTBEAT);
        frame[end..].copy_from_slice(&crc.to_le_bytes());
        frame
    }

    #[test]
    fn heartbeat_is_found_inside_noise_and_corruption_is_rejected() {
        let mut stream = vec![0x11, 0xFD, 0x22];
        stream.extend(heartbeat_frame());
        assert_eq!(find_heartbeat(&stream), Some((1, 1)));
        let mut damaged = heartbeat_frame();
        damaged[12] ^= 0xFF;
        assert_eq!(find_heartbeat(&damaged), None);
    }

    struct Scripted {
        incoming: Vec<u8>,
        written: Arc<Mutex<Vec<u8>>>,
    }

    impl Transport for Scripted {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = buf.len().min(self.incoming.len());
            buf[..n].copy_from_slice(&self.incoming[..n]);
            self.incoming.drain(..n);
            Ok(n)
        }
        fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
            self.written.lock().unwrap().extend_from_slice(data);
            Ok(())
        }
        fn label(&self) -> String {
            "scripted".into()
        }
    }

    #[test]
    fn commands_target_the_heartbeat_sender_and_carry_the_throttle() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let transport = Scripted { incoming: heartbeat_frame(), written: written.clone() };
        let mut driver = MavlinkDriver::with_transport(Box::new(transport), Duration::from_millis(200)).unwrap();
        assert!(driver.set_throttle(0.3).is_err()); // not armed yet
        driver.arm().unwrap();
        written.lock().unwrap().clear();
        driver.set_throttle(0.25).unwrap();

        let frame = written.lock().unwrap().clone();
        assert_eq!(frame[0], 0xFD);
        assert_eq!(u32::from_le_bytes([frame[7], frame[8], frame[9], 0]), MSG_COMMAND_LONG);
        let payload = &frame[10..frame.len() - 2];
        assert_eq!(f32::from_le_bytes(payload[0..4].try_into().unwrap()), 0.25);
        assert_eq!(f32::from_le_bytes(payload[16..20].try_into().unwrap()), OUTPUT_FUNCTION_MOTOR1);
        assert_eq!(u16::from_le_bytes(payload[28..30].try_into().unwrap()), MAV_CMD_ACTUATOR_TEST);
        assert_eq!((payload[30], payload[31]), (1, 1));
        let crc = crc_of(&frame[1..frame.len() - 2], CRC_EXTRA_COMMAND_LONG);
        assert_eq!(crc.to_le_bytes(), frame[frame.len() - 2..]);

        driver.estop();
        assert_eq!(driver.state(), Some(ActuatorState::EStop));
        assert!(driver.arm().is_err());
    }
}
