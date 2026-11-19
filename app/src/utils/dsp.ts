// Display-only filters. Recorded data is never filtered; these only shape
// what the live chart shows. They mirror tb-core's analysis::spectrum.

/** Centred moving average of n samples. */
export function movingAverage(values: number[], n: number): number[] {
  if (n <= 1 || values.length === 0) return values;
  const half = Math.floor(n / 2);
  const prefix = new Float64Array(values.length + 1);
  for (let i = 0; i < values.length; i++) prefix[i + 1] = prefix[i] + values[i];
  return values.map((_, i) => {
    const a = Math.max(0, i - half);
    const b = Math.min(values.length, i + n - half);
    return (prefix[b] - prefix[a]) / (b - a);
  });
}

/**
 * Where mains hum lands after sampling. At 80 samples per second the limit is
 * 40 Hz, so 50 Hz hum appears at 30 Hz and 60 Hz hum at 20 Hz.
 */
export function aliasFrequency(signalHz: number, rateHz: number): number {
  const folded = signalHz % rateHz;
  return folded > rateHz / 2 ? rateHz - folded : folded;
}

/** Zero-phase second-order notch (forward and backward pass). */
export function notch(values: number[], rateHz: number, notchHz: number, q = 4): number[] {
  if (values.length < 3 || notchHz <= 0 || notchHz >= rateHz / 2) return values;
  const w0 = (2 * Math.PI * notchHz) / rateHz;
  const alpha = Math.sin(w0) / (2 * q);
  const a0 = 1 + alpha;
  const b0 = 1 / a0, b1 = (-2 * Math.cos(w0)) / a0, b2 = 1 / a0;
  const a1 = (-2 * Math.cos(w0)) / a0, a2 = (1 - alpha) / a0;
  const pass = (input: number[]) => {
    let x1 = input[0], x2 = input[0], y1 = input[0], y2 = input[0];
    return input.map((x) => {
      const y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
      x2 = x1; x1 = x; y2 = y1; y1 = y;
      return y;
    });
  };
  return pass(pass(values).reverse()).reverse();
}
