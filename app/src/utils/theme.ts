// Light/dark theme. This is a display preference only, not device or firmware
// configuration, so it lives in localStorage (keyed per browser/install) rather than
// AppSettings, and applies instantly via a `data-theme` attribute on <html> (see the
// `:root[data-theme="light"]` overrides in styles.css).

import { useEffect, useState } from "react";

export type Theme = "dark" | "light";

const STORAGE_KEY = "tb-theme";
const listeners = new Set<(theme: Theme) => void>();
let current: Theme = "dark";

function isTheme(v: string | null): v is Theme {
  return v === "dark" || v === "light";
}

function readStored(): Theme {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return isTheme(v) ? v : "dark";
  } catch {
    return "dark";
  }
}

export function getTheme(): Theme {
  return current;
}

export function applyTheme(theme: Theme) {
  current = theme;
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Private browsing or a blocked store: the choice just won't survive a restart.
  }
  listeners.forEach((fn) => fn(theme));
}

/** Call once at startup, before the first paint, so there is no flash of the wrong theme. */
export function initTheme() {
  applyTheme(readStored());
}

/** Current theme, re-rendering the component whenever it changes (e.g. from Settings). */
export function useTheme(): Theme {
  const [theme, setTheme] = useState(current);
  useEffect(() => {
    listeners.add(setTheme);
    return () => void listeners.delete(setTheme);
  }, []);
  return theme;
}
