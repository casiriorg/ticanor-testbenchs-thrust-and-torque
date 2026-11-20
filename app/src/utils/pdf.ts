// A basic, text-only PDF summary of one recording: metadata, per-channel
// statistics, the event log and the throttle curve. Chart images are a
// separate concern (each chart has its own "save as PNG/JPEG" button).

import { jsPDF } from "jspdf";

import type { AppSettings, CurvePoint, RecordingData, Stats } from "../services/types";
import { channelDisplay, formatDuration } from "./units";

const MARGIN = 40;
const PAGE_BOTTOM = 780;

export function buildRecordingPdf(data: RecordingData, curve: CurvePoint[], stats: Record<string, Stats | null>, units: AppSettings["units"]): Uint8Array {
  const doc = new jsPDF({ unit: "pt", format: "a4" });
  let y = MARGIN;

  const line = (text: string, size = 10, bold = false, gap = size + 6) => {
    doc.setFont("helvetica", bold ? "bold" : "normal");
    doc.setFontSize(size);
    doc.text(text, MARGIN, y);
    y += gap;
  };
  const ensureRoom = () => {
    if (y > PAGE_BOTTOM) {
      doc.addPage();
      y = MARGIN;
    }
  };
  const heading = (text: string) => {
    ensureRoom();
    y += 8;
    line(text, 12, true, 18);
  };

  line(`ThrustBench recording: ${data.meta.id}`, 15, true, 22);
  line(`Started ${new Date(data.meta.started_at).toLocaleString()} · duration ${formatDuration(data.meta.duration_s)} · state ${data.meta.state}`);
  const motor = [data.setup.motor.brand, data.setup.motor.model].filter(Boolean).join(" ") || "—";
  const propeller = [data.setup.propeller.brand, data.setup.propeller.diameter_in ? `${data.setup.propeller.diameter_in} in` : ""].filter(Boolean).join(" ") || "—";
  line(`Motor: ${motor}     Propeller: ${propeller}`);
  if (data.setup.test.notes) line(`Notes: ${data.setup.test.notes}`);

  heading("Statistics");
  for (const c of data.channels) {
    ensureRoom();
    const s = stats[c.key];
    const d = channelDisplay(c.sensor_type, units);
    line(
      s
        ? `${c.name} (${d.unit}): mean ${d.convert(s.mean).toFixed(d.digits)}  std ${d.convert(s.std).toFixed(d.digits)}  min ${d.convert(s.min).toFixed(d.digits)}  max ${d.convert(s.max).toFixed(d.digits)}  rate ${s.rate_hz.toFixed(1)} Hz`
        : `${c.name}: no data`,
      9.5,
    );
  }

  if (data.meta.events.length > 0) {
    heading("Events");
    for (const e of data.meta.events) {
      ensureRoom();
      line(`${formatDuration(e.t_s)}  ${e.kind}  ${e.message}`, 9.5);
    }
  }

  if (curve.length > 0) {
    const force = channelDisplay(1, units);
    const torque = channelDisplay(2, units);
    heading("Throttle curve");
    line(`Throttle (%)     Thrust (${force.unit})     Torque (${torque.unit})     Thrust/Torque`, 9, true);
    for (const p of curve) {
      ensureRoom();
      line(
        [
          (p.throttle * 100).toFixed(0).padEnd(13),
          force.convert(p.thrust_mean).toFixed(force.digits).padEnd(13),
          (p.torque_mean === null ? "—" : torque.convert(p.torque_mean).toFixed(torque.digits)).padEnd(13),
          p.thrust_per_torque === null ? "—" : p.thrust_per_torque.toFixed(1),
        ].join(""),
        9,
      );
    }
  }

  return new Uint8Array(doc.output("arraybuffer"));
}
