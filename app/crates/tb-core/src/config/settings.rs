//! User settings, persisted as JSON. Only values that differ from the
//! defaults need to be present in the file: it is merged over
//! `defaults.json`, so new settings get their default automatically after an
//! upgrade.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::defaults::{defaults, defaults_json};
use crate::domain::DriverKind;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Units {
    /// "gf", "kgf" or "n".
    pub force: String,
    /// "nm" or "nmm".
    pub torque: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnNaming {
    /// Mapped channel name, e.g. `thrust_N`.
    Name,
    /// Channel identifier, e.g. `ch1`.
    Id,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsvOptions {
    pub column_naming: ColumnNaming,
    /// Also store raw counts, so a recording can be recalibrated later.
    pub include_raw: bool,
}

/// PC-side limits. The device enforces its own, independent, set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyLimits {
    /// Highest throttle the app will command, 0..1.
    pub max_throttle: f64,
    /// Highest rise rate in throttle fraction per second.
    pub max_rise_per_s: f64,
    /// Cut the throttle when thrust exceeds this (N). `None` disables it.
    pub max_thrust_n: Option<f64>,
    pub max_torque_nm: Option<f64>,
    /// Disarm when the window stops reporting for this long while armed.
    pub ui_timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayOptions {
    pub window_s: f64,
    pub smoothing_samples: u32,
    /// "off", "50" or "60": notch at the mains frequency (or its alias).
    pub mains_filter: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationOptions {
    /// Distance from the motor axis to where the calibration weight hangs.
    pub torque_arm_mm: f64,
    /// Force on the cell divided by the weight applied, for rigs that load
    /// the cell through a lever or pulley. 1.0 for a direct load.
    pub force_ratio: f64,
    pub capture_seconds: f64,
    pub thrust_points_g: Vec<f64>,
    pub torque_points_g: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub language: String,
    /// Recordings root. `None` means `<Documents>/ThrustBench`.
    pub data_dir: Option<String>,
    pub auto_connect: bool,
    pub preferred_driver: Option<DriverKind>,
    pub units: Units,
    /// Local gravity in m/s², used to turn calibration masses into forces.
    pub gravity: f64,
    pub csv: CsvOptions,
    pub safety: SafetyLimits,
    pub display: DisplayOptions,
    pub calibration: CalibrationOptions,
    /// Warn before recording when the last tare is older than this.
    pub tare_max_age_min: u32,
    /// Display/CSV name overrides keyed by channel id (as a string).
    pub channel_names: BTreeMap<String, String>,
    /// Load cell profile id applied to each channel (channel id as a
    /// string), used to show that profile's `safe_max_kgf` in the monitor.
    /// Set when the operator applies a profile from the Device view.
    pub channel_load_cell: BTreeMap<String, String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        defaults().settings.clone()
    }
}

impl AppSettings {
    // `!(x > 0.0)` is deliberate: unlike `x <= 0.0` it also rejects NaN.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn validate(&self) -> Result<()> {
        let s = &self.safety;
        if !(0.0..=1.0).contains(&s.max_throttle) {
            return Err(Error::Invalid("safety.max_throttle must be between 0 and 1".into()));
        }
        if !(s.max_rise_per_s > 0.0) {
            return Err(Error::Invalid("safety.max_rise_per_s must be positive".into()));
        }
        if s.ui_timeout_ms < 500 {
            return Err(Error::Invalid("safety.ui_timeout_ms must be at least 500".into()));
        }
        if !(9.7..=9.9).contains(&self.gravity) {
            return Err(Error::Invalid("gravity must be between 9.7 and 9.9 m/s²".into()));
        }
        if !(self.calibration.force_ratio > 0.0) || !(self.calibration.torque_arm_mm > 0.0) {
            return Err(Error::Invalid("calibration geometry must be positive".into()));
        }
        Ok(())
    }
}

/// Recursively overlays `overlay` on `base` (objects merge, the rest replace).
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(existing) if existing.is_object() && value.is_object() => merge(existing, value),
                    _ => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

/// Loads and saves `settings.json`.
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(config_dir: &Path) -> Self {
        Self { path: config_dir.join("settings.json") }
    }

    /// Never fails: an unreadable or invalid file falls back to defaults, and
    /// the problem is returned alongside so the UI can tell the user.
    pub fn load(&self) -> (AppSettings, Option<String>) {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (AppSettings::default(), None),
            Err(e) => return (AppSettings::default(), Some(e.to_string())),
        };
        let parsed = serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()).and_then(|user| {
            let mut merged = defaults_json()["settings"].clone();
            merge(&mut merged, user);
            let settings: AppSettings = serde_json::from_value(merged).map_err(|e| e.to_string())?;
            settings.validate().map_err(|e| e.to_string())?;
            Ok(settings)
        });
        match parsed {
            Ok(settings) => (settings, None),
            Err(problem) => (AppSettings::default(), Some(problem)),
        }
    }

    /// Writes atomically: a crash mid-write cannot corrupt the settings.
    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        settings.validate()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(settings)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_file_is_merged_over_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), r#"{"language":"es","safety":{"max_throttle":0.4}}"#).unwrap();
        let (settings, problem) = SettingsStore::new(dir.path()).load();
        assert!(problem.is_none());
        assert_eq!(settings.language, "es");
        assert_eq!(settings.safety.max_throttle, 0.4);
        assert_eq!(settings.safety.ui_timeout_ms, 1500); // untouched default
    }

    #[test]
    fn invalid_file_falls_back_and_reports() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), "{ not json").unwrap();
        let (settings, problem) = SettingsStore::new(dir.path()).load();
        assert_eq!(settings, AppSettings::default());
        assert!(problem.is_some());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        let settings = AppSettings { language: "es".into(), gravity: 9.78, ..AppSettings::default() };
        store.save(&settings).unwrap();
        assert_eq!(store.load().0, settings);
    }
}
