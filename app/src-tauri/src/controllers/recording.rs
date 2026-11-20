use tauri::State;
use tb_core::recording::recorder::SessionMeta;
use tb_core::recording::{self, RecordingData, RecordingSummary, TestSetup};
use tb_core::services::bench::RecordingStatus;

use super::CommandResult;
use crate::state::AppState;

#[tauri::command(async)]
pub fn start_recording(state: State<'_, AppState>, setup: TestSetup) -> CommandResult<RecordingStatus> {
    state.bench.start_recording(setup)
}

#[tauri::command(async)]
pub fn stop_recording(state: State<'_, AppState>) -> CommandResult<SessionMeta> {
    state.bench.stop_recording()
}

#[tauri::command(async)]
pub fn list_recordings(state: State<'_, AppState>) -> Vec<RecordingSummary> {
    let active = state.bench.active_recording_id();
    recording::list_recordings(&state.bench.data_dir(), active.as_deref())
}

#[tauri::command(async)]
pub fn load_recording(state: State<'_, AppState>, id: String) -> CommandResult<RecordingData> {
    let dir = recording::recording_dir(&state.bench.data_dir(), &id)?;
    recording::load_recording(&dir)
}

/// Copies a recording's CSV to a path the operator chose (the window's "save as").
#[tauri::command(async)]
pub fn export_recording_csv(state: State<'_, AppState>, id: String, dest: String) -> CommandResult<()> {
    let dir = recording::recording_dir(&state.bench.data_dir(), &id)?;
    let src = recording::data_file_path(&dir);
    std::fs::copy(&src, &dest)?;
    Ok(())
}
