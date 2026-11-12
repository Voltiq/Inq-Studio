/*
 * Updates from GitHub Releases. The app asks the address in tauri.conf.json
 * for the newest version; an update only installs if it is signed with the
 * key whose public half is in that file. The installer runs on its own
 * (passive mode), then Inq Studio starts again.
 */

import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

const LAST_CHECK = "inq.lastUpdateCheck";
const SKIPPED = "inq.skippedUpdate";

export type { Update };

/** The newer version, if there is one. Throws when the check itself fails (offline, say). */
export async function findUpdate(): Promise<Update | null> {
  const update = await check();
  try {
    localStorage.setItem(LAST_CHECK, String(Date.now()));
  } catch {
    // Not fatal: it just checks again next start.
  }
  return update;
}

/** At every start: never complains when offline, and leaves out a version the user chose to skip. */
export async function findUpdateQuietly(): Promise<Update | null> {
  try {
    const update = await findUpdate();
    return update && update.version !== skippedVersion() ? update : null;
  } catch {
    return null;
  }
}

function skippedVersion(): string | null {
  try {
    return localStorage.getItem(SKIPPED);
  } catch {
    return null;
  }
}

/** The start-up question is not asked again for this version. Settings › Updates still offers it. */
export function skipVersion(version: string) {
  try {
    localStorage.setItem(SKIPPED, version);
  } catch {
    // Asked again next time.
  }
}

/** Downloads and installs `update`, reporting progress from 0 to 1, then restarts. */
export async function installUpdate(update: Update, onProgress?: (fraction: number | null) => void): Promise<void> {
  let total = 0;
  let done = 0;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") total = event.data.contentLength ?? 0;
    else if (event.event === "Progress") {
      done += event.data.chunkLength;
      onProgress?.(total ? done / total : null);
    }
  });
  await relaunch();
}
