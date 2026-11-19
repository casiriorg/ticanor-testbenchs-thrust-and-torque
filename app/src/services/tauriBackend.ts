// Adapter to the Rust core through Tauri commands and events.
// Command names and arguments match app/src-tauri/src/controllers.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open } from "@tauri-apps/plugin-dialog";

import type { Backend } from "./backend";
import type { BenchEvent } from "./types";

export const tauriBackend: Backend = {
  async onEvent(handler) {
    return listen<BenchEvent>("bench-event", (event) => handler(event.payload));
  },
  async onCloseRequested(guard) {
    await getCurrentWindow().onCloseRequested(async (event) => {
      if (!(await guard())) event.preventDefault();
    });
  },
  async pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    return typeof picked === "string" ? picked : null;
  },
  // The webview's own window.confirm() is unreliable inside Tauri (it can
  // hang or resolve without ever showing), which left the close guard stuck
  // forever; the dialog plugin's native dialog does not have that problem.
  confirm: (message) => ask(message),

  getSnapshot: () => invoke("get_snapshot"),
  listPorts: () => invoke("list_ports"),
  connectSimulator: () => invoke("connect_simulator"),
  connectPort: (port) => invoke("connect_port", { port }),
  disconnect: () => invoke("disconnect"),
  selectDriver: (kind) => invoke("select_driver", { kind }),

  arm: (confirmed) => invoke("arm", { confirmed }),
  disarm: () => invoke("disarm"),
  setThrottle: (fraction) => invoke("set_throttle", { fraction }),
  estop: () => invoke("estop"),
  clearEstop: () => invoke("clear_estop"),
  uiAlive: () => invoke("ui_alive"),
  builtinProfiles: () => invoke("builtin_profiles"),
  runProfile: (profile) => invoke("run_profile", { profile }),
  abortProfile: () => invoke("abort_profile"),

  tare: (channel) => invoke("tare", { channel }),
  setChannelParam: (channel, field, kind, value) => invoke("set_channel_param", { channel, field, kind, value }),
  getGlobalParams: () => invoke("get_global_params"),
  setGlobalParam: (name, value) => invoke("set_global_param", { name, value }),
  saveDeviceConfig: () => invoke("save_device_config"),
  loadDeviceDefaults: () => invoke("load_device_defaults"),
  applyLoadCellProfile: (channel, profileId, torqueArmMm) => invoke("apply_load_cell_profile", { channel, profileId, torqueArmMm }),
  captureCalibrationPoint: (channel, kind, massG) => invoke("capture_calibration_point", { channel, kind, massG }),
  fitCalibration: (points) => invoke("fit_calibration", { points }),
  applyCalibration: (channel, points) => invoke("apply_calibration", { channel, points }),

  startRecording: (setup) => invoke("start_recording", { setup }),
  stopRecording: () => invoke("stop_recording"),
  listRecordings: () => invoke("list_recordings"),
  loadRecording: (id) => invoke("load_recording", { id }),

  computeStats: (series) => invoke("compute_stats", { series }),
  computeSpectrum: (series, segmentLen) => invoke("compute_spectrum", { series, segmentLen: segmentLen ?? null }),
  computeCurve: (throttle, thrust, torque) => invoke("compute_curve", { throttle, thrust, torque }),

  getSettings: () => invoke("get_settings"),
  updateSettings: (settings) => invoke("update_settings", { settings }),
  getReference: () => invoke("get_reference"),
};
