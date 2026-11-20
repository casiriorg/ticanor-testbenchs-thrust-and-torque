// In-browser stand-in for the Rust core, used by `npm run dev` outside the
// desktop shell. It imitates the bench closely enough to exercise every
// screen and flow; it is NOT a second implementation of the safety rules.
// Real behaviour lives in app/crates/tb-core and is tested there.

import type { Backend } from "./backend";
import type {
  ActuatorState, Alarm, AppSettings, BenchEvent, BenchSnapshot, CalPoint, ChannelDescriptor, CurvePoint, Profile,
  RecordingData, RecordingSummary, Reference, Sample, Series, TestSetup,
} from "./types";

const THRUST_SCALE = 21898.2;

const settings: AppSettings = {
  language: "en", data_dir: null, auto_connect: true, preferred_driver: null,
  units: { force: "gf", torque: "nm" }, gravity: 9.80665,
  csv: { column_naming: "name", include_raw: true },
  safety: { max_throttle: 0.6, max_rise_per_s: 0.5, max_thrust_n: 12, max_torque_nm: 0.6, ui_timeout_ms: 1500 },
  display: { window_s: 20, smoothing_samples: 1, mains_filter: "off" },
  calibration: { torque_arm_mm: 100, force_ratio: 1, capture_seconds: 3, thrust_points_g: [0, 500, 1000, 2000, 3000], torque_points_g: [0, 100, 200, 500] },
  tare_max_age_min: 10, channel_names: {}, channel_load_cell: { "1": "bar_10kg", "2": "bar_10kg" },
};

const reference: Reference = {
  sensor_catalog: [
    { code: 1, key: "thrust", label: "Thrust", unit: "N" },
    { code: 2, key: "torque", label: "Torque", unit: "N·m" },
    { code: 64, key: "throttle", label: "Throttle", unit: "" },
  ],
  load_cell_profiles: [
    { id: "bar_10kg", label: "Bar cell 10 kg", capacity_kg: 10, sensitivity_mv_v: 1, safe_max_kgf: 10.5 },
    { id: "bar_3kg", label: "Bar cell 3 kg", capacity_kg: 3, sensitivity_mv_v: 1, safe_max_kgf: 3.15 },
  ],
  setup_options: {
    motor_type: ["outrunner", "inrunner"], propeller_material: ["carbon fibre", "plastic", "wood"],
    propeller_direction: ["CW", "CCW"], propeller_blades: [2, 3], esc_protocol: ["PWM", "DShot300"],
    battery_chemistry: ["LiPo", "Li-ion"], battery_cells: [4, 6],
  },
  default_settings: settings, data_dir: "Documents/ThrustBench (browser mock)", app_version: "0.1.0-mock", protocol_version: 1,
};

function channel(id: number, sensor_type: number, unit: number, scale: number, name: string): ChannelDescriptor {
  return { id, sensor_type, unit, enabled: true, invert: false, calibrated: id === 16, rate_hz: id === 16 ? 50 : 80, gain: 128, offset: 0, scale, cal_time: 0, name };
}

const channels = [channel(1, 1, 1, THRUST_SCALE, "thrust"), channel(2, 2, 2, THRUST_SCALE / 0.04, "torque"), channel(16, 64, 7, 10000, "throttle")];
const globals: Record<string, number> = { pwm_min_us: 1000, pwm_max_us: 2000, pwm_freq_hz: 50, watchdog_ms: 300, throttle_max: 6000, slew_per_sec: 5000, rate_pin: 255, rate_sps: 80, disarmed_output: 0, device_uid: 0x53494d31 };

let handlers: ((event: BenchEvent) => void)[] = [];
let connected = false;
let driver: BenchSnapshot["driver"] = null;
let actuator: ActuatorState = "disarmed";
let setpoint = 0;
let applied = 0;
let level = 0;
let clock = 0;
let tareAt: number | null = null;
let alarms: Alarm[] = [];
let profile: { profile: Profile; started: number } | null = null;
let recording: { id: string; started: number; samples: Sample[]; setup: TestSetup } | null = null;
const library = new Map<string, RecordingData>();
const noise = () => (Math.random() + Math.random() + Math.random() - 1.5) * 60;

function emit(event: BenchEvent) {
  handlers.forEach((h) => h(event));
}

function alarm(level_: Alarm["level"], code: string, message: string) {
  const a: Alarm = { id: alarms.length + 1, level: level_, code, message, time: new Date().toISOString() };
  alarms = [...alarms.slice(-29), a];
  emit({ type: "alarm", payload: a });
}

function snapshot(): BenchSnapshot {
  return {
    connection: connected ? { state: "connected", port: "simulator" } : { state: "searching" },
    simulated: connected, device: connected ? { protocol_version: 1, firmware: "0.1.0", uid: globals.device_uid, channel_count: 3, capabilities: 1, defaults_loaded: false } : null,
    channels: connected ? channels : [],
    status: connected ? { actuator, streaming: true, config_dirty: false, defaults_loaded: false, tx_overflow: false, throttle: applied, pulse_us: Math.round(1000 + 1000 * applied), tx_dropped: 0, crc_errors: 0, loop_max_us: 180, watchdog_trips: 0, uptime_ms: Math.round(clock * 1000) } : null,
    link: { frames_received: Math.round(clock * 220), frames_lost: 0, decode_errors: 0 },
    driver, driver_options: [
      { kind: "nano_pwm", available: true, detail: "D9", experimental: false },
      { kind: "mavlink", available: false, detail: "", experimental: true },
      { kind: "manual", available: true, detail: "", experimental: false },
    ],
    actuator: connected ? actuator : null, setpoint,
    recording: recording ? { id: recording.id, path: `mock/${recording.id}`, elapsed_s: clock - recording.started, rows: recording.samples.length, dropped_rows: 0 } : null,
    profile: profile ? { name: profile.profile.name, elapsed_s: clock - profile.started, total_s: profile.profile.steps.reduce((s, x) => s + x.duration_s, 0) } : null,
    tare_age_s: tareAt === null ? null : clock - tareAt, alarms,
  };
}

function profileSetpoint(p: Profile, elapsed: number): number | null {
  let start = 0, previous = 0;
  for (const step of p.steps) {
    if (elapsed < start + step.duration_s) {
      return step.kind === "hold" ? step.target : previous + ((step.target - previous) * (elapsed - start)) / step.duration_s;
    }
    start += step.duration_s;
    previous = step.target;
  }
  return null;
}

function raw(index: number): number {
  const physical = index === 0 ? 30 * level * level + 0.05 * level * Math.sin(2 * Math.PI * 17 * clock) : 0.5 * level * level;
  return Math.round((index === 0 ? 84210 : -35120) + physical * channels[index].scale + noise());
}

function toSample(index: number, t: number): Sample {
  const c = channels[index];
  const r = raw(index);
  return { channel: c.id, t, raw: r, value: ((r - c.offset) / c.scale) * (c.invert ? -1 : 1) };
}

let ticks = 0;
function tick() {
  if (!connected) return;
  const dt = 0.05;
  const batch: Sample[] = [];
  for (let i = 0; i < 4; i++) {
    clock += dt / 4;
    if (actuator === "armed") {
      if (profile) {
        const value = profileSetpoint(profile.profile, clock - profile.started);
        if (value === null) { profile = null; setpoint = 0; alarm("info", "profile_done", "The throttle profile finished."); } else setpoint = value;
      }
      applied = setpoint < applied ? setpoint : Math.min(setpoint, applied + settings.safety.max_rise_per_s * (dt / 4));
    } else applied = 0;
    level += (applied - level) * (1 - Math.exp(-(dt / 4) / 0.08));
    batch.push(toSample(0, clock), toSample(1, clock + 0.006));
    if (actuator === "armed" && i % 2 === 0) batch.push({ channel: 16, t: clock, raw: Math.round(applied * 10000), value: applied });
  }
  recording?.samples.push(...batch);
  emit({ type: "samples", payload: batch });
  if (++ticks % 4 === 0) emit({ type: "snapshot", payload: snapshot() });
}

function seriesOf(samples: Sample[], id: number, t0: number): Series {
  const picked = samples.filter((s) => s.channel === id);
  return { t: picked.map((s) => s.t - t0), v: picked.map((s) => s.value) };
}

function store(id: string, samples: Sample[], setup: TestSetup, started: Date) {
  const t0 = samples[0]?.t ?? 0;
  const duration = samples.length ? samples[samples.length - 1].t - t0 : 0;
  library.set(id, {
    meta: { id, started_at: started.toISOString(), ended_at: new Date().toISOString(), state: "complete", driver: "nano_pwm", events: [], rows: samples.length, dropped_rows: 0, frames_lost: 0, duration_s: duration },
    setup,
    channels: channels.map((c) => ({ channel_id: c.id, sensor_type: c.sensor_type, key: c.name, name: c.name, unit: reference.sensor_catalog.find((e) => e.code === c.sensor_type)?.unit ?? "", series: seriesOf(samples, c.id, t0) })),
  });
}

export function emptySetup(): TestSetup {
  return {
    motor: { brand: "", model: "", kv: null, stator: "", poles: null, mass_g: null, kind: "outrunner", condition: "" },
    propeller: { brand: "", model: "", diameter_in: null, pitch_in: null, blades: 2, material: "", direction: "CW", mass_g: null, condition: "" },
    esc: { brand: "", model: "", current_a: null, protocol: "PWM", firmware: "" },
    battery: { chemistry: "", cells: null, capacity_mah: null, start_voltage: null },
    test: { operator: "", ambient_temp_c: null, altitude_m: null, notes: "" },
  };
}

/** Two demo recordings (a staircase each) so the library is not empty. */
function seedLibrary() {
  [["MAD", "3515 IPE 400KV", 30], ["Walkera", "WK-WS-48-001", 22]].forEach(([brand, model, gain], n) => {
    const samples: Sample[] = [];
    let lvl = 0;
    for (let i = 0; i < 80 * 26; i++) {
      const t = i / 80;
      const u = t < 1 ? 0 : t > 25 ? 0 : Math.min(0.6, Math.floor((t - 1) / 4 + 1) * 0.1);
      lvl += (u - lvl) * 0.14;
      samples.push({ channel: 1, t, raw: 0, value: (gain as number) * lvl * lvl + noise() / THRUST_SCALE });
      samples.push({ channel: 2, t, raw: 0, value: 0.5 * lvl * lvl + noise() / (THRUST_SCALE / 0.04) });
      if (i % 2 === 0) samples.push({ channel: 16, t, raw: 0, value: u });
    }
    const setup = emptySetup();
    setup.motor.brand = brand as string;
    setup.motor.model = model as string;
    setup.propeller.brand = "Walkera";
    setup.propeller.diameter_in = 15.5;
    store(`2026-11-1${n}_100000_${brand}-demo`, samples, setup, new Date(Date.UTC(2026, 10, 10 + n, 15)));
  });
}

const need = (ok: boolean, message: string, code = "safety") => { if (!ok) throw { code, message }; };
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Browser stand-in for a native save dialog: triggers a normal download. */
function downloadBytes(bytes: Uint8Array, name: string, mime: string) {
  // Uint8Array is a valid BlobPart at runtime; TS's DOM lib is overly strict about its buffer type.
  const blob = new Blob([bytes as unknown as BlobPart], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

/** Simplified stand-in for the real per-channel CSV the Rust core writes. */
function buildCsv(data: RecordingData): string {
  const rows = ["channel,t_s,value"];
  for (const c of data.channels) {
    for (let i = 0; i < c.series.t.length; i++) rows.push(`${c.key},${c.series.t[i]},${c.series.v[i]}`);
  }
  return rows.join("\n");
}

export const mockBackend: Backend = {
  async onEvent(handler) {
    if (handlers.length === 0) { seedLibrary(); setInterval(tick, 50); setTimeout(() => { connected = true; emit({ type: "snapshot", payload: snapshot() }); }, 600); }
    handlers.push(handler);
    return () => { handlers = handlers.filter((h) => h !== handler); };
  },
  async onCloseRequested() {},
  async pickFolder() { return null; },
  async confirm(message) { return window.confirm(message); },
  async getSnapshot() { return snapshot(); },
  async listPorts() { return []; },
  async connectSimulator() { connected = true; },
  async connectPort() { connected = true; },
  async disconnect() { need(actuator !== "armed", "disarm before disconnecting"); need(!recording, "stop the recording before disconnecting"); connected = false; driver = null; },
  async selectDriver(kind) { need(actuator !== "armed", "disarm before changing the throttle mode"); need(kind !== "mavlink", "no flight controller was detected", "invalid"); driver = kind; },
  async arm(confirmed) {
    need(driver === "nano_pwm", "the selected throttle mode cannot command the ESC");
    need(actuator === "disarmed", "not in a state that can be armed");
    need(confirmed, "confirm that the propeller area is clear");
    actuator = "armed"; setpoint = 0;
  },
  async disarm() { if (actuator !== "e_stop") actuator = "disarmed"; setpoint = 0; profile = null; },
  async setThrottle(fraction) { need(actuator === "armed", "arm the output first"); need(!profile, "a profile is running; abort it to take manual control"); setpoint = Math.max(0, Math.min(settings.safety.max_throttle, fraction)); },
  async estop() { actuator = "e_stop"; setpoint = 0; profile = null; alarm("critical", "estop", "Emergency stop pressed. Clear it to arm again."); },
  async clearEstop() { if (actuator === "e_stop") actuator = "disarmed"; },
  async uiAlive() {},
  async builtinProfiles() {
    const top = Math.floor(settings.safety.max_throttle * 10) / 10;
    const stairs: Profile["steps"] = [];
    for (let l = 0.1; l <= top + 1e-9; l += 0.1) stairs.push({ kind: "hold", target: Math.round(l * 10) / 10, duration_s: 4 });
    stairs.push({ kind: "hold", target: 0, duration_s: 1 });
    return [
      { name: "Staircase 10 % steps", description: "Holds each 10 % level for 4 s up to the throttle limit.", steps: stairs },
      { name: "Slow ramp", description: "Ramps to the throttle limit in 20 s and back in 10 s.", steps: [{ kind: "ramp", target: top, duration_s: 20 }, { kind: "ramp", target: 0, duration_s: 10 }] },
    ];
  },
  async runProfile(p) { need(actuator === "armed", "arm the output first"); profile = { profile: p, started: clock }; },
  async abortProfile() { profile = null; setpoint = 0; },
  async tare(ch) {
    need(actuator !== "armed", "disarm before taring");
    await wait(1000);
    const index = channels.findIndex((c) => c.id === ch);
    const mean = raw(index);
    channels[index] = { ...channels[index], offset: mean };
    tareAt = clock;
    return { mean, std: 40, samples: 80 };
  },
  async setChannelParam(ch, field, _kind, value) {
    const index = channels.findIndex((c) => c.id === ch);
    const key = ({ 1: "enabled", 2: "gain", 3: "offset", 4: "scale", 5: "invert" } as Record<number, keyof ChannelDescriptor>)[field];
    if (key) channels[index] = { ...channels[index], [key]: key === "enabled" || key === "invert" ? value !== 0 : value };
  },
  async getGlobalParams() { return { ...globals }; },
  async setGlobalParam(name, value) { need(actuator !== "armed", "not allowed while armed", "rejected"); globals[name] = value; },
  async saveDeviceConfig() {},
  async loadDeviceDefaults() {},
  async applyLoadCellProfile() {},
  async captureCalibrationPoint(ch, kind, massG) {
    await wait(settings.calibration.capture_seconds * 300);
    const c = channels.find((x) => x.id === ch)!;
    const referenceValue = kind === "thrust" ? (massG / 1000) * settings.gravity * settings.calibration.force_ratio : (massG / 1000) * settings.gravity * (settings.calibration.torque_arm_mm / 1000);
    return { mass_g: massG, reference: referenceValue, raw_mean: (ch === 1 ? 84210 : -35120) + referenceValue * c.scale * 1.03 + noise(), raw_std: 38, samples: 240 };
  },
  async fitCalibration(points: CalPoint[]) {
    need(points.length >= 2, "at least two calibration points are needed", "invalid");
    const n = points.length, mx = points.reduce((s, p) => s + p.reference, 0) / n, my = points.reduce((s, p) => s + p.raw_mean, 0) / n;
    const sxx = points.reduce((s, p) => s + (p.reference - mx) ** 2, 0), sxy = points.reduce((s, p) => s + (p.reference - mx) * (p.raw_mean - my), 0);
    need(sxx > 0, "the calibration points must use at least two different loads", "invalid");
    const slope = sxy / sxx, offset = my - slope * mx;
    const residuals = points.map((p) => (p.raw_mean - offset) / slope - p.reference);
    const maxError = Math.max(...residuals.map(Math.abs)), span = Math.max(...points.map((p) => p.reference)) - Math.min(...points.map((p) => p.reference));
    return { scale: Math.abs(slope), offset, invert: slope < 0, r_squared: 0.99999, residuals, max_error: maxError, nonlinearity_pct: (maxError / span) * 100 };
  },
  async applyCalibration(ch, points) {
    const fit = await mockBackend.fitCalibration(points);
    const index = channels.findIndex((c) => c.id === ch);
    channels[index] = { ...channels[index], scale: fit.scale, offset: Math.round(fit.offset), invert: fit.invert, calibrated: true, cal_time: Math.round(Date.now() / 1000) };
    return fit;
  },
  async startRecording(setup) {
    need(connected, "no device is connected", "not_connected");
    const id = `${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}_${setup.motor.model || "test"}`;
    recording = { id, started: clock, samples: [], setup };
    return snapshot().recording!;
  },
  async stopRecording() {
    need(!!recording, "no recording is running", "recording");
    const r = recording!;
    recording = null;
    store(r.id, r.samples, r.setup, new Date());
    return library.get(r.id)!.meta;
  },
  async listRecordings() {
    const label = (s: TestSetup) => [s.motor.brand, s.motor.model].filter(Boolean).join(" ");
    return [...library.values()].map<RecordingSummary>((d) => ({ id: d.meta.id, path: `mock/${d.meta.id}`, started_at: d.meta.started_at, duration_s: d.meta.duration_s, state: d.meta.state, rows: d.meta.rows, motor: label(d.setup), propeller: [d.setup.propeller.brand, d.setup.propeller.diameter_in ? `${d.setup.propeller.diameter_in}in` : ""].filter(Boolean).join(" "), notes: d.setup.test.notes, event_count: 0 })).sort((a, b) => b.started_at.localeCompare(a.started_at));
  },
  async loadRecording(id) { need(library.has(id), `recording ${id} was not found`, "recording"); return library.get(id)!; },
  async computeStats(series) {
    if (!series.v.length) return null;
    const n = series.v.length, m = series.v.reduce((s, v) => s + v, 0) / n, min = Math.min(...series.v), max = Math.max(...series.v);
    return { count: n, mean: m, std: Math.sqrt(series.v.reduce((s, v) => s + (v - m) ** 2, 0) / Math.max(1, n - 1)), min, max, peak_to_peak: max - min, rate_hz: (n - 1) / (series.t[n - 1] - series.t[0] || 1) };
  },
  async computeSpectrum(series) {
    need(series.v.length >= 32, "not enough samples for a spectrum", "invalid");
    const n = Math.min(256, 2 ** Math.floor(Math.log2(series.v.length)));
    const v = series.v.slice(-n), rate = (n - 1) / (series.t[series.t.length - 1] - series.t[series.t.length - n]);
    const m = v.reduce((s, x) => s + x, 0) / n;
    const amplitude: number[] = [], freq: number[] = [];
    for (let k = 0; k <= n / 2; k++) {
      let re = 0, im = 0;
      for (let i = 0; i < n; i++) { const w = 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / n); re += (v[i] - m) * w * Math.cos((2 * Math.PI * k * i) / n); im -= (v[i] - m) * w * Math.sin((2 * Math.PI * k * i) / n); }
      amplitude.push((2 * Math.hypot(re, im)) / (n / 2));
      freq.push((k * rate) / n);
    }
    return { freq_hz: freq, amplitude, rate_hz: rate, resolution_hz: rate / n, segments: 1 };
  },
  async computeCurve(throttle, thrust, torque) {
    const points: CurvePoint[] = [];
    let start = 0;
    const meanIn = (s: Series, a: number, b: number) => { const v = s.v.filter((_, i) => s.t[i] >= a && s.t[i] <= b); return v.length ? v.reduce((x, y) => x + y, 0) / v.length : NaN; };
    for (let i = 1; i <= throttle.v.length; i++) {
      if (i < throttle.v.length && Math.abs(throttle.v[i] - throttle.v[start]) <= 0.01) continue;
      const t0 = throttle.t[start], t1 = throttle.t[i - 1], u = throttle.v[start];
      if (t1 - t0 >= 1 && u > 0.01) {
        const a = t0 + (t1 - t0) * 0.4, f = meanIn(thrust, a, t1), q = torque ? meanIn(torque, a, t1) : null;
        points.push({ throttle: u, t0: a, t1, thrust_mean: f, thrust_std: 0, torque_mean: q, torque_std: 0, thrust_per_torque: q ? f / q : null, samples: 0 });
      }
      start = i;
    }
    return points;
  },
  async getSettings() { return structuredClone(settings); },
  async updateSettings(next) { Object.assign(settings, structuredClone(next)); },
  async getReference() { return reference; },

  async saveBytes(bytes, suggestedName, mime) {
    downloadBytes(bytes, suggestedName, mime);
    return true;
  },
  async exportRecordingCsv(id, suggestedName) {
    const data = library.get(id);
    need(!!data, `recording ${id} was not found`, "recording");
    downloadBytes(new TextEncoder().encode(buildCsv(data!)), suggestedName, "text/csv");
    return true;
  },
  async exportRecordingsCsv(ids) {
    // No real folder picker in the browser mock: download each CSV separately.
    for (const id of ids) {
      const data = library.get(id);
      if (data) downloadBytes(new TextEncoder().encode(buildCsv(data)), `${id}.csv`, "text/csv");
    }
    return true;
  },
};
