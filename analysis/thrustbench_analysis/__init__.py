"""Offline analysis of ThrustBench recordings.

Typical use, from a notebook::

    import thrustbench_analysis as tba

    rec = tba.pick_recording()        # opens a folder dialog
    tba.plot_overview(rec)
    tba.throttle_curve(rec)

Nothing here needs editing between tests: recordings are chosen through
dialogs, and names, units and calibration come from each recording's own
``session.json``.
"""

from .dialogs import default_data_dir, pick_recording, pick_recordings
from .io import Recording, list_recordings, load_recording
from .metrics import describe, detect_plateaus, spectrum, throttle_curve
from .plots import plot_comparison, plot_curve, plot_overview, plot_spectrum

__all__ = [
    "Recording", "default_data_dir", "describe", "detect_plateaus", "list_recordings", "load_recording",
    "pick_recording", "pick_recordings", "plot_comparison", "plot_curve", "plot_overview", "plot_spectrum",
    "spectrum", "throttle_curve",
]
