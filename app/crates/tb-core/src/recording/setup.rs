//! Description of what was on the bench during a recording. Stored next to
//! the data as `setup.json` so recordings can be filtered and compared by
//! motor, propeller, ESC and battery.
//!
//! Every field is optional text or a number: the form must never block a
//! recording because some detail is unknown.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MotorSpec {
    pub brand: String,
    pub model: String,
    pub kv: Option<f64>,
    /// Stator size as printed, e.g. "3515".
    pub stator: String,
    pub poles: Option<u32>,
    pub mass_g: Option<f64>,
    /// "outrunner" or "inrunner".
    pub kind: String,
    /// Free text for condition, e.g. "recovered from crash, bearing replaced".
    pub condition: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropellerSpec {
    pub brand: String,
    pub model: String,
    pub diameter_in: Option<f64>,
    pub pitch_in: Option<f64>,
    pub blades: Option<u32>,
    pub material: String,
    /// "CW" or "CCW".
    pub direction: String,
    pub mass_g: Option<f64>,
    pub condition: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EscSpec {
    pub brand: String,
    pub model: String,
    pub current_a: Option<f64>,
    pub protocol: String,
    pub firmware: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BatterySpec {
    pub chemistry: String,
    pub cells: Option<u32>,
    pub capacity_mah: Option<f64>,
    /// Voltage measured before the test, if known.
    pub start_voltage: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TestInfo {
    pub operator: String,
    pub ambient_temp_c: Option<f64>,
    pub altitude_m: Option<f64>,
    pub notes: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TestSetup {
    pub motor: MotorSpec,
    pub propeller: PropellerSpec,
    pub esc: EscSpec,
    pub battery: BatterySpec,
    pub test: TestInfo,
}

impl TestSetup {
    /// Short motor label for folder names and lists.
    pub fn motor_label(&self) -> String {
        join_words(&[&self.motor.brand, &self.motor.model])
    }

    pub fn propeller_label(&self) -> String {
        let size = match (self.propeller.diameter_in, self.propeller.pitch_in) {
            (Some(d), Some(p)) => format!("{d}x{p}"),
            (Some(d), None) => format!("{d}in"),
            _ => String::new(),
        };
        join_words(&[&self.propeller.brand, &self.propeller.model, &size])
    }
}

fn join_words(parts: &[&str]) -> String {
    parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")
}
