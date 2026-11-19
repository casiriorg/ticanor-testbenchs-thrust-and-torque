import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../services/backend";
import { useBench } from "../stores/benchStore";
import { formatDuration } from "../utils/units";
import { DriverDialog } from "./DriverDialog";

/**
 * Always-visible strip: connection, throttle mode, actuator state, recording
 * and the emergency stop. The stop button is never disabled and never asks
 * for confirmation.
 */
export function SafetyBar() {
  const { t } = useTranslation();
  const snapshot = useBench((s) => s.snapshot);
  const run = useBench((s) => s.run);
  const [changingDriver, setChangingDriver] = useState(false);

  // Space bar stops the motor from anywhere, unless the operator is typing.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT");
      if (e.code === "Space" && (!typing || e.ctrlKey)) {
        e.preventDefault();
        void api.estop();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!snapshot) return null;
  const c = snapshot.connection;
  const connected = c.state === "connected";
  const actuator = snapshot.actuator;
  const connectionText =
    c.state === "connected" ? t("conn.connected", { port: c.port })
    : c.state === "connecting" ? t("conn.connecting", { port: c.port })
    : c.state === "lost" ? `${t("conn.lost")}: ${c.reason}`
    : t(`conn.${c.state}`);

  return (
    <header className="safety-bar">
      <div className="bar-group">
        <span className={`chip conn-${c.state}`} title={connectionText}>{connectionText}</span>
        {snapshot.simulated && <span className="chip sim">{t("conn.simulated")}</span>}
        {!connected && c.state !== "connecting" && <button onClick={() => void run(() => api.connectSimulator())}>{t("conn.useSimulator")}</button>}
        {connected && actuator !== "armed" && !snapshot.recording && <button className="link" onClick={() => void run(() => api.disconnect())}>{t("conn.disconnect")}</button>}
        {connected && snapshot.link.frames_lost > 0 && <span className="chip warn">{t("conn.framesLost", { count: snapshot.link.frames_lost })}</span>}
      </div>

      <div className="bar-group">
        {connected && (
          <button className="chip button" disabled={actuator === "armed"} onClick={() => setChangingDriver(true)}>
            {snapshot.driver ? t(`driver.${snapshot.driver}.name`) : t("driver.notChosen")}
          </button>
        )}
        <span className={`state state-${actuator ?? "none"}`}>{t(`actuator.${actuator ?? "none"}`)}</span>
        {snapshot.recording && <span className="chip rec">● {t("rec.active")} {formatDuration(snapshot.recording.elapsed_s)}</span>}
      </div>

      <div className="bar-group">
        {actuator === "e_stop" && <button onClick={() => void run(() => api.clearEstop())}>{t("safety.clearEstop")}</button>}
        {actuator === "failsafe" && <button onClick={() => void run(() => api.disarm())} title={t("safety.failsafeHint")}>{t("safety.disarm")}</button>}
        <button id="estop" className="estop" title={t("safety.estopHint")} onClick={() => void api.estop()}>{t("safety.estop")}</button>
      </div>

      {changingDriver && <DriverDialog onClose={() => setChangingDriver(false)} />}
    </header>
  );
}
