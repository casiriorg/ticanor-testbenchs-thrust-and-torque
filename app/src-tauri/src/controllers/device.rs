use std::collections::BTreeMap;

use tauri::State;
use tb_core::calibration::{self, CalFit, CalPoint, ReferenceKind};
use tb_core::protocol::ids::global_field;
use tb_core::services::bench::RawCapture;
use tb_core::Error;

use super::CommandResult;
use crate::state::AppState;

#[tauri::command(async)]
pub fn tare(state: State<'_, AppState>, channel: u8) -> CommandResult<RawCapture> {
    state.bench.tare(channel)
}

/// `kind` says how `value` is encoded on the wire: "u32", "i32" or "f32".
#[tauri::command(async)]
pub fn set_channel_param(state: State<'_, AppState>, channel: u8, field: u8, kind: String, value: f64) -> CommandResult<()> {
    let bytes = match kind.as_str() {
        "u32" if value >= 0.0 => (value.round() as u32).to_le_bytes(),
        "i32" => (value.round() as i32).to_le_bytes(),
        "f32" => (value as f32).to_le_bytes(),
        _ => return Err(Error::Invalid(format!("cannot encode {value} as {kind}"))),
    };
    state.bench.set_channel_param(channel, field, bytes)
}

const GLOBAL_FIELDS: [(&str, u8); 10] = [
    ("pwm_min_us", global_field::PWM_MIN_US),
    ("pwm_max_us", global_field::PWM_MAX_US),
    ("pwm_freq_hz", global_field::PWM_FREQ_HZ),
    ("watchdog_ms", global_field::WATCHDOG_MS),
    ("throttle_max", global_field::THROTTLE_MAX),
    ("slew_per_sec", global_field::SLEW_PER_SEC),
    ("rate_pin", global_field::RATE_PIN),
    ("rate_sps", global_field::RATE_SPS),
    ("disarmed_output", global_field::DISARMED_OUTPUT),
    ("device_uid", global_field::DEVICE_UID),
];

/// Reads every board-level setting, keyed by name.
#[tauri::command(async)]
pub fn get_global_params(state: State<'_, AppState>) -> CommandResult<BTreeMap<String, u32>> {
    GLOBAL_FIELDS
        .iter()
        .map(|(name, field)| Ok((name.to_string(), state.bench.get_global_param(*field)?)))
        .collect()
}

#[tauri::command(async)]
pub fn set_global_param(state: State<'_, AppState>, name: String, value: u32) -> CommandResult<()> {
    let field = GLOBAL_FIELDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .ok_or_else(|| Error::Invalid(format!("unknown setting {name}")))?;
    state.bench.set_global_param(field, value)
}

#[tauri::command(async)]
pub fn save_device_config(state: State<'_, AppState>) -> CommandResult<()> {
    state.bench.save_device_config()
}

#[tauri::command(async)]
pub fn load_device_defaults(state: State<'_, AppState>) -> CommandResult<()> {
    state.bench.load_device_defaults()
}

#[tauri::command(async)]
pub fn apply_load_cell_profile(state: State<'_, AppState>, channel: u8, profile_id: String, torque_arm_mm: Option<f64>) -> CommandResult<()> {
    state.bench.apply_load_cell_profile(channel, &profile_id, torque_arm_mm)
}

#[tauri::command(async)]
pub fn capture_calibration_point(state: State<'_, AppState>, channel: u8, kind: ReferenceKind, mass_g: f64) -> CommandResult<CalPoint> {
    state.bench.capture_calibration_point(channel, kind, mass_g)
}

#[tauri::command(async)]
pub fn fit_calibration(points: Vec<CalPoint>) -> CommandResult<CalFit> {
    calibration::fit(&points)
}

/// Re-fits from the points (the fit is never trusted from the frontend),
/// writes the result to the board and saves it to EEPROM.
#[tauri::command(async)]
pub fn apply_calibration(state: State<'_, AppState>, channel: u8, points: Vec<CalPoint>) -> CommandResult<CalFit> {
    let fit = calibration::fit(&points)?;
    state.bench.apply_calibration(channel, &fit)?;
    Ok(fit)
}
