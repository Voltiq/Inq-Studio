import { useState } from "react";
import { KeyRound, TriangleAlert } from "lucide-react";
import { Modal } from "./ui/Modal";
import { Button } from "./ui/Button";
import { Select, TextField, Toggle } from "./ui/Field";
import type { Settings } from "../lib/storage";
import type { ThemePreference } from "../lib/types";

export function SettingsDialog({
  settings,
  onChange,
  onClose,
}: {
  settings: Settings;
  onChange: (s: Settings) => void;
  onClose: () => void;
}) {
  return (
    <Modal title="Settings" onClose={onClose} width="medium" footer={<Button appearance="primary" onClick={onClose}>Done</Button>}>
      <div style={{ display: "grid", gap: 16 }}>
        <Select<ThemePreference>
          label="Theme"
          value={settings.theme}
          onChange={(theme) => onChange({ ...settings, theme })}
          options={[
            { value: "system", label: "Match Windows" },
            { value: "dark", label: "Graphite (dark)" },
            { value: "light", label: "Porcelain (light)" },
          ]}
        />
        <Select<Settings["defaultZoom"]>
          label="Open documents at"
          value={settings.defaultZoom}
          onChange={(defaultZoom) => onChange({ ...settings, defaultZoom })}
          options={[
            { value: "fit-width", label: "Page width" },
            { value: "fit-page", label: "Whole page" },
            { value: "100", label: "100 %" },
          ]}
        />
        <Select<Settings["units"]>
          label="Units"
          value={settings.units}
          onChange={(units) => onChange({ ...settings, units })}
          options={[
            { value: "mm", label: "Millimetres" },
            { value: "in", label: "Inches" },
            { value: "pt", label: "Points" },
          ]}
        />
        <TextField
          label="Author name"
          helper="Written into notes, highlights and drawings you add."
          value={settings.author}
          onChange={(e) => onChange({ ...settings, author: e.target.value })}
        />
        <Toggle
          label="Outline text lines while editing"
          description="Shows a dashed box around every editable line when the Edit text tool is active."
          checked={settings.showLineBoxes}
          onChange={(showLineBoxes) => onChange({ ...settings, showLineBoxes })}
        />
      </div>
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
      title="Password required"
      titleIcon={<KeyRound size={16} aria-hidden="true" />}
      onClose={onClose}
      width="small"
      closeOnBlanket={false}
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button appearance="primary" disabled={!pw} onClick={() => onSubmit(pw)}>
            Open
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
          <strong>{name}</strong> is protected. Enter its password to open it.
        </p>
        <TextField
          label="Password"
          type="password"
          value={pw}
          autoFocus
          data-autofocus
          error={wrong ? "That password is not right." : null}
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
      title="Unsaved changes"
      titleIcon={<TriangleAlert size={16} aria-hidden="true" style={{ color: "var(--icon-warning)" }} />}
      onClose={onCancel}
      width="small"
      footer={
        <>
          <Button appearance="subtle" onClick={onCancel}>
            Cancel
          </Button>
          <Button appearance="danger" onClick={onDiscard}>
            Don’t save
          </Button>
          <Button appearance="primary" onClick={onSave} data-autofocus>
            Save
          </Button>
        </>
      }
    >
      <p>
        {one ? (
          <>
            <strong>{names[0]}</strong> has changes that are not saved.
          </>
        ) : (
          <>
            {names.length} documents have changes that are not saved: {names.join(", ")}.
          </>
        )}{" "}
        Save before closing?
      </p>
    </Modal>
  );
}
