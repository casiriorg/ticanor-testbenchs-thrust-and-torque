// "Save as" for a chart: works on uPlot's own <canvas> or on an inline <svg>
// (CurveChart), rasterizing the latter through an off-screen canvas. One
// always-visible "Export" button per chart opens a small PNG/JPEG/BMP menu;
// picking a format opens a native save dialog defaulting to the Downloads
// folder (desktop) or triggers a normal browser download (mock).

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage } from "../services/backend";
import { useBench } from "../stores/benchStore";

type Format = "png" | "jpeg" | "bmp";

const MIME: Record<Format, string> = { png: "image/png", jpeg: "image/jpeg", bmp: "image/bmp" };
const EXT: Record<Format, string> = { png: "png", jpeg: "jpg", bmp: "bmp" };

function dataUrlToBytes(dataUrl: string): Uint8Array {
  const base64 = dataUrl.slice(dataUrl.indexOf(",") + 1);
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/** Hand-rolled 24-bit BMP encoder: canvas/image export doesn't support this format natively. */
function encodeBmp(image: ImageData): Uint8Array {
  const { width, height, data } = image;
  const rowSize = Math.ceil((width * 3) / 4) * 4;
  const pixelArraySize = rowSize * height;
  const fileSize = 54 + pixelArraySize;
  const buffer = new ArrayBuffer(fileSize);
  const view = new DataView(buffer);

  view.setUint8(0, 0x42); // 'B'
  view.setUint8(1, 0x4d); // 'M'
  view.setUint32(2, fileSize, true);
  view.setUint32(10, 54, true); // pixel data offset
  view.setUint32(14, 40, true); // DIB header size
  view.setInt32(18, width, true);
  view.setInt32(22, height, true); // positive height: bottom-up rows
  view.setUint16(26, 1, true); // planes
  view.setUint16(28, 24, true); // bits per pixel
  view.setUint32(34, pixelArraySize, true);

  const bytes = new Uint8Array(buffer);
  for (let y = 0; y < height; y++) {
    // BMP rows are stored bottom-up.
    const srcY = height - 1 - y;
    const rowStart = 54 + y * rowSize;
    for (let x = 0; x < width; x++) {
      const si = (srcY * width + x) * 4;
      const di = rowStart + x * 3;
      bytes[di] = data[si + 2]; // B
      bytes[di + 1] = data[si + 1]; // G
      bytes[di + 2] = data[si]; // R
    }
  }
  return bytes;
}

async function rasterize(el: HTMLCanvasElement | SVGSVGElement): Promise<HTMLCanvasElement> {
  if (el instanceof HTMLCanvasElement) return el;
  const xml = new XMLSerializer().serializeToString(el);
  const url = URL.createObjectURL(new Blob([xml], { type: "image/svg+xml;charset=utf-8" }));
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error("could not rasterize the chart"));
      img.src = url;
    });
    const rect = el.getBoundingClientRect();
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(rect.width) || 600;
    canvas.height = Math.round(rect.height) || 300;
    const ctx = canvas.getContext("2d")!;
    // The SVG itself is transparent; fill the panel colour so exports aren't black.
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--panel").trim() || "#15212a";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
    return canvas;
  } finally {
    URL.revokeObjectURL(url);
  }
}

async function elementToImageBytes(el: HTMLCanvasElement | SVGSVGElement, format: Format): Promise<Uint8Array> {
  const canvas = await rasterize(el);
  if (format === "bmp") {
    const ctx = canvas.getContext("2d")!;
    return encodeBmp(ctx.getImageData(0, 0, canvas.width, canvas.height));
  }
  return dataUrlToBytes(canvas.toDataURL(MIME[format], format === "jpeg" ? 0.92 : undefined));
}

/**
 * Always-visible small "Export" button with a PNG/JPEG/BMP menu. Renders
 * nothing when `exportName` is omitted, so inline/minor charts can skip it.
 */
export function ChartExportBar({ getElement, exportName }: { getElement: () => HTMLCanvasElement | SVGSVGElement | null; exportName?: string }) {
  const { t } = useTranslation();
  const setError = useBench((s) => s.setError);
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  if (!exportName) return null;

  const run = async (format: Format) => {
    setOpen(false);
    try {
      const el = getElement();
      if (!el) return;
      const bytes = await elementToImageBytes(el, format);
      await api.saveBytes(bytes, `${exportName}.${EXT[format]}`, MIME[format], [{ name: format.toUpperCase(), extensions: [EXT[format]] }]);
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="chart-export" ref={root}>
      <button type="button" className="chart-export-trigger" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {t("export.button")}
      </button>
      {open && (
        <div className="chart-export-menu" role="menu">
          {(["png", "jpeg", "bmp"] as Format[]).map((format) => (
            <button key={format} type="button" role="menuitem" onClick={() => void run(format)}>
              {format.toUpperCase()}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
