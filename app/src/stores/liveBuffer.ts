// Rolling buffer of live samples, kept outside React state on purpose:
// hundreds of samples a second must not trigger re-renders. Charts poll it.

import type { Sample, Series } from "../services/types";

const KEEP_SECONDS = 300;

const buffers = new Map<number, Series>();
let latest = 0;

export const liveBuffer = {
  push(samples: Sample[]) {
    for (const s of samples) {
      let b = buffers.get(s.channel);
      if (!b) buffers.set(s.channel, (b = { t: [], v: [] }));
      b.t.push(s.t);
      b.v.push(s.value);
      if (s.t > latest) latest = s.t;
    }
    // Trim in chunks so the arrays are not shifted on every batch.
    for (const b of buffers.values()) {
      if (b.t.length > 0 && latest - b.t[0] > KEEP_SECONDS + 30) {
        const cut = b.t.findIndex((t) => latest - t <= KEEP_SECONDS);
        b.t.splice(0, cut);
        b.v.splice(0, cut);
      }
    }
  },

  /** Most recent device time seen on any channel, in seconds. */
  now: () => latest,

  /** The last `seconds` of a channel. */
  window(channel: number, seconds: number): Series {
    const b = buffers.get(channel);
    if (!b) return { t: [], v: [] };
    const from = latest - seconds;
    let start = b.t.length;
    while (start > 0 && b.t[start - 1] >= from) start--;
    return { t: b.t.slice(start), v: b.v.slice(start) };
  },

  /** Latest value of a channel, averaged over a short time for a steady readout. */
  recent(channel: number, seconds = 0.25): number | null {
    const w = this.window(channel, seconds);
    return w.v.length ? w.v.reduce((a, b) => a + b, 0) / w.v.length : null;
  },

  clear() {
    buffers.clear();
    latest = 0;
  },
};
