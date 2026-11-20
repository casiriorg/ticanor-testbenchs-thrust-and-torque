# Export and spectrogram (2026-11-19 00:18)

Added export (PNG/JPEG per chart, PDF/CSV per recording) and a spectrogram view alongside the existing
line spectrum.

- **New dependencies**: `@tauri-apps/plugin-fs` + `tauri-plugin-fs` (writes a file to a path the operator
  picked), `jspdf` (builds the PDF report). Registered the fs plugin in `main.rs`; added
  `dialog:allow-save` and `fs:allow-write-file` to `capabilities/default.json`.
- **Chart export**: new `ChartExportBar` (`src/components/ChartExport.tsx`) shows "PNG"/"JPG" buttons on
  hover; works on uPlot's own canvas (`LineChart`) and on inline SVG (`CurveChart`, rasterized through an
  off-screen canvas) and on the new `Heatmap`'s canvas. Wired into every chart instance in Monitor and
  Recordings via a new optional `exportName` prop; a chart without one shows no export buttons.
- **Spectrogram**: new `Heatmap` component (time vs frequency, amplitude as colour) and a shared
  `SpectrumPanel` (icon toggle: line vs heatmap) that replaces the separate ad hoc spectrum code
  previously duplicated in the Monitor and Recordings views. The heatmap is built by slicing the series
  into windows and calling the existing `compute_spectrum` command once per slice; no backend change was
  needed. Colour ramp is the dataviz skill's validated sequential blue, anchored for this app's dark
  surface (low magnitude recedes toward the panel colour, high magnitude is the brightest step).
- **Recording export**: "Export PDF…" and "Save CSV as…" buttons in the Recordings detail view.
  - CSV: new `export_recording_csv(id, dest)` Tauri command copies the recording's actual `data.csv` file
    (exact, including raw columns and ISO timestamps); added `recording::data_file_path` to find it.
  - PDF: `src/utils/pdf.ts`'s `buildRecordingPdf` is a basic, text-only report (metadata, per-channel
    stats, events, throttle curve) built client-side with jsPDF; no Rust involvement, consistent with
    `tb-core` having no UI/export-format concerns.
- **Backend interface**: added `saveBytes` (generic "save as" via a native dialog on desktop, a browser
  download in the mock) and `exportRecordingCsv` to `Backend`, implemented in both `tauriBackend.ts` and
  `mockBackend.ts` (the mock reconstructs a simplified CSV from its in-memory recording for dev testing).
- **Verification**: full `cargo build`/`cargo test -p tb-core` passed; ran the Vite dev server under a
  scratch Playwright install and confirmed, with real downloaded files, that the PDF has a valid header,
  the CSV has real rows, and the PNG export has a valid PNG signature; also confirmed the heatmap renders
  with correct axes/legend and no console errors.
