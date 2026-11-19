//! The bench service: one object that owns the whole runtime state and that
//! the UI layer drives through plain method calls.
//!
//! A background thread (the "tick", 50 Hz) finds and connects to the board,
//! pumps samples, applies the safety rules, feeds the recorder and runs
//! throttle profiles. The UI never touches the device directly.
//!
//! Locking: all state sits behind one mutex that is held only for short,
//! non-blocking work. The two slow operations are kept outside it:
//! connecting (several seconds while the board boots) and emergency stop,
//! which uses its own handle so it can never wait for anything else.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use serde::Serialize;

use crate::actuators::mavlink::MavlinkDriver;
use crate::actuators::{ManualDriver, NanoPwmDriver, ThrottleDriver};
use crate::analysis::stats::{mean, std_dev};
use crate::calibration::{reference_value, CalFit, CalPoint, ReferenceKind};
use crate::config::{defaults, AppSettings, SettingsStore};
use crate::device::{start_session, LinkStats, SessionEvent, SessionHandle};
use crate::domain::{ActuatorState, ChannelDescriptor, DeviceInfo, DeviceStatus, DriverKind, Sample};
use crate::error::{Error, Result};
use crate::profile::Profile;
use crate::protocol::ids::{self, capability, channel_field, sensor};
use crate::recording::recorder::{Recorder, RecordingOptions, RecordingState, SessionMeta};
use crate::recording::TestSetup;
use crate::safety::{ArmPreconditions, SafetySupervisor, Trip};
use crate::transport::serial::{list_ports, PortCandidate, PortRole, SerialTransport};
use crate::transport::sim::{SimDevice, SimOptions};
use crate::transport::Transport;

const TICK: Duration = Duration::from_millis(20);
const SCAN_PERIOD: Duration = Duration::from_millis(1500);
const SNAPSHOT_PERIOD: Duration = Duration::from_millis(200);
/// Re-send the setpoint at least this often while armed.
const SETPOINT_REFRESH: Duration = Duration::from_millis(100);
const MAX_ALARMS: usize = 30;
/// Channel carrying the commanded throttle.
const THROTTLE_CHANNEL: u8 = 16;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConnectionState {
    /// Automatic search is off and nothing is connected.
    Idle,
    Searching,
    Connecting { port: String },
    Connected { port: String },
    Lost { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlarmLevel {
    Info,
    Warning,
    Critical,
}

/// Something the operator must know about. `code` selects the translated
/// text in the UI; `message` is the English fallback with details.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alarm {
    pub id: u64,
    pub level: AlarmLevel,
    pub code: String,
    pub message: String,
    pub time: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DriverOption {
    pub kind: DriverKind,
    pub available: bool,
    /// Port or reason, for display.
    pub detail: String,
    pub experimental: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordingStatus {
    pub id: String,
    pub path: String,
    pub elapsed_s: f64,
    pub rows: u64,
    pub dropped_rows: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileStatus {
    pub name: String,
    pub elapsed_s: f64,
    pub total_s: f64,
}

/// Everything the UI needs to draw its state. Sent on change and at 5 Hz.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BenchSnapshot {
    pub connection: ConnectionState,
    pub simulated: bool,
    pub device: Option<DeviceInfo>,
    pub channels: Vec<ChannelDescriptor>,
    pub status: Option<DeviceStatus>,
    pub link: LinkStats,
    /// `None` until the operator chooses how the ESC is controlled.
    pub driver: Option<DriverKind>,
    pub driver_options: Vec<DriverOption>,
    pub actuator: Option<ActuatorState>,
    /// Throttle currently requested by the operator or the profile, 0..1.
    pub setpoint: f64,
    pub recording: Option<RecordingStatus>,
    pub profile: Option<ProfileStatus>,
    /// Seconds since the last tare, if any was done this session.
    pub tare_age_s: Option<f64>,
    pub alarms: Vec<Alarm>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum BenchEvent {
    Snapshot(Box<BenchSnapshot>),
    Samples(Vec<Sample>),
    Alarm(Alarm),
}

/// Where the service publishes events (the Tauri layer forwards them to the
/// window; tests collect them).
pub trait EventSink: Send + Sync {
    fn emit(&self, event: BenchEvent);
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RawCapture {
    pub mean: f64,
    pub std: f64,
    pub samples: usize,
}

struct Capture {
    channel: u8,
    values: Vec<f64>,
}

struct Connection {
    session: SessionHandle,
    events: Receiver<SessionEvent>,
    simulated: bool,
    device: DeviceInfo,
    /// Added to device time so recordings stay continuous across reconnects.
    time_offset: f64,
}

struct Inner {
    settings: AppSettings,
    settings_store: SettingsStore,
    default_data_dir: PathBuf,
    app_version: String,
    supervisor: SafetySupervisor,
    connection: Option<Connection>,
    connection_state: ConnectionState,
    want_simulator: bool,
    /// Port chosen by hand; overrides automatic discovery.
    manual_port: Option<String>,
    last_scan: Option<Instant>,
    status: Option<DeviceStatus>,
    driver: Option<Box<dyn ThrottleDriver>>,
    /// Remembered across reconnects so the operator is not asked again.
    driver_kind: Option<DriverKind>,
    driver_options: Vec<DriverOption>,
    setpoint: f64,
    last_sent_setpoint: Option<(f64, Instant)>,
    recorder: Option<Recorder>,
    profile: Option<(Profile, Instant)>,
    capture: Option<Capture>,
    latest_t: f64,
    lost_at: Option<Instant>,
    tare_at: Option<Instant>,
    alarms: VecDeque<Alarm>,
    alarm_seq: u64,
    dirty: bool,
    /// State the board was just commanded into. Its status report arrives
    /// only every 100 ms; until then this stands in, so a command issued
    /// right after arming is not judged against stale information.
    assumed: Option<(ActuatorState, Instant)>,
}

/// How long a commanded state is trusted before the board confirms it.
const ASSUMED_STATE_WINDOW: Duration = Duration::from_millis(400);

impl Inner {
    fn actuator_state(&self) -> Option<ActuatorState> {
        if let Some(state) = self.driver.as_ref().and_then(|d| d.state()) {
            return Some(state);
        }
        let reported = self.status.as_ref().map(|s| s.actuator)?;
        match self.assumed {
            Some((state, at)) if at.elapsed() < ASSUMED_STATE_WINDOW => Some(state),
            _ => Some(reported),
        }
    }

    fn assume(&mut self, state: ActuatorState) {
        self.assumed = Some((state, Instant::now()));
        self.dirty = true;
    }

    fn is_armed(&self) -> bool {
        self.driver.as_ref().is_some_and(|d| d.can_command()) && self.actuator_state() == Some(ActuatorState::Armed)
    }

    fn alarm(&mut self, level: AlarmLevel, code: &str, message: impl Into<String>) -> Alarm {
        self.alarm_seq += 1;
        let alarm = Alarm {
            id: self.alarm_seq,
            level,
            code: code.to_string(),
            message: message.into(),
            time: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        };
        self.alarms.push_back(alarm.clone());
        while self.alarms.len() > MAX_ALARMS {
            self.alarms.pop_front();
        }
        if let Some(recorder) = self.recorder.as_mut() {
            recorder.mark_event(code, &alarm.message);
        }
        self.dirty = true;
        alarm
    }

    /// Brings the throttle to zero and forgets any pending command. Used by
    /// every automatic cut-off; `disarm` decides whether to also disarm.
    fn cut_throttle(&mut self, disarm: bool) {
        self.setpoint = 0.0;
        self.profile = None;
        self.last_sent_setpoint = None;
        self.supervisor.on_zeroed();
        if let Some(driver) = self.driver.as_mut() {
            let result = if disarm { driver.disarm() } else { driver.set_throttle(0.0) };
            if result.is_err() {
                // The polite way failed: use the one that cannot fail.
                driver.estop();
                self.assume(ActuatorState::EStop);
            } else if disarm {
                self.assume(ActuatorState::Disarmed);
            }
        }
        self.dirty = true;
    }

    fn data_dir(&self) -> PathBuf {
        self.settings.data_dir.as_ref().map(PathBuf::from).unwrap_or_else(|| self.default_data_dir.clone())
    }

    fn snapshot(&self) -> BenchSnapshot {
        BenchSnapshot {
            connection: self.connection_state.clone(),
            simulated: self.connection.as_ref().is_some_and(|c| c.simulated),
            device: self.connection.as_ref().map(|c| c.device.clone()),
            channels: self.connection.as_ref().map(|c| c.session.channels()).unwrap_or_default(),
            status: self.status.clone(),
            link: self.connection.as_ref().map(|c| c.session.stats()).unwrap_or_default(),
            driver: self.driver.as_ref().map(|d| d.kind()),
            driver_options: self.driver_options.clone(),
            actuator: self.actuator_state(),
            setpoint: self.setpoint,
            recording: self.recorder.as_ref().map(|r| RecordingStatus {
                id: r.id().to_string(),
                path: r.dir().to_string_lossy().into_owned(),
                elapsed_s: r.elapsed(),
                rows: r.rows(),
                dropped_rows: r.dropped_rows(),
            }),
            profile: self.profile.as_ref().map(|(p, started)| ProfileStatus {
                name: p.name.clone(),
                elapsed_s: started.elapsed().as_secs_f64(),
                total_s: p.total_duration(),
            }),
            tare_age_s: self.tare_at.map(|t| t.elapsed().as_secs_f64()),
            alarms: self.alarms.iter().cloned().collect(),
        }
    }

    fn build_driver(&mut self, kind: DriverKind) -> Result<Box<dyn ThrottleDriver>> {
        let connection = self.connection.as_ref().ok_or(Error::NotConnected)?;
        match kind {
            DriverKind::NanoPwm => {
                if connection.device.capabilities & capability::PWM_ACTUATOR == 0 {
                    return Err(Error::Invalid("this firmware has no PWM output".into()));
                }
                Ok(Box::new(NanoPwmDriver::new(connection.session.clone())))
            }
            DriverKind::Manual => Ok(Box::new(ManualDriver)),
            DriverKind::Mavlink => {
                let port = self
                    .driver_options
                    .iter()
                    .find(|o| o.kind == DriverKind::Mavlink && o.available)
                    .map(|o| o.detail.clone())
                    .ok_or_else(|| Error::Invalid("no flight controller was detected".into()))?;
                Ok(Box::new(MavlinkDriver::connect(&port)?))
            }
        }
    }
}

fn driver_options(device: &DeviceInfo, ports: &[PortCandidate]) -> Vec<DriverOption> {
    let has_pwm = device.capabilities & capability::PWM_ACTUATOR != 0;
    let flight_controller = ports.iter().find(|p| p.role == PortRole::FlightController);
    vec![
        DriverOption {
            kind: DriverKind::NanoPwm,
            available: has_pwm,
            detail: if has_pwm { "D9".into() } else { "not supported by this firmware".into() },
            experimental: false,
        },
        DriverOption {
            kind: DriverKind::Mavlink,
            available: flight_controller.is_some(),
            detail: flight_controller.map(|p| p.port.clone()).unwrap_or_default(),
            experimental: true,
        },
        DriverOption { kind: DriverKind::Manual, available: true, detail: String::new(), experimental: false },
    ]
}

pub struct Bench {
    inner: Arc<Mutex<Inner>>,
    /// Separate from `inner` on purpose: see the module documentation.
    estop_session: Arc<Mutex<Option<SessionHandle>>>,
    sink: Arc<dyn EventSink>,
    stop: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

pub struct BenchConfig {
    /// Folder for `settings.json`.
    pub config_dir: PathBuf,
    /// Recordings folder used when the settings do not name one.
    pub default_data_dir: PathBuf,
    pub app_version: String,
}

impl Bench {
    pub fn start(config: BenchConfig, sink: Arc<dyn EventSink>) -> Arc<Self> {
        let settings_store = SettingsStore::new(&config.config_dir);
        let (settings, problem) = settings_store.load();
        let mut inner = Inner {
            supervisor: SafetySupervisor::new(settings.safety.clone()),
            connection_state: if settings.auto_connect { ConnectionState::Searching } else { ConnectionState::Idle },
            settings,
            settings_store,
            default_data_dir: config.default_data_dir,
            app_version: config.app_version,
            connection: None,
            want_simulator: false,
            manual_port: None,
            last_scan: None,
            status: None,
            driver: None,
            driver_kind: None,
            driver_options: Vec::new(),
            setpoint: 0.0,
            last_sent_setpoint: None,
            recorder: None,
            profile: None,
            capture: None,
            latest_t: 0.0,
            lost_at: None,
            tare_at: None,
            alarms: VecDeque::new(),
            alarm_seq: 0,
            dirty: true,
            assumed: None,
        };
        if let Some(problem) = problem {
            inner.alarm(AlarmLevel::Warning, "settings_invalid", format!("settings.json could not be read, defaults are in use: {problem}"));
        }
        let bench = Arc::new(Self {
            inner: Arc::new(Mutex::new(inner)),
            estop_session: Arc::new(Mutex::new(None)),
            sink,
            stop: Arc::new(AtomicBool::new(false)),
            worker: Mutex::new(None),
        });
        let for_thread = bench.clone();
        let handle = thread::Builder::new()
            .name("tb-bench".into())
            .spawn(move || for_thread.run())
            .expect("spawn bench thread");
        *bench.worker.lock().expect("worker lock") = Some(handle);
        bench
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // A panic while holding the lock must not take the safety paths down
        // with it, so a poisoned lock is still used.
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // ---------------------------------------------------------------- tick

    fn run(&self) {
        let mut last_snapshot = Instant::now() - SNAPSHOT_PERIOD;
        while !self.stop.load(Ordering::SeqCst) {
            let started = Instant::now();
            self.try_connect();
            let (samples, alarms, snapshot) = {
                let mut inner = self.lock();
                let mut alarms = Vec::new();
                let samples = self.pump(&mut inner, &mut alarms);
                self.supervise(&mut inner, &mut alarms);
                let due = inner.dirty || last_snapshot.elapsed() >= SNAPSHOT_PERIOD;
                let snapshot = due.then(|| {
                    inner.dirty = false;
                    inner.snapshot()
                });
                (samples, alarms, snapshot)
            };
            // Events are emitted outside the lock: a slow UI cannot hold it.
            if !samples.is_empty() {
                self.sink.emit(BenchEvent::Samples(samples));
            }
            for alarm in alarms {
                self.sink.emit(BenchEvent::Alarm(alarm));
            }
            if let Some(snapshot) = snapshot {
                last_snapshot = Instant::now();
                self.sink.emit(BenchEvent::Snapshot(Box::new(snapshot)));
            }
            thread::sleep(TICK.saturating_sub(started.elapsed()));
        }
    }

    /// Looks for a board and connects to it. Runs outside the state lock
    /// because the handshake waits for the board's bootloader.
    fn try_connect(&self) {
        let (simulator, port) = {
            let mut inner = self.lock();
            if inner.connection.is_some() {
                return;
            }
            if inner.want_simulator {
                (true, "simulator".to_string())
            } else {
                let allowed = inner.settings.auto_connect || inner.manual_port.is_some();
                if !allowed || inner.last_scan.is_some_and(|t| t.elapsed() < SCAN_PERIOD) {
                    return;
                }
                inner.last_scan = Some(Instant::now());
                let candidate = inner.manual_port.clone().or_else(|| {
                    list_ports().into_iter().find(|p| p.role == PortRole::AcquisitionBoard).map(|p| p.port)
                });
                let Some(port) = candidate else { return };
                inner.connection_state = ConnectionState::Connecting { port: port.clone() };
                inner.dirty = true;
                (false, port)
            }
        };

        let attempt = || -> Result<(SessionHandle, Receiver<SessionEvent>, DeviceInfo)> {
            let transport: Box<dyn Transport> = if simulator {
                Box::new(SimDevice::new(SimOptions::default()))
            } else {
                Box::new(SerialTransport::open(&port, ids::BAUD_RATE)?)
            };
            let (session, events) = start_session(transport);
            match session.handshake() {
                Ok((device, _channels)) => Ok((session, events, device)),
                Err(e) => {
                    session.close();
                    Err(e)
                }
            }
        };
        let result = attempt();

        let mut inner = self.lock();
        match result {
            Ok((session, events, device)) => {
                *self.estop_session.lock().unwrap_or_else(|p| p.into_inner()) = Some(session.clone());
                let time_offset = match inner.lost_at.take() {
                    Some(lost_at) if inner.latest_t > 0.0 => inner.latest_t + lost_at.elapsed().as_secs_f64(),
                    _ => 0.0,
                };
                inner.driver_options = driver_options(&device, &if simulator { Vec::new() } else { list_ports() });
                let defaults_loaded = device.defaults_loaded;
                inner.connection = Some(Connection { session, events, simulated: simulator, device, time_offset });
                inner.connection_state = ConnectionState::Connected { port };
                inner.status = None;
                // Restore the previous choice (never the armed state).
                if let Some(kind) = inner.driver_kind {
                    inner.driver = inner.build_driver(kind).ok();
                }
                if defaults_loaded {
                    let alarm = inner.alarm(AlarmLevel::Warning, "device_defaults", "The board started with factory defaults: its calibration is nominal. Calibrate before trusting the values.");
                    drop(inner);
                    self.sink.emit(BenchEvent::Alarm(alarm));
                    return;
                }
                inner.dirty = true;
            }
            Err(e) => {
                if simulator {
                    inner.want_simulator = false;
                }
                let next = if inner.manual_port.take().is_some() || simulator {
                    ConnectionState::Lost { reason: e.to_string() }
                } else {
                    // During automatic search a silent port is normal (another
                    // CH340 device, board still booting): keep looking quietly.
                    log::info!("{port}: {e}");
                    ConnectionState::Searching
                };
                inner.connection_state = next;
                inner.dirty = true;
            }
        }
    }

    /// Moves session events into the application: samples, status, link loss.
    fn pump(&self, inner: &mut Inner, alarms: &mut Vec<Alarm>) -> Vec<Sample> {
        let Some(connection) = inner.connection.as_ref() else {
            return Vec::new();
        };
        let events: Vec<SessionEvent> = connection.events.try_iter().collect();
        let time_offset = connection.time_offset;
        let board_owns_throttle = inner.driver.as_ref().map_or(true, |d| d.kind() == DriverKind::NanoPwm);
        let armed = inner.is_armed();
        let mut out = Vec::new();
        let mut lost = None;

        for event in events {
            match event {
                SessionEvent::Samples(batch) => {
                    for mut sample in batch {
                        sample.t += time_offset;
                        inner.latest_t = inner.latest_t.max(sample.t);
                        // With another driver the board's throttle channel is
                        // always zero; the commanded value is injected below.
                        if sample.channel == THROTTLE_CHANNEL && !board_owns_throttle {
                            continue;
                        }
                        if let Some(capture) = inner.capture.as_mut().filter(|c| c.channel == sample.channel) {
                            capture.values.push(sample.raw as f64);
                        }
                        if armed {
                            let sensor_type = inner
                                .connection
                                .as_ref()
                                .and_then(|c| c.session.channels().into_iter().find(|d| d.id == sample.channel))
                                .map(|d| d.sensor_type);
                            if let Some(trip) = sensor_type.and_then(|s| inner.supervisor.check_sample(s, sample.value)) {
                                inner.cut_throttle(true);
                                let (code, text) = match trip {
                                    Trip::ThrustLimit { value, limit } => ("thrust_limit", format!("Thrust {value:.1} N exceeded the limit of {limit:.1} N. Throttle cut and disarmed.")),
                                    Trip::TorqueLimit { value, limit } => ("torque_limit", format!("Torque {value:.3} N·m exceeded the limit of {limit:.3} N·m. Throttle cut and disarmed.")),
                                    Trip::UiSilent => unreachable!("not produced by check_sample"),
                                };
                                alarms.push(inner.alarm(AlarmLevel::Critical, code, text));
                            }
                        }
                        if let Some(recorder) = inner.recorder.as_mut() {
                            recorder.push(&sample);
                        }
                        out.push(sample);
                    }
                }
                SessionEvent::Status(status) => {
                    let previous = inner.status.as_ref().map(|s| s.actuator);
                    if status.actuator == ActuatorState::Failsafe && previous != Some(ActuatorState::Failsafe) {
                        inner.setpoint = 0.0;
                        inner.profile = None;
                        inner.supervisor.on_zeroed();
                        alarms.push(inner.alarm(AlarmLevel::Critical, "device_failsafe", "The board stopped the motor because it lost contact with this application. Disarm to reset."));
                    }
                    if previous != Some(status.actuator) {
                        inner.dirty = true;
                    }
                    inner.status = Some(status);
                }
                SessionEvent::Lost(reason) => lost = Some(reason),
            }
        }

        // Synthesise the throttle channel for drivers the board cannot see.
        if !board_owns_throttle && inner.driver.as_ref().is_some_and(|d| d.can_command()) && !out.is_empty() {
            let value = inner.last_sent_setpoint.map_or(0.0, |(v, _)| v);
            let sample = Sample { channel: THROTTLE_CHANNEL, t: inner.latest_t, raw: (value * ids::THROTTLE_FULL as f64) as i32, value };
            if let Some(recorder) = inner.recorder.as_mut() {
                recorder.push(&sample);
            }
            out.push(sample);
        }

        if let Some(reason) = lost {
            self.on_link_lost(inner, reason, alarms);
        }
        out
    }

    fn on_link_lost(&self, inner: &mut Inner, reason: String, alarms: &mut Vec<Alarm>) {
        let was_armed = inner.is_armed();
        *self.estop_session.lock().unwrap_or_else(|p| p.into_inner()) = None;
        inner.connection = None;
        inner.status = None;
        inner.assumed = None;
        inner.driver = None; // rebuilt, disarmed, after reconnecting
        inner.setpoint = 0.0;
        inner.profile = None;
        inner.last_sent_setpoint = None;
        inner.capture = None;
        inner.supervisor.on_zeroed();
        inner.lost_at = Some(Instant::now());
        inner.last_scan = None;
        inner.want_simulator = false;
        inner.connection_state = ConnectionState::Lost { reason: reason.clone() };
        let (level, text) = if was_armed {
            (AlarmLevel::Critical, format!("Connection lost while ARMED ({reason}). The board cuts the throttle by itself within its watchdog time. Check the motor has stopped before approaching."))
        } else {
            (AlarmLevel::Warning, format!("Connection lost ({reason}). Reconnecting automatically."))
        };
        alarms.push(inner.alarm(level, "link_lost", text));
    }

    /// Safety rules that run every tick, plus setpoint delivery.
    fn supervise(&self, inner: &mut Inner, alarms: &mut Vec<Alarm>) {
        let now = Instant::now();
        if let Some(driver) = inner.driver.as_mut() {
            if let Err(e) = driver.tick() {
                inner.cut_throttle(true);
                alarms.push(inner.alarm(AlarmLevel::Critical, "driver_error", format!("The throttle link failed: {e}. Throttle cut.")));
            }
        }
        if !inner.is_armed() {
            inner.last_sent_setpoint = None;
            return;
        }

        // Dead-man: a frozen or closed window must not leave a motor running.
        if inner.supervisor.ui_expired(now) {
            inner.cut_throttle(true);
            alarms.push(inner.alarm(AlarmLevel::Critical, "ui_silent", "The window stopped responding while armed. Throttle cut and disarmed."));
            return;
        }

        // Profile playback overrides the manual setpoint.
        if let Some((profile, started)) = inner.profile.as_ref() {
            match profile.setpoint_at(started.elapsed().as_secs_f64()) {
                Some(value) => inner.setpoint = value,
                None => {
                    inner.setpoint = 0.0;
                    inner.profile = None;
                    alarms.push(inner.alarm(AlarmLevel::Info, "profile_done", "The throttle profile finished."));
                }
            }
        }

        let shaped = inner.supervisor.shape_setpoint(inner.setpoint, now);
        let stale = inner.last_sent_setpoint.map_or(true, |(value, at)| (value - shaped).abs() > 1e-4 || at.elapsed() >= SETPOINT_REFRESH);
        if stale {
            let result = inner.driver.as_mut().map(|d| d.set_throttle(shaped));
            match result {
                Some(Ok(())) => inner.last_sent_setpoint = Some((shaped, now)),
                Some(Err(e)) => {
                    inner.cut_throttle(true);
                    alarms.push(inner.alarm(AlarmLevel::Critical, "throttle_rejected", format!("The throttle command failed: {e}. Throttle cut.")));
                }
                None => {}
            }
        }
    }

    // ------------------------------------------------------------- queries

    pub fn snapshot(&self) -> BenchSnapshot {
        self.lock().snapshot()
    }

    pub fn settings(&self) -> AppSettings {
        self.lock().settings.clone()
    }

    pub fn data_dir(&self) -> PathBuf {
        self.lock().data_dir()
    }

    pub fn ports(&self) -> Vec<PortCandidate> {
        list_ports()
    }

    pub fn active_recording_id(&self) -> Option<String> {
        self.lock().recorder.as_ref().map(|r| r.id().to_string())
    }

    // ---------------------------------------------------------- connection

    pub fn connect_simulator(&self) -> Result<()> {
        let mut inner = self.lock();
        if inner.connection.is_some() {
            return Err(Error::Invalid("disconnect the current device first".into()));
        }
        inner.want_simulator = true;
        inner.connection_state = ConnectionState::Connecting { port: "simulator".into() };
        inner.dirty = true;
        Ok(())
    }

    pub fn connect_port(&self, port: String) -> Result<()> {
        let mut inner = self.lock();
        if inner.connection.is_some() {
            return Err(Error::Invalid("disconnect the current device first".into()));
        }
        inner.manual_port = Some(port);
        inner.last_scan = None;
        inner.dirty = true;
        Ok(())
    }

    /// Disconnects. Refused while armed or recording, so it cannot be used to
    /// skip the stop confirmations by accident.
    pub fn disconnect(&self) -> Result<()> {
        let mut inner = self.lock();
        if inner.is_armed() {
            return Err(Error::Safety("disarm before disconnecting".into()));
        }
        if inner.recorder.is_some() {
            return Err(Error::Safety("stop the recording before disconnecting".into()));
        }
        if let Some(connection) = inner.connection.take() {
            connection.session.close();
        }
        *self.estop_session.lock().unwrap_or_else(|p| p.into_inner()) = None;
        inner.driver = None;
        inner.driver_kind = None;
        inner.status = None;
        inner.want_simulator = false;
        inner.lost_at = None;
        // Stay idle for a moment so automatic search does not grab it right back.
        inner.last_scan = Some(Instant::now() + Duration::from_secs(5));
        inner.connection_state = if inner.settings.auto_connect { ConnectionState::Searching } else { ConnectionState::Idle };
        inner.dirty = true;
        Ok(())
    }

    pub fn select_driver(&self, kind: DriverKind) -> Result<()> {
        let mut inner = self.lock();
        if inner.is_armed() {
            return Err(Error::Safety("disarm before changing the throttle mode".into()));
        }
        let driver = inner.build_driver(kind)?;
        inner.driver = Some(driver);
        inner.driver_kind = Some(kind);
        inner.setpoint = 0.0;
        inner.dirty = true;
        Ok(())
    }

    // ------------------------------------------------------------ actuator

    /// Arms the ESC output. `confirmed` is the operator's explicit statement
    /// that the propeller area is clear.
    pub fn arm(&self, confirmed: bool) -> Result<()> {
        let mut inner = self.lock();
        let pre = ArmPreconditions {
            connected: inner.connection.is_some(),
            driver_can_command: inner.driver.as_ref().is_some_and(|d| d.can_command()),
            actuator: inner.actuator_state(),
            operator_confirmed: confirmed,
        };
        inner.supervisor.check_arm(&pre)?;
        inner.setpoint = 0.0;
        inner.profile = None;
        inner.driver.as_mut().ok_or(Error::NotConnected)?.arm()?;
        inner.supervisor.on_armed(Instant::now());
        inner.assume(ActuatorState::Armed);
        if let Some(recorder) = inner.recorder.as_mut() {
            recorder.mark_event("armed", "Output armed");
        }
        inner.dirty = true;
        Ok(())
    }

    pub fn disarm(&self) -> Result<()> {
        let mut inner = self.lock();
        inner.setpoint = 0.0;
        inner.profile = None;
        inner.last_sent_setpoint = None;
        inner.supervisor.on_zeroed();
        inner.dirty = true;
        let result = match inner.driver.as_mut() {
            Some(driver) => driver.disarm(),
            None => Ok(()),
        };
        // Disarming does not release a latched emergency stop.
        if result.is_ok() && inner.actuator_state() != Some(ActuatorState::EStop) {
            inner.assume(ActuatorState::Disarmed);
        }
        result
    }

    /// Requests a throttle (0..1). The value is clamped and rate limited by
    /// the supervisor before it reaches the driver on the next tick.
    pub fn set_throttle(&self, fraction: f64) -> Result<()> {
        let mut inner = self.lock();
        if !inner.is_armed() {
            return Err(Error::Safety("arm the output first".into()));
        }
        if inner.profile.is_some() {
            return Err(Error::Safety("a profile is running; abort it to take manual control".into()));
        }
        if !fraction.is_finite() {
            return Err(Error::Invalid("throttle must be a number".into()));
        }
        inner.setpoint = fraction.clamp(0.0, inner.supervisor.limits().max_throttle);
        inner.supervisor.note_ui_alive(Instant::now());
        Ok(())
    }

    /// Emergency stop. Always succeeds, from any state, and never waits for
    /// the main state lock before stopping the motor.
    pub fn estop(&self) {
        if let Some(session) = self.estop_session.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
            session.estop();
        }
        let alarm = {
            let mut inner = self.lock();
            if let Some(driver) = inner.driver.as_mut() {
                driver.estop();
            }
            inner.setpoint = 0.0;
            inner.profile = None;
            inner.last_sent_setpoint = None;
            inner.supervisor.on_zeroed();
            inner.assume(ActuatorState::EStop);
            inner.alarm(AlarmLevel::Critical, "estop", "Emergency stop pressed. Clear it to arm again.")
        };
        self.sink.emit(BenchEvent::Alarm(alarm));
    }

    pub fn clear_estop(&self) -> Result<()> {
        let mut inner = self.lock();
        let driver_owns_state = inner.driver.as_ref().is_some_and(|d| d.state().is_some() && d.can_command());
        if driver_owns_state {
            inner.driver.as_mut().expect("checked").clear_estop()?;
        }
        // The board latches its own stop regardless of the driver in use.
        // Ask it whenever it is connected; "not allowed" only means it was
        // not latched, which is the state we want anyway.
        if let Some(connection) = inner.connection.as_ref() {
            match connection.session.clear_estop() {
                Ok(()) | Err(Error::Rejected(_)) => {}
                Err(e) => return Err(e),
            }
        }
        inner.assume(ActuatorState::Disarmed);
        Ok(())
    }

    /// Heartbeat from the window. Without it the output is disarmed.
    pub fn ui_alive(&self) {
        self.lock().supervisor.note_ui_alive(Instant::now());
    }

    pub fn run_profile(&self, profile: Profile) -> Result<()> {
        let mut inner = self.lock();
        if !inner.is_armed() {
            return Err(Error::Safety("arm the output first".into()));
        }
        profile.validate(inner.supervisor.limits().max_throttle)?;
        if let Some(recorder) = inner.recorder.as_mut() {
            recorder.mark_event("profile_start", &profile.name);
        }
        inner.profile = Some((profile, Instant::now()));
        inner.dirty = true;
        Ok(())
    }

    /// Stops the profile and brings the throttle to zero; stays armed.
    pub fn abort_profile(&self) {
        let mut inner = self.lock();
        if inner.profile.take().is_some() {
            inner.setpoint = 0.0;
            if let Some(recorder) = inner.recorder.as_mut() {
                recorder.mark_event("profile_abort", "Profile aborted by the operator");
            }
            inner.dirty = true;
        }
    }

    pub fn builtin_profiles(&self) -> Vec<Profile> {
        Profile::builtins(self.lock().supervisor.limits().max_throttle)
    }

    // ----------------------------------------------------- device settings

    fn session(&self) -> Result<SessionHandle> {
        self.lock().connection.as_ref().map(|c| c.session.clone()).ok_or(Error::NotConnected)
    }

    fn refresh_channels(&self, session: &SessionHandle) -> Result<()> {
        let count = self.lock().connection.as_ref().map(|c| c.device.channel_count).ok_or(Error::NotConnected)?;
        session.refresh_channels(count)?;
        // Samples converted with the previous calibration are still in
        // transit for a few tens of milliseconds; let them pass so that a
        // recording started right after a tare does not begin with them.
        thread::sleep(Duration::from_millis(100));
        self.lock().dirty = true;
        Ok(())
    }

    pub fn set_channel_param(&self, channel: u8, field: u8, value: [u8; 4]) -> Result<()> {
        let session = self.session()?;
        session.set_param(channel, field, value)?;
        self.refresh_channels(&session)
    }

    pub fn get_global_param(&self, field: u8) -> Result<u32> {
        Ok(u32::from_le_bytes(self.session()?.get_param(ids::SCOPE_GLOBAL, field)?))
    }

    pub fn set_global_param(&self, field: u8, value: u32) -> Result<()> {
        self.session()?.set_param(ids::SCOPE_GLOBAL, field, value.to_le_bytes())
    }

    /// Persists the board's configuration to its EEPROM.
    pub fn save_device_config(&self) -> Result<()> {
        if self.lock().is_armed() {
            return Err(Error::Safety("disarm before saving to the board".into()));
        }
        self.session()?.save_config()
    }

    pub fn load_device_defaults(&self) -> Result<()> {
        if self.lock().is_armed() {
            return Err(Error::Safety("disarm before restoring defaults".into()));
        }
        let session = self.session()?;
        session.load_defaults()?;
        self.refresh_channels(&session)
    }

    /// Writes the nominal scale of a catalogued load cell to a channel.
    /// The channel is marked uncalibrated until a real calibration is done.
    pub fn apply_load_cell_profile(&self, channel: u8, profile_id: &str, torque_arm_mm: Option<f64>) -> Result<()> {
        let profile = defaults()
            .load_cell_profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| Error::Invalid(format!("unknown load cell profile {profile_id}")))?;
        let session = self.session()?;
        let descriptor = session.channels().into_iter().find(|c| c.id == channel).ok_or_else(|| Error::Invalid("unknown channel".into()))?;
        let mut scale = profile.nominal_counts_per_newton(descriptor.gain.max(64), 9.80665);
        if descriptor.sensor_type == sensor::LOAD_CELL_TORQUE {
            let arm = torque_arm_mm.filter(|a| *a > 0.0).ok_or_else(|| Error::Invalid("the torque arm length is required".into()))?;
            scale /= arm * 1e-3;
        }
        session.set_param(channel, channel_field::SCALE, (scale as f32).to_le_bytes())?;
        session.set_param(channel, channel_field::CAL_TIME, 0u32.to_le_bytes())?;
        self.refresh_channels(&session)
    }

    /// Averages a channel's raw reading for `seconds`. Blocks the caller,
    /// not the tick thread.
    pub fn capture_raw(&self, channel: u8, seconds: f64) -> Result<RawCapture> {
        {
            let mut inner = self.lock();
            if inner.connection.is_none() {
                return Err(Error::NotConnected);
            }
            if inner.capture.is_some() {
                return Err(Error::Invalid("another capture is in progress".into()));
            }
            inner.capture = Some(Capture { channel, values: Vec::new() });
        }
        thread::sleep(Duration::from_secs_f64(seconds.clamp(0.2, 30.0)));
        let capture = self.lock().capture.take().ok_or(Error::NotConnected)?;
        if capture.values.len() < 5 {
            return Err(Error::Invalid("the channel produced too few samples; is it enabled?".into()));
        }
        Ok(RawCapture { mean: mean(&capture.values), std: std_dev(&capture.values), samples: capture.values.len() })
    }

    /// Zeroes a channel at the current load. Refused while armed: a tare
    /// taken with the propeller turning would hide real thrust.
    pub fn tare(&self, channel: u8) -> Result<RawCapture> {
        if self.lock().is_armed() {
            return Err(Error::Safety("disarm before taring".into()));
        }
        let capture = self.capture_raw(channel, 1.0)?;
        let session = self.session()?;
        session.set_param(channel, channel_field::OFFSET, (capture.mean.round() as i32).to_le_bytes())?;
        self.refresh_channels(&session)?;
        let mut inner = self.lock();
        inner.tare_at = Some(Instant::now());
        if let Some(recorder) = inner.recorder.as_mut() {
            recorder.mark_event("tare", &format!("Channel {channel} tared"));
        }
        Ok(capture)
    }

    /// Captures one calibration point with a known mass on the fixture.
    pub fn capture_calibration_point(&self, channel: u8, kind: ReferenceKind, mass_g: f64) -> Result<CalPoint> {
        if self.lock().is_armed() {
            return Err(Error::Safety("disarm before calibrating".into()));
        }
        let (gravity, options) = {
            let inner = self.lock();
            (inner.settings.gravity, inner.settings.calibration.clone())
        };
        let capture = self.capture_raw(channel, options.capture_seconds)?;
        Ok(CalPoint {
            mass_g,
            reference: reference_value(kind, mass_g, gravity, &options),
            raw_mean: capture.mean,
            raw_std: capture.std,
            samples: capture.samples,
        })
    }

    /// Writes a fitted calibration to the board and saves it to EEPROM.
    pub fn apply_calibration(&self, channel: u8, fit: &CalFit) -> Result<()> {
        if self.lock().is_armed() {
            return Err(Error::Safety("disarm before calibrating".into()));
        }
        let session = self.session()?;
        let now = chrono::Utc::now().timestamp().max(1) as u32;
        session.set_param(channel, channel_field::SCALE, (fit.scale as f32).to_le_bytes())?;
        session.set_param(channel, channel_field::OFFSET, (fit.offset.round() as i32).to_le_bytes())?;
        session.set_param(channel, channel_field::INVERT, (fit.invert as u32).to_le_bytes())?;
        session.set_param(channel, channel_field::CAL_TIME, now.to_le_bytes())?;
        session.save_config()?;
        self.refresh_channels(&session)?;
        self.lock().tare_at = Some(Instant::now());
        Ok(())
    }

    // ------------------------------------------------------------ settings

    pub fn update_settings(&self, settings: AppSettings) -> Result<()> {
        settings.validate()?;
        let mut inner = self.lock();
        if inner.is_armed() && settings.safety != inner.settings.safety {
            return Err(Error::Safety("disarm before changing safety limits".into()));
        }
        if inner.recorder.is_some() && settings.data_dir != inner.settings.data_dir {
            return Err(Error::Invalid("stop the recording before changing the data folder".into()));
        }
        inner.settings_store.save(&settings)?;
        inner.supervisor.set_limits(settings.safety.clone());
        inner.settings = settings;
        inner.dirty = true;
        Ok(())
    }

    // ----------------------------------------------------------- recording

    pub fn start_recording(&self, setup: TestSetup) -> Result<RecordingStatus> {
        let mut inner = self.lock();
        if inner.recorder.is_some() {
            return Err(Error::Recording("a recording is already running".into()));
        }
        let connection = inner.connection.as_ref().ok_or(Error::NotConnected)?;
        let options = RecordingOptions {
            root: inner.data_dir(),
            naming: inner.settings.csv.column_naming,
            include_raw: inner.settings.csv.include_raw,
            channel_names: inner.settings.channel_names.clone(),
            app_version: inner.app_version.clone(),
            gravity: inner.settings.gravity,
        };
        let driver = inner.driver.as_ref().map_or(DriverKind::Manual, |d| d.kind());
        let recorder = Recorder::start(&options, &setup, Some(connection.device.clone()), &connection.session.channels(), driver)?;
        let status = RecordingStatus {
            id: recorder.id().to_string(),
            path: recorder.dir().to_string_lossy().into_owned(),
            elapsed_s: 0.0,
            rows: 0,
            dropped_rows: 0,
        };
        inner.recorder = Some(recorder);
        inner.dirty = true;
        Ok(status)
    }

    pub fn stop_recording(&self) -> Result<SessionMeta> {
        let (recorder, frames_lost) = {
            let mut inner = self.lock();
            let recorder = inner.recorder.take().ok_or_else(|| Error::Recording("no recording is running".into()))?;
            let frames_lost = inner.connection.as_ref().map_or(0, |c| c.session.stats().frames_lost);
            inner.dirty = true;
            (recorder, frames_lost)
        };
        // Joining the disk thread happens outside the lock.
        recorder.stop(RecordingState::Complete, frames_lost)
    }

    /// Orderly shutdown: stop the motor, close the recording, release the port.
    pub fn shutdown(&self) {
        self.estop();
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.lock().unwrap_or_else(|p| p.into_inner()).take() {
            let _ = worker.join();
        }
        let mut inner = self.lock();
        if let Some(recorder) = inner.recorder.take() {
            let _ = recorder.stop(RecordingState::Interrupted, 0);
        }
        if let Some(connection) = inner.connection.take() {
            let _ = connection.session.disarm();
            connection.session.close();
        }
    }
}
