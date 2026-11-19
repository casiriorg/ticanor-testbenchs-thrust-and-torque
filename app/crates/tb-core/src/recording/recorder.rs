//! Writes one recording to disk.
//!
//! Layout of a recording folder:
//!
//! ```text
//! 2026-11-13_142233_MAD-3515_Walkera-15x5.5/
//!   data.csv        samples (data.csv.partial while recording)
//!   session.json    device, channels, calibration, events, counters
//!   setup.json      motor / propeller / ESC / battery description
//! ```
//!
//! Robustness rules:
//! * Disk I/O happens on a dedicated thread behind a bounded queue, so a slow
//!   disk can never stall acquisition or the safety logic. If the queue
//!   fills up, rows are dropped and counted rather than blocking.
//! * Data is written to `data.csv.partial`, flushed twice a second and renamed
//!   to `data.csv` only on a clean stop. After a crash or power cut the
//!   partial file is recovered on the next start (see `library`).
//! * `session.json` is written at start and rewritten on every event, so the
//!   metadata needed to read a recovered file always exists.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::{DateTime, Local, SecondsFormat};
use crossbeam_channel::{bounded, RecvTimeoutError, Sender, TrySendError};
use serde::{Deserialize, Serialize};

use super::setup::TestSetup;
use crate::config::defaults;
use crate::config::settings::ColumnNaming;
use crate::domain::{ChannelDescriptor, DeviceInfo, DriverKind, Sample};
use crate::error::{Error, Result};

pub const FORMAT_VERSION: u32 = 1;
pub const DATA_FILE: &str = "data.csv";
pub const PARTIAL_FILE: &str = "data.csv.partial";
pub const SESSION_FILE: &str = "session.json";
pub const SETUP_FILE: &str = "setup.json";

/// Rows buffered between acquisition and the disk thread (about 30 s at the
/// current sample rates).
const QUEUE_CAPACITY: usize = 8192;
const FLUSH_PERIOD: Duration = Duration::from_millis(500);
/// Refuse to start a recording with less free space than this.
const MIN_FREE_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    Recording,
    Complete,
    /// Stopped by a fault (for example the application closing mid-test).
    Interrupted,
    /// Found unfinished at start-up and salvaged.
    Recovered,
}

/// How one channel is stored in the CSV.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelColumn {
    pub descriptor: ChannelDescriptor,
    /// Stable key from the sensor catalogue, e.g. "thrust".
    pub key: String,
    pub unit: String,
    /// Column with the calibrated value.
    pub column: String,
    /// Column with raw counts, when recorded.
    pub raw_column: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingEvent {
    /// Seconds since the first recorded sample.
    pub t_s: f64,
    /// Machine-readable kind: "estop", "link_lost", "profile_start", ...
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionMeta {
    pub format_version: u32,
    pub app_version: String,
    pub id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub state: RecordingState,
    pub device: Option<DeviceInfo>,
    pub driver: DriverKind,
    /// Local gravity used for calibration, for traceability.
    pub gravity: f64,
    pub channels: Vec<ChannelColumn>,
    pub events: Vec<RecordingEvent>,
    pub rows: u64,
    /// Rows lost because the disk could not keep up.
    pub dropped_rows: u64,
    /// Frames lost on the serial link during the recording.
    pub frames_lost: u64,
    pub duration_s: f64,
}

#[derive(Debug, Clone)]
pub struct RecordingOptions {
    /// Folder that holds all recordings.
    pub root: PathBuf,
    pub naming: ColumnNaming,
    pub include_raw: bool,
    /// Name overrides keyed by channel id.
    pub channel_names: BTreeMap<String, String>,
    pub app_version: String,
    pub gravity: f64,
}

enum Row {
    Sample { t: f64, channel: u8, raw: i32, value: f64 },
    Stop,
}

pub struct Recorder {
    dir: PathBuf,
    meta: SessionMeta,
    tx: Sender<Row>,
    writer: Option<JoinHandle<std::io::Result<()>>>,
    rows: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    /// Device time of the first recorded sample; recording time starts there.
    origin: Option<f64>,
    last_t: f64,
}

/// Keeps a label safe for every file system and free of separators.
fn sanitize(label: &str) -> String {
    let cleaned: String = label
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '-' })
        .collect();
    let mut out = String::new();
    for c in cleaned.chars() {
        if !(c == '-' && out.ends_with('-')) {
            out.push(c);
        }
    }
    out.trim_matches('-').chars().take(40).collect()
}

fn unit_suffix(unit: &str) -> String {
    match unit {
        "N·m" => "Nm".into(),
        "m/s²" => "ms2".into(),
        other => other.chars().filter(|c| c.is_ascii_alphanumeric()).collect(),
    }
}

/// Decides column names for every enabled channel.
pub fn plan_columns(
    channels: &[ChannelDescriptor],
    naming: ColumnNaming,
    include_raw: bool,
    overrides: &BTreeMap<String, String>,
) -> Vec<ChannelColumn> {
    let catalog = &defaults().sensor_catalog;
    channels
        .iter()
        .filter(|c| c.enabled)
        .map(|c| {
            let entry = catalog.iter().find(|e| e.code == c.sensor_type);
            let key = entry.map_or_else(|| format!("sensor{}", c.sensor_type), |e| e.key.clone());
            let unit = entry.map_or_else(String::new, |e| e.unit.clone());
            let base = match naming {
                ColumnNaming::Id => format!("ch{}", c.id),
                ColumnNaming::Name => {
                    let name = overrides.get(&c.id.to_string()).cloned().unwrap_or_else(|| c.name.clone());
                    let name = sanitize(&name).replace('-', "_");
                    if name.is_empty() {
                        format!("ch{}", c.id)
                    } else {
                        name
                    }
                }
            };
            let suffix = unit_suffix(&unit);
            let column = if suffix.is_empty() || naming == ColumnNaming::Id { base.clone() } else { format!("{base}_{suffix}") };
            let raw_column = include_raw.then(|| format!("{base}_raw"));
            ChannelColumn { descriptor: c.clone(), key, unit, column, raw_column }
        })
        .collect()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

impl Recorder {
    pub fn start(
        options: &RecordingOptions,
        setup: &TestSetup,
        device: Option<DeviceInfo>,
        channels: &[ChannelDescriptor],
        driver: DriverKind,
    ) -> Result<Self> {
        fs::create_dir_all(&options.root)?;
        if let Ok(free) = fs2::available_space(&options.root) {
            if free < MIN_FREE_BYTES {
                return Err(Error::Recording(format!(
                    "only {} MB free in {}; free some space before recording",
                    free / (1024 * 1024),
                    options.root.display()
                )));
            }
        }
        let columns = plan_columns(channels, options.naming, options.include_raw, &options.channel_names);
        if columns.is_empty() {
            return Err(Error::Recording("no channel is enabled".into()));
        }

        let started: DateTime<Local> = Local::now();
        let mut id = started.format("%Y-%m-%d_%H%M%S").to_string();
        for label in [setup.motor_label(), setup.propeller_label()] {
            let label = sanitize(&label);
            if !label.is_empty() {
                id.push('_');
                id.push_str(&label);
            }
        }
        let dir = options.root.join(&id);
        fs::create_dir(&dir).map_err(|e| Error::Recording(format!("cannot create {}: {e}", dir.display())))?;

        let meta = SessionMeta {
            format_version: FORMAT_VERSION,
            app_version: options.app_version.clone(),
            id,
            started_at: started.to_rfc3339_opts(SecondsFormat::Millis, false),
            ended_at: None,
            state: RecordingState::Recording,
            device,
            driver,
            gravity: options.gravity,
            channels: columns.clone(),
            events: Vec::new(),
            rows: 0,
            dropped_rows: 0,
            frames_lost: 0,
            duration_s: 0.0,
        };
        write_json(&dir.join(SETUP_FILE), setup)?;
        write_json(&dir.join(SESSION_FILE), &meta)?;

        let file = File::create(dir.join(PARTIAL_FILE))?;
        let (tx, rx) = bounded::<Row>(QUEUE_CAPACITY);
        let rows = Arc::new(AtomicU64::new(0));
        let rows_in_thread = rows.clone();
        let writer = thread::Builder::new().name("tb-recorder".into()).spawn(move || -> std::io::Result<()> {
            let mut out = BufWriter::with_capacity(64 * 1024, file);
            // Header: wall-clock time, recording time, then value and raw columns.
            let mut header = vec!["time_iso".to_string(), "t_s".to_string()];
            header.extend(columns.iter().map(|c| c.column.clone()));
            header.extend(columns.iter().filter_map(|c| c.raw_column.clone()));
            writeln!(out, "{}", header.join(","))?;

            let value_slot: BTreeMap<u8, usize> = columns.iter().enumerate().map(|(i, c)| (c.descriptor.id, i)).collect();
            let raw_slot: BTreeMap<u8, usize> = columns
                .iter()
                .filter(|c| c.raw_column.is_some())
                .enumerate()
                .map(|(i, c)| (c.descriptor.id, i))
                .collect();
            let raw_count = raw_slot.len();
            let mut last_flush = std::time::Instant::now();
            loop {
                match rx.recv_timeout(FLUSH_PERIOD) {
                    Ok(Row::Sample { t, channel, raw, value }) => {
                        let Some(&slot) = value_slot.get(&channel) else { continue };
                        let wall = started + chrono::Duration::microseconds((t * 1e6) as i64);
                        write!(out, "{},{:.6}", wall.to_rfc3339_opts(SecondsFormat::Millis, false), t)?;
                        // Sparse row: only this sample's columns are filled.
                        for i in 0..columns.len() {
                            if i == slot {
                                write!(out, ",{value:.6}")?;
                            } else {
                                out.write_all(b",")?;
                            }
                        }
                        let raw_index = raw_slot.get(&channel).copied();
                        for i in 0..raw_count {
                            if Some(i) == raw_index {
                                write!(out, ",{raw}")?;
                            } else {
                                out.write_all(b",")?;
                            }
                        }
                        out.write_all(b"\n")?;
                        rows_in_thread.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(Row::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
                if last_flush.elapsed() >= FLUSH_PERIOD {
                    out.flush()?;
                    last_flush = std::time::Instant::now();
                }
            }
            out.flush()?;
            out.get_ref().sync_all()
        })?;

        Ok(Self { dir, meta, tx, writer: Some(writer), rows, dropped: Arc::new(AtomicU64::new(0)), origin: None, last_t: 0.0 })
    }

    pub fn id(&self) -> &str {
        &self.meta.id
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn rows(&self) -> u64 {
        self.rows.load(Ordering::Relaxed)
    }

    pub fn dropped_rows(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Seconds of data recorded so far.
    pub fn elapsed(&self) -> f64 {
        self.last_t
    }

    /// Queues one sample. Never blocks.
    pub fn push(&mut self, sample: &Sample) {
        let origin = *self.origin.get_or_insert(sample.t);
        let t = sample.t - origin;
        if t > self.last_t {
            self.last_t = t;
        }
        let row = Row::Sample { t, channel: sample.channel, raw: sample.raw, value: sample.value };
        if let Err(TrySendError::Full(_)) = self.tx.try_send(row) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Notes something that happened during the recording (emergency stop,
    /// link loss, profile start...). Persisted immediately.
    pub fn mark_event(&mut self, kind: &str, message: &str) {
        self.meta.events.push(RecordingEvent { t_s: self.last_t, kind: kind.to_string(), message: message.to_string() });
        if let Err(e) = write_json(&self.dir.join(SESSION_FILE), &self.meta) {
            log::warn!("could not update session.json: {e}");
        }
    }

    /// Finishes the recording: drains the queue, syncs the file to disk,
    /// renames it to its final name and writes the final metadata.
    pub fn stop(mut self, state: RecordingState, frames_lost: u64) -> Result<SessionMeta> {
        self.finish(state, frames_lost)
    }

    fn finish(&mut self, state: RecordingState, frames_lost: u64) -> Result<SessionMeta> {
        let Some(writer) = self.writer.take() else {
            return Ok(self.meta.clone());
        };
        // A blocking send is fine here: we are stopping and must not lose the marker.
        let _ = self.tx.send(Row::Stop);
        let write_result = writer.join().map_err(|_| Error::Recording("the recorder thread panicked".into()))?;

        self.meta.ended_at = Some(Local::now().to_rfc3339_opts(SecondsFormat::Millis, false));
        self.meta.rows = self.rows();
        self.meta.dropped_rows = self.dropped_rows();
        self.meta.frames_lost = frames_lost;
        self.meta.duration_s = self.last_t;
        self.meta.state = if write_result.is_ok() { state } else { RecordingState::Interrupted };
        fs::rename(self.dir.join(PARTIAL_FILE), self.dir.join(DATA_FILE))?;
        write_json(&self.dir.join(SESSION_FILE), &self.meta)?;
        write_result.map_err(|e| Error::Recording(format!("writing data failed: {e}")))?;
        Ok(self.meta.clone())
    }
}

impl Drop for Recorder {
    /// A recorder dropped without `stop()` (an error path) still leaves a
    /// complete, readable recording behind, marked as interrupted.
    fn drop(&mut self) {
        if self.writer.is_some() {
            let _ = self.finish(RecordingState::Interrupted, 0);
        }
    }
}
