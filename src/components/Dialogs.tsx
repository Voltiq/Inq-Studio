import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { CloudDownload, FolderOpen, KeyRound, RefreshCw, TriangleAlert } from "lucide-react";
import { api, errorText } from "../lib/api";
import { findUpdate, installUpdate, skipVersion, type Update } from "../lib/updates";
import { MarkdownBlocks, parseManual } from "./Manual";
import { ProgressBar } from "./ui/Feedback";
import { Modal } from "./ui/Modal";
import { Button } from "./ui/Button";
import { Checkbox, Select, TextField, Toggle } from "./ui/Field";
import type { Settings } from "../lib/storage";
import type { ThemePreference } from "../lib/types";
import { t } from "../lib/i18n";

export function SettingsDialog({
  settings,
  onChange,
  onClose,
}: {
  settings: Settings;
  onChange: (s: Settings) => void;
  onClose: () => void;
}) {
  const [version, setVersion] = useState("");
  const [update, setUpdate] = useState<Update | null>(null);
  const [updateState, setUpdateState] = useState<"idle" | "checking" | "none" | "found" | "installing" | "failed">("idle");
  const [updateNote, setUpdateNote] = useState<string | null>(null);
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);
  const checkForUpdate = async () => {
    setUpdateState("checking");
    setUpdateNote(null);
    try {
      const found = await findUpdate();
      setUpdate(found);
      setUpdateState(found ? "found" : "none");
    } catch (e) {
      setUpdateState("failed");
      setUpdateNote(errorText(e));
    }
  };
  const install = async () => {
    if (!update) return;
    setUpdateState("installing");
    try {
      await installUpdate(update, (f) => setUpdateNote(f == null ? null : `${Math.round(f * 100)} %`));
    } catch (e) {
      setUpdateState("failed");
      setUpdateNote(errorText(e));
    }
  };
  return (
    <Modal title={t("Settings")} onClose={onClose} width="medium" footer={<Button appearance="primary" onClick={onClose}>{t("Done")}</Button>}>
      <div style={{ display: "grid", gap: 16 }}>
        <Select<Settings["language"]>
          label={t("Language")}
          value={settings.language}
          onChange={(language) => onChange({ ...settings, language })}
          options={[
            { value: "system", label: t("Match Windows") },
            { value: "en", label: "English" },
            { value: "de", label: "Deutsch" },
          ]}
          helper={t("Applies when Inq Studio is started again.")}
        />
        <Select<ThemePreference>
          label={t("Theme")}
          value={settings.theme}
          onChange={(theme) => onChange({ ...settings, theme })}
          options={[
            { value: "system", label: t("Match Windows") },
            { value: "dark", label: t("Graphite (dark)") },
            { value: "light", label: t("Porcelain (light)") },
          ]}
        />
        <Select<Settings["defaultZoom"]>
          label={t("Open documents at")}
          value={settings.defaultZoom}
          onChange={(defaultZoom) => onChange({ ...settings, defaultZoom })}
          options={[
            { value: "fit-width", label: t("Page width") },
            { value: "fit-page", label: t("Whole page") },
            { value: "100", label: "100 %" },
          ]}
        />
        <Select<Settings["units"]>
          label={t("Units")}
          value={settings.units}
          onChange={(units) => onChange({ ...settings, units })}
          options={[
            { value: "mm", label: t("Millimetres") },
            { value: "in", label: t("Inches") },
            { value: "pt", label: t("Points") },
          ]}
        />
        <TextField
          label={t("Author name")}
          helper={t("Written into notes, highlights and drawings you add.")}
          value={settings.author}
          onChange={(e) => onChange({ ...settings, author: e.target.value })}
        />
        <Select<Settings["saveMode"]>
          label={t("Saving")}
          value={settings.saveMode}
          onChange={(saveMode) => onChange({ ...settings, saveMode })}
          options={[
            { value: "auto", label: t("Automatic: append changes to signed PDFs, rewrite others") },
            { value: "full", label: t("Always rewrite (smallest file)") },
            { value: "incremental", label: t("Always append changes (keeps the original bytes)") },
          ]}
          helper={t("Appending keeps the original file intact inside the new one, so digital signatures stay valid.")}
        />
        <Toggle
          label={t("Snap moved text")}
          description={t("Moved lines snap to the edges and baselines of other lines. Hold Alt while dragging to move freely.")}
          checked={settings.snap}
          onChange={(snap) => onChange({ ...settings, snap })}
        />
        <Toggle
          label={t("Realign text automatically")}
          description={t("When a line of a paragraph gets shorter or longer, the words after it are wrapped again; when a line is deleted, the lines below move up. Other paragraphs and the rest of the page stay where they are.")}
          checked={settings.reflow}
          onChange={(reflow) => onChange({ ...settings, reflow })}
        />
        <Toggle
          label={t("Outline text lines while editing")}
          description={t("Shows a dashed box around every editable line when the Edit text tool is active.")}
          checked={settings.showLineBoxes}
          onChange={(showLineBoxes) => onChange({ ...settings, showLineBoxes })}
        />
        <TextField
          label={t("Timestamp server for signatures")}
          placeholder={t("http://timestamp.digicert.com")}
          helper={t("Optional. A timestamp authority confirms when a document was signed, so the signature stays checkable after the certificate expires. The server only receives a hash, never the document.")}
          value={settings.timestampUrl}
          onChange={(e) => onChange({ ...settings, timestampUrl: e.target.value.trim() })}
        />
        <div className="settings-block">
          <p className="eyebrow">{t("Updates")}</p>
          <div className="settings-line">
            <span>
              {updateState === "found" && update
                ? t("Inq Studio {0} is available", update.version)
                : updateState === "none"
                  ? t("Inq Studio is up to date")
                  : updateState === "installing"
                    ? t("Installing the update…")
                    : updateState === "failed"
                      ? t("The update check did not work")
                      : t("Checks for a new version every time it starts")}
              <small>
                {updateState === "found"
                  ? t("It installs on its own and Inq Studio starts again. Save your documents first.")
                  : updateNote ?? t("New versions come from the project's releases on GitHub, signed so only real ones install.")}
              </small>
            </span>
            {updateState === "found" ? (
              <Button spacing="compact" appearance="primary" iconBefore={CloudDownload} onClick={install}>
                {t("Install and restart")}
              </Button>
            ) : (
              <Button spacing="compact" iconBefore={RefreshCw} loading={updateState === "checking" || updateState === "installing"} onClick={checkForUpdate}>
                {t("Check now")}
              </Button>
            )}
          </div>
        </div>
        <Toggle
          label={t("Ask about updates at start")}
          description={t("When a newer version is out, Inq Studio asks at start whether to install it. Switched off, it only shows a short notice.")}
          checked={settings.updatePrompt}
          onChange={(updatePrompt) => onChange({ ...settings, updatePrompt })}
        />
        <div className="settings-block">
          <p className="eyebrow">{t("Diagnostics")}</p>
          <div className="settings-line">
            <span>
              {t("Log file and crash reports")}
              <small>{t("Kept on this computer only. They hold what the app did and what went wrong, never the content of your documents.")}</small>
            </span>
            <Button
              spacing="compact"
              iconBefore={FolderOpen}
              onClick={() =>
                api
                  .logInfo()
                  .then((l) => revealItemInDir(l.file))
                  .catch(() => {})
              }
            >
              {t("Show in folder")}
            </Button>
          </div>
        </div>
        <p className="is-muted" style={{ fontSize: 12 }}>
          {t("Inq Studio")}{version ? ` ${version}` : ""}{t(". Your documents stay on this computer: nothing is uploaded.")}
        </p>
      </div>
    </Modal>
  );
}

/** Asked at start when a newer version is out. */
export function UpdateDialog({
  update,
  unsaved,
  onQuiet,
  onClose,
}: {
  update: Update;
  /** Documents with changes that an update would throw away. */
  unsaved: number;
  /** The user ticked that the question should not come at start any more. */
  onQuiet: () => void;
  onClose: () => void;
}) {
  const [quiet, setQuiet] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const notes = parseManual(update.body ?? "");
  const leave = (skip: boolean) => {
    if (skip) skipVersion(update.version);
    if (quiet) onQuiet();
    onClose();
  };
  const install = async () => {
    if (quiet) onQuiet();
    setInstalling(true);
    setError(null);
    try {
      await installUpdate(update, setProgress);
    } catch (e) {
      setInstalling(false);
      setError(errorText(e));
    }
  };
  return (
    <Modal
      title={t("Inq Studio {0} is available", update.version)}
      titleIcon={<CloudDownload size={18} aria-hidden="true" />}
      width="medium"
      required={installing}
      closeOnBlanket={false}
      onClose={() => leave(false)}
      footer={
        <>
          <Button appearance="subtle" disabled={installing} onClick={() => leave(true)}>
            {t("Skip this version")}
          </Button>
          <Button disabled={installing} onClick={() => leave(false)}>
            {t("Later")}
          </Button>
          <Button appearance="primary" iconBefore={CloudDownload} loading={installing} onClick={install} data-autofocus>
            {t("Update now")}
          </Button>
        </>
      }
    >
      <p>
        {t("You have version {0}. The update downloads, installs on its own, and Inq Studio starts again.", update.currentVersion)}
      </p>
      {unsaved > 0 && (
        <p className="update-warning">
          <TriangleAlert size={15} aria-hidden="true" />
          {t("Save your documents first: changes that are not saved are lost when Inq Studio restarts.")}
        </p>
      )}
      {notes.length > 0 && (
        <div className="update-notes manual-text">
          <MarkdownBlocks blocks={notes} />
        </div>
      )}
      <Checkbox checked={quiet} onChange={setQuiet} disabled={installing} label={t("Don’t ask at start")} />
      {installing && <ProgressBar value={progress ?? 0} />}
      {error && <p className="update-warning">{error}</p>}
    </Modal>
  );
}

export function PasswordDialog({
  name,
  wrong,
  onSubmit,
  onClose,
}: {
  name: string;
  wrong: boolean;
  onSubmit: (password: string) => void;
  onClose: () => void;
}) {
  const [pw, setPw] = useState("");
  return (
    <Modal
      title={t("Password required")}
      titleIcon={<KeyRound size={16} aria-hidden="true" />}
      onClose={onClose}
      width="small"
      closeOnBlanket={false}
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            {t("Cancel")}
          </Button>
          <Button appearance="primary" disabled={!pw} onClick={() => onSubmit(pw)}>
            {t("Open")}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (pw) onSubmit(pw);
        }}
      >
        <p style={{ marginBottom: 12 }}>
          <strong>{name}</strong> {t("is protected. Enter its password to open it.")}
        </p>
        <TextField
          label={t("Password")}
          type="password"
          value={pw}
          autoFocus
          data-autofocus
          error={wrong ? t("That password is not right.") : null}
          onChange={(e) => setPw(e.target.value)}
        />
      </form>
    </Modal>
  );
}

export function UnsavedDialog({
  names,
  onSave,
  onDiscard,
  onCancel,
}: {
  names: string[];
  onSave: () => void;
  onDiscard: () => void;
  onCancel: () => void;
}) {
  const one = names.length === 1;
  return (
    <Modal
      title={t("Unsaved changes")}
      titleIcon={<TriangleAlert size={16} aria-hidden="true" style={{ color: "var(--icon-warning)" }} />}
      onClose={onCancel}
      width="small"
      footer={
        <>
          <Button appearance="subtle" onClick={onCancel}>
            {t("Cancel")}
          </Button>
          <Button appearance="danger" onClick={onDiscard}>
            {t("Don’t save")}
          </Button>
          <Button appearance="primary" onClick={onSave} data-autofocus>
            {t("Save")}
          </Button>
        </>
      }
    >
      <p>
        {one ? (
          <>
            <strong>{names[0]}</strong> {t("has changes that are not saved.")}
          </>
        ) : (
          <>
            {names.length} {t("documents have changes that are not saved:")}{" "}{names.join(", ")}.
          </>
        )}{" "}
        {t("Save before closing?")}
      </p>
    </Modal>
  );
}
