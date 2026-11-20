import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { CurveChart, SERIES_COLORS, type Curve } from "../components/CurveChart";
import { Section, SelectField } from "../components/Fields";
import { LineChart } from "../components/LineChart";
import { SpectrumPanel } from "../components/SpectrumPanel";
import { api, errorMessage } from "../services/backend";
import type { ChannelSeries, CurvePoint, RecordingData, RecordingSummary, Stats } from "../services/types";
import { useBench } from "../stores/benchStore";
import { buildRecordingPdf } from "../utils/pdf";
import { channelDisplay, formatDuration } from "../utils/units";

const CHANNEL_COLORS: Record<string, string> = { thrust: "#3cb8c6", torque: "#f58518", throttle: "#b279a2" };

interface Loaded {
  data: RecordingData;
  curve: CurvePoint[];
}

async function loadWithCurve(id: string): Promise<Loaded> {
  const data = await api.loadRecording(id);
  const by = (key: string) => data.channels.find((c) => c.key === key)?.series;
  const throttle = by("throttle");
  const thrust = by("thrust");
  const curve = throttle && thrust && throttle.t.length > 0 ? await api.computeCurve(throttle, thrust, by("torque") ?? null) : [];
  return { data, curve };
}

function label(summary: { motor: string; propeller: string; id: string }): string {
  return [summary.motor, summary.propeller].filter(Boolean).join(" · ") || summary.id;
}

/** One channel of a recording, drawn up to the replay cursor. */
function ReplayChart({ channel, cursor, recordingId }: { channel: ChannelSeries; cursor: number; recordingId: string }) {
  const units = useBench((s) => s.settings)!.units;
  const display = channelDisplay(channel.sensor_type, units);
  const data = useMemo(() => {
    const end = channel.series.t.findIndex((t) => t > cursor);
    const n = end === -1 ? channel.series.t.length : end;
    return { x: channel.series.t.slice(0, n), y: channel.series.v.slice(0, n).map(display.convert) };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channel, cursor, units]);
  const lines = useMemo(() => [{ label: channel.name, color: CHANNEL_COLORS[channel.key] ?? "#9aa", y: data.y }], [data, channel]);
  return (
    <LineChart
      x={data.x}
      lines={lines}
      height={channel.key === "throttle" ? 100 : 150}
      yLabel={`${channel.name} (${display.unit})`}
      exportName={`${recordingId}_${channel.key}`}
    />
  );
}

function Detail({ loaded }: { loaded: Loaded }) {
  const { t } = useTranslation();
  const units = useBench((s) => s.settings)!.units;
  const { data, curve } = loaded;
  const duration = data.meta.duration_s;
  const [cursor, setCursor] = useState(duration);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [stats, setStats] = useState<Record<string, Stats | null>>({});
  const [spectrumKey, setSpectrumKey] = useState("thrust");
  const [exporting, setExporting] = useState<"pdf" | "csv" | null>(null);
  const run = useBench((s) => s.run);
  const setError = useBench((s) => s.setError);

  useEffect(() => {
    setCursor(duration);
    setPlaying(false);
    void Promise.all(data.channels.map(async (c) => [c.key, await api.computeStats(c.series)] as const)).then((entries) => setStats(Object.fromEntries(entries)));
  }, [data, duration]);

  // Replay: advance the cursor in real time, scaled by the chosen speed.
  useEffect(() => {
    if (!playing) return;
    const id = window.setInterval(() => {
      setCursor((c) => {
        const next = c + 0.1 * speed;
        if (next >= duration) {
          setPlaying(false);
          return duration;
        }
        return next;
      });
    }, 100);
    return () => window.clearInterval(id);
  }, [playing, speed, duration]);

  const spectrumChannel = data.channels.find((c) => c.key === spectrumKey);
  const spectrumDisplay = channelDisplay(spectrumChannel?.sensor_type ?? 0, units);
  const force = channelDisplay(1, units);
  const torque = channelDisplay(2, units);

  const exportPdf = async () => {
    setExporting("pdf");
    try {
      const bytes = buildRecordingPdf(data, curve, stats, units);
      await api.saveBytes(bytes, `${data.meta.id}.pdf`, "application/pdf", [{ name: "PDF", extensions: ["pdf"] }]);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setExporting(null);
    }
  };
  const exportCsv = async () => {
    setExporting("csv");
    await run(() => api.exportRecordingCsv(data.meta.id, `${data.meta.id}.csv`));
    setExporting(null);
  };

  return (
    <>
      <Section
        title={label({ motor: [data.setup.motor.brand, data.setup.motor.model].filter(Boolean).join(" "), propeller: data.setup.propeller.brand, id: data.meta.id })}
        aside={<span className="muted small">{new Date(data.meta.started_at).toLocaleString()} · {formatDuration(duration)} · {t(`state.${data.meta.state}`)}</span>}
      >
        <div className="row tight">
          <button id="replay-play" onClick={() => { if (cursor >= duration) setCursor(0); setPlaying((p) => !p); }}>{playing ? t("lib.pause") : t("lib.play")}</button>
          <button onClick={() => { setPlaying(false); setCursor(duration); }}>{t("lib.restart")}</button>
          <SelectField id="replay-speed" label={t("lib.speed")} value={speed} options={[1, 4, 16].map((s) => ({ value: s, label: `${s}×` }))} onChange={(v) => v && setSpeed(v)} />
          <input className="grow" type="range" min={0} max={duration} step={0.05} value={cursor} onChange={(e) => { setPlaying(false); setCursor(e.target.valueAsNumber); }} aria-label={t("lib.play")} />
          <span className="mono">{formatDuration(cursor)} / {formatDuration(duration)}</span>
        </div>
        <div className="row tight">
          <span className="grow" />
          <button id="export-pdf" disabled={exporting !== null} onClick={() => void exportPdf()}>{exporting === "pdf" ? t("lib.exporting") : t("lib.exportPdf")}</button>
          <button id="export-csv" disabled={exporting !== null} onClick={() => void exportCsv()}>{exporting === "csv" ? t("lib.exporting") : t("lib.exportCsv")}</button>
        </div>
        {data.channels.filter((c) => c.series.t.length > 0).map((c) => <ReplayChart key={c.key} channel={c} cursor={cursor} recordingId={data.meta.id} />)}
      </Section>

      <div className="columns">
        <Section title={t("lib.stats")}>
          <table>
            <thead><tr><th>{t("lib.channel")}</th><th className="n">{t("lib.mean")}</th><th className="n">{t("lib.std")}</th><th className="n">{t("lib.min")}</th><th className="n">{t("lib.max")}</th><th className="n">{t("lib.rate")}</th></tr></thead>
            <tbody>
              {data.channels.map((c) => {
                const s = stats[c.key];
                const d = channelDisplay(c.sensor_type, units);
                const f = (v: number) => d.convert(v).toFixed(d.digits);
                return (
                  <tr key={c.key}>
                    <td>{c.name} ({d.unit})</td>
                    {s ? <><td className="n">{f(s.mean)}</td><td className="n">{d.convert(s.std).toFixed(d.digits)}</td><td className="n">{f(s.min)}</td><td className="n">{f(s.max)}</td><td className="n">{s.rate_hz.toFixed(1)}</td></> : <td colSpan={5} className="muted">—</td>}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </Section>
        <Section title={t("lib.events")}>
          {data.meta.events.length === 0 ? <p className="muted">{t("lib.noEvents")}</p> : (
            <ul className="events">
              {data.meta.events.map((e, i) => <li key={i}><span className="mono">{formatDuration(e.t_s)}</span> <strong>{e.kind}</strong> {e.message}</li>)}
            </ul>
          )}
        </Section>
      </div>

      <div className="columns">
        <Section title={t("lib.curve")}>
          {curve.length === 0 ? <p className="muted">{t("lib.noCurve")}</p> : (
            <table>
              <thead><tr><th className="n">{t("lib.throttle")} (%)</th><th className="n">{t("lib.thrust")} ({force.unit})</th><th className="n">±</th><th className="n">{t("lib.torque")} ({torque.unit})</th><th className="n">{t("lib.thrustPerTorque")}</th></tr></thead>
              <tbody>
                {curve.map((p, i) => (
                  <tr key={i}>
                    <td className="n">{(p.throttle * 100).toFixed(0)}</td>
                    <td className="n">{force.convert(p.thrust_mean).toFixed(force.digits)}</td>
                    <td className="n">{force.convert(p.thrust_std).toFixed(force.digits)}</td>
                    <td className="n">{p.torque_mean === null ? "—" : torque.convert(p.torque_mean).toFixed(torque.digits)}</td>
                    <td className="n">{p.thrust_per_torque === null ? "—" : p.thrust_per_torque.toFixed(1)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Section>
        <Section title={t("live.spectrum")} aside={<SelectField id="spectrum-channel" label={t("lib.spectrumChannel")} value={spectrumKey} options={data.channels.filter((c) => c.key !== "throttle").map((c) => ({ value: c.key, label: c.name }))} onChange={(v) => v && setSpectrumKey(v)} />}>
          <SpectrumPanel
            series={spectrumChannel?.series ?? null}
            color={CHANNEL_COLORS[spectrumKey] ?? "#9aa"}
            unit={spectrumDisplay.unit}
            convert={spectrumDisplay.convert}
            exportName={`${data.meta.id}_${spectrumKey}`}
          />
        </Section>
      </div>
    </>
  );
}

/** Thrust and torque curves of several recordings on shared axes. */
function Comparison({ items }: { items: { summary: RecordingSummary; loaded: Loaded }[] }) {
  const { t } = useTranslation();
  const units = useBench((s) => s.settings)!.units;
  const force = channelDisplay(1, units);
  const torque = channelDisplay(2, units);
  const build = (pick: (p: CurvePoint) => number | null): Curve[] =>
    items.map(({ summary, loaded }, i) => ({
      label: label(summary),
      color: SERIES_COLORS[i % SERIES_COLORS.length],
      points: loaded.curve.map((p) => ({ x: p.throttle * 100, y: pick(p) })).filter((p): p is { x: number; y: number } => p.y !== null),
    }));

  return (
    <Section title={t("lib.compareTitle")}>
      {items.every((i) => i.loaded.curve.length === 0) ? <p className="muted">{t("lib.noCurve")}</p> : (
        <div className="columns three">
          <div><h4>{t("lib.thrustVsThrottle")}</h4><CurveChart curves={build((p) => force.convert(p.thrust_mean))} xLabel={`${t("lib.throttle")} (%)`} yLabel={force.unit} exportName="thrust_vs_throttle" /></div>
          <div><h4>{t("lib.torqueVsThrottle")}</h4><CurveChart curves={build((p) => (p.torque_mean === null ? null : torque.convert(p.torque_mean)))} xLabel={`${t("lib.throttle")} (%)`} yLabel={torque.unit} exportName="torque_vs_throttle" /></div>
          <div><h4>{t("lib.thrustPerTorque")}</h4><CurveChart curves={build((p) => p.thrust_per_torque)} xLabel={`${t("lib.throttle")} (%)`} yLabel="1/m" exportName="thrust_per_torque" /></div>
        </div>
      )}
    </Section>
  );
}

export function RecordingsView() {
  const { t } = useTranslation();
  const run = useBench((s) => s.run);
  const dataDir = useBench((s) => s.reference?.data_dir ?? "");
  const [list, setList] = useState<RecordingSummary[]>([]);
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [checked, setChecked] = useState<string[]>([]);
  const [cache, setCache] = useState<Record<string, Loaded>>({});
  const [savingCsv, setSavingCsv] = useState(false);

  const refresh = useCallback(async () => setList(await api.listRecordings()), []);
  useEffect(() => void refresh(), [refresh]);

  const saveCheckedCsv = async () => {
    setSavingCsv(true);
    await run(() => api.exportRecordingsCsv(checked));
    setSavingCsv(false);
  };

  // Load on demand whatever is selected or ticked, once.
  useEffect(() => {
    const wanted = [...new Set([...(selected ? [selected] : []), ...checked])].filter((id) => !cache[id]);
    wanted.forEach((id) => void run(() => loadWithCurve(id)).then((loaded) => loaded && setCache((c) => ({ ...c, [id]: loaded }))));
  }, [selected, checked, cache, run]);

  const visible = list.filter((r) => `${r.motor} ${r.propeller} ${r.notes} ${r.id}`.toLowerCase().includes(filter.toLowerCase()));
  const comparison = checked.map((id) => ({ summary: list.find((r) => r.id === id), loaded: cache[id] })).filter((x): x is { summary: RecordingSummary; loaded: Loaded } => !!x.summary && !!x.loaded);

  return (
    <div className="library">
      <aside className="library-list">
        <div className="row tight">
          <input id="lib-filter" type="search" className="grow" placeholder={t("lib.filter")} value={filter} onChange={(e) => setFilter(e.target.value)} />
          <button onClick={() => void refresh()}>{t("lib.refresh")}</button>
          <button id="lib-save-csv" disabled={checked.length === 0 || savingCsv} onClick={() => void saveCheckedCsv()}>
            {savingCsv ? t("lib.exporting") : t("lib.saveCsv")}
          </button>
        </div>
        <p className="muted small path" title={dataDir}>{dataDir}</p>
        {visible.length === 0 && <p className="muted">{t("lib.empty")}</p>}
        <ul>
          {visible.map((r) => (
            <li key={r.id} className={selected === r.id ? "selected" : ""}>
              <input type="checkbox" aria-label={label(r)} checked={checked.includes(r.id)} onChange={(e) => setChecked((c) => (e.target.checked ? [...c, r.id] : c.filter((id) => id !== r.id)))} />
              <button className="item" onClick={() => setSelected(r.id)}>
                <strong>{label(r)}</strong>
                <span>{new Date(r.started_at).toLocaleString()} · {formatDuration(r.duration_s)}{r.state !== "complete" ? ` · ${t(`state.${r.state}`)}` : ""}</span>
                {r.notes && <span className="muted">{r.notes}</span>}
              </button>
            </li>
          ))}
        </ul>
      </aside>
      <div className="library-detail">
        {comparison.length >= 2 && <Comparison items={comparison} />}
        {selected && cache[selected] ? <Detail loaded={cache[selected]} /> : comparison.length < 2 && <p className="muted empty">{t("lib.selectHint")}</p>}
      </div>
    </div>
  );
}
