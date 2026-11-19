// Application state shared by every view. Fed by backend events; views read
// it with selectors and act through `api`.

import { create } from "zustand";

import i18n from "../i18n";
import { api, errorMessage } from "../services/backend";
import type { Alarm, AppSettings, BenchSnapshot, Reference } from "../services/types";
import { liveBuffer } from "./liveBuffer";

/** How often the window tells the core it is alive (dead-man check). */
const UI_HEARTBEAT_MS = 300;

interface BenchStore {
  ready: boolean;
  snapshot: BenchSnapshot | null;
  settings: AppSettings | null;
  reference: Reference | null;
  /** Alarms not yet dismissed by the operator. */
  activeAlarms: Alarm[];
  /** Last failed action, shown as a banner until dismissed. */
  error: string | null;

  init(): Promise<void>;
  dismissAlarm(id: number): void;
  setError(message: string | null): void;
  /** Runs a backend action and surfaces its failure in the banner. */
  run<T>(action: () => Promise<T>): Promise<T | undefined>;
  saveSettings(next: AppSettings): Promise<boolean>;
}

export const useBench = create<BenchStore>((set, get) => ({
  ready: false,
  snapshot: null,
  settings: null,
  reference: null,
  activeAlarms: [],
  error: null,

  async init() {
    if (get().ready) return;
    const [settings, reference, snapshot] = await Promise.all([api.getSettings(), api.getReference(), api.getSnapshot()]);
    await i18n.changeLanguage(settings.language);
    set({ settings, reference, snapshot, ready: true });

    await api.onEvent((event) => {
      if (event.type === "samples") {
        liveBuffer.push(event.payload);
      } else if (event.type === "snapshot") {
        const previous = get().snapshot?.connection.state;
        // A new connection starts a new time base.
        if (event.payload.connection.state === "connected" && previous !== "connected") liveBuffer.clear();
        set({ snapshot: event.payload });
      } else {
        set((s) => ({ activeAlarms: [...s.activeAlarms.filter((a) => a.id !== event.payload.id), event.payload].slice(-6) }));
      }
    });

    window.setInterval(() => void api.uiAlive().catch(() => undefined), UI_HEARTBEAT_MS);

    await api.onCloseRequested(async () => {
      const s = get().snapshot;
      if (s?.actuator !== "armed" && !s?.recording) return true;
      // Closing is still safe (the core stops the motor and finishes the
      // file), but an accidental close should not end a test.
      return api.confirm(i18n.t("confirm.closeWhileActive"));
    });
  },

  dismissAlarm: (id) => set((s) => ({ activeAlarms: s.activeAlarms.filter((a) => a.id !== id) })),
  setError: (error) => set({ error }),

  async run(action) {
    try {
      const result = await action();
      set({ error: null });
      return result;
    } catch (e) {
      set({ error: errorMessage(e) });
      return undefined;
    }
  },

  async saveSettings(next) {
    try {
      await api.updateSettings(next);
      if (next.language !== get().settings?.language) await i18n.changeLanguage(next.language);
      set({ settings: next, error: null });
      return true;
    } catch (e) {
      set({ error: errorMessage(e) });
      return false;
    }
  },
}));
