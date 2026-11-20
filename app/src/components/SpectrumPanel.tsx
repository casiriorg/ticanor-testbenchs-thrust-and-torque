// Shared spectrum display for the Monitor and Recordings views: a line
// (amplitude vs frequency) or a spectrogram heatmap (frequency vs time,
// amplitude as colour), picked with a pair of icon toggle buttons.

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../services/backend";
import type { Series, Spectrum } from "../services/types";
import { Heatmap } from "./Heatmap";
import type { Line } from "./LineChart";
import { LineChart } from "./LineChart";

type Mode = "line" | "heatmap";

interface Spectrogram {
  /** Slice centres, seconds since the series' first sample. */
  times: number[];
  freq: number[];
  matrix: number[][];
}

/** Splits `series` into `slices` consecutive windows and spectrum-analyses each. */
async function computeSpectrogram(series: Series, slices: number): Promise<Spectrogram | null> {
  const t0 = series.t[0];
  const t1 = series.t[series.t.length - 1];
  if (!(t1 > t0)) return null;
  const step = (t1 - t0) / slices;
  const windows = Array.from({ length: slices }, (_, i) => ({ a: t0 + i * step, b: t0 + (i + 1) * step }));
  const results = await Promise.all(
    windows.map(async ({ a, b }) => {
      const start = series.t.findIndex((t) => t >= a);
      if (start === -1) return null;
      let end = series.t.findIndex((t) => t > b);
      if (end === -1) end = series.t.length;
      if (end - start < 16) return null;
      try {
        const spectrum = await api.computeSpectrum({ t: series.t.slice(start, end), v: series.v.slice(start, end) });
        return { t: (a + b) / 2 - t0, spectrum };
      } catch {
        return null;
      }
    }),
  );
  const ok = results.filter((r): r is { t: number; spectrum: Spectrum } => r !== null);
  if (ok.length === 0) return null;
  return { times: ok.map((r) => r.t), freq: ok[0].spectrum.freq_hz, matrix: ok.map((r) => r.spectrum.amplitude) };
}

export function SpectrumPanel({
  series,
  color,
  unit,
  convert,
  exportName,
  heatmapSlices = 16,
}: {
  /** Full series to analyse; `null`/too short shows the "waiting" message. */
  series: Series | null;
  color: string;
  unit: string;
  convert: (v: number) => number;
  exportName: string;
  heatmapSlices?: number;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Mode>("line");
  const [spectrum, setSpectrum] = useState<Spectrum | null>(null);
  const [spectrogram, setSpectrogram] = useState<Spectrogram | null>(null);

  useEffect(() => {
    if (!series || series.v.length < 64) {
      setSpectrum(null);
      setSpectrogram(null);
      return;
    }
    let cancelled = false;
    if (mode === "line") {
      api.computeSpectrum(series, 256).then((s) => !cancelled && setSpectrum(s)).catch(() => !cancelled && setSpectrum(null));
    } else {
      computeSpectrogram(series, heatmapSlices).then((s) => !cancelled && setSpectrogram(s)).catch(() => !cancelled && setSpectrogram(null));
    }
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [series, mode, heatmapSlices]);

  const lines = useMemo<Line[]>(() => [{ label: exportName, color, y: (spectrum?.amplitude ?? []).map(convert) }], [spectrum, color, convert, exportName]);
  const heatMatrix = useMemo(() => (spectrogram?.matrix ?? []).map((row) => row.map(convert)), [spectrogram, convert]);

  return (
    <div>
      <div className="row tight spectrum-modes" role="group" aria-label={t("live.spectrumView")}>
        <button type="button" className={mode === "line" ? "active" : ""} aria-pressed={mode === "line"} title={t("live.spectrumLine")} onClick={() => setMode("line")}>
          <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
            <polyline points="1,11 5,5 8,8 13,2" fill="none" stroke="currentColor" strokeWidth="1.6" />
          </svg>
        </button>
        <button type="button" className={mode === "heatmap" ? "active" : ""} aria-pressed={mode === "heatmap"} title={t("live.spectrumHeatmap")} onClick={() => setMode("heatmap")}>
          <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
            <rect x="0.5" y="0.5" width="4" height="4" fill="currentColor" opacity="0.3" />
            <rect x="5" y="0.5" width="4" height="4" fill="currentColor" opacity="0.65" />
            <rect x="9.5" y="0.5" width="4" height="4" fill="currentColor" />
            <rect x="0.5" y="5" width="4" height="4" fill="currentColor" opacity="0.65" />
            <rect x="5" y="5" width="4" height="4" fill="currentColor" />
            <rect x="9.5" y="5" width="4" height="4" fill="currentColor" opacity="0.3" />
            <rect x="0.5" y="9.5" width="4" height="4" fill="currentColor" />
            <rect x="5" y="9.5" width="4" height="4" fill="currentColor" opacity="0.3" />
            <rect x="9.5" y="9.5" width="4" height="4" fill="currentColor" opacity="0.65" />
          </svg>
        </button>
      </div>
      {mode === "line" ? (
        spectrum ? (
          <LineChart x={spectrum.freq_hz} lines={lines} height={190} xLabel="Hz" yLabel={unit} exportName={`${exportName}_spectrum`} />
        ) : (
          <p className="muted">{t("live.noData")}</p>
        )
      ) : spectrogram ? (
        <Heatmap times={spectrogram.times} freq={spectrogram.freq} matrix={heatMatrix} height={190} xLabel="s" yLabel="Hz" unit={unit} exportName={`${exportName}_spectrogram`} />
      ) : (
        <p className="muted">{t("live.noData")}</p>
      )}
    </div>
  );
}
