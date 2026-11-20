import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { NumberField, Section, SelectField } from "../components/Fields";
import { languages } from "../i18n";
import { api, isDesktop } from "../services/backend";
import type { AppSettings } from "../services/types";
import { useBench } from "../stores/benchStore";
import { applyTheme, useTheme } from "../utils/theme";

export function SettingsView() {
  const { t } = useTranslation();
  const stored = useBench((s) => s.settings)!;
  const reference = useBench((s) => s.reference)!;
  const save = useBench((s) => s.saveSettings);
  const theme = useTheme();
  const [draft, setDraft] = useState<AppSettings>(stored);
  const [saved, setSaved] = useState(false);

  useEffect(() => setDraft(stored), [stored]);
  const edit = (patch: Partial<AppSettings>) => {
    setSaved(false);
    setDraft((d) => ({ ...d, ...patch }));
  };
  const safety = (patch: Partial<AppSettings["safety"]>) => edit({ safety: { ...draft.safety, ...patch } });
  const dirty = JSON.stringify(draft) !== JSON.stringify(stored);

  // The language applies immediately and is remembered for the next start.
  const changeLanguage = async (language: string) => {
    await save({ ...stored, language });
    setDraft((d) => ({ ...d, language }));
  };

  return (
    <div className="stack narrow">
      <Section title={t("settings.general")}>
        <div className="form-grid">
          <SelectField id="s-language" label={t("settings.language")} value={draft.language} options={languages.map((l) => ({ value: l.code as string, label: l.label }))} onChange={(v) => v && void changeLanguage(v)} />
          <SelectField
            id="s-theme"
            label={t("settings.theme")}
            value={theme}
            options={[{ value: "dark" as const, label: t("settings.themeDark") }, { value: "light" as const, label: t("settings.themeLight") }]}
            onChange={(v) => v && applyTheme(v)}
          />
          <label className="check"><input id="s-auto" type="checkbox" checked={draft.auto_connect} onChange={(e) => edit({ auto_connect: e.target.checked })} />{t("settings.autoConnect")}</label>
        </div>
        <label className="field wide" htmlFor="s-dir">
          <span>{t("settings.dataDir")}</span>
          <div className="row tight">
            <input id="s-dir" className="grow" type="text" value={draft.data_dir ?? ""} placeholder={reference.data_dir} onChange={(e) => edit({ data_dir: e.target.value || null })} />
            {isDesktop && <button onClick={() => void api.pickFolder().then((dir) => dir && edit({ data_dir: dir }))}>{t("settings.browse")}</button>}
            <button disabled={draft.data_dir === null} onClick={() => edit({ data_dir: null })}>{t("settings.useDefault")}</button>
          </div>
        </label>
      </Section>

      <Section title={t("settings.units")}>
        <div className="form-grid">
          <SelectField id="s-force" label={t("settings.force")} value={draft.units.force} options={[{ value: "gf" as const, label: "gf" }, { value: "kgf" as const, label: "kgf" }, { value: "n" as const, label: "N" }]} onChange={(v) => v && edit({ units: { ...draft.units, force: v } })} />
          <SelectField id="s-torque" label={t("settings.torque")} value={draft.units.torque} options={[{ value: "nm" as const, label: "N·m" }, { value: "nmm" as const, label: "N·mm" }]} onChange={(v) => v && edit({ units: { ...draft.units, torque: v } })} />
          <NumberField id="s-gravity" label={t("settings.gravity")} value={draft.gravity} step={0.0001} onChange={(v) => v && edit({ gravity: v })} />
        </div>
      </Section>

      <Section title={t("settings.safety")} info={t("settings.safetyInfo")}>
        <div className="form-grid">
          <NumberField id="s-maxthr" label={t("settings.maxThrottle")} value={Math.round(draft.safety.max_throttle * 1000) / 10} onChange={(v) => v !== null && safety({ max_throttle: v / 100 })} />
          <NumberField id="s-rise" label={t("settings.maxRise")} value={Math.round(draft.safety.max_rise_per_s * 1000) / 10} onChange={(v) => v !== null && safety({ max_rise_per_s: v / 100 })} />
          <NumberField id="s-maxthrust" label={t("settings.maxThrust")} info={t("settings.maxThrustInfo")} hint={t("settings.empty")} value={draft.safety.max_thrust_n} onChange={(v) => safety({ max_thrust_n: v })} />
          <NumberField id="s-maxtorque" label={t("settings.maxTorque")} info={t("settings.maxTorqueInfo")} hint={t("settings.empty")} value={draft.safety.max_torque_nm} onChange={(v) => safety({ max_torque_nm: v })} />
          <NumberField id="s-uitimeout" label={t("settings.uiTimeout")} info={t("settings.uiTimeoutInfo")} value={draft.safety.ui_timeout_ms} onChange={(v) => v !== null && safety({ ui_timeout_ms: v })} />
          <NumberField id="s-tare" label={t("settings.tareAge")} info={t("settings.tareAgeInfo")} value={draft.tare_max_age_min} onChange={(v) => v !== null && edit({ tare_max_age_min: v })} />
        </div>
      </Section>

      <Section title={t("settings.csv")}>
        <div className="form-grid">
          <SelectField id="s-naming" label={t("settings.naming")} value={draft.csv.column_naming} options={[{ value: "name" as const, label: t("settings.byName") }, { value: "id" as const, label: t("settings.byId") }]} onChange={(v) => v && edit({ csv: { ...draft.csv, column_naming: v } })} />
          <label className="check"><input id="s-raw" type="checkbox" checked={draft.csv.include_raw} onChange={(e) => edit({ csv: { ...draft.csv, include_raw: e.target.checked } })} />{t("settings.includeRaw")}</label>
        </div>
      </Section>

      <Section title={t("settings.calibration")}>
        <div className="form-grid">
          <NumberField id="s-calarm" label={t("device.calArm")} value={draft.calibration.torque_arm_mm} onChange={(v) => v && edit({ calibration: { ...draft.calibration, torque_arm_mm: v } })} />
          <NumberField id="s-ratio" label={t("device.forceRatio")} value={draft.calibration.force_ratio} onChange={(v) => v && edit({ calibration: { ...draft.calibration, force_ratio: v } })} />
          <NumberField id="s-capture" label={t("settings.captureSeconds")} value={draft.calibration.capture_seconds} onChange={(v) => v && edit({ calibration: { ...draft.calibration, capture_seconds: v } })} />
        </div>
      </Section>

      <div className="row">
        <span className="muted small">{t("settings.version", { app: reference.app_version, protocol: reference.protocol_version })}</span>
        <span className="grow" />
        {saved && <span className="small">{t("settings.saved")}</span>}
        <button id="s-save" className="primary" disabled={!dirty} onClick={() => void save(draft).then(setSaved)}>{t("common.save")}</button>
      </div>
    </div>
  );
}
