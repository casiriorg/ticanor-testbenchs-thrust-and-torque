// Small "i" button that reveals an explanation on hover or keyboard focus.
// Used next to short section/field labels instead of packing detail into
// the label itself (e.g. "Output & safety" + a tip, not "Output and safety
// (stored on the board)"). Shown/hidden with CSS (:hover/:focus-within), not
// JS state, so there is no focus-vs-click race and keyboard users (Tab) get
// it for free.

import { useId } from "react";
import { useTranslation } from "react-i18next";

export function InfoTip({ text }: { text: string }) {
  const { t } = useTranslation();
  const id = useId();
  return (
    <span className="info-tip">
      <button type="button" className="info-tip-trigger" aria-label={t("common.moreInfo")} aria-describedby={id}>
        i
      </button>
      <span role="tooltip" id={id} className="info-tip-bubble">
        {text}
      </span>
    </span>
  );
}
