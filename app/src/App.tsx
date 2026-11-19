import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { AlarmStack } from "./components/AlarmStack";
import { DriverDialog } from "./components/DriverDialog";
import { SafetyBar } from "./components/SafetyBar";
import { useBench } from "./stores/benchStore";
import { DeviceView } from "./views/DeviceView";
import { LiveView } from "./views/LiveView";
import { RecordingsView } from "./views/RecordingsView";
import { SettingsView } from "./views/SettingsView";

const VIEWS = { live: LiveView, recordings: RecordingsView, device: DeviceView, settings: SettingsView } as const;
type ViewName = keyof typeof VIEWS;

export function App() {
  const { t } = useTranslation();
  const ready = useBench((s) => s.ready);
  const init = useBench((s) => s.init);
  const snapshot = useBench((s) => s.snapshot);
  const [view, setView] = useState<ViewName>("live");

  useEffect(() => void init(), [init]);
  if (!ready || !snapshot) return <p className="muted empty">{t("common.loading")}</p>;

  const View = VIEWS[view];
  // Once connected, the throttle mode must be chosen before anything else.
  const mustChooseDriver = snapshot.connection.state === "connected" && snapshot.driver === null;

  return (
    <div className="app">
      <SafetyBar />
      <nav className="tabs">
        {(Object.keys(VIEWS) as ViewName[]).map((name) => (
          <button key={name} id={`tab-${name}`} className={view === name ? "active" : ""} onClick={() => setView(name)}>{t(`nav.${name}`)}</button>
        ))}
      </nav>
      <AlarmStack />
      <main><View /></main>
      {mustChooseDriver && <DriverDialog />}
    </div>
  );
}
