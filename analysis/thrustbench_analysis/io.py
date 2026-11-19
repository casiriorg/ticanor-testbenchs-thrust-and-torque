"""Reading recordings written by the desktop application.

A recording is a folder with ``data.csv`` (sparse wide table: each row holds
one channel's sample), ``session.json`` (device, channels, calibration,
events) and ``setup.json`` (motor, propeller, ESC, battery).
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
import pandas as pd

DATA_FILE = "data.csv"
PARTIAL_FILE = "data.csv.partial"
SESSION_FILE = "session.json"
SETUP_FILE = "setup.json"


@dataclass
class Channel:
    """One measured quantity of a recording."""

    key: str            # stable name from the sensor catalogue: "thrust", "torque", "throttle"
    name: str
    unit: str
    t: np.ndarray       # seconds since the first sample
    v: np.ndarray       # calibrated values
    raw: np.ndarray | None = None  # ADC counts, when they were recorded
    descriptor: dict = field(default_factory=dict)  # calibration in force during the test

    @property
    def rate_hz(self) -> float:
        """Median sample rate."""
        if len(self.t) < 3:
            return float("nan")
        return float(1.0 / np.median(np.diff(self.t)))

    def window(self, t0: float, t1: float) -> "Channel":
        keep = (self.t >= t0) & (self.t <= t1)
        raw = None if self.raw is None else self.raw[keep]
        return Channel(self.key, self.name, self.unit, self.t[keep], self.v[keep], raw, self.descriptor)

    def recalibrated(self, scale: float, offset: float, invert: bool = False) -> "Channel":
        """Applies a different calibration to the raw counts.

        Lets a recording made with a nominal or wrong calibration be corrected
        afterwards, without repeating the test.
        """
        if self.raw is None:
            raise ValueError(f"{self.key}: raw counts were not recorded (enable 'Include raw counts')")
        values = (self.raw - offset) / scale
        return Channel(self.key, self.name, self.unit, self.t, -values if invert else values, self.raw, self.descriptor)


@dataclass
class Recording:
    path: Path
    meta: dict
    setup: dict
    channels: dict[str, Channel]
    table: pd.DataFrame

    def __getitem__(self, key: str) -> Channel:
        try:
            return self.channels[key]
        except KeyError:
            raise KeyError(f"no channel '{key}' in this recording; available: {', '.join(self.channels)}") from None

    def get(self, key: str) -> Channel | None:
        return self.channels.get(key)

    @property
    def label(self) -> str:
        """Short description for legends and titles."""
        motor = " ".join(p for p in (self.setup.get("motor", {}).get("brand"), self.setup.get("motor", {}).get("model")) if p)
        prop = self.setup.get("propeller", {})
        size = f'{prop["diameter_in"]:g}"' if prop.get("diameter_in") else ""
        propeller = " ".join(p for p in (prop.get("brand"), prop.get("model"), size) if p)
        return " · ".join(p for p in (motor, propeller) if p) or self.meta.get("id", self.path.name)

    @property
    def events(self) -> pd.DataFrame:
        return pd.DataFrame(self.meta.get("events", []), columns=["t_s", "kind", "message"])


def _read_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def load_recording(folder: str | Path) -> Recording:
    """Loads one recording folder."""
    folder = Path(folder)
    if not (folder / SESSION_FILE).exists():
        raise FileNotFoundError(f"{folder} is not a ThrustBench recording (no {SESSION_FILE})")
    meta = _read_json(folder / SESSION_FILE)
    setup = _read_json(folder / SETUP_FILE) if (folder / SETUP_FILE).exists() else {}
    data_path = folder / DATA_FILE if (folder / DATA_FILE).exists() else folder / PARTIAL_FILE
    # A recording cut short by a power loss may end in a torn line: skip it.
    table = pd.read_csv(data_path, on_bad_lines="skip")

    channels: dict[str, Channel] = {}
    for column in meta["channels"]:
        if column["column"] not in table:
            continue
        rows = table[column["column"]].notna()
        raw_column = column.get("raw_column")
        raw = table.loc[rows, raw_column].to_numpy(dtype=float) if raw_column and raw_column in table else None
        channels[column["key"]] = Channel(
            key=column["key"],
            name=column["descriptor"]["name"],
            unit=column["unit"],
            t=table.loc[rows, "t_s"].to_numpy(dtype=float),
            v=table.loc[rows, column["column"]].to_numpy(dtype=float),
            raw=raw,
            descriptor=column["descriptor"],
        )
    return Recording(folder, meta, setup, channels, table)


def list_recordings(root: str | Path) -> pd.DataFrame:
    """Table of the recordings under ``root``, newest first."""
    rows = []
    for folder in sorted(Path(root).iterdir()) if Path(root).exists() else []:
        if not (folder / SESSION_FILE).exists():
            continue
        meta = _read_json(folder / SESSION_FILE)
        setup = _read_json(folder / SETUP_FILE) if (folder / SETUP_FILE).exists() else {}
        motor = setup.get("motor", {})
        prop = setup.get("propeller", {})
        rows.append({
            "id": meta["id"],
            "started_at": meta["started_at"],
            "duration_s": meta.get("duration_s", 0.0),
            "state": meta.get("state", ""),
            "motor": " ".join(p for p in (motor.get("brand"), motor.get("model")) if p),
            "propeller": " ".join(p for p in (prop.get("brand"), prop.get("model")) if p),
            "notes": setup.get("test", {}).get("notes", ""),
            "path": str(folder),
        })
    columns = ["id", "started_at", "duration_s", "state", "motor", "propeller", "notes", "path"]
    return pd.DataFrame(rows, columns=columns).sort_values("started_at", ascending=False, ignore_index=True)
