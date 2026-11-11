import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { ThemePreference } from "./types";

export type Theme = "light" | "dark";

// Window backgrounds, matching --bg in each theme.
const BACKGROUNDS: Record<Theme, [number, number, number]> = {
  light: [236, 238, 242],
  dark: [13, 14, 17],
};

const darkQuery = () => window.matchMedia("(prefers-color-scheme: dark)");

/** The light or dark mode set in Windows. */
export function systemTheme(): Theme {
  return darkQuery().matches ? "dark" : "light";
}

export function resolveTheme(preference: ThemePreference): Theme {
  return preference === "system" ? systemTheme() : preference;
}

let settle = 0;

/**
 * Switches the page's colours right away, without waiting for the window.
 * Transitions are held off while the colours change: otherwise every element
 * fades to the new theme at its own speed and the switch arrives in pieces.
 */
export function setPageTheme(theme: Theme) {
  const root = document.documentElement;
  if (root.dataset.theme === theme) return;
  root.classList.add("theme-switching");
  root.dataset.theme = theme;
  // Apply the new colours now, then let transitions run again two frames later.
  void root.offsetHeight;
  cancelAnimationFrame(settle);
  settle = requestAnimationFrame(() => {
    settle = requestAnimationFrame(() => root.classList.remove("theme-switching"));
  });
}

/**
 * Applies a theme to the page, the title bar and the window background.
 * Returns the theme that ended up in use.
 */
export async function applyTheme(preference: ThemePreference): Promise<Theme> {
  setPageTheme(resolveTheme(preference));
  const win = getCurrentWebviewWindow();
  // "system" hands the title bar (and the web view's colour scheme) back to Windows.
  await win.setTheme(preference === "system" ? null : preference).catch(() => {});
  const theme = resolveTheme(preference);
  setPageTheme(theme);
  await win.setBackgroundColor(BACKGROUNDS[theme]).catch(() => {});
  return theme;
}

/** Calls `onChange` when Windows switches between light and dark mode. */
export function watchSystemTheme(onChange: () => void): () => void {
  const query = darkQuery();
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}
