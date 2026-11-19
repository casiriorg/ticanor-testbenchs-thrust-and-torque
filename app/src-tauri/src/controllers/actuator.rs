use tauri::State;
use tb_core::profile::Profile;

use super::CommandResult;
use crate::state::AppState;

#[tauri::command(async)]
pub fn arm(state: State<'_, AppState>, confirmed: bool) -> CommandResult<()> {
    state.bench.arm(confirmed)
}

#[tauri::command(async)]
pub fn disarm(state: State<'_, AppState>) -> CommandResult<()> {
    state.bench.disarm()
}

#[tauri::command(async)]
pub fn set_throttle(state: State<'_, AppState>, fraction: f64) -> CommandResult<()> {
    state.bench.set_throttle(fraction)
}

/// Cannot fail: the emergency stop has no error path.
#[tauri::command(async)]
pub fn estop(state: State<'_, AppState>) {
    state.bench.estop();
}

#[tauri::command(async)]
pub fn clear_estop(state: State<'_, AppState>) -> CommandResult<()> {
    state.bench.clear_estop()
}

/// Window heartbeat for the dead-man check.
#[tauri::command(async)]
pub fn ui_alive(state: State<'_, AppState>) {
    state.bench.ui_alive();
}

#[tauri::command(async)]
pub fn builtin_profiles(state: State<'_, AppState>) -> Vec<Profile> {
    state.bench.builtin_profiles()
}

#[tauri::command(async)]
pub fn run_profile(state: State<'_, AppState>, profile: Profile) -> CommandResult<()> {
    state.bench.run_profile(profile)
}

#[tauri::command(async)]
pub fn abort_profile(state: State<'_, AppState>) {
    state.bench.abort_profile();
}
