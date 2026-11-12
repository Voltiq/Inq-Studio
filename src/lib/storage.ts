import type { ThemePreference } from "./types";

export interface Settings {
  theme: ThemePreference;
  /** Zoom new documents open at. */
  defaultZoom: "fit-width" | "fit-page" | "100";
  /** Name written into notes and highlights. */
  author: string;
  units: "mm" | "in" | "pt";
  /** Outline every text line while the Edit text tool is active. */
  showLineBoxes: boolean;
  /** Colour per markup tool. */
  toolColors: Record<string, string>;
  inkWidth: number;
  /** auto: incremental for signed documents, full otherwise. */
  saveMode: "auto" | "full" | "incremental";
  /** Snap moved text to other lines' edges and baselines. */
  snap: boolean;
  /** Keep paragraphs in shape when one of their lines is rewritten or deleted. */
  reflow: boolean;
  /** A line dropped at the end or start of another joins it. */
  joinLines: boolean;
  /** Ask at start whether to install a newer version; otherwise only a short notice appears. */
  updatePrompt: boolean;
  /** Language of the interface; "system" follows Windows. */
  language: "system" | "en" | "de" | "es" | "fr";
  /** RFC 3161 timestamp server for signatures; empty for none. */
  timestampUrl: string;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  defaultZoom: "fit-width",
  author: "",
  units: "mm",
  showLineBoxes: true,
  toolColors: {
    highlight: "#ffd43b",
    underline: "#74c0fc",
    strikeout: "#e5484d",
    note: "#ffd43b",
    ink: "#6457f0",
    rect: "#e5484d",
  },
  inkWidth: 2,
  saveMode: "auto",
  snap: true,
  reflow: true,
  joinLines: true,
  updatePrompt: true,
  language: "system",
  timestampUrl: "",
};

const SETTINGS_KEY = "inq.settings";
const RECENT_KEY = "inq.recent";

export function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return { ...DEFAULT_SETTINGS, ...parsed, toolColors: { ...DEFAULT_SETTINGS.toolColors, ...parsed.toolColors } };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(s: Settings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
  } catch {
    /* settings stay in memory */
  }
}

export interface RecentFile {
  path: string;
  name: string;
  openedAt: number;
  pages: number;
}

export function loadRecent(): RecentFile[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    return raw ? (JSON.parse(raw) as RecentFile[]) : [];
  } catch {
    return [];
  }
}

export function rememberRecent(file: Omit<RecentFile, "openedAt">): RecentFile[] {
  const list = [{ ...file, openedAt: Date.now() }, ...loadRecent().filter((r) => r.path !== file.path)].slice(0, 12);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    /* not remembered */
  }
  return list;
}

export function forgetRecent(path: string): RecentFile[] {
  const list = loadRecent().filter((r) => r.path !== path);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    /* ignore */
  }
  return list;
}
