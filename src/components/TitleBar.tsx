import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "../lib/i18n";

/*
 * The window's own title bar: icon and name on the left, the document in
 * the middle, and the three window buttons on the right. The whole bar
 * drags the window; a double click maximises it.
 */
export function TitleBar({ document: doc, dirty }: { document: string | null; dirty: boolean }) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let live = true;
    const read = () =>
      win
        .isMaximized()
        .then((m) => live && setMaximized(!!m))
        .catch(() => {});
    read();
    const un = win.onResized(read);
    return () => {
      live = false;
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  const win = () => getCurrentWindow();
  return (
    <div className={maximized ? "titlebar is-maximized" : "titlebar"} data-tauri-drag-region>
      <div className="titlebar-brand" data-tauri-drag-region>
        <img className="titlebar-icon" src="/app-icon.svg" alt="" data-tauri-drag-region />
        <span className="titlebar-name" data-tauri-drag-region>
          {t("Inq")} <span className="titlebar-studio" data-tauri-drag-region>{t("Studio")}</span>
        </span>
      </div>
      <div className="titlebar-document" data-tauri-drag-region>
        {doc && (
          <>
            {dirty && <span className="titlebar-dirty" aria-hidden="true" />}
            {doc}
          </>
        )}
      </div>
      {/* The free space left of the buttons drags the window too; the buttons themselves do not. */}
      <div className="titlebar-buttons" data-tauri-drag-region>
        <button type="button" className="titlebar-button" aria-label={t("Minimise")} data-tooltip={t("Minimise")} tabIndex={-1} onClick={() => win().minimize().catch(() => {})}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path d="M0 5h10" fill="none" stroke="currentColor" />
          </svg>
        </button>
        <button
          type="button"
          className="titlebar-button"
          aria-label={maximized ? t("Restore") : t("Maximise")}
          data-tooltip={maximized ? t("Restore") : t("Maximise")}
          tabIndex={-1}
          onClick={() => win().toggleMaximize().catch(() => {})}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            {maximized ? (
              <path d="M2.5 2.5v-2h7v7h-2M0.5 2.5h7v7h-7z" fill="none" stroke="currentColor" />
            ) : (
              <path d="M0.5 0.5h9v9h-9z" fill="none" stroke="currentColor" />
            )}
          </svg>
        </button>
        <button type="button" className="titlebar-button is-close" aria-label={t("Close window")} data-tooltip={t("Close window")} tabIndex={-1} onClick={() => win().close().catch(() => {})}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path d="M0 0l10 10M10 0L0 10" fill="none" stroke="currentColor" />
          </svg>
        </button>
      </div>
    </div>
  );
}
