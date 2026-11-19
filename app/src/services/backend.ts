// The one place the UI talks to the outside world.
//
// Views and stores import `api` from here and never call Tauri directly.
// Inside the desktop app the calls go to the Rust core; in a plain browser
// (`npm run dev`) a small in-memory mock stands in, which is enough to work
// on screens and flows without building the native shell.

import { mockBackend } from "./mockBackend";
import { tauriBackend } from "./tauriBackend";
import type {
  AppSettings, BenchEvent, BenchSnapshot, CalFit, CalPoint, CurvePoint, DriverKind, PortCandidate, Profile,
  RawCapture, RecordingData, RecordingStatus, RecordingSummary, Reference, Series, SessionMeta, Spectrum, Stats, TestSetup,
} from "./types";

export interface Backend {
  onEvent(handler: (event: BenchEvent) => void): Promise<() => void>;
  /** Asks before the window closes; the guard returns false to veto. */
  onCloseRequested(guard: () => Promise<boolean>): Promise<void>;
  pickFolder(): Promise<string | null>;
  /** Yes/no confirmation dialog; true means the operator chose to proceed. */
  confirm(message: string): Promise<boolean>;

  getSnapshot(): Promise<BenchSnapshot>;
  listPorts(): Promise<PortCandidate[]>;
  connectSimulator(): Promise<void>;
  connectPort(port: string): Promise<void>;
  disconnect(): Promise<void>;
  selectDriver(kind: DriverKind): Promise<void>;

  arm(confirmed: boolean): Promise<void>;
  disarm(): Promise<void>;
  setThrottle(fraction: number): Promise<void>;
  estop(): Promise<void>;
  clearEstop(): Promise<void>;
  uiAlive(): Promise<void>;
  builtinProfiles(): Promise<Profile[]>;
  runProfile(profile: Profile): Promise<void>;
  abortProfile(): Promise<void>;

  tare(channel: number): Promise<RawCapture>;
  setChannelParam(channel: number, field: number, kind: "u32" | "i32" | "f32", value: number): Promise<void>;
  getGlobalParams(): Promise<Record<string, number>>;
  setGlobalParam(name: string, value: number): Promise<void>;
  saveDeviceConfig(): Promise<void>;
  loadDeviceDefaults(): Promise<void>;
  applyLoadCellProfile(channel: number, profileId: string, torqueArmMm: number | null): Promise<void>;
  captureCalibrationPoint(channel: number, kind: "thrust" | "torque", massG: number): Promise<CalPoint>;
  fitCalibration(points: CalPoint[]): Promise<CalFit>;
  applyCalibration(channel: number, points: CalPoint[]): Promise<CalFit>;

  startRecording(setup: TestSetup): Promise<RecordingStatus>;
  stopRecording(): Promise<SessionMeta>;
  listRecordings(): Promise<RecordingSummary[]>;
  loadRecording(id: string): Promise<RecordingData>;

  computeStats(series: Series): Promise<Stats | null>;
  computeSpectrum(series: Series, segmentLen?: number): Promise<Spectrum>;
  computeCurve(throttle: Series, thrust: Series, torque: Series | null): Promise<CurvePoint[]>;

  getSettings(): Promise<AppSettings>;
  updateSettings(settings: AppSettings): Promise<void>;
  getReference(): Promise<Reference>;
}

export const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: Backend = isDesktop ? tauriBackend : mockBackend;

/** Text of an error thrown by the backend, whatever its shape. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) return String((error as { message: unknown }).message);
  return String(error);
}
