"""Numbers derived from a recording.

The definitions match the desktop application (tb-core ``analysis``), so a
curve computed here equals the one shown in the Recordings tab.
"""

from __future__ import annotations

import numpy as np
import pandas as pd
from scipy import signal

from .io import Channel, Recording

STANDARD_GRAVITY = 9.80665


def describe(recording: Recording, t0: float | None = None, t1: float | None = None) -> pd.DataFrame:
    """Mean, spread and sample rate of every channel over a time window."""
    rows = []
    for channel in recording.channels.values():
        c = channel.window(t0 if t0 is not None else -np.inf, t1 if t1 is not None else np.inf)
        if len(c.v) == 0:
            continue
        rows.append({
            "channel": c.key, "unit": c.unit, "samples": len(c.v), "mean": c.v.mean(),
            "std": c.v.std(ddof=1) if len(c.v) > 1 else 0.0, "min": c.v.min(), "max": c.v.max(), "rate_hz": c.rate_hz,
        })
    return pd.DataFrame(rows).set_index("channel")


def detect_plateaus(throttle: Channel, tolerance: float = 0.01, min_duration: float = 1.0) -> pd.DataFrame:
    """Stretches of constant, non-zero throttle (columns t0, t1, throttle)."""
    plateaus = []
    start = 0
    n = len(throttle.t)
    for i in range(1, n + 1):
        if i < n and abs(throttle.v[i] - throttle.v[start]) <= tolerance:
            continue
        t0, t1 = throttle.t[start], throttle.t[i - 1]
        level = float(throttle.v[start:i].mean())
        if t1 - t0 >= min_duration and level > tolerance:
            plateaus.append({"t0": t0, "t1": t1, "throttle": level})
        start = i
    return pd.DataFrame(plateaus, columns=["t0", "t1", "throttle"])


def throttle_curve(recording: Recording, settle_fraction: float = 0.4) -> pd.DataFrame:
    """Steady thrust and torque at each throttle step of a staircase test.

    The first ``settle_fraction`` of every plateau is discarded, because the
    motor is still accelerating there.
    """
    throttle = recording.get("throttle")
    thrust = recording.get("thrust")
    if throttle is None or thrust is None or len(throttle.t) == 0:
        raise ValueError("the recording needs 'throttle' and 'thrust' channels (was the ESC driven by the application?)")
    torque = recording.get("torque")
    rows = []
    for plateau in detect_plateaus(throttle).itertuples():
        t0 = plateau.t0 + (plateau.t1 - plateau.t0) * settle_fraction
        f = thrust.window(t0, plateau.t1).v
        if len(f) == 0:
            continue
        row = {
            "throttle_pct": plateau.throttle * 100.0,
            "thrust_N": f.mean(), "thrust_std_N": f.std(ddof=1) if len(f) > 1 else 0.0,
            "thrust_gf": f.mean() / STANDARD_GRAVITY * 1000.0, "samples": len(f),
        }
        if torque is not None:
            q = torque.window(t0, plateau.t1).v
            if len(q):
                row["torque_Nm"] = q.mean()
                row["torque_std_Nm"] = q.std(ddof=1) if len(q) > 1 else 0.0
                row["thrust_per_torque_1_m"] = f.mean() / q.mean() if abs(q.mean()) > 1e-6 else np.nan
        rows.append(row)
    return pd.DataFrame(rows)


def spectrum(channel: Channel, t0: float | None = None, t1: float | None = None, segment: int = 256) -> pd.DataFrame:
    """Amplitude spectrum (Welch, Hann window) of a channel.

    The HX711 samples at 80 Hz, so load-cell spectra end at 40 Hz. Faster
    vibration needs the accelerometer channels.
    """
    c = channel.window(t0 if t0 is not None else -np.inf, t1 if t1 is not None else np.inf)
    rate = c.rate_hz
    if not np.isfinite(rate) or len(c.v) < 32:
        raise ValueError("not enough samples for a spectrum")
    # The converter free-runs, so resample onto an even grid first.
    grid = np.arange(c.t[0], c.t[-1], 1.0 / rate)
    values = np.interp(grid, c.t, c.v)
    nperseg = min(segment, len(values))
    freq, power = signal.welch(values, fs=rate, window="hann", nperseg=nperseg, noverlap=nperseg // 2, detrend="constant", scaling="spectrum")
    # 'spectrum' scaling gives mean-square per bin; a sine of amplitude A reads A.
    return pd.DataFrame({"freq_hz": freq, "amplitude": np.sqrt(2.0 * power)})
