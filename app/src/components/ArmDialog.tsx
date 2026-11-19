import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../services/backend";
import { useBench } from "../stores/benchStore";
import { Dialog } from "./Dialog";

/**
 * Arming always goes through this dialog. The checkbox starts unticked every
 * time: the confirmation is about this moment, not a remembered preference.
 */
export function ArmDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [confirmed, setConfirmed] = useState(false);
  const snapshot = useBench((s) => s.snapshot);
  const settings = useBench((s) => s.settings);
  const run = useBench((s) => s.run);
  if (!snapshot || !settings) return null;

  const tareMinutes = snapshot.tare_age_s === null ? null : snapshot.tare_age_s / 60;
  const uncalibrated = snapshot.channels.some((c) => c.sensor_type <= 3 && c.enabled && !c.calibrated);

  const arm = async () => {
    await run(() => api.arm(confirmed));
    onClose();
  };

  return (
    <Dialog
      title={t("safety.armTitle")}
      onClose={onClose}
      actions={
        <>
          <button onClick={onClose}>{t("common.cancel")}</button>
          <button className="danger" disabled={!confirmed} onClick={() => void arm()}>{t("safety.arm")}</button>
        </>
      }
    >
      <p>{t("safety.armLimits", { max: Math.round(settings.safety.max_throttle * 100), rise: Math.round(settings.safety.max_rise_per_s * 100) })}</p>
      {tareMinutes === null && <p className="notice">{t("safety.armNoTare")}</p>}
      {tareMinutes !== null && tareMinutes > settings.tare_max_age_min && <p className="notice">{t("safety.armTareOld", { minutes: Math.round(tareMinutes) })}</p>}
      {uncalibrated && <p className="notice">{t("safety.armNotCalibrated")}</p>}
      <label className="check big">
        <input id="arm-confirm" type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} />
        {t("safety.armConfirm")}
      </label>
    </Dialog>
  );
}
