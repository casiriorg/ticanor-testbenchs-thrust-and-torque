import { useTranslation } from "react-i18next";

import { api } from "../services/backend";
import type { DriverKind } from "../services/types";
import { useBench } from "../stores/benchStore";
import { Dialog } from "./Dialog";

/**
 * Asked once per connection: how is the ESC controlled? Until it is answered
 * nothing can be armed. If a flight controller was detected it is offered
 * first, but the choice is always the operator's.
 */
export function DriverDialog({ onClose }: { onClose?: () => void }) {
  const { t } = useTranslation();
  const options = useBench((s) => s.snapshot?.driver_options ?? []);
  const current = useBench((s) => s.snapshot?.driver ?? null);
  const run = useBench((s) => s.run);
  const flightController = options.find((o) => o.kind === "mavlink" && o.available);
  const ordered = flightController ? [flightController, ...options.filter((o) => o !== flightController)] : options;

  const choose = async (kind: DriverKind) => {
    await run(() => api.selectDriver(kind));
    onClose?.();
  };

  return (
    <Dialog title={t("driver.title")} onClose={onClose} actions={onClose && <button onClick={onClose}>{t("common.cancel")}</button>}>
      <p className="muted">{t("driver.question")}</p>
      {flightController && <p className="notice">{t("driver.fcDetected", { port: flightController.detail })}</p>}
      <div className="options">
        {ordered.map((o) => (
          <button key={o.kind} className={`option${current === o.kind ? " selected" : ""}`} disabled={!o.available} onClick={() => void choose(o.kind)}>
            <strong>{t(`driver.${o.kind}.name`)}</strong>
            <span>{t(`driver.${o.kind}.desc`)}</span>
            {!o.available && <em>{t("driver.unavailable")}</em>}
            {o.experimental && o.available && <em className="warn">{t("driver.experimental")}</em>}
          </button>
        ))}
      </div>
    </Dialog>
  );
}
