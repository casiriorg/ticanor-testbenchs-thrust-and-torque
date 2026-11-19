//! Compiled-in defaults, read from `defaults.json` next to this file.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::settings::AppSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbId {
    pub vid: u16,
    /// `None` matches every product of the vendor.
    pub pid: Option<u16>,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbIds {
    pub acquisition_board: Vec<UsbId>,
    pub flight_controller: Vec<UsbId>,
}

/// Maps a sensor type code from the wire to a stable key and display text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorCatalogEntry {
    pub code: u8,
    pub key: String,
    pub label: String,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadCellProfile {
    pub id: String,
    pub label: String,
    pub capacity_kg: f64,
    pub sensitivity_mv_v: f64,
    /// Highest load the cell is rated to survive, kgf. Usually a little above
    /// `capacity_kg` (manufacturer overload margin); shown as the channel's
    /// maximum line in the monitor.
    pub safe_max_kgf: f64,
}

impl LoadCellProfile {
    /// Nominal HX711 counts per newton: `counts = mV/V * 1e-3 * gain * 2^24`
    /// at rated capacity. An estimate until the cell is calibrated.
    pub fn nominal_counts_per_newton(&self, gain: u8, gravity: f64) -> f64 {
        let counts_at_capacity = self.sensitivity_mv_v * 1e-3 * gain as f64 * 16_777_216.0;
        counts_at_capacity / (self.capacity_kg * gravity)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    pub usb_ids: UsbIds,
    pub sensor_catalog: Vec<SensorCatalogEntry>,
    pub load_cell_profiles: Vec<LoadCellProfile>,
    pub default_load_cell_profile: String,
    /// Choices offered by the test set-up form.
    pub setup_options: Value,
    pub settings: AppSettings,
}

const DEFAULTS_JSON: &str = include_str!("defaults.json");

/// Raw JSON, used as the base when merging user settings.
pub fn defaults_json() -> Value {
    serde_json::from_str(DEFAULTS_JSON).expect("defaults.json is valid JSON")
}

pub fn defaults() -> &'static Defaults {
    static CELL: OnceLock<Defaults> = OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str(DEFAULTS_JSON).expect("defaults.json matches the Defaults type"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parse_and_match_firmware_nominal_scale() {
        let d = defaults();
        let profile = d.load_cell_profiles.iter().find(|p| p.id == d.default_load_cell_profile).unwrap();
        // Same number as kThrustCountsPerNewton in firmware Defaults.h.
        let scale = profile.nominal_counts_per_newton(128, 9.80665);
        assert!((scale - 21_898.2).abs() < 1.0, "scale was {scale}");
    }
}
