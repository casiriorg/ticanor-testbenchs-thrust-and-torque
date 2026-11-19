use serde::Serialize;
use serde_json::Value;
use tauri::State;
use tb_core::config::defaults::{LoadCellProfile, SensorCatalogEntry};
use tb_core::config::{defaults, AppSettings};
use tb_core::protocol::ids::PROTOCOL_VERSION;

use super::CommandResult;
use crate::state::AppState;

#[tauri::command(async)]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.bench.settings()
}

#[tauri::command(async)]
pub fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> CommandResult<()> {
    state.bench.update_settings(settings)
}

/// Static reference data the UI needs to label and populate its forms.
#[derive(Serialize)]
pub struct Reference {
    sensor_catalog: Vec<SensorCatalogEntry>,
    load_cell_profiles: Vec<LoadCellProfile>,
    setup_options: Value,
    default_settings: AppSettings,
    data_dir: String,
    app_version: String,
    protocol_version: u8,
}

#[tauri::command(async)]
pub fn get_reference(state: State<'_, AppState>, app: tauri::AppHandle) -> Reference {
    let d = defaults();
    Reference {
        sensor_catalog: d.sensor_catalog.clone(),
        load_cell_profiles: d.load_cell_profiles.clone(),
        setup_options: d.setup_options.clone(),
        default_settings: d.settings.clone(),
        data_dir: state.bench.data_dir().to_string_lossy().into_owned(),
        app_version: app.package_info().version.to_string(),
        protocol_version: PROTOCOL_VERSION,
    }
}
