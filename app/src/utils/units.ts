// Unit conversion for display. Everything is stored and transmitted in SI
// (newtons, newton metres); only the screen converts.

import type { AppSettings, ChannelDescriptor, LoadCellProfile } from "../services/types";
import { SENSOR } from "../services/types";

export const STANDARD_GRAVITY = 9.80665;

export function forceUnitLabel(unit: AppSettings["units"]["force"]): string {
  return { gf: "gf", kgf: "kgf", n: "N" }[unit];
}

/** Grams-force are defined with standard gravity, not local gravity. */
export function forceFromNewton(newton: number, unit: AppSettings["units"]["force"]): number {
  if (unit === "gf") return (newton / STANDARD_GRAVITY) * 1000;
  if (unit === "kgf") return newton / STANDARD_GRAVITY;
  return newton;
}

export function torqueUnitLabel(unit: AppSettings["units"]["torque"]): string {
  return unit === "nm" ? "N·m" : "N·mm";
}

export function torqueFromNewtonMetre(value: number, unit: AppSettings["units"]["torque"]): number {
  return unit === "nm" ? value : value * 1000;
}

/** Converter and label for a channel, chosen by its sensor type code. */
export function channelDisplay(sensorType: number, units: AppSettings["units"]): { convert: (v: number) => number; unit: string; digits: number } {
  switch (sensorType) {
    case 1:
    case 3:
      return { convert: (v) => forceFromNewton(v, units.force), unit: forceUnitLabel(units.force), digits: units.force === "gf" ? 1 : 3 };
    case 2:
      return { convert: (v) => torqueFromNewtonMetre(v, units.torque), unit: torqueUnitLabel(units.torque), digits: units.torque === "nm" ? 4 : 1 };
    case 64:
      return { convert: (v) => v * 100, unit: "%", digits: 1 };
    default:
      return { convert: (v) => v, unit: "", digits: 3 };
  }
}

/**
 * Configured limits for a load cell channel, in SI (N, or N·m for torque).
 * `safety` is the operator-configured cut-off (Settings > safety); `cellMax`
 * is the assigned load cell profile's rated overload ceiling. Either can be
 * `null` when not configured/assigned.
 */
export function channelLimitsSI(channel: ChannelDescriptor, settings: AppSettings, profiles: LoadCellProfile[]): { safety: number | null; cellMax: number | null } {
  if (channel.sensor_type > 3) return { safety: null, cellMax: null };
  const isTorque = channel.sensor_type === SENSOR.torque;
  const safety = isTorque ? settings.safety.max_torque_nm : settings.safety.max_thrust_n;
  const profile = profiles.find((p) => p.id === settings.channel_load_cell[String(channel.id)]);
  if (!profile) return { safety, cellMax: null };
  const forceN = profile.safe_max_kgf * settings.gravity;
  return { safety, cellMax: isTorque ? forceN * (settings.calibration.torque_arm_mm / 1000) : forceN };
}

export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const pad = (n: number) => String(n).padStart(2, "0");
  return s >= 3600 ? `${Math.floor(s / 3600)}:${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}` : `${pad(Math.floor(s / 60))}:${pad(s % 60)}`;
}
