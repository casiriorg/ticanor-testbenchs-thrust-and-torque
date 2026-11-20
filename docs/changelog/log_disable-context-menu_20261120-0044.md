# Disable context menu (2026-11-20 00:44)

Right-clicking anywhere in the app opened the webview's native browser context menu ("Back",
"Reload", "Save page as...", "Inspect"), which looks out of place in a desktop instrument app
and exposes browser chrome nobody should need.

- **Fix**: new `src/utils/contextMenu.ts`, wired up in `main.tsx` next to `initTheme()`. A single
  `contextmenu` listener on `window` calls `preventDefault()` everywhere except on text inputs,
  textareas and contenteditable elements, so native cut/copy/paste in form fields (recordings
  folder path, motor/propeller notes, etc.) still works.
- **No replacement menu**: nothing in the app currently needs a right-click menu of its own
  (chart export already has its own always-visible "Export" button, see the grid-blowout-fix
  changelog entry), so this only suppresses the browser's default; it is not hiding a feature
  that should exist as a custom menu instead.
- **Verification**: ran the Vite dev server and dispatched a synthetic `contextmenu` event on
  the page background and on a text input; confirmed the background event's default is
  prevented and the input's is not. `tsc --noEmit` passes.
