import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { ArmDialog } from "../components/ArmDialog";
import { Dialog } from "../components/Dialog";
import { Section, SelectField } from "../components/Fields";
import type { Line } from "../components/LineChart";
import { LineChart } from "../components/LineChart";
import { SetupForm } from "../components/SetupForm";
import { SpectrumPanel } from "../components/SpectrumPanel";
import { api } from "../services/backend";
import type { AppSettings, ChannelDescriptor, Profile } from "../services/types";
import { SENSOR } from "../services/types";
import { useBench } from "../stores/benchStore";
import { liveBuffer } from "../stores/liveBuffer";
import { aliasFrequency, movingAverage, notch } from "../utils/dsp";
import { useTick } from "../utils/hooks";
import { channelDisplay, channelLimitsSI, formatDuration } from "../utils/units";

const COLORS: Record<number, string> = { [SENSOR.thrust]: "#3cb8c6", [SENSOR.torque]: "#f58518", [SENSOR.throttle]: "#b279a2" };

function channelLabel(channel: ChannelDescriptor, settings: AppSettings): string {
  const name = settings.channel_names[String(channel.id)] ?? channel.name;
  return name.charAt(0).toUpperCase() + name.slice(1);
}

/** Large numeric readout of one channel, with its tare button. */
function Readout({ channel }: { channel: ChannelDescriptor }) {
  const { t } = useTranslation();
  useTick(200);
  const settings = useBench((s) => s.settings)!;
  const profiles = useBench((s) => s.reference?.load_cell_profiles ?? []);
  const armed = useBench((s) => s.snapshot?.actuator === "armed");
  const run = useBench((s) => s.run);
  const [taring, setTaring] = useState(false);
  const display = channelDisplay(channel.sensor_type, settings.units);
  const value = liveBuffer.recent(channel.id);
  const isLoadCell = channel.sensor_type <= 3;
  const limits = channelLimitsSI(channel, settings, profiles);

  const tare = async () => {
    setTaring(true);
    await run(() => api.tare(channel.id));
    setTaring(false);
  };

  return (
    <div className="readout" style={{ borderColor: COLORS[channel.sensor_type] }}>
      <div className="readout-head">
        <span>{channelLabel(channel, settings)}</span>
        {isLoadCell && !channel.calibrated && <em title={t("device.nominal")}>{t("live.uncalibrated")}</em>}
      </div>
      <div className="readout-value">
        {value === null ? "—" : display.convert(value).toFixed(display.digits)}
        <small>{display.unit}</small>
      </div>
      {isLoadCell && (limits.safety !== null || limits.cellMax !== null) && (
        <p className="muted small limits">
          {limits.safety !== null && <span>{t("live.safetyLimit")}: <strong>{display.convert(limits.safety).toFixed(display.digits)} {display.unit}</strong></span>}
          {limits.cellMax !== null && <span>{t("live.cellMax")}: <strong>{display.convert(limits.cellMax).toFixed(display.digits)} {display.unit}</strong></span>}
        </p>
      )}
      {isLoadCell && (
        <button disabled={armed || taring} onClick={() => void tare()}>{taring ? t("live.taring") : t("live.tare")}</button>
      )}
    </div>
  );
}

/** Scrolling chart of one channel. Filters here affect the display only. */
function LiveChart({ channel }: { channel: ChannelDescriptor }) {
  const { t } = useTranslation();
  const tick = useTick(100);
  const settings = useBench((s) => s.settings)!;
  const profiles = useBench((s) => s.reference?.load_cell_profiles ?? []);
  const { window_s, smoothing_samples, mains_filter } = settings.display;
  const display = channelDisplay(channel.sensor_type, settings.units);
  const limits = channelLimitsSI(channel, settings, profiles);

  const data = useMemo(() => {
    const series = liveBuffer.window(channel.id, window_s);
    const now = liveBuffer.now();
    let values = series.v;
    if (channel.sensor_type <= 3) {
      if (mains_filter !== "off") values = notch(values, channel.rate_hz, aliasFrequency(Number(mains_filter), channel.rate_hz));
      values = movingAverage(values, smoothing_samples);
    }
    return { x: series.t.map((t) => t - now), y: values.map(display.convert) };
    // `tick` drives the refresh; the buffer itself is not React state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tick, channel.id, channel.rate_hz, window_s, smoothing_samples, mains_filter, settings.units]);

  const lines = useMemo(() => {
    const out: Line[] = [{ label: channel.name, color: COLORS[channel.sensor_type] ?? "#9aa", y: data.y }];
    const flat = (v: number) => data.x.map(() => display.convert(v));
    if (limits.safety !== null) out.push({ label: t("live.safetyLimit"), color: "#e45756", y: flat(limits.safety), dash: [4, 3], width: 1 });
    if (limits.cellMax !== null) out.push({ label: t("live.cellMax"), color: "#eeca3b", y: flat(limits.cellMax), dash: [2, 2], width: 1 });
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, channel, limits.safety, limits.cellMax, t]);
  return (
    <LineChart
      x={data.x}
      lines={lines}
      height={channel.sensor_type === SENSOR.throttle ? 110 : 170}
      yLabel={`${channelLabel(channel, settings)} (${display.unit})`}
      exportName={`${channel.name}_monitor`}
    />
  );
}

function ThrottlePanel() {
  const { t } = useTranslation();
  const snapshot = useBench((s) => s.snapshot)!;
  const settings = useBench((s) => s.settings)!;
  const run = useBench((s) => s.run);
  const [arming, setArming] = useState(false);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [profileName, setProfileName] = useState("");
  const [slider, setSlider] = useState(0);
  const lastSent = useRef(0);

  const armed = snapshot.actuator === "armed";
  const max = settings.safety.max_throttle;

  useEffect(() => {
    void api.builtinProfiles().then((list) => {
      setProfiles(list);
      setProfileName((current) => current || (list[0]?.name ?? ""));
    });
  }, [max]);
  // Whenever the output is not armed the slider goes back to zero, so arming
  // can never resume an old setpoint.
  useEffect(() => {
    if (!armed) setSlider(0);
  }, [armed]);
  useEffect(() => {
    if (snapshot.profile) setSlider(snapshot.setpoint);
  }, [snapshot.profile, snapshot.setpoint]);

  if (!snapshot.driver) return <p className="muted">{t("driver.notChosen")}</p>;
  if (snapshot.driver === "manual") return <p className="muted">{t("throttle.manualOnly")}</p>;

  const send = (value: number) => {
    const clamped = Math.max(0, Math.min(max, value));
    setSlider(clamped);
    // At most 20 commands a second while dragging; the last one always goes out.
    const now = performance.now();
    if (now - lastSent.current > 50 || clamped === 0) {
      lastSent.current = now;
      void run(() => api.setThrottle(clamped));
    }
  };
  const selected = profiles.find((p) => p.name === profileName);
  const canDrive = armed && !snapshot.profile;

  return (
    <>
      <div className="row">
        {armed ? (
          <button id="disarm" className="primary" onClick={() => void run(() => api.disarm())}>{t("safety.disarm")}</button>
        ) : (
          <button id="arm" className="danger" disabled={snapshot.actuator !== "disarmed"} onClick={() => setArming(true)}>{t("safety.arm")}…</button>
        )}
        <span className="grow" />
        <span className="muted">{t("throttle.applied")}: <strong>{((snapshot.status?.throttle ?? 0) * 100).toFixed(1)} %</strong> · {snapshot.status?.pulse_us ?? 0} µs</span>
      </div>

      <label className="slider" htmlFor="throttle-slider">
        <span>{t("throttle.setpoint")}: <strong>{(slider * 100).toFixed(1)} %</strong></span>
        <input
          id="throttle-slider"
          type="range"
          min={0}
          max={max}
          step={0.005}
          value={slider}
          disabled={!canDrive}
          onChange={(e) => send(e.target.valueAsNumber)}
          onPointerUp={() => canDrive && void run(() => api.setThrottle(slider))}
        />
      </label>
      <div className="row tight">
        <button disabled={!armed} onClick={() => { void api.abortProfile(); send(0); }}>{t("throttle.zero")}</button>
        {[-0.05, -0.01, 0.01, 0.05].map((step) => (
          <button key={step} disabled={!canDrive} onClick={() => send(slider + step)}>{step > 0 ? "+" : "−"}{Math.abs(step * 100)} %</button>
        ))}
      </div>
      {!armed && <p className="muted small">{t("throttle.armFirst")}</p>}

      <div className="row profile-row">
        <SelectField id="profile" label={t("throttle.profile")} value={profileName} options={profiles.map((p) => ({ value: p.name, label: p.name }))} onChange={(v) => setProfileName(v ?? "")} />
        {snapshot.profile ? (
          <button className="primary" onClick={() => void api.abortProfile()}>{t("throttle.abort")}</button>
        ) : (
          <button disabled={!armed || !selected} onClick={() => selected && void run(() => api.runProfile(selected))}>{t("throttle.run")}</button>
        )}
      </div>
      {snapshot.profile ? (
        <p className="small">{t("throttle.running", { name: snapshot.profile.name, elapsed: formatDuration(snapshot.profile.elapsed_s), total: formatDuration(snapshot.profile.total_s) })}</p>
      ) : (
        selected && <p className="muted small">{selected.description}</p>
      )}
      {arming && <ArmDialog onClose={() => setArming(false)} />}
    </>
  );
}

function RecordingPanel() {
  const { t } = useTranslation();
  const snapshot = useBench((s) => s.snapshot)!;
  const run = useBench((s) => s.run);
  const [asking, setAsking] = useState<"setup" | "stop" | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const recording = snapshot.recording;

  const stop = async () => {
    setAsking(null);
    const meta = await run(() => api.stopRecording());
    if (meta) setSaved(meta.id);
  };

  return (
    <>
      {recording ? (
        <>
          <div className="row">
            <span className="chip rec">● {formatDuration(recording.elapsed_s)}</span>
            <span className="muted">{t("rec.rows", { count: recording.rows })}</span>
            <span className="grow" />
            <button id="rec-stop" onClick={() => setAsking("stop")}>{t("rec.stop")}</button>
          </div>
          {recording.dropped_rows > 0 && <p className="notice">{t("rec.dropped", { count: recording.dropped_rows })}</p>}
          <p className="muted small path" title={recording.path}>{recording.path}</p>
        </>
      ) : (
        <>
          <button id="rec-start" className="primary" disabled={snapshot.connection.state !== "connected"} onClick={() => { setSaved(null); setAsking("setup"); }}>{t("rec.start")}…</button>
          {saved && <p className="small">{t("rec.savedTo", { id: saved })}</p>}
        </>
      )}
      {asking === "setup" && (
        <SetupForm onCancel={() => setAsking(null)} onSubmit={(setup) => { setAsking(null); void run(() => api.startRecording(setup)); }} />
      )}
      {asking === "stop" && (
        <Dialog
          title={t("rec.stopTitle")}
          onClose={() => setAsking(null)}
          actions={
            <>
              <button onClick={() => setAsking(null)}>{t("common.cancel")}</button>
              <button id="rec-stop-confirm" className="primary" onClick={() => void stop()}>{t("rec.stop")}</button>
            </>
          }
        >
          <p>{t("rec.stopConfirm")}</p>
          {snapshot.actuator === "armed" && <p className="notice">{t("rec.stopWhileArmed")}</p>}
        </Dialog>
      )}
    </>
  );
}

function DisplayPanel({ showSpectrum, onToggleSpectrum }: { showSpectrum: boolean; onToggleSpectrum: (v: boolean) => void }) {
  const { t } = useTranslation();
  const settings = useBench((s) => s.settings)!;
  const save = useBench((s) => s.saveSettings);
  const patch = (display: Partial<AppSettings["display"]>) => void save({ ...settings, display: { ...settings.display, ...display } });
  return (
    <div className="form-inline">
      <SelectField id="d-window" label={t("live.window")} value={settings.display.window_s} options={[10, 20, 60, 120].map((s) => ({ value: s, label: t("live.seconds", { count: s }) }))} onChange={(v) => v && patch({ window_s: v })} />
      <SelectField id="d-smooth" label={t("live.smoothing")} value={settings.display.smoothing_samples} options={[1, 4, 8, 16].map((n) => ({ value: n, label: n === 1 ? t("live.off") : t("live.samples", { count: n }) }))} onChange={(v) => v && patch({ smoothing_samples: v })} />
      <SelectField id="d-mains" label={t("live.mains")} value={settings.display.mains_filter} options={[{ value: "off" as const, label: t("live.off") }, { value: "50" as const, label: "50 Hz" }, { value: "60" as const, label: "60 Hz" }]} onChange={(v) => v && patch({ mains_filter: v })} />
      <label className="check"><input id="d-spectrum" type="checkbox" checked={showSpectrum} onChange={(e) => onToggleSpectrum(e.target.checked)} />{t("live.spectrum")}</label>
    </div>
  );
}

/** Live spectrum of the thrust channel over the displayed window. */
function LiveSpectrum({ channel }: { channel: ChannelDescriptor }) {
  const { t } = useTranslation();
  const tick = useTick(1000);
  const settings = useBench((s) => s.settings)!;
  const display = channelDisplay(channel.sensor_type, settings.units);
  const series = useMemo(() => liveBuffer.window(channel.id, settings.display.window_s), [tick, channel.id, settings.display.window_s]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <>
      <p className="muted small">{t("live.spectrumOf", { name: channel.name, seconds: settings.display.window_s })}</p>
      <SpectrumPanel series={series} color={COLORS[channel.sensor_type]} unit={display.unit} convert={display.convert} exportName={`${channel.name}_monitor`} />
    </>
  );
}

export function LiveView() {
  const { t } = useTranslation();
  const snapshot = useBench((s) => s.snapshot)!;
  const run = useBench((s) => s.run);
  const [showSpectrum, setShowSpectrum] = useState(false);
  // `tick` drives the refresh so a channel's readout/chart appears as soon as it starts
  // streaming (e.g. throttle feedback only flows once armed) and drops out again once it
  // stops, instead of leaving an empty card and an empty chart in place.
  const tick = useTick(250);
  const channels = snapshot.channels.filter((c) => c.enabled);
  const liveChannels = useMemo(
    () => channels.filter((c) => liveBuffer.recent(c.id) !== null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [tick, snapshot.channels],
  );
  const thrust = channels.find((c) => c.sensor_type === SENSOR.thrust);

  if (snapshot.connection.state !== "connected") {
    return (
      <div className="empty">
        <p>{t("conn.waiting")}</p>
        <button className="primary" onClick={() => void run(() => api.connectSimulator())}>{t("conn.useSimulator")}</button>
      </div>
    );
  }

  return (
    <div className="live">
      <div className="live-main">
        <div className="readouts">{liveChannels.map((c) => <Readout key={c.id} channel={c} />)}</div>
        {liveChannels.map((c) => <LiveChart key={c.id} channel={c} />)}
        {showSpectrum && thrust && <LiveSpectrum channel={thrust} />}
      </div>
      <aside className="live-side">
        <Section title={t("throttle.title")}><ThrottlePanel /></Section>
        <Section title={t("rec.title")}><RecordingPanel /></Section>
        <Section title={t("live.display")}><DisplayPanel showSpectrum={showSpectrum} onToggleSpectrum={setShowSpectrum} /></Section>
      </aside>
    </div>
  );
}
