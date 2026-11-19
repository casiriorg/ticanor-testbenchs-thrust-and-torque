# Analysis notebooks

```
cd analysis
python -m venv .venv && .venv/Scripts/activate      # Linux/macOS: source .venv/bin/activate
pip install -r requirements.txt
jupyter lab notebooks
```

- `01_single_recording.ipynb`: overview, statistics, throttle curve and spectrum of one recording.
- `02_compare_recordings.ipynb`: throttle curves and spectra of several recordings.

Recordings are chosen in dialogs, so the notebooks need no editing between tests. The dialogs start in
`Documents/ThrustBench`; set the `THRUSTBENCH_DATA` environment variable if the application saves elsewhere.

No hardware yet? Create a demo recording with the simulator:

```
cd app && cargo run -p tb-core --example demo_recording -- <folder> "My motor"
```

The package (`thrustbench_analysis`) can also be used from scripts: `load_recording`, `throttle_curve`, `spectrum`,
`describe`, `plot_*`.
