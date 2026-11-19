import { useTranslation } from "react-i18next";

import { useBench } from "../stores/benchStore";

/** Alarms raised by the core, newest last. They stay until dismissed. */
export function AlarmStack() {
  const { t } = useTranslation();
  const alarms = useBench((s) => s.activeAlarms);
  const dismiss = useBench((s) => s.dismissAlarm);
  const error = useBench((s) => s.error);
  const setError = useBench((s) => s.setError);

  return (
    <div className="alarms" aria-live="assertive">
      {error && (
        <div className="alarm warning">
          <span>{error}</span>
          <button className="link" onClick={() => setError(null)}>{t("common.dismiss")}</button>
        </div>
      )}
      {alarms.map((a) => (
        <div key={a.id} className={`alarm ${a.level}`}>
          {/* Translated headline, with the core's detailed English text as a tooltip. */}
          <span title={a.message}>{t(`alarm.${a.code}`, { defaultValue: a.message })}</span>
          <button className="link" onClick={() => dismiss(a.id)}>{t("common.dismiss")}</button>
        </div>
      ))}
    </div>
  );
}
