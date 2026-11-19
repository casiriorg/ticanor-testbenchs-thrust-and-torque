//! One live connection to an acquisition board.
//!
//! A worker thread owns the transport. It decodes incoming frames, converts
//! samples to engineering units, runs request/reply exchanges, keeps the
//! device watchdog fed and declares the link lost when the device goes quiet.
//!
//! Safety-relevant design points:
//! * Emergency stop bypasses the request queue (`SessionHandle::estop`), so
//!   it is never stuck behind a slow or retried command.
//! * Heartbeats are sent by this thread, independently of the UI. Whether
//!   the UI itself is alive is supervised separately (see `safety`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use serde::Serialize;

use crate::domain::{ChannelDescriptor, DeviceInfo, DeviceStatus, Sample};
use crate::error::{DeviceError, Error, Result};
use crate::protocol::ids;
use crate::protocol::{encode_frame, Command, DeviceMessage, FrameDecoder};
use crate::transport::Transport;

const REPLY_TIMEOUT: Duration = Duration::from_millis(150);
const HEARTBEAT_PERIOD: Duration = Duration::from_millis(80);
const SAMPLE_FLUSH_PERIOD: Duration = Duration::from_millis(25);
/// The device reports status at 10 Hz; this much silence means it is gone.
const LINK_TIMEOUT: Duration = Duration::from_millis(1500);
/// Before the first frame the board may still be in its bootloader.
const BOOT_GRACE: Duration = Duration::from_secs(6);

#[derive(Debug)]
pub enum SessionEvent {
    Samples(Vec<Sample>),
    Status(DeviceStatus),
    /// The link is gone; the session has ended. Carries the reason.
    Lost(String),
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LinkStats {
    pub frames_received: u64,
    /// Frames the device sent that never arrived (gaps in its sequence).
    pub frames_lost: u64,
    /// Frames that arrived damaged (failed COBS/CRC validation).
    pub decode_errors: u64,
}

enum Outgoing {
    Request { command: Command, retries: u8, reply: Sender<Result<DeviceMessage>> },
    /// Written immediately, ahead of everything else, without waiting for a reply.
    Urgent(Command),
    Close,
}

struct Shared {
    channels: RwLock<Vec<ChannelDescriptor>>,
    alive: AtomicBool,
    frames_received: AtomicU64,
    frames_lost: AtomicU64,
    decode_errors: AtomicU64,
}

/// Cheap, cloneable handle to the session worker.
#[derive(Clone)]
pub struct SessionHandle {
    tx: Sender<Outgoing>,
    shared: Arc<Shared>,
    label: String,
}

/// Turns the device's wrapping 32-bit microsecond clock into seconds since
/// the first sample, tolerating slightly out-of-order timestamps.
#[derive(Default)]
struct TimeUnwrapper {
    last: Option<u32>,
    current_us: i64,
}

impl TimeUnwrapper {
    fn seconds(&mut self, time_us: u32) -> f64 {
        match self.last {
            None => {
                self.last = Some(time_us);
                self.current_us = 0;
                0.0
            }
            Some(last) => {
                // Signed difference handles both the 71-minute wrap and
                // timestamps that arrive a little out of order.
                let diff = time_us.wrapping_sub(last) as i32 as i64;
                let t = self.current_us + diff;
                if diff > 0 {
                    self.last = Some(time_us);
                    self.current_us = t;
                }
                t as f64 * 1e-6
            }
        }
    }
}

struct InFlight {
    command: Command,
    seq: u8,
    sent_at: Instant,
    retries_left: u8,
    reply: Sender<Result<DeviceMessage>>,
}

impl InFlight {
    /// Does `message` answer this request?
    fn matches(&self, message: &DeviceMessage) -> bool {
        match message {
            DeviceMessage::Ack { cmd, seq } | DeviceMessage::Nack { cmd, seq, .. } => {
                *cmd == self.command.kind() && *seq == self.seq
            }
            DeviceMessage::Info(_) => self.command == Command::GetInfo,
            DeviceMessage::Channel { index, .. } => self.command == Command::GetChannel(*index),
            DeviceMessage::Param { scope, field, .. } => {
                matches!(&self.command, Command::GetParam { scope: s, field: f } if s == scope && f == field)
            }
            _ => false,
        }
    }
}

struct Worker {
    transport: Box<dyn Transport>,
    rx: Receiver<Outgoing>,
    events: Sender<SessionEvent>,
    shared: Arc<Shared>,
    decoder: FrameDecoder,
    queue: VecDeque<(Command, u8, Sender<Result<DeviceMessage>>)>,
    in_flight: Option<InFlight>,
    host_seq: u8,
    device_seq: Option<u8>,
    clock: TimeUnwrapper,
    batch: Vec<Sample>,
    last_tx: Instant,
    last_rx: Instant,
    last_flush: Instant,
    got_first_frame: bool,
}

impl Worker {
    fn write(&mut self, command: &Command) -> std::io::Result<u8> {
        let seq = self.host_seq;
        self.host_seq = self.host_seq.wrapping_add(1);
        let wire = encode_frame(command.kind(), seq, &command.payload())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
        self.transport.write_all(&wire)?;
        self.last_tx = Instant::now();
        Ok(seq)
    }

    fn run(mut self) {
        let reason = self.run_loop().err();
        self.shared.alive.store(false, Ordering::SeqCst);
        // Unblock every caller still waiting for a reply.
        if let Some(pending) = self.in_flight.take() {
            let _ = pending.reply.send(Err(Error::NotConnected));
        }
        for (_, _, reply) in self.queue.drain(..) {
            let _ = reply.send(Err(Error::NotConnected));
        }
        if !self.batch.is_empty() {
            let _ = self.events.send(SessionEvent::Samples(std::mem::take(&mut self.batch)));
        }
        if let Some(reason) = reason {
            let _ = self.events.send(SessionEvent::Lost(reason));
        }
    }

    /// Returns `Ok` on a requested close and `Err(reason)` when the link is lost.
    fn run_loop(&mut self) -> std::result::Result<(), String> {
        let mut buf = [0u8; 512];
        let mut frames = Vec::new();
        loop {
            // 1. Outgoing traffic; urgent commands jump the queue.
            while let Ok(outgoing) = self.rx.try_recv() {
                match outgoing {
                    Outgoing::Urgent(command) => {
                        self.write(&command).map_err(|e| e.to_string())?;
                    }
                    Outgoing::Request { command, retries, reply } => self.queue.push_back((command, retries, reply)),
                    Outgoing::Close => return Ok(()),
                }
            }
            if self.in_flight.is_none() {
                if let Some((command, retries, reply)) = self.queue.pop_front() {
                    let seq = self.write(&command).map_err(|e| e.to_string())?;
                    self.in_flight = Some(InFlight { command, seq, sent_at: Instant::now(), retries_left: retries, reply });
                }
            }
            if self.last_tx.elapsed() >= HEARTBEAT_PERIOD {
                self.write(&Command::Heartbeat).map_err(|e| e.to_string())?;
            }

            // 2. Incoming traffic.
            let n = self.transport.read(&mut buf).map_err(|e| e.to_string())?;
            if n > 0 {
                frames.clear();
                self.decoder.push(&buf[..n], &mut frames);
                self.shared.decode_errors.store(self.decoder.errors(), Ordering::Relaxed);
                for frame in frames.drain(..) {
                    self.last_rx = Instant::now();
                    self.got_first_frame = true;
                    self.shared.frames_received.fetch_add(1, Ordering::Relaxed);
                    if let Some(previous) = self.device_seq {
                        let gap = frame.seq.wrapping_sub(previous).wrapping_sub(1);
                        if gap > 0 {
                            self.shared.frames_lost.fetch_add(gap as u64, Ordering::Relaxed);
                        }
                    }
                    self.device_seq = Some(frame.seq);
                    match DeviceMessage::parse(&frame) {
                        Ok(message) => self.on_message(message),
                        Err(e) => log::warn!("ignoring frame 0x{:02X}: {e}", frame.kind),
                    }
                }
            }

            // 3. Reply timeout and retry.
            if let Some(pending) = &self.in_flight {
                if pending.sent_at.elapsed() >= REPLY_TIMEOUT {
                    let mut pending = self.in_flight.take().expect("checked above");
                    if pending.retries_left > 0 {
                        pending.retries_left -= 1;
                        pending.seq = self.write(&pending.command).map_err(|e| e.to_string())?;
                        pending.sent_at = Instant::now();
                        self.in_flight = Some(pending);
                    } else {
                        let _ = pending.reply.send(Err(Error::Timeout("no reply to command")));
                    }
                }
            }

            // 4. Hand samples over in small batches.
            if !self.batch.is_empty() && self.last_flush.elapsed() >= SAMPLE_FLUSH_PERIOD {
                self.last_flush = Instant::now();
                let _ = self.events.send(SessionEvent::Samples(std::mem::take(&mut self.batch)));
            }

            // 5. Link supervision.
            let limit = if self.got_first_frame { LINK_TIMEOUT } else { BOOT_GRACE };
            if self.last_rx.elapsed() > limit {
                return Err(if self.got_first_frame {
                    "the device stopped responding".to_string()
                } else {
                    "no ThrustBench device answered on this port".to_string()
                });
            }
        }
    }

    fn on_message(&mut self, message: DeviceMessage) {
        if self.in_flight.as_ref().is_some_and(|p| p.matches(&message)) {
            let pending = self.in_flight.take().expect("checked above");
            let result = match message {
                DeviceMessage::Nack { error, .. } => Err(Error::Rejected(DeviceError::from(error))),
                other => Ok(other),
            };
            let _ = pending.reply.send(result);
            return;
        }
        match message {
            DeviceMessage::Data { time_us, samples } => {
                let t = self.clock.seconds(time_us);
                let channels = self.shared.channels.read().expect("channel lock");
                for (id, raw) in samples {
                    // Samples for channels not yet described are dropped; the
                    // stream is only enabled after the handshake read them.
                    if let Some(descriptor) = channels.iter().find(|c| c.id == id) {
                        self.batch.push(Sample { channel: id, t, raw, value: descriptor.to_engineering(raw) });
                    }
                }
            }
            DeviceMessage::Status(status) => {
                let _ = self.events.send(SessionEvent::Status(status));
            }
            // A late reply to a request that already timed out.
            other => log::debug!("unsolicited message 0x{:02X}", other.kind()),
        }
    }
}

/// Starts a session on an already opened transport.
pub fn start_session(transport: Box<dyn Transport>) -> (SessionHandle, Receiver<SessionEvent>) {
    let (tx, rx) = unbounded();
    let (event_tx, event_rx) = unbounded();
    let shared = Arc::new(Shared {
        channels: RwLock::new(Vec::new()),
        alive: AtomicBool::new(true),
        frames_received: AtomicU64::new(0),
        frames_lost: AtomicU64::new(0),
        decode_errors: AtomicU64::new(0),
    });
    let label = transport.label();
    let now = Instant::now();
    let worker = Worker {
        transport,
        rx,
        events: event_tx,
        shared: shared.clone(),
        decoder: FrameDecoder::new(),
        queue: VecDeque::new(),
        in_flight: None,
        host_seq: 0,
        device_seq: None,
        clock: TimeUnwrapper::default(),
        batch: Vec::new(),
        last_tx: now,
        last_rx: now,
        last_flush: now,
        got_first_frame: false,
    };
    thread::Builder::new()
        .name("tb-session".into())
        .spawn(move || worker.run())
        .expect("spawn session thread");
    (SessionHandle { tx, shared, label }, event_rx)
}

impl SessionHandle {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::SeqCst)
    }

    pub fn stats(&self) -> LinkStats {
        LinkStats {
            frames_received: self.shared.frames_received.load(Ordering::Relaxed),
            frames_lost: self.shared.frames_lost.load(Ordering::Relaxed),
            decode_errors: self.shared.decode_errors.load(Ordering::Relaxed),
        }
    }

    pub fn channels(&self) -> Vec<ChannelDescriptor> {
        self.shared.channels.read().expect("channel lock").clone()
    }

    /// Sends a command and waits for its reply. `retries` re-sends on
    /// timeout; use 0 for commands that must not be repeated blindly.
    pub fn request(&self, command: Command, retries: u8) -> Result<DeviceMessage> {
        if !self.is_alive() {
            return Err(Error::NotConnected);
        }
        let (reply_tx, reply_rx) = bounded(1);
        self.tx
            .send(Outgoing::Request { command, retries, reply: reply_tx })
            .map_err(|_| Error::NotConnected)?;
        // Generous outer bound: queueing plus every retry.
        match reply_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(result) => result,
            Err(_) => Err(if self.is_alive() { Error::Timeout("request queue stalled") } else { Error::NotConnected }),
        }
    }

    fn expect_ack(&self, command: Command, retries: u8) -> Result<()> {
        match self.request(command, retries)? {
            DeviceMessage::Ack { .. } => Ok(()),
            other => Err(Error::Protocol(format!("expected ACK, got 0x{:02X}", other.kind()))),
        }
    }

    /// Waits for the board to boot, reads its identity and channel list, and
    /// starts the data stream.
    pub fn handshake(&self) -> Result<(DeviceInfo, Vec<ChannelDescriptor>)> {
        let deadline = Instant::now() + BOOT_GRACE;
        let info = loop {
            match self.request(Command::GetInfo, 0) {
                Ok(DeviceMessage::Info(info)) => break info,
                Ok(other) => return Err(Error::Protocol(format!("unexpected reply 0x{:02X}", other.kind()))),
                Err(Error::Timeout(_)) if Instant::now() < deadline => continue,
                Err(e) => return Err(e),
            }
        };
        if info.protocol_version != ids::PROTOCOL_VERSION {
            return Err(Error::Protocol(format!(
                "the device speaks protocol {} but this application needs {}; update the firmware or the application",
                info.protocol_version,
                ids::PROTOCOL_VERSION
            )));
        }
        let channels = self.refresh_channels(info.channel_count)?;
        self.expect_ack(Command::SetStream(true), 3)?;
        Ok((info, channels))
    }

    /// Re-reads every channel descriptor. Call after changing parameters so
    /// the conversion to engineering units uses the new calibration.
    pub fn refresh_channels(&self, count: u8) -> Result<Vec<ChannelDescriptor>> {
        let mut channels = Vec::with_capacity(count as usize);
        for index in 0..count {
            match self.request(Command::GetChannel(index), 3)? {
                DeviceMessage::Channel { descriptor, .. } => channels.push(descriptor),
                other => return Err(Error::Protocol(format!("unexpected reply 0x{:02X}", other.kind()))),
            }
        }
        *self.shared.channels.write().expect("channel lock") = channels.clone();
        Ok(channels)
    }

    pub fn get_param(&self, scope: u8, field: u8) -> Result<[u8; 4]> {
        match self.request(Command::GetParam { scope, field }, 3)? {
            DeviceMessage::Param { value, .. } => Ok(value),
            other => Err(Error::Protocol(format!("unexpected reply 0x{:02X}", other.kind()))),
        }
    }

    pub fn set_param(&self, scope: u8, field: u8, value: [u8; 4]) -> Result<()> {
        self.expect_ack(Command::SetParam { scope, field, value }, 3)
    }

    pub fn save_config(&self) -> Result<()> {
        self.expect_ack(Command::SaveConfig, 0)
    }

    pub fn load_defaults(&self) -> Result<()> {
        self.expect_ack(Command::LoadDefaults, 0)
    }

    pub fn arm(&self) -> Result<()> {
        self.expect_ack(Command::Arm, 0)
    }

    pub fn disarm(&self) -> Result<()> {
        self.expect_ack(Command::Disarm, 3)
    }

    pub fn clear_estop(&self) -> Result<()> {
        self.expect_ack(Command::ClearEStop, 0)
    }

    /// Throttle as a fraction 0..1.
    pub fn set_throttle(&self, fraction: f64) -> Result<()> {
        let units = (fraction.clamp(0.0, 1.0) * ids::THROTTLE_FULL as f64).round() as u16;
        self.expect_ack(Command::SetThrottle(units), 0)
    }

    /// Emergency stop: bypasses the queue and is repeated in case a frame is
    /// lost. Never blocks and never fails; if the link is already down the
    /// device watchdog has stopped the motor on its own.
    pub fn estop(&self) {
        for _ in 0..3 {
            let _ = self.tx.send(Outgoing::Urgent(Command::EStop));
        }
    }

    pub fn close(&self) {
        let _ = self.tx.send(Outgoing::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ActuatorState;
    use crate::transport::sim::{SimDevice, SimOptions};

    fn wait_status(events: &Receiver<SessionEvent>, want: impl Fn(&DeviceStatus) -> bool) -> DeviceStatus {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Ok(SessionEvent::Status(status)) = events.recv_timeout(Duration::from_millis(200)) {
                if want(&status) {
                    return status;
                }
            }
        }
        panic!("status condition not reached");
    }

    #[test]
    fn time_unwrapper_handles_wrap_and_reordering() {
        let mut clock = TimeUnwrapper::default();
        assert_eq!(clock.seconds(u32::MAX - 999), 0.0);
        assert!((clock.seconds(1000) - 0.002).abs() < 1e-9); // wrapped
        assert!((clock.seconds(500) - 0.0015).abs() < 1e-9); // slightly late
        assert!((clock.seconds(2000) - 0.003).abs() < 1e-9);
    }

    #[test]
    fn handshake_streams_calibrated_samples() {
        let (session, events) = start_session(Box::new(SimDevice::new(SimOptions::default())));
        let (info, channels) = session.handshake().unwrap();
        assert_eq!(info.channel_count, 3);
        assert_eq!(channels[0].name, "thrust");
        let mut seen = 0;
        let deadline = Instant::now() + Duration::from_secs(2);
        while seen < 20 && Instant::now() < deadline {
            if let Ok(SessionEvent::Samples(batch)) = events.recv_timeout(Duration::from_millis(200)) {
                seen += batch.len();
            }
        }
        assert!(seen >= 20, "only {seen} samples arrived");
        assert_eq!(session.stats().frames_lost, 0);
        session.close();
    }

    #[test]
    fn throttle_is_refused_until_armed_and_estop_latches() {
        let (session, events) = start_session(Box::new(SimDevice::new(SimOptions::default())));
        session.handshake().unwrap();
        assert!(matches!(session.set_throttle(0.2), Err(Error::Rejected(DeviceError::NotAllowed))));
        session.arm().unwrap();
        session.set_throttle(0.2).unwrap();
        wait_status(&events, |s| s.actuator == ActuatorState::Armed && s.throttle > 0.05);
        session.estop();
        let status = wait_status(&events, |s| s.actuator == ActuatorState::EStop);
        assert_eq!(status.throttle, 0.0);
        // Latched: arming is refused until the stop is cleared.
        assert!(session.arm().is_err());
        session.clear_estop().unwrap();
        session.arm().unwrap();
        session.close();
    }

    #[test]
    fn device_ceiling_clamps_throttle() {
        let (session, events) = start_session(Box::new(SimDevice::new(SimOptions::default())));
        session.handshake().unwrap();
        session.arm().unwrap();
        session.set_throttle(1.0).unwrap();
        thread::sleep(Duration::from_millis(1500));
        let status = wait_status(&events, |s| s.actuator == ActuatorState::Armed);
        assert!(status.throttle <= 0.6 + 1e-9, "throttle {} above device ceiling", status.throttle);
        session.close();
    }

    #[test]
    fn lost_link_is_reported_and_unblocks_callers() {
        let options = SimOptions { disconnect_after: Some(Duration::from_millis(400)), ..Default::default() };
        let (session, events) = start_session(Box::new(SimDevice::new(options)));
        session.handshake().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut lost = false;
        while Instant::now() < deadline && !lost {
            lost = matches!(events.recv_timeout(Duration::from_millis(200)), Ok(SessionEvent::Lost(_)));
        }
        assert!(lost);
        assert!(!session.is_alive());
        assert!(matches!(session.arm(), Err(Error::NotConnected)));
    }
}
