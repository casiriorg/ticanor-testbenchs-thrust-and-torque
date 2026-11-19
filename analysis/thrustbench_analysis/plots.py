"""Standard figures. Each function returns the Matplotlib figure so it can be
saved or adjusted."""

from __future__ import annotations

import matplotlib.pyplot as plt

from .io import Recording
from .metrics import spectrum, throttle_curve

COLORS = {"thrust": "#1f9aa8", "torque": "#e07b00", "throttle": "#8e5a9e"}


def plot_overview(recording: Recording):
    """Every channel against time, with recorded events marked."""
    channels = [c for c in recording.channels.values() if len(c.t)]
    fig, axes = plt.subplots(len(channels), 1, sharex=True, figsize=(11, 2.4 * len(channels)), squeeze=False)
    for ax, channel in zip(axes[:, 0], channels):
        scale = 100.0 if channel.key == "throttle" else 1.0
        ax.plot(channel.t, channel.v * scale, color=COLORS.get(channel.key, "0.3"), linewidth=1)
        ax.set_ylabel(f"{channel.name} ({'%' if channel.key == 'throttle' else channel.unit})")
        ax.grid(alpha=0.3)
        for event in recording.events.itertuples():
            ax.axvline(event.t_s, color="crimson", linewidth=0.8, alpha=0.6)
    axes[-1, 0].set_xlabel("time (s)")
    fig.suptitle(recording.label)
    fig.tight_layout()
    return fig


def plot_curve(recording: Recording):
    """Thrust and torque against throttle for one recording."""
    return plot_comparison([recording])


def plot_comparison(recordings: list[Recording]):
    """Throttle curves of several recordings on shared axes."""
    fig, (ax_thrust, ax_torque, ax_ratio) = plt.subplots(1, 3, figsize=(15, 4.2))
    for recording in recordings:
        curve = throttle_curve(recording)
        if curve.empty:
            print(f"{recording.label}: no throttle plateaus found, skipped")
            continue
        ax_thrust.errorbar(curve["throttle_pct"], curve["thrust_gf"], yerr=curve["thrust_std_N"] / 9.80665 * 1000, marker="o", capsize=3, label=recording.label)
        if "torque_Nm" in curve:
            ax_torque.errorbar(curve["throttle_pct"], curve["torque_Nm"], yerr=curve["torque_std_Nm"], marker="o", capsize=3, label=recording.label)
            ax_ratio.plot(curve["throttle_pct"], curve["thrust_per_torque_1_m"], marker="o", label=recording.label)
    for ax, ylabel, title in (
        (ax_thrust, "thrust (gf)", "Thrust vs throttle"),
        (ax_torque, "torque (N·m)", "Torque vs throttle"),
        (ax_ratio, "thrust / torque (1/m)", "Thrust per unit torque"),
    ):
        ax.set_xlabel("throttle (%)")
        ax.set_ylabel(ylabel)
        ax.set_title(title)
        ax.grid(alpha=0.3)
    ax_thrust.legend(fontsize=8)
    fig.tight_layout()
    return fig


def plot_spectrum(recordings: list[Recording] | Recording, key: str = "thrust", t0: float | None = None, t1: float | None = None):
    """Amplitude spectrum of one channel, for one or several recordings."""
    if isinstance(recordings, Recording):
        recordings = [recordings]
    fig, ax = plt.subplots(figsize=(11, 4))
    for recording in recordings:
        s = spectrum(recording[key], t0, t1)
        ax.semilogy(s["freq_hz"], s["amplitude"], label=recording.label, linewidth=1)
    ax.set_xlabel("frequency (Hz)")
    ax.set_ylabel(f"{key} amplitude ({recordings[0][key].unit})")
    ax.grid(alpha=0.3, which="both")
    ax.legend(fontsize=8)
    fig.tight_layout()
    return fig
