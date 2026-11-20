# Light theme (2026-11-20 00:41)

Added a Dark/Light theme selector to Settings -> General, and composed a light theme to go with
it. Previously every color in `styles.css` was a literal value tuned for the dark instrument
panel, with no way to switch.

- **Theme preference**: new `src/utils/theme.ts` holds the current theme and applies it as a
  `data-theme` attribute on `<html>`. It is a pure display preference, not device or firmware
  configuration, so (unlike `language`, which round-trips through `AppSettings` and the Rust
  backend) it lives in `localStorage` and applies instantly with no "Save" step. `initTheme()`
  runs once at startup in `main.tsx`, before the first paint, so there is no flash of the wrong
  theme on launch.
- **Light palette**: added a `:root[data-theme="light"]` block in `styles.css` re-tuning the
  same tokens (`--bg`, `--panel`, `--panel-2`, `--fg`, `--muted`, `--line`, `--grid`, `--accent`,
  `--on-accent`, `--warn`, `--danger`, `--ok`) for a white/light surface; almost every rule in
  the stylesheet already referenced these variables, so it repaints correctly with no further
  changes. The status colors (`--warn`/`--danger`/`--ok`) are a shade darker than their dark-
  theme counterparts so plain-text uses (e.g. `.chip.conn-lost`) stay legible on a light
  background. Two colors that were baked in as literals instead of variables (`.alarm.critical`'s
  background, `.chip.rec`'s text) are now `--danger-bg`/`--danger-weak`, each with its own
  dark/light value, since a literal dark-maroon or light-pink only works on a dark panel.
- **Canvas/WebGL charts**: `LineChart` (uPlot) and `Heatmap` (plain canvas) read axis/grid colors
  from CSS variables once, at chart-creation time, so switching theme while one was already
  mounted would not recolor it. Both now take the current theme from a `useTheme()` hook and
  include it in their rebuild/redraw dependencies, so they recolor immediately. The spectrogram
  heatmap's color ramp anchors its low end to `--panel` at draw time (previously a hardcoded
  dark hex) so "low values recede into the surface" still holds on a light panel.
- **Settings UI**: added a "Theme" dropdown next to "Language" in Settings -> General.
- **Verification**: ran the Vite dev server against `mockBackend` under a scratch Playwright
  install; screenshotted Settings and the Monitor view (readouts, line charts, recording chip,
  safety bar) in both themes and confirmed text contrast and chart colors hold up on the light
  background. `tsc --noEmit` passes.
