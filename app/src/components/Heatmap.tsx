// Spectrogram: time (x) vs frequency (y), amplitude as color. A canvas, like
// LineChart, so the same export-as-image button works on it.
//
// Sequential colour (magnitude: how much), one hue, light-to-dark per the
// data-viz method, anchored for THIS app's dark surface: low values recede
// toward the panel colour and high values are the brightest step (the
// anchor flips on a dark surface; see the sequential blue ramp in the
// dataviz skill's reference palette).

import { useEffect, useRef } from "react";

import { useTheme } from "../utils/theme";
import { ChartExportBar } from "./ChartExport";

export interface HeatmapProps {
  /** Column centres, seconds. */
  times: number[];
  /** Row centres, Hz, ascending. */
  freq: number[];
  /** `matrix[timeIndex][freqIndex]`, same units as `yLabel`. */
  matrix: number[][];
  height?: number;
  xLabel?: string;
  yLabel?: string;
  /** Unit of the colour scale (amplitude), shown next to the legend's "max". */
  unit?: string;
  exportName?: string;
}

// The 0.0 stop is the surface the low end recedes into (see the module comment); it is
// filled in per theme from `--panel` at draw time instead of being baked in here, so the
// anchor holds on both a dark and a light surface.
const STOPS: [number, string][] = [
  [0.1, "#0d366b"],
  [0.25, "#184f95"],
  [0.4, "#1c5cab"],
  [0.55, "#256abf"],
  [0.7, "#2a78d6"],
  [0.82, "#3987e5"],
  [0.92, "#6da7ec"],
  [1.0, "#cde2fb"],
];

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function heatColor(t: number, stopRgb: readonly (readonly [number, readonly [number, number, number]])[]): string {
  const v = Math.max(0, Math.min(1, t));
  for (let i = 1; i < stopRgb.length; i++) {
    const [t0, c0] = stopRgb[i - 1];
    const [t1, c1] = stopRgb[i];
    if (v <= t1) {
      const f = t1 > t0 ? (v - t0) / (t1 - t0) : 0;
      const r = Math.round(c0[0] + (c1[0] - c0[0]) * f);
      const g = Math.round(c0[1] + (c1[1] - c0[1]) * f);
      const b = Math.round(c0[2] + (c1[2] - c0[2]) * f);
      return `rgb(${r},${g},${b})`;
    }
  }
  const [, last] = stopRgb[stopRgb.length - 1];
  return `rgb(${last[0]},${last[1]},${last[2]})`;
}

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export function Heatmap({ times, freq, matrix, height = 220, xLabel, yLabel, unit, exportName }: HeatmapProps) {
  const host = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const theme = useTheme();

  useEffect(() => {
    const element = host.current;
    const canvas = canvasRef.current;
    if (!element || !canvas) return;
    const draw = () => {
      const width = element.clientWidth || 600;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = width * dpr;
      canvas.height = height * dpr;
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.scale(dpr, dpr);
      const muted = cssVar("--muted") || "#92a4ae";
      const panelHex = cssVar("--panel") || "#15212a";
      const stopRgb = [[0.0, hexToRgb(panelHex)] as const, ...STOPS.map(([t, hex]) => [t, hexToRgb(hex)] as const)];
      const pad = { left: 44, right: 46, top: 6, bottom: 20 };
      const plotW = width - pad.left - pad.right;
      const plotH = height - pad.top - pad.bottom;
      ctx.fillStyle = panelHex;
      ctx.fillRect(0, 0, width, height);

      if (times.length === 0 || freq.length === 0) {
        ctx.fillStyle = muted;
        ctx.font = "12px system-ui";
        ctx.fillText("—", pad.left, height / 2);
        return;
      }

      const max = Math.max(1e-12, ...matrix.flat());
      const colW = plotW / times.length;
      const rowH = plotH / freq.length;
      for (let ti = 0; ti < times.length; ti++) {
        for (let fi = 0; fi < freq.length; fi++) {
          ctx.fillStyle = heatColor((matrix[ti]?.[fi] ?? 0) / max, stopRgb);
          // Row 0 is the lowest frequency, drawn at the bottom.
          const y = pad.top + plotH - (fi + 1) * rowH;
          ctx.fillRect(pad.left + ti * colW, y, colW + 0.5, rowH + 0.5);
        }
      }

      // Axes.
      ctx.strokeStyle = muted;
      ctx.fillStyle = muted;
      ctx.font = "10.5px system-ui";
      ctx.textAlign = "center";
      const freqTicks = 4;
      for (let i = 0; i <= freqTicks; i++) {
        const value = (freq[freq.length - 1] * i) / freqTicks;
        const y = pad.top + plotH - (value / freq[freq.length - 1]) * plotH;
        ctx.textAlign = "right";
        ctx.fillText(value.toFixed(0), pad.left - 6, y + 3);
      }
      const timeTicks = Math.min(6, times.length);
      for (let i = 0; i < timeTicks; i++) {
        const idx = Math.round((i * (times.length - 1)) / Math.max(1, timeTicks - 1));
        const x = pad.left + (idx + 0.5) * colW;
        ctx.textAlign = "center";
        ctx.fillText(times[idx].toFixed(1), x, height - 5);
      }
      if (xLabel) {
        ctx.textAlign = "right";
        ctx.fillText(xLabel, width - pad.right - 2, height - 5);
      }
      if (yLabel) {
        ctx.save();
        ctx.translate(12, pad.top + plotH / 2);
        ctx.rotate(-Math.PI / 2);
        ctx.textAlign = "center";
        ctx.fillText(yLabel, 0, 0);
        ctx.restore();
      }

      // Colour legend: a thin vertical ramp with low/high labels.
      const legendX = width - pad.right + 10;
      const legendW = 10;
      for (let y = 0; y < plotH; y++) {
        ctx.fillStyle = heatColor(1 - y / plotH, stopRgb);
        ctx.fillRect(legendX, pad.top + y, legendW, 1);
      }
      ctx.strokeStyle = muted;
      ctx.strokeRect(legendX, pad.top, legendW, plotH);
      ctx.textAlign = "left";
      ctx.fillStyle = muted;
      ctx.font = "9.5px system-ui";
      ctx.fillText(unit || "max", legendX + legendW + 3, pad.top + 8);
      ctx.fillText("0", legendX + legendW + 3, pad.top + plotH);
    };
    draw();
    const observer = new ResizeObserver(draw);
    observer.observe(element);
    return () => observer.disconnect();
  }, [times, freq, matrix, height, xLabel, yLabel, unit, theme]);

  return (
    <div className="chart-wrap">
      <div ref={host} className="chart heatmap">
        <canvas ref={canvasRef} />
      </div>
      <ChartExportBar exportName={exportName} getElement={() => canvasRef.current} />
    </div>
  );
}
