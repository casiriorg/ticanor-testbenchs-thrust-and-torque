import { useEffect, useRef } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

import { useTheme } from "../utils/theme";
import { ChartExportBar } from "./ChartExport";

export interface Line {
  label: string;
  color: string;
  y: number[];
  /** uPlot dash pattern (e.g. [4, 3]); omit for a solid line. */
  dash?: number[];
  width?: number;
}

interface LineChartProps {
  x: number[];
  lines: Line[];
  height?: number;
  xLabel?: string;
  yLabel?: string;
  /** Shows "save as PNG/JPG" buttons with this as the suggested file name. */
  exportName?: string;
}

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Thin React wrapper around uPlot: one shared x axis, one or more lines. */
export function LineChart({ x, lines, height = 180, xLabel, yLabel, exportName }: LineChartProps) {
  const host = useRef<HTMLDivElement>(null);
  const plot = useRef<uPlot | null>(null);
  const theme = useTheme();
  // The plot is rebuilt only when the set of lines, the labels, or the theme's axis
  // colors (read from CSS vars at creation time) change.
  const signature = lines.map((l) => `${l.label}|${l.color}|${l.dash}|${l.width}`).join(",") + `|${xLabel}|${yLabel}|${height}|${theme}`;

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const axis = { stroke: cssVar("--muted"), grid: { stroke: cssVar("--grid"), width: 1 }, ticks: { stroke: cssVar("--grid") }, font: "11px system-ui" };
    const instance = new uPlot(
      {
        width: element.clientWidth || 600,
        height,
        scales: { x: { time: false } },
        legend: { show: lines.length > 1 },
        cursor: { drag: { x: true, y: false } },
        series: [{ label: xLabel ?? "x" }, ...lines.map((l) => ({ label: l.label, stroke: l.color, width: l.width ?? 1.5, dash: l.dash, points: { show: false } }))],
        axes: [{ ...axis, label: xLabel, labelSize: xLabel ? 18 : 0 }, { ...axis, label: yLabel, labelSize: yLabel ? 18 : 0, size: 56 }],
      },
      [x, ...lines.map((l) => l.y)],
      element,
    );
    plot.current = instance;
    const observer = new ResizeObserver(() => instance.setSize({ width: element.clientWidth, height }));
    observer.observe(element);
    return () => {
      observer.disconnect();
      instance.destroy();
      plot.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signature]);

  useEffect(() => {
    plot.current?.setData([x, ...lines.map((l) => l.y)]);
  }, [x, lines]);

  return (
    <div className="chart-wrap">
      <div ref={host} className="chart" />
      <ChartExportBar exportName={exportName} getElement={() => host.current?.querySelector("canvas") ?? null} />
    </div>
  );
}
