import { useState } from "react";
import { useTranslation } from "react-i18next";

import type { TestSetup } from "../services/types";
import { useBench } from "../stores/benchStore";
import { Dialog } from "./Dialog";
import { NumberField, SelectField, TextField } from "./Fields";

const STORAGE_KEY = "thrustbench.lastSetup";

export function emptySetup(): TestSetup {
  return {
    motor: { brand: "", model: "", kv: null, stator: "", poles: null, mass_g: null, kind: "outrunner", condition: "" },
    propeller: { brand: "", model: "", diameter_in: null, pitch_in: null, blades: 2, material: "", direction: "CW", mass_g: null, condition: "" },
    esc: { brand: "", model: "", current_a: null, protocol: "PWM", firmware: "" },
    battery: { chemistry: "", cells: null, capacity_mah: null, start_voltage: null },
    test: { operator: "", ambient_temp_c: null, altitude_m: null, notes: "" },
  };
}

/** The previous set-up is offered again: consecutive tests usually differ in one part. */
function lastSetup(): TestSetup {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored) as Partial<TestSetup>;
      const base = emptySetup();
      return { motor: { ...base.motor, ...parsed.motor }, propeller: { ...base.propeller, ...parsed.propeller }, esc: { ...base.esc, ...parsed.esc }, battery: { ...base.battery, ...parsed.battery }, test: { ...base.test, ...parsed.test, notes: "" } };
    }
  } catch {
    // Unavailable or corrupt storage only costs the convenience.
  }
  return emptySetup();
}

/** Describes what is on the bench. Shown before a recording starts. */
export function SetupForm({ onSubmit, onCancel }: { onSubmit: (setup: TestSetup) => void; onCancel: () => void }) {
  const { t } = useTranslation();
  const options = useBench((s) => s.reference?.setup_options ?? {});
  const [setup, setSetup] = useState<TestSetup>(lastSetup);
  const edit = <K extends keyof TestSetup>(group: K, patch: Partial<TestSetup[K]>) => setSetup((s) => ({ ...s, [group]: { ...s[group], ...patch } }));
  const choices = (key: string) => (options[key] ?? []).map((v) => ({ value: v as string, label: String(v) }));
  const numbers = (key: string) => (options[key] ?? []).map((v) => ({ value: v as number, label: String(v) }));

  const submit = () => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(setup));
    } catch {
      // See lastSetup().
    }
    onSubmit(setup);
  };

  return (
    <Dialog
      wide
      title={t("rec.setupTitle")}
      onClose={onCancel}
      actions={
        <>
          <button onClick={onCancel}>{t("common.cancel")}</button>
          <button id="setup-start" className="primary" onClick={submit}>{t("rec.startNow")}</button>
        </>
      }
    >
      <p className="muted">{t("rec.setupHint")}</p>
      <div className="form-groups">
        <fieldset>
          <legend>{t("setup.motor")}</legend>
          <TextField id="m-brand" label={t("setup.brand")} value={setup.motor.brand} onChange={(v) => edit("motor", { brand: v })} hint="MAD" />
          <TextField id="m-model" label={t("setup.model")} value={setup.motor.model} onChange={(v) => edit("motor", { model: v })} hint="3515 IPE" />
          <NumberField id="m-kv" label={t("setup.kv")} value={setup.motor.kv} onChange={(v) => edit("motor", { kv: v })} hint="400" />
          <TextField id="m-stator" label={t("setup.stator")} value={setup.motor.stator} onChange={(v) => edit("motor", { stator: v })} hint="3515" />
          <NumberField id="m-poles" label={t("setup.poles")} value={setup.motor.poles} onChange={(v) => edit("motor", { poles: v })} />
          <NumberField id="m-mass" label={t("setup.mass")} value={setup.motor.mass_g} onChange={(v) => edit("motor", { mass_g: v })} />
          <SelectField id="m-kind" label={t("setup.kind")} value={setup.motor.kind} options={choices("motor_type")} onChange={(v) => edit("motor", { kind: v ?? "" })} />
          <TextField id="m-cond" label={t("setup.condition")} value={setup.motor.condition} onChange={(v) => edit("motor", { condition: v })} />
        </fieldset>
        <fieldset>
          <legend>{t("setup.propeller")}</legend>
          <TextField id="p-brand" label={t("setup.brand")} value={setup.propeller.brand} onChange={(v) => edit("propeller", { brand: v })} />
          <TextField id="p-model" label={t("setup.model")} value={setup.propeller.model} onChange={(v) => edit("propeller", { model: v })} />
          <NumberField id="p-dia" label={t("setup.diameter")} value={setup.propeller.diameter_in} onChange={(v) => edit("propeller", { diameter_in: v })} hint="16" />
          <NumberField id="p-pitch" label={t("setup.pitch")} value={setup.propeller.pitch_in} onChange={(v) => edit("propeller", { pitch_in: v })} hint="5.4" />
          <SelectField id="p-blades" label={t("setup.blades")} value={setup.propeller.blades} options={numbers("propeller_blades")} onChange={(v) => edit("propeller", { blades: v })} allowEmpty />
          <SelectField id="p-mat" label={t("setup.material")} value={setup.propeller.material} options={choices("propeller_material")} onChange={(v) => edit("propeller", { material: v ?? "" })} allowEmpty />
          <SelectField id="p-dir" label={t("setup.direction")} value={setup.propeller.direction} options={choices("propeller_direction")} onChange={(v) => edit("propeller", { direction: v ?? "" })} />
          <TextField id="p-cond" label={t("setup.condition")} value={setup.propeller.condition} onChange={(v) => edit("propeller", { condition: v })} />
        </fieldset>
        <fieldset>
          <legend>{t("setup.esc")}</legend>
          <TextField id="e-brand" label={t("setup.brand")} value={setup.esc.brand} onChange={(v) => edit("esc", { brand: v })} />
          <TextField id="e-model" label={t("setup.model")} value={setup.esc.model} onChange={(v) => edit("esc", { model: v })} hint="AMPX 40A" />
          <NumberField id="e-cur" label={t("setup.current")} value={setup.esc.current_a} onChange={(v) => edit("esc", { current_a: v })} />
          <SelectField id="e-proto" label={t("setup.protocol")} value={setup.esc.protocol} options={choices("esc_protocol")} onChange={(v) => edit("esc", { protocol: v ?? "" })} />
        </fieldset>
        <fieldset>
          <legend>{t("setup.battery")}</legend>
          <SelectField id="b-chem" label={t("setup.chemistry")} value={setup.battery.chemistry} options={choices("battery_chemistry")} onChange={(v) => edit("battery", { chemistry: v ?? "" })} allowEmpty />
          <SelectField id="b-cells" label={t("setup.cells")} value={setup.battery.cells} options={numbers("battery_cells")} onChange={(v) => edit("battery", { cells: v })} allowEmpty />
          <NumberField id="b-cap" label={t("setup.capacity")} value={setup.battery.capacity_mah} onChange={(v) => edit("battery", { capacity_mah: v })} />
          <NumberField id="b-volt" label={t("setup.startVoltage")} value={setup.battery.start_voltage} onChange={(v) => edit("battery", { start_voltage: v })} />
        </fieldset>
        <fieldset className="span">
          <legend>{t("setup.test")}</legend>
          <TextField id="t-op" label={t("setup.operator")} value={setup.test.operator} onChange={(v) => edit("test", { operator: v })} />
          <NumberField id="t-temp" label={t("setup.temperature")} value={setup.test.ambient_temp_c} onChange={(v) => edit("test", { ambient_temp_c: v })} />
          <NumberField id="t-alt" label={t("setup.altitude")} value={setup.test.altitude_m} onChange={(v) => edit("test", { altitude_m: v })} />
          <label className="field wide" htmlFor="t-notes">
            <span>{t("setup.notes")}</span>
            <textarea id="t-notes" rows={2} value={setup.test.notes} onChange={(e) => edit("test", { notes: e.target.value })} />
          </label>
        </fieldset>
      </div>
    </Dialog>
  );
}
