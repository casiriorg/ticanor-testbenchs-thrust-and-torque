//! ThrustBench desktop shell.
//!
//! This crate is deliberately thin: it creates the bench service from
//! `tb-core`, forwards its events to the window and exposes its methods as
//! Tauri commands (see `controllers`). All behaviour lives in `tb-core`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod controllers;
mod state;

use std::sync::Arc;

use tauri::{Manager, RunEvent};
use tb_core::services::bench::{Bench, BenchConfig};

use state::{AppState, WindowSink};

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let paths = app.path();
            let config_dir = paths.app_config_dir()?;
            // Recordings go to Documents/ThrustBench unless the user picks a folder.
            let documents = paths.document_dir().or_else(|_| paths.home_dir())?;
            let bench = Bench::start(
                BenchConfig {
                    config_dir,
                    default_data_dir: documents.join("ThrustBench"),
                    app_version: app.package_info().version.to_string(),
                },
                Arc::new(WindowSink::new(app.handle().clone())),
            );
            app.manage(AppState { bench });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            controllers::connection::get_snapshot,
            controllers::connection::list_ports,
            controllers::connection::connect_simulator,
            controllers::connection::connect_port,
            controllers::connection::disconnect,
            controllers::connection::select_driver,
            controllers::actuator::arm,
            controllers::actuator::disarm,
            controllers::actuator::set_throttle,
            controllers::actuator::estop,
            controllers::actuator::clear_estop,
            controllers::actuator::ui_alive,
            controllers::actuator::builtin_profiles,
            controllers::actuator::run_profile,
            controllers::actuator::abort_profile,
            controllers::device::tare,
            controllers::device::set_channel_param,
            controllers::device::get_global_params,
            controllers::device::set_global_param,
            controllers::device::save_device_config,
            controllers::device::load_device_defaults,
            controllers::device::apply_load_cell_profile,
            controllers::device::capture_calibration_point,
            controllers::device::fit_calibration,
            controllers::device::apply_calibration,
            controllers::recording::start_recording,
            controllers::recording::stop_recording,
            controllers::recording::list_recordings,
            controllers::recording::load_recording,
            controllers::analysis::compute_stats,
            controllers::analysis::compute_spectrum,
            controllers::analysis::compute_curve,
            controllers::settings::get_settings,
            controllers::settings::update_settings,
            controllers::settings::get_reference,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start ThrustBench");

    app.run(|handle, event| {
        // Whatever closes the application, stop the motor and finish the
        // recording file first.
        if let RunEvent::Exit = event {
            handle.state::<AppState>().bench.shutdown();
        }
    });
}
