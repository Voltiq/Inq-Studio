import { count, currentLanguage } from "./i18n";
import type { FontSummary } from "./api";

export function rgbToHex([r, g, b]: [number, number, number]): string {
  const h = (v: number) =>
    Math.round(Math.max(0, Math.min(1, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${h(r)}${h(g)}${h(b)}`;
}

export function hexToRgb(hex: string): [number, number, number] {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  if (!m) return [0, 0, 0];
  return [parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255];
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** Points to the chosen unit. */
export function length(pt: number, unit: "mm" | "in" | "pt"): string {
  if (unit === "mm") return `${((pt / 72) * 25.4).toFixed(1)} mm`;
  if (unit === "in") return `${(pt / 72).toFixed(2)} in`;
  return `${pt.toFixed(1)} pt`;
}

const PAPER: [string, number, number][] = [
  ["A3", 842, 1191],
  ["A4", 595, 842],
  ["A5", 420, 595],
  ["Letter", 612, 792],
  ["Legal", 612, 1008],
  ["Tabloid", 792, 1224],
];

export function paperName(w: number, h: number): string | null {
  const [a, b] = [Math.min(w, h), Math.max(w, h)];
  const hit = PAPER.find(([, pw, ph]) => Math.abs(pw - a) < 3 && Math.abs(ph - b) < 3);
  return hit ? `${hit[0]}${w > h ? " landscape" : ""}` : null;
}

/** "D:20240102030405+01'00'" → a readable local date. */
export function pdfDate(s: string): string {
  const m = /D:(\d{4})(\d{2})?(\d{2})?(\d{2})?(\d{2})?(\d{2})?/.exec(s);
  if (!m) return s;
  const d = new Date(
    Date.UTC(+m[1], +(m[2] ?? 1) - 1, +(m[3] ?? 1), +(m[4] ?? 0), +(m[5] ?? 0), +(m[6] ?? 0)),
  );
  return d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

export function fontLabel(f: Pick<FontSummary, "family" | "style">): string {
  return f.style ? `${f.family} ${f.style}` : f.family;
}

export function plural(n: number, word: string, many = `${word}s`): string {
  if (currentLanguage() !== "en") return count(n, word, many);
  return `${n.toLocaleString()} ${n === 1 ? word : many}`;
}
