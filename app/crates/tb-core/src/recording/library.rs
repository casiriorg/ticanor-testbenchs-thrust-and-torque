//! Reading recordings back: listing the library, salvaging unfinished
//! recordings and loading one for playback and analysis.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::recorder::{
    write_json, RecordingState, SessionMeta, DATA_FILE, PARTIAL_FILE, SESSION_FILE, SETUP_FILE,
};
use super::setup::TestSetup;
use crate::analysis::Series;
use crate::error::{Error, Result};

/// One row of the recordings list.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordingSummary {
    pub id: String,
    pub path: String,
    pub started_at: String,
    pub duration_s: f64,
    pub state: RecordingState,
    pub rows: u64,
    pub motor: String,
    pub propeller: String,
    pub notes: String,
    pub event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelSeries {
    pub channel_id: u8,
    pub sensor_type: u8,
    pub key: String,
    pub name: String,
    pub unit: String,
    pub series: Series,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordingData {
    pub meta: SessionMeta,
    pub setup: TestSetup,
    pub channels: Vec<ChannelSeries>,
}

impl RecordingData {
    pub fn by_key(&self, key: &str) -> Option<&ChannelSeries> {
        self.channels.iter().find(|c| c.key == key)
    }
}

fn read_meta(dir: &Path) -> Result<SessionMeta> {
    let text = fs::read_to_string(dir.join(SESSION_FILE))?;
    Ok(serde_json::from_str(&text)?)
}

fn read_setup(dir: &Path) -> TestSetup {
    fs::read_to_string(dir.join(SETUP_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Finishes a recording that was cut short (crash, power loss): gives the
/// data file its final name and fills in the counters from its contents.
fn recover(dir: &Path, meta: &mut SessionMeta) -> Result<()> {
    let partial = dir.join(PARTIAL_FILE);
    if partial.exists() {
        fs::rename(&partial, dir.join(DATA_FILE))?;
    }
    let mut rows = 0u64;
    let mut last_t = 0.0f64;
    if let Ok(file) = fs::File::open(dir.join(DATA_FILE)) {
        let mut lines = BufReader::new(file).lines().map_while(|l| l.ok());
        let columns = lines.next().map_or(0, |header| header.split(',').count());
        for line in lines {
            // A torn last line (cut mid-write) has too few fields and is not counted.
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() != columns {
                continue;
            }
            if let Ok(t) = fields[1].parse::<f64>() {
                rows += 1;
                last_t = last_t.max(t);
            }
        }
    }
    meta.rows = rows;
    meta.duration_s = last_t;
    meta.state = RecordingState::Recovered;
    write_json(&dir.join(SESSION_FILE), meta)
}

/// Lists every recording under `root`, newest first. Unfinished recordings
/// are salvaged on the way, except `active_id` (the one being written now).
pub fn list_recordings(root: &Path, active_id: Option<&str>) -> Vec<RecordingSummary> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut summaries: Vec<RecordingSummary> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|entry| {
            let dir = entry.path();
            let mut meta = read_meta(&dir).ok()?;
            let is_active = active_id == Some(meta.id.as_str());
            if meta.state == RecordingState::Recording && !is_active {
                if let Err(e) = recover(&dir, &mut meta) {
                    log::warn!("could not recover {}: {e}", dir.display());
                }
            }
            let setup = read_setup(&dir);
            Some(RecordingSummary {
                id: meta.id.clone(),
                path: dir.to_string_lossy().into_owned(),
                started_at: meta.started_at.clone(),
                duration_s: meta.duration_s,
                state: meta.state,
                rows: meta.rows,
                motor: setup.motor_label(),
                propeller: setup.propeller_label(),
                notes: setup.test.notes.clone(),
                event_count: meta.events.len(),
            })
        })
        .collect();
    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    summaries
}

/// Resolves a recording id to its folder, refusing anything that would
/// escape the recordings root.
pub fn recording_dir(root: &Path, id: &str) -> Result<PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
        return Err(Error::Invalid(format!("invalid recording id: {id}")));
    }
    let dir = root.join(id);
    if !dir.join(SESSION_FILE).exists() {
        return Err(Error::Recording(format!("recording {id} was not found")));
    }
    Ok(dir)
}

/// Path to a recording's CSV, whichever of the final or partial name is present.
pub fn data_file_path(dir: &Path) -> PathBuf {
    if dir.join(DATA_FILE).exists() { dir.join(DATA_FILE) } else { dir.join(PARTIAL_FILE) }
}

/// Loads a whole recording into memory as one series per channel.
pub fn load_recording(dir: &Path) -> Result<RecordingData> {
    let meta = read_meta(dir)?;
    let setup = read_setup(dir);
    let data_path = data_file_path(dir);
    let file = fs::File::open(&data_path)
        .map_err(|e| Error::Recording(format!("cannot open {}: {e}", data_path.display())))?;
    let mut lines = BufReader::new(file).lines();
    let header = lines.next().transpose()?.ok_or_else(|| Error::Format("the data file is empty".into()))?;
    let names: Vec<&str> = header.split(',').collect();

    // Column index of each channel's calibrated value.
    let mut channels: Vec<(usize, ChannelSeries)> = meta
        .channels
        .iter()
        .filter_map(|c| {
            let index = names.iter().position(|n| *n == c.column)?;
            Some((
                index,
                ChannelSeries {
                    channel_id: c.descriptor.id,
                    sensor_type: c.descriptor.sensor_type,
                    key: c.key.clone(),
                    name: c.descriptor.name.clone(),
                    unit: c.unit.clone(),
                    series: Series::default(),
                },
            ))
        })
        .collect();

    for line in lines.map_while(|l| l.ok()) {
        let fields: Vec<&str> = line.split(',').collect();
        let Some(t) = fields.get(1).and_then(|f| f.parse::<f64>().ok()) else { continue };
        for (index, channel) in channels.iter_mut() {
            if let Some(value) = fields.get(*index).filter(|f| !f.is_empty()).and_then(|f| f.parse::<f64>().ok()) {
                channel.series.t.push(t);
                channel.series.v.push(value);
            }
        }
    }
    Ok(RecordingData { meta, setup, channels: channels.into_iter().map(|(_, c)| c).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::ColumnNaming;
    use crate::domain::{ChannelDescriptor, DriverKind, Sample};
    use crate::recording::recorder::{Recorder, RecordingOptions};
    use std::collections::BTreeMap;

    fn channel(id: u8, sensor_type: u8, name: &str) -> ChannelDescriptor {
        ChannelDescriptor {
            id,
            sensor_type,
            unit: 1,
            enabled: true,
            invert: false,
            calibrated: true,
            rate_hz: 80,
            gain: 128,
            offset: 0,
            scale: 1000.0,
            cal_time: 1,
            name: name.into(),
        }
    }

    fn options(root: &Path, naming: ColumnNaming) -> RecordingOptions {
        RecordingOptions {
            root: root.to_path_buf(),
            naming,
            include_raw: true,
            channel_names: BTreeMap::new(),
            app_version: "test".into(),
            gravity: 9.78,
        }
    }

    fn setup() -> TestSetup {
        let mut setup = TestSetup::default();
        setup.motor.brand = "MAD".into();
        setup.motor.model = "3515 IPE".into();
        setup.propeller.brand = "Walkera".into();
        setup.propeller.diameter_in = Some(15.5);
        setup
    }

    fn record(root: &Path, naming: ColumnNaming) -> Recorder {
        let channels = [channel(1, 1, "thrust"), channel(2, 2, "torque")];
        let mut recorder = Recorder::start(&options(root, naming), &setup(), None, &channels, DriverKind::Manual).unwrap();
        for i in 0..100 {
            let t = 5.0 + i as f64 * 0.0125; // device time does not start at zero
            recorder.push(&Sample { channel: 1, t, raw: i, value: i as f64 * 0.5 });
            recorder.push(&Sample { channel: 2, t: t + 0.006, raw: -i, value: i as f64 * 0.01 });
        }
        recorder
    }

    #[test]
    fn recording_round_trips_through_csv() {
        let root = tempfile::tempdir().unwrap();
        let mut recorder = record(root.path(), ColumnNaming::Name);
        recorder.mark_event("estop", "operator pressed stop");
        let meta = recorder.stop(RecordingState::Complete, 0).unwrap();
        assert_eq!(meta.rows, 200);
        assert_eq!(meta.state, RecordingState::Complete);
        assert!(meta.id.contains("MAD-3515-IPE"));

        let list = list_recordings(root.path(), None);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].motor, "MAD 3515 IPE");
        assert_eq!(list[0].event_count, 1);

        let dir = recording_dir(root.path(), &meta.id).unwrap();
        let header = fs::read_to_string(dir.join(DATA_FILE)).unwrap().lines().next().unwrap().to_string();
        assert_eq!(header, "time_iso,t_s,thrust_N,torque_Nm,thrust_raw,torque_raw");

        let data = load_recording(&dir).unwrap();
        let thrust = data.by_key("thrust").unwrap();
        assert_eq!(thrust.series.len(), 100);
        assert_eq!(thrust.series.t[0], 0.0); // recording time starts at the first sample
        assert!((thrust.series.v[99] - 49.5).abs() < 1e-9);
        assert_eq!(data.by_key("torque").unwrap().series.len(), 100);
    }

    #[test]
    fn id_naming_uses_channel_identifiers() {
        let root = tempfile::tempdir().unwrap();
        let meta = record(root.path(), ColumnNaming::Id).stop(RecordingState::Complete, 0).unwrap();
        let dir = recording_dir(root.path(), &meta.id).unwrap();
        let text = fs::read_to_string(dir.join(DATA_FILE)).unwrap();
        assert!(text.starts_with("time_iso,t_s,ch1,ch2,ch1_raw,ch2_raw"));
    }

    #[test]
    fn unfinished_recording_is_recovered_with_torn_last_line() {
        let root = tempfile::tempdir().unwrap();
        let recorder = record(root.path(), ColumnNaming::Name);
        let dir = recorder.dir().to_path_buf();
        // Simulate a crash: the recorder never gets to finish.
        std::thread::sleep(std::time::Duration::from_millis(700)); // let it flush
        std::mem::forget(recorder);
        assert!(dir.join(PARTIAL_FILE).exists());
        // Power cut in the middle of a row.
        let mut text = fs::read_to_string(dir.join(PARTIAL_FILE)).unwrap();
        text.push_str("2026-11-13T10:00:00.000-05:00,9.9");
        fs::write(dir.join(PARTIAL_FILE), text).unwrap();

        let list = list_recordings(root.path(), None);
        assert_eq!(list[0].state, RecordingState::Recovered);
        assert_eq!(list[0].rows, 200); // the torn row is not counted
        assert!(dir.join(DATA_FILE).exists() && !dir.join(PARTIAL_FILE).exists());
        assert_eq!(load_recording(&dir).unwrap().by_key("thrust").unwrap().series.len(), 100);
    }

    #[test]
    fn active_recording_is_left_alone_and_ids_cannot_escape_root() {
        let root = tempfile::tempdir().unwrap();
        let recorder = record(root.path(), ColumnNaming::Name);
        let id = recorder.id().to_string();
        assert_eq!(list_recordings(root.path(), Some(&id))[0].state, RecordingState::Recording);
        recorder.stop(RecordingState::Complete, 0).unwrap();
        assert!(recording_dir(root.path(), "../etc").is_err());
        assert!(recording_dir(root.path(), "missing").is_err());
    }
}
