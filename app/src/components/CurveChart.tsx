// Small SVG chart for curves with few points and independent x values
// (one polyline per recording), which uPlot's shared-x model does not fit.

export interface Curve {
  label: string;
  color: string;
  points: { x: number; y: number }[];
}

interface CurveChartProps {
  curves: Curve[];
  xLabel: string;
  yLabel: string;
  height?: number;
}

const W = 520;
const PAD = { left: 56, right: 14, top: 12, bottom: 34 };

function niceTicks(max: number, count = 5): number[] {
  if (!(max > 0)) return [0, 1];
  const step = Math.pow(10, Math.floor(Math.log10(max / count)));
  const nice = [1, 2, 5, 10].map((m) => m * step).find((s) => max / s <= count) ?? step * 10;
  const ticks: number[] = [];
  // The last tick is always at or above the data, so nothing is clipped.
  for (let v = 0; v < max + nice; v += nice) ticks.push(Number(v.toPrecision(10)));
  return ticks;
}

export function CurveChart({ curves, xLabel, yLabel, height = 240 }: CurveChartProps) {
  const all = curves.flatMap((c) => c.points).filter((p) => Number.isFinite(p.x) && Number.isFinite(p.y));
  const xTicks = niceTicks(Math.max(...all.map((p) => p.x), 0));
  const yTicks = niceTicks(Math.max(...all.map((p) => p.y), 0));
  const xMax = xTicks[xTicks.length - 1];
  const yMax = yTicks[yTicks.length - 1];
  const sx = (x: number) => PAD.left + (x / xMax) * (W - PAD.left - PAD.right);
  const sy = (y: number) => height - PAD.bottom - (y / yMax) * (height - PAD.top - PAD.bottom);

  return (
    <div className="curve-chart">
      <svg viewBox={`0 0 ${W} ${height}`} role="img" aria-label={`${yLabel} / ${xLabel}`}>
        {yTicks.map((t) => (
          <g key={`y${t}`}>
            <line x1={PAD.left} x2={W - PAD.right} y1={sy(t)} y2={sy(t)} className="grid-line" />
            <text x={PAD.left - 6} y={sy(t) + 4} textAnchor="end" className="tick">{t}</text>
          </g>
        ))}
        {xTicks.map((t) => (
          <text key={`x${t}`} x={sx(t)} y={height - PAD.bottom + 16} textAnchor="middle" className="tick">{t}</text>
        ))}
        <text x={(W + PAD.left) / 2} y={height - 4} textAnchor="middle" className="axis-label">{xLabel}</text>
        <text x={12} y={height / 2} textAnchor="middle" className="axis-label" transform={`rotate(-90 12 ${height / 2})`}>{yLabel}</text>
        {curves.map((c) => (
          <g key={c.label}>
            <polyline fill="none" stroke={c.color} strokeWidth={2} points={c.points.map((p) => `${sx(p.x)},${sy(p.y)}`).join(" ")} />
            {c.points.map((p, i) => <circle key={i} cx={sx(p.x)} cy={sy(p.y)} r={3.5} fill={c.color} />)}
          </g>
        ))}
      </svg>
      <ul className="legend">
        {curves.map((c) => (
          <li key={c.label}><span className="swatch" style={{ background: c.color }} />{c.label}</li>
        ))}
      </ul>
    </div>
  );
}

/** Colours for overlaid recordings; distinct in hue and lightness. */
export const SERIES_COLORS = ["#3cb8c6", "#f58518", "#b279a2", "#54a24b", "#e45756", "#eeca3b"];
