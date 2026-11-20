# Grid blowout fix (2026-11-19 22:33)

Fixed five bugs reported against the previous iteration's spectrum/export work, all tracing back to one
CSS layout bug plus a UX redesign of chart export.

- **Root cause (CSS grid blowout)**: `.panel`, `.stack`, `.live-main` and `.library-detail` were all
  `display: grid` with no `grid-template-columns`, so their single implicit column sized itself to its
  content's max-content width instead of the available space. A chart whose content (uPlot's legend
  table, once it started showing "Safety limit"/"Cell max" rows) was even slightly wider than its
  container made the column grow to fit it; a resize-observing chart then read that larger size back as
  its new target width and grew again, every tick, without bound. Fixed by giving all four an explicit
  `grid-template-columns: minmax(0, 1fr)`, which can shrink but never grows past available space. This
  was both the Monitor view's endlessly widening charts and the Recordings view's spectrum section
  pushing its channel dropdown off-screen (same bug, one continuous/animated because it re-measures every
  tick, one a one-time static overflow because it doesn't).
- **Chart export redesign**: replaced the hover-revealed inline "PNG"/"JPG" buttons with one
  always-visible "Export" button per chart that opens a PNG/JPEG/BMP menu (`ChartExport.tsx`). BMP is
  hand-encoded (24-bit, bottom-up rows) since canvas/image export doesn't support it natively. The save
  dialog now defaults to the Downloads folder (`@tauri-apps/api/path`'s `downloadDir`/`join`), and a
  failure now surfaces in the error banner instead of being silently swallowed.
- **"dialog.save not allowed"**: verified this is a stale-build symptom, not a misconfiguration.
  `dialog:allow-save` and `fs:allow-write-file` were already correctly present in
  `capabilities/default.json`, and `src-tauri/gen/schemas/capabilities.json` (Tauri's compiled snapshot)
  already includes both; Tauri bakes ACL permissions in at build time, so a `tauri dev` session left
  running from before they were added would not pick them up without a restart.
- **Bulk CSV export**: the Recordings list's checkboxes previously only fed the (>=2) comparison view, so
  checking a single box looked like it did nothing. Added a "Save CSV" button next to "Refresh", enabled
  once >=1 recordings are checked; it asks once for a destination folder and writes each checked
  recording's CSV into it (`exportRecordingsCsv` on `Backend`).
- **Verification**: re-ran the Vite dev server under a scratch Playwright install; confirmed chart widths
  stay constant over 20+ seconds (previously ran away to 6500px+), confirmed the spectrum section's
  dropdown sits on the title row again, confirmed the new BMP download has a valid "BM" header, and
  confirmed the bulk CSV button is disabled/enabled correctly and produces one file per checked recording.
  `cargo build` passes clean.
