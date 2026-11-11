/*
 * Translation. English is the language the interface is written in; the
 * English text is also the key. `t("Open PDF")` gives the text in the
 * language in use, or the English when a translation is missing.
 * Placeholders are numbered: t("Page {0} of {1}", 3, 12).
 *
 * The language is fixed when the app starts (from Settings, or from
 * Windows), because labels are also built once when the modules load.
 * Changing it in Settings applies at the next start.
 */

import { de } from "./de";

export type Language = "en" | "de";

function detect(): Language {
  let preference = "system";
  try {
    preference = (JSON.parse(localStorage.getItem("inq.settings") ?? "{}") as { language?: string }).language ?? "system";
  } catch {
    // No settings yet: follow the system.
  }
  if (preference === "en" || preference === "de") return preference;
  const system = typeof navigator === "undefined" ? "en" : navigator.language;
  return system.toLowerCase().startsWith("de") ? "de" : "en";
}

let language: Language = detect();
const tables: Record<Language, Record<string, string>> = { en: {}, de };

if (typeof document !== "undefined") document.documentElement.lang = language;

export function currentLanguage(): Language {
  return language;
}

/** For tests. */
export function setLanguage(l: Language) {
  language = l;
}

export function t(text: string, ...args: (string | number | null | undefined)[]): string {
  const found = tables[language][text] ?? text;
  return args.length ? found.replace(/\{(\d+)\}/g, (_, i) => String(args[Number(i)] ?? "")) : found;
}

/**
 * Text that comes from the engine (errors, notices, history labels). The
 * engine writes English; sentences the table knows are shown translated,
 * with their numbers and names put back in.
 */
export function te(text: string): string {
  if (language === "en" || !text) return text;
  const direct = tables[language][text];
  if (direct) return direct;
  // "Deleted 3 pages" matches the key "Deleted {0} pages": numbers and quoted names are placeholders.
  const args: string[] = [];
  const key = text.replace(/“[^”]*”|\d+(?:[.,]\d+)?/g, (m) => {
    args.push(m);
    return `{${args.length - 1}}`;
  });
  const found = tables[language][key];
  return found ? found.replace(/\{(\d+)\}/g, (_, i) => args[Number(i)] ?? "") : text;
}

/** "3 pages" in the language in use. `word` is the English singular. */
export function count(n: number, word: string, many = `${word}s`): string {
  return `${n} ${t(n === 1 ? word : many)}`;
}
