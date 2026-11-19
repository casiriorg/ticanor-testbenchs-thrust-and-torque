// Small labelled inputs shared by the forms.

import type { ReactNode } from "react";

import { InfoTip } from "./InfoTip";

interface BaseProps {
  label: string;
  id: string;
  /** Shown as the input's placeholder (e.g. "empty = off"). */
  hint?: string;
  /** Longer explanation, shown on demand via an info button next to the label. */
  info?: string;
}

function Label({ label, info }: { label: string; info?: string }) {
  return (
    <span className="field-label">
      {label}
      {info && <InfoTip text={info} />}
    </span>
  );
}

export function TextField({ label, id, value, onChange, hint, info }: BaseProps & { value: string; onChange: (v: string) => void }) {
  return (
    <label className="field" htmlFor={id}>
      <Label label={label} info={info} />
      <input id={id} type="text" value={value} placeholder={hint} onChange={(e) => onChange(e.target.value)} />
    </label>
  );
}

/** Numeric input where an empty box means "unknown" (null). */
export function NumberField({ label, id, value, onChange, hint, info, step }: BaseProps & { value: number | null; onChange: (v: number | null) => void; step?: number }) {
  return (
    <label className="field" htmlFor={id}>
      <Label label={label} info={info} />
      <input
        id={id}
        type="number"
        step={step ?? "any"}
        value={value ?? ""}
        placeholder={hint}
        onChange={(e) => onChange(e.target.value === "" || Number.isNaN(e.target.valueAsNumber) ? null : e.target.valueAsNumber)}
      />
    </label>
  );
}

export function SelectField<T extends string | number>({ label, id, value, options, onChange, allowEmpty, info }: BaseProps & { value: T | null | ""; options: { value: T; label: string }[]; onChange: (v: T | null) => void; allowEmpty?: boolean }) {
  return (
    <label className="field" htmlFor={id}>
      <Label label={label} info={info} />
      <select
        id={id}
        value={value ?? ""}
        onChange={(e) => {
          const picked = options.find((o) => String(o.value) === e.target.value);
          onChange(picked ? picked.value : null);
        }}
      >
        {allowEmpty && <option value="">—</option>}
        {options.map((o) => <option key={String(o.value)} value={String(o.value)}>{o.label}</option>)}
      </select>
    </label>
  );
}

export function Section({ title, children, aside, info }: { title: string; children: ReactNode; aside?: ReactNode; info?: string }) {
  return (
    <section className="panel">
      <div className="panel-head">
        <h3>{title}{info && <InfoTip text={info} />}</h3>
        {aside}
      </div>
      {children}
    </section>
  );
}
