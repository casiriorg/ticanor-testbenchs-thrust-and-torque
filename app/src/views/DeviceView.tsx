import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { InfoTip } from "../components/InfoTip";
import { NumberField, Section, SelectField } from "../components/Fields";
import { api } from "../services/backend";
import type { CalFit, CalPoint, ChannelDescriptor } from "../services/types";
import { CHANNEL_FIELD, SENSOR } from "../services/types";
import { useBench } from "../stores/benchStore";

/** Board settings shown in the form, in display order. Percent fields are stored as 0..10000. */
const GLOBAL_FIELDS: { name: string; percent?: boolean; choices?: { value: number; key: string }[]; rateOptions?: boolean; info?: string }[] = [
  { name: "pwm_min_us" },
  { name: "pwm_max_us" },
  { name: "pwm_freq_hz" },
  { name: "throttle_max", percent: true },
  { name: "slew_per_sec", percent: true, info: "device.slewInfo" },
  { name: "watchdog_ms", info: "device.watchdogInfo" },
  { name: "disarmed_output", choices: [{ value: 0, key: "minPulse" }, { value: 1, key: "noSignal" }] },
  { name: "rate_pin", info: "device.ratePinInfo" },
  { name: "rate_sps", rateOptions: true, info: "device.rateSpsInfo" },
];

function ChannelRow({ channel }: { channel: ChannelDescriptor }) {
  const { t } = useTranslation();
  const run = useBench((s) => s.run);
  const profiles = useBench((s) => s.reference?.load_cell_profiles ?? []);
  const settings = useBench((s) => s.settings);
  const save = useBench((s) => s.saveSettings);
  const [profileId, setProfileId] = useState(settings?.channel_load_cell[String(channel.id)] ?? profiles[0]?.id ?? "");
  const [arm, setArm] = useState<number | null>(40);
  const isTorque = channel.sensor_type === SENSOR.torque;
  const set = (field: number, value: number) => void run(() => api.setChannelParam(channel.id, field, "u32", value));

  const applyProfile = async () => {
    const applied = await run(() => api.applyLoadCellProfile(channel.id, profileId, isTorque ? arm : null));
    if (applied === undefined || !settings) return;
    // Remembered so the monitor knows which profile's safe_max_kgf applies to this channel.
    void save({ ...settings, channel_load_cell: { ...settings.channel_load_cell, [String(channel.id)]: profileId } });
  };

  return (
    <tr>
      <td><strong>{channel.name}</strong></td>
      <td><input type="checkbox" aria-label={t("device.enabled")} checked={channel.enabled} onChange={(e) => set(CHANNEL_FIELD.enabled, e.target.checked ? 1 : 0)} /></td>
      <td>
        <select aria-label={t("device.gain")} value={channel.gain} onChange={(e) => set(CHANNEL_FIELD.gain, Number(e.target.value))}>
          <option value={128}>128</option>
          <option value={64}>64</option>
        </select>
      </td>
      <td><input type="checkbox" aria-label={t("device.invert")} checked={channel.invert} onChange={(e) => set(CHANNEL_FIELD.invert, e.target.checked ? 1 : 0)} /></td>
      <td className="n mono">{channel.offset}</td>
      <td className="n mono">{channel.scale.toFixed(1)}</td>
      <td>{channel.calibrated ? new Date(channel.cal_time * 1000).toLocaleDateString() : <em className="warn">{t("device.nominal")}</em>}</td>
      <td>
        <div className="row tight">
          <select aria-label={t("device.cellProfile")} value={profileId} onChange={(e) => setProfileId(e.target.value)}>
            {profiles.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
          </select>
          {isTorque && <input type="number" className="short" aria-label={t("device.torqueArm")} title={t("device.torqueArm")} value={arm ?? ""} onChange={(e) => setArm(Number.isNaN(e.target.valueAsNumber) ? null : e.target.valueAsNumber)} />}
          <button title={t("device.applyProfileInfo")} onClick={() => void applyProfile()}>{t("device.applyProfile")}</button>
        </div>
      </td>
    </tr>
  );
}

function BoardSettings() {
  const { t } = useTranslation();
  const run = useBench((s) => s.run);
  const dirty = useBench((s) => s.snapshot?.status?.config_dirty ?? false);
  const armed = useBench((s) => s.snapshot?.actuator === "armed");
  const [values, setValues] = useState<Record<string, number> | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const loaded = await run(() => api.getGlobalParams());
    if (loaded) setValues(loaded);
  }, [run]);
  useEffect(() => void reload(), [reload]);
  if (!values) return <p className="muted">{t("common.loading")}</p>;

  // Each field is written to the board when it loses focus or changes.
  const write = async (name: string, shown: number | null, percent?: boolean) => {
    if (shown === null) return;
    const value = Math.round(percent ? shown * 100 : shown);
    await run(() => api.setGlobalParam(name, value));
    await reload();
  };

  return (
    <>
      <div className="form-grid">
        {GLOBAL_FIELDS.map((f) => {
          const shown = f.percent ? values[f.name] / 100 : values[f.name];
          if (f.choices) {
            return <SelectField key={f.name} id={`g-${f.name}`} label={t(`device.${f.name}`)} info={f.info && t(f.info)} value={values[f.name]} options={f.choices.map((c) => ({ value: c.value, label: t(`device.${c.key}`) }))} onChange={(v) => void write(f.name, v)} />;
          }
          if (f.rateOptions) {
            return <SelectField key={f.name} id={`g-${f.name}`} label={t(`device.${f.name}`)} info={f.info && t(f.info)} value={values[f.name]} options={[10, 80].map((v) => ({ value: v, label: String(v) }))} onChange={(v) => void write(f.name, v)} />;
          }
          return (
            <label key={f.name} className="field" htmlFor={`g-${f.name}`}>
              <span className="field-label">{t(`device.${f.name}`)}{f.info && <InfoTip text={t(f.info)} />}</span>
              <input id={`g-${f.name}`} type="number" disabled={armed} defaultValue={shown} key={`${f.name}-${shown}`} onBlur={(e) => e.target.valueAsNumber !== shown && void write(f.name, Number.isNaN(e.target.valueAsNumber) ? null : e.target.valueAsNumber, f.percent)} />
            </label>
          );
        })}
      </div>
      <div className="row">
        {dirty && <span className="chip warn">{t("device.dirty")}</span>}
        {message && <span className="small">{message}</span>}
        <span className="grow" />
        <button disabled={armed} onClick={() => void run(() => api.loadDeviceDefaults()).then(reload)}>{t("device.loadDefaults")}</button>
        <button id="save-eeprom" className="primary" disabled={armed} onClick={() => void run(() => api.saveDeviceConfig()).then((ok) => ok !== undefined && setMessage(t("device.saved")))}>{t("device.saveEeprom")}</button>
      </div>
    </>
  );
}

/** Multi-point calibration with known masses. */
function Calibration({ channels }: { channels: ChannelDescriptor[] }) {
  const { t } = useTranslation();
  const settings = useBench((s) => s.settings)!;
  const save = useBench((s) => s.saveSettings);
  const run = useBench((s) => s.run);
  const cells = channels.filter((c) => c.sensor_type <= 3);
  const [channelId, setChannelId] = useState(cells[0]?.id ?? 1);
  const [mass, setMass] = useState<number | null>(0);
  const [points, setPoints] = useState<CalPoint[]>([]);
  const [fit, setFit] = useState<CalFit | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);

  const channel = cells.find((c) => c.id === channelId);
  const kind = channel?.sensor_type === SENSOR.torque ? "torque" : "thrust";
  const unit = kind === "torque" ? "N·m" : "N";
  const suggested = kind === "torque" ? settings.calibration.torque_points_g : settings.calibration.thrust_points_g;
  const geometry = (patch: Partial<typeof settings.calibration>) => void save({ ...settings, calibration: { ...settings.calibration, ...patch } });

  useEffect(() => {
    setPoints([]);
    setFit(null);
    setDone(false);
  }, [channelId]);
  useEffect(() => {
    if (points.length < 2) return setFit(null);
    api.fitCalibration(points).then(setFit).catch(() => setFit(null));
  }, [points]);

  const capture = async () => {
    if (mass === null) return;
    setBusy(true);
    const point = await run(() => api.captureCalibrationPoint(channelId, kind, mass));
    setBusy(false);
    if (point) setPoints((p) => [...p.filter((x) => x.mass_g !== point.mass_g), point].sort((a, b) => a.mass_g - b.mass_g));
  };

  const apply = async () => {
    const applied = await run(() => api.applyCalibration(channelId, points));
    if (applied) setDone(true);
  };

  if (!channel) return null;
  return (
    <>
      <div className="form-grid">
        <SelectField id="cal-channel" label={t("device.calChannel")} value={channelId} options={cells.map((c) => ({ value: c.id, label: c.name }))} onChange={(v) => v && setChannelId(v)} />
        {kind === "torque"
          ? <NumberField id="cal-arm" label={t("device.calArm")} value={settings.calibration.torque_arm_mm} onChange={(v) => v && geometry({ torque_arm_mm: v })} />
          : <NumberField id="cal-ratio" label={t("device.forceRatio")} value={settings.calibration.force_ratio} onChange={(v) => v && geometry({ force_ratio: v })} />}
        <label className="field"><span>{t("device.gravity")}</span><input value={settings.gravity} readOnly /></label>
      </div>
      <div className="row tight">
        <NumberField id="cal-mass" label={t("device.mass")} value={mass} onChange={setMass} />
        <button id="cal-capture" className="primary" disabled={busy || mass === null} onClick={() => void capture()}>{busy ? t("device.capturing") : t("device.capture")}</button>
        <span className="muted small">{t("device.suggested")}:</span>
        {suggested.map((g) => <button key={g} className="link" onClick={() => setMass(g)}>{g} g</button>)}
      </div>

      {points.length > 0 && (
        <table>
          <thead><tr><th className="n">g</th><th className="n">{t("device.reference")} ({unit})</th><th className="n">{t("device.rawMean")}</th><th className="n">{t("device.noise")}</th><th className="n">{t("device.residual")} ({unit})</th></tr></thead>
          <tbody>
            {points.map((p, i) => (
              <tr key={p.mass_g}>
                <td className="n">{p.mass_g}</td><td className="n">{p.reference.toFixed(4)}</td><td className="n mono">{p.raw_mean.toFixed(0)}</td><td className="n mono">{p.raw_std.toFixed(0)}</td>
                <td className="n">{fit ? fit.residuals[i]?.toFixed(4) : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {fit && (
        <>
          <p className="fit">
            {t("device.scale")}: <strong>{fit.scale.toFixed(1)}</strong> · {t("device.offset")}: <strong>{fit.offset.toFixed(0)}</strong> · {t("device.r2")}: <strong>{fit.r_squared.toFixed(6)}</strong> · {t("device.maxError")}: <strong>{fit.max_error.toFixed(4)} {unit}</strong> · {t("device.nonlinearity")}: <strong>{fit.nonlinearity_pct.toFixed(3)} %</strong>
          </p>
          {fit.invert && <p className="notice">{t("device.inverted")}</p>}
        </>
      )}
      <div className="row">
        {done && <span className="small">{t("device.calApplied")}</span>}
        <span className="grow" />
        <button disabled={points.length === 0} onClick={() => setPoints([])}>{t("device.clear")}</button>
        <button id="cal-apply" className="primary" disabled={!fit} onClick={() => void apply()}>{t("device.applyCal")}</button>
      </div>
    </>
  );
}

export function DeviceView() {
  const { t } = useTranslation();
  const snapshot = useBench((s) => s.snapshot)!;
  const device = snapshot.device;
  if (!device) return <p className="muted empty">{t("device.needConnection")}</p>;
  const cells = snapshot.channels.filter((c) => c.sensor_type <= 3);

  return (
    <div className="stack">
      <Section title={t("device.info")}>
        <p>{t("device.firmware")} <strong>{device.firmware}</strong> · {t("device.protocol")} <strong>{device.protocol_version}</strong> · {t("device.uid")} <span className="mono">{device.uid.toString(16).toUpperCase().padStart(8, "0")}</span></p>
      </Section>
      <Section title={t("device.channels")}>
        <div className="table-scroll">
          <table>
            <thead><tr><th /><th>{t("device.enabled")}</th><th>{t("device.gain")}</th><th>{t("device.invert")}</th><th className="n">{t("device.offset")}</th><th className="n">{t("device.scale")}</th><th>{t("device.calibrated")}</th><th>{t("device.cellProfile")}</th></tr></thead>
            <tbody>{cells.map((c) => <ChannelRow key={c.id} channel={c} />)}</tbody>
          </table>
        </div>
      </Section>
      <Section title={t("device.calibration")} info={t("device.calibrationInfo")}><Calibration channels={snapshot.channels} /></Section>
      <Section title={t("device.board")} info={t("device.boardInfo")}><BoardSettings /></Section>
    </div>
  );
}
