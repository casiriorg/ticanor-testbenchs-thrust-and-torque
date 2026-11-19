//! Runs the desktop core against the REAL firmware code, built for the PC by
//! `firmware/host_sim/build.sh`. This is the test that proves both sides of
//! the wire protocol agree.
//!
//! Skipped unless the TB_HOST_SIM environment variable points to the binary:
//!
//! ```text
//! firmware/host_sim/build.sh /tmp/tb_host_sim
//! TB_HOST_SIM=/tmp/tb_host_sim cargo test -p tb-core --test firmware_sim
//! ```

use std::io::{self, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use tb_core::device::{start_session, SessionEvent, SessionHandle};
use tb_core::domain::{ActuatorState, DeviceStatus};
use tb_core::error::DeviceError;
use tb_core::protocol::ids::{channel_field, global_field, SCOPE_GLOBAL};
use tb_core::transport::Transport;
use tb_core::Error;

/// Talks to a child process over its stdin/stdout.
struct ProcessTransport {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Vec<u8>>,
    pending: Vec<u8>,
}

impl ProcessTransport {
    fn spawn(binary: &str, eeprom: Option<&std::path::Path>) -> Self {
        let mut command = Command::new(binary);
        command.stdin(Stdio::piped()).stdout(Stdio::piped());
        if let Some(path) = eeprom {
            command.env("TB_EEPROM_FILE", path);
        }
        let mut child = command.spawn().expect("start host firmware");
        let stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 256];
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self { child, stdin, rx, pending: Vec::new() }
    }
}

impl Transport for ProcessTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pending.is_empty() {
            match self.rx.recv_timeout(Duration::from_millis(5)) {
                Ok(chunk) => self.pending = chunk,
                Err(RecvTimeoutError::Timeout) => return Ok(0),
                Err(RecvTimeoutError::Disconnected) => return Err(io::ErrorKind::BrokenPipe.into()),
            }
        }
        let n = buf.len().min(self.pending.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.stdin.write_all(data)?;
        self.stdin.flush()
    }

    fn label(&self) -> String {
        "host firmware".into()
    }
}

impl Drop for ProcessTransport {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn binary() -> Option<String> {
    let path = std::env::var("TB_HOST_SIM").ok()?;
    std::path::Path::new(&path).exists().then_some(path)
}

fn wait_status(events: &Receiver<SessionEvent>, want: impl Fn(&DeviceStatus) -> bool) -> DeviceStatus {
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if let Ok(SessionEvent::Status(status)) = events.recv_timeout(Duration::from_millis(200)) {
            if want(&status) {
                return status;
            }
        }
    }
    panic!("status condition not reached");
}

/// Mean thrust (channel 1) over the next `seconds`.
fn mean_thrust(events: &Receiver<SessionEvent>, seconds: f64) -> f64 {
    let end = Instant::now() + Duration::from_secs_f64(seconds);
    let (mut sum, mut n) = (0.0, 0usize);
    while Instant::now() < end {
        if let Ok(SessionEvent::Samples(batch)) = events.recv_timeout(Duration::from_millis(100)) {
            for s in batch.iter().filter(|s| s.channel == 1) {
                sum += s.value;
                n += 1;
            }
        }
    }
    assert!(n > 10, "only {n} thrust samples");
    sum / n as f64
}

fn connect(binary: &str, eeprom: Option<&std::path::Path>) -> (SessionHandle, crossbeam_channel::Receiver<SessionEvent>) {
    let (session, events) = start_session(Box::new(ProcessTransport::spawn(binary, eeprom)));
    session.handshake().expect("handshake with host firmware");
    (session, events)
}

// crossbeam and std receivers share the methods used above.
fn as_std(events: crossbeam_channel::Receiver<SessionEvent>) -> Receiver<SessionEvent> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for event in events.iter() {
            if tx.send(event).is_err() {
                break;
            }
        }
    });
    rx
}

#[test]
fn real_firmware_handshake_stream_and_actuator_safety() {
    let Some(binary) = binary() else {
        eprintln!("TB_HOST_SIM not set: skipping the firmware integration test");
        return;
    };
    let (session, events) = connect(&binary, None);
    let events = as_std(events);

    let channels = session.channels();
    let names: Vec<&str> = channels.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["thrust", "torque", "throttle"]);
    assert_eq!(channels[0].rate_hz, 80);
    assert!((channels[0].scale - 21_898.2).abs() < 1.0, "nominal scale {}", channels[0].scale);
    assert!(!channels[0].calibrated);

    // Tare the way the application does: write the raw mean as the offset.
    session.set_param(1, channel_field::OFFSET, 84_210i32.to_le_bytes()).unwrap();
    session.refresh_channels(3).unwrap();
    // Discard samples converted before the new offset was in place.
    std::thread::sleep(Duration::from_millis(100));
    while events.try_recv().is_ok() {}
    let idle = mean_thrust(&events, 0.5);
    assert!(idle.abs() < 0.05, "idle thrust after tare: {idle} N");

    // Throttle is refused until armed, then drives the simulated motor.
    assert!(matches!(session.set_throttle(0.3), Err(Error::Rejected(DeviceError::NotAllowed))));
    session.arm().unwrap();
    session.set_throttle(0.5).unwrap();
    wait_status(&events, |s| s.actuator == ActuatorState::Armed && (s.throttle - 0.5).abs() < 0.01);
    let running = mean_thrust(&events, 0.6);
    assert!((running - 7.5).abs() < 0.6, "thrust at 50 %: {running} N (model: 30 N x 0.5^2)");

    // Above the firmware ceiling (60 % by default) the command is clamped.
    session.set_throttle(1.0).unwrap();
    let status = wait_status(&events, |s| s.throttle > 0.5999);
    assert!(status.throttle <= 0.6 + 1e-9);
    assert_eq!(status.pulse_us, 1600);

    // Safety settings cannot change under a running motor.
    assert!(matches!(
        session.set_param(SCOPE_GLOBAL, global_field::THROTTLE_MAX, 10_000u32.to_le_bytes()),
        Err(Error::Rejected(DeviceError::NotAllowed))
    ));
    assert!(matches!(session.save_config(), Err(Error::Rejected(DeviceError::NotAllowed))));

    // Emergency stop is immediate and latched.
    session.estop();
    let status = wait_status(&events, |s| s.actuator == ActuatorState::EStop);
    assert_eq!(status.throttle, 0.0);
    assert_eq!(status.pulse_us, 1000);
    assert!(session.arm().is_err());
    session.clear_estop().unwrap();
    session.arm().unwrap();
    session.disarm().unwrap();

    assert_eq!(session.stats().frames_lost, 0);
    assert_eq!(session.stats().decode_errors, 0);
    session.close();
}

#[test]
fn real_firmware_persists_calibration_in_eeprom() {
    let Some(binary) = binary() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let eeprom = dir.path().join("eeprom.bin");

    let (session, _events) = connect(&binary, Some(&eeprom));
    let uid = session.get_param(SCOPE_GLOBAL, global_field::DEVICE_UID).unwrap();
    session.set_param(2, channel_field::SCALE, 123_456.0f32.to_le_bytes()).unwrap();
    session.set_param(2, channel_field::CAL_TIME, 1_800_000_000u32.to_le_bytes()).unwrap();
    session.set_param(SCOPE_GLOBAL, global_field::WATCHDOG_MS, 500u32.to_le_bytes()).unwrap();
    // Out-of-range values are refused and leave the setting untouched.
    assert!(matches!(
        session.set_param(SCOPE_GLOBAL, global_field::WATCHDOG_MS, 5u32.to_le_bytes()),
        Err(Error::Rejected(DeviceError::BadValue))
    ));
    session.save_config().unwrap();
    session.close();
    std::thread::sleep(Duration::from_millis(200));

    // "Power cycle": a new process reads the same EEPROM image.
    let (session, _events) = connect(&binary, Some(&eeprom));
    let torque = &session.channels()[1];
    assert_eq!(torque.scale, 123_456.0);
    assert!(torque.calibrated);
    assert_eq!(u32::from_le_bytes(session.get_param(SCOPE_GLOBAL, global_field::WATCHDOG_MS).unwrap()), 500);
    assert_eq!(session.get_param(SCOPE_GLOBAL, global_field::DEVICE_UID).unwrap(), uid);
    session.close();
}
