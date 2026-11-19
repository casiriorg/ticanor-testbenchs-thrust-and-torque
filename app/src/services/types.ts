// Data shapes exchanged with the Rust core. They mirror the `serde` output
// of the types in app/crates/tb-core; keep both sides in sync.

export type DriverKind = "nano_pwm" | "mavlink" | "manual";
export type ActuatorState = "disarmed" | "armed" | "e_stop" | "failsafe";

export type ConnectionState =
  | { state: "idle" }
  | { state: "searching" }
  | { state: "connecting"; port: string }
  | { state: "connected"; port: string }
  | { state: "lost"; reason: string };

export interface ChannelDescriptor {
  id: number;
  sensor_type: number;
  unit: number;
  enabled: boolean;
  invert: boolean;
  calibrated: boolean;
  rate_hz: number;
  gain: number;
  offset: number;
  scale: number;
  cal_time: number;
  name: string;
}

export interface DeviceInfo {
  protocol_version: number;
  firmware: string;
  uid: number;
  channel_count: number;
  capabilities: number;
  defaults_loaded: boolean;
}

export interface DeviceStatus {
  actuator: ActuatorState;
  streaming: boolean;
  config_dirty: boolean;
  defaults_loaded: boolean;
  tx_overflow: boolean;
  throttle: number;
  pulse_us: number;
  tx_dropped: number;
  crc_errors: number;
  loop_max_us: number;
  watchdog_trips: number;
  uptime_ms: number;
}

export interface LinkStats {
  frames_received: number;
  frames_lost: number;
  decode_errors: number;
}

export interface DriverOption {
  kind: DriverKind;
  available: boolean;
  detail: string;
  experimental: boolean;
}

export type AlarmLevel = "info" | "warning" | "critical";

export interface Alarm {
  id: number;
  level: AlarmLevel;
  code: string;
  message: string;
  time: string;
}

export interface RecordingStatus {
  id: string;
  path: string;
  elapsed_s: number;
  rows: number;
  dropped_rows: number;
}

export interface ProfileStatus {
  name: string;
  elapsed_s: number;
  total_s: number;
}

export interface BenchSnapshot {
  connection: ConnectionState;
  simulated: boolean;
  device: DeviceInfo | null;
  channels: ChannelDescriptor[];
  status: DeviceStatus | null;
  link: LinkStats;
  driver: DriverKind | null;
  driver_options: DriverOption[];
  actuator: ActuatorState | null;
  setpoint: number;
  recording: RecordingStatus | null;
  profile: ProfileStatus | null;
  tare_age_s: number | null;
  alarms: Alarm[];
}

export interface Sample {
  channel: number;
  t: number;
  raw: number;
  value: number;
}

export type BenchEvent =
  | { type: "snapshot"; payload: BenchSnapshot }
  | { type: "samples"; payload: Sample[] }
  | { type: "alarm"; payload: Alarm };

export interface Series {
  t: number[];
  v: number[];
}

export interface Stats {
  count: number;
  mean: number;
  std: number;
  min: number;
  max: number;
  peak_to_peak: number;
  rate_hz: number;
}

export interface Spectrum {
  freq_hz: number[];
  amplitude: number[];
  rate_hz: number;
  resolution_hz: number;
  segments: number;
}

export interface CurvePoint {
  throttle: number;
  t0: number;
  t1: number;
  thrust_mean: number;
  thrust_std: number;
  torque_mean: number | null;
  torque_std: number | null;
  thrust_per_torque: number | null;
  samples: number;
}

export interface ProfileStep {
  kind: "hold" | "ramp";
  target: number;
  duration_s: number;
}

export interface Profile {
  name: string;
  description: string;
  steps: ProfileStep[];
}

export interface TestSetup {
  motor: { brand: string; model: string; kv: number | null; stator: string; poles: number | null; mass_g: number | null; kind: string; condition: string };
  propeller: { brand: string; model: string; diameter_in: number | null; pitch_in: number | null; blades: number | null; material: string; direction: string; mass_g: number | null; condition: string };
  esc: { brand: string; model: string; current_a: number | null; protocol: string; firmware: string };
  battery: { chemistry: string; cells: number | null; capacity_mah: number | null; start_voltage: number | null };
  test: { operator: string; ambient_temp_c: number | null; altitude_m: number | null; notes: string };
}

export interface RecordingSummary {
  id: string;
  path: string;
  started_at: string;
  duration_s: number;
  state: "recording" | "complete" | "interrupted" | "recovered";
  rows: number;
  motor: string;
  propeller: string;
  notes: string;
  event_count: number;
}

export interface RecordingEvent {
  t_s: number;
  kind: string;
  message: string;
}

export interface SessionMeta {
  id: string;
  started_at: string;
  ended_at: string | null;
  state: RecordingSummary["state"];
  driver: DriverKind;
  events: RecordingEvent[];
  rows: number;
  dropped_rows: number;
  frames_lost: number;
  duration_s: number;
}

export interface ChannelSeries {
  channel_id: number;
  sensor_type: number;
  key: string;
  name: string;
  unit: string;
  series: Series;
}

export interface RecordingData {
  meta: SessionMeta;
  setup: TestSetup;
  channels: ChannelSeries[];
}

export interface AppSettings {
  language: string;
  data_dir: string | null;
  auto_connect: boolean;
  preferred_driver: DriverKind | null;
  units: { force: "gf" | "kgf" | "n"; torque: "nm" | "nmm" };
  gravity: number;
  csv: { column_naming: "name" | "id"; include_raw: boolean };
  safety: { max_throttle: number; max_rise_per_s: number; max_thrust_n: number | null; max_torque_nm: number | null; ui_timeout_ms: number };
  display: { window_s: number; smoothing_samples: number; mains_filter: "off" | "50" | "60" };
  calibration: { torque_arm_mm: number; force_ratio: number; capture_seconds: number; thrust_points_g: number[]; torque_points_g: number[] };
  tare_max_age_min: number;
  channel_names: Record<string, string>;
  /** Load cell profile id applied to each channel (channel id as a string). */
  channel_load_cell: Record<string, string>;
}

export interface LoadCellProfile {
  id: string;
  label: string;
  capacity_kg: number;
  sensitivity_mv_v: number;
  /** Highest load the cell is rated to survive, kgf; shown as the monitor's maximum line. */
  safe_max_kgf: number;
}

export interface Reference {
  sensor_catalog: { code: number; key: string; label: string; unit: string }[];
  load_cell_profiles: LoadCellProfile[];
  setup_options: Record<string, (string | number)[]>;
  default_settings: AppSettings;
  data_dir: string;
  app_version: string;
  protocol_version: number;
}

export interface PortCandidate {
  port: string;
  vid: number | null;
  pid: number | null;
  product: string | null;
  role: "acquisition_board" | "flight_controller" | "unknown";
}

export interface RawCapture {
  mean: number;
  std: number;
  samples: number;
}

export interface CalPoint {
  mass_g: number;
  reference: number;
  raw_mean: number;
  raw_std: number;
  samples: number;
}

export interface CalFit {
  scale: number;
  offset: number;
  invert: boolean;
  r_squared: number;
  residuals: number[];
  max_error: number;
  nonlinearity_pct: number;
}

/** Error returned by every backend call that fails. */
export interface BackendError {
  code: string;
  message: string;
}

/** Sensor type codes (protocol ids). */
export const SENSOR = { thrust: 1, torque: 2, throttle: 64 } as const;
/** Parameter field ids for load-cell channels. */
export const CHANNEL_FIELD = { enabled: 1, gain: 2, offset: 3, scale: 4, invert: 5, calTime: 6, sensorType: 7 } as const;
