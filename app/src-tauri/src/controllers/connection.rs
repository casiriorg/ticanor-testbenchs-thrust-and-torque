use tauri::State;
use tb_core::domain::DriverKind;
use tb_core::services::bench::BenchSnapshot;
use tb_core::transport::serial::PortCandidate;

use super::CommandResult;
use crate::state::AppState;

#[tauri::command(async)]
pub fn get_snapshot(state: State<'_, AppState>) -> BenchSnapshot {
    state.bench.snapshot()
}

#[tauri::command(async)]
pub fn list_ports(state: State<'_, AppState>) -> Vec<PortCandidate> {
    state.bench.ports()
}

#[tauri::command(async)]
pub fn connect_simulator(state: State<'_, AppState>) -> CommandResult<()> {
    state.bench.connect_simulator()
}

#[tauri::command(async)]
pub fn connect_port(state: State<'_, AppState>, port: String) -> CommandResult<()> {
    state.bench.connect_port(port)
}

#[tauri::command(async)]
pub fn disconnect(state: State<'_, AppState>) -> CommandResult<()> {
    state.bench.disconnect()
}

#[tauri::command(async)]
pub fn select_driver(state: State<'_, AppState>, kind: DriverKind) -> CommandResult<()> {
    state.bench.select_driver(kind)
}
