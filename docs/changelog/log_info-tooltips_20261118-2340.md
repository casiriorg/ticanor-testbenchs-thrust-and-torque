# Info tooltips (2026-11-18 23:40)

Replaced long, implementation-specific section/field titles with short labels plus an on-demand info button,
and added a writing-style rule and this changelog process itself.

- **`InfoTip` component** (`src/components/InfoTip.tsx`): a small "i" button that reveals an explanation on
  hover or keyboard focus (CSS `:hover`/`:focus-within`, no JS toggle state, so there is no focus/click race
  and Tab users get it for free). `role="tooltip"` + `aria-describedby` for accessibility.
- **`Section` and the field components** (`src/components/Fields.tsx`) gained an optional `info` prop that
  renders an `InfoTip` next to the title/label.
- **Shortened titles**, moving the dropped detail into the new tooltip text (en + es):
  - Device: "Output and safety (stored on the board)" became "Output & safety".
  - Settings: "Safety limits (application)" became "Safety limits".
  - Field labels with parenthetical trivia ("HX711 RATE pin (255 = not wired)", "Rise limit (%/s, 0 = none)",
    "Cut above thrust (N)", "Disarm if the window freezes for (ms)", etc.) were shortened and the detail moved
    to each field's tooltip.
  - The three standing explanation paragraphs (`ratePinHint`, `calHint`, `safetyNote`) were removed and their
    text reused as the matching section's tooltip instead of permanent on-page text.
- **Verification**: ran the Vite dev server under a scratch Playwright install and screenshotted the Monitor,
  Device and Settings views; confirmed the shortened titles, hover tooltips and no console errors.
- **Process**: added a "Writing style" rule to `CLAUDE.md` (no em dash/double dash in comments, docs or
  changelog text) and fixed the handful of em dashes already written in `CLAUDE.md` and the previous
  changelog entry.
