import { useEffect, useMemo, useState } from "react";
import { Info, Pencil, Save, Trash2 } from "lucide-react";
import { Button } from "./ui/Button";
import { Lozenge } from "./ui/Feedback";
import { Select, TextField, Toggle } from "./ui/Field";
import type { DocInfo, FontCatalog, FontChoice, FontSummary, LineEdit, Meta } from "../lib/api";
import { previewStack } from "../lib/fontPreview";
import { bytes, fontLabel, length, paperName, pdfDate, rgbToHex, hexToRgb } from "../lib/format";
import type { Settings } from "../lib/storage";
import type { Tool } from "../lib/types";
import type { Selection, TextStyle } from "./Viewer";
import { cx } from "../lib/utils";

const MARK_SWATCHES = ["#ffd43b", "#8ce99a", "#74c0fc", "#f783ac", "#ffa94d", "#e5484d", "#6457f0", "#111318"];

export function Inspector({
  docId,
  info,
  selection,
  tool,
  fonts,
  catalog,
  textStyle,
  onTextStyle,
  markColor,
  onMarkColor,
  settings,
  onSettings,
  pageSize,
  onEditLine,
  onStartEditing,
  onDeleteAnnot,
  onSaveMeta,
}: {
  docId: number;
  info: DocInfo;
  selection: Selection;
  tool: Tool;
  fonts: FontSummary[] | null;
  catalog: FontCatalog | null;
  textStyle: TextStyle;
  onTextStyle: (s: TextStyle) => void;
  markColor: string;
  onMarkColor: (c: string) => void;
  settings: Settings;
  onSettings: (s: Settings) => void;
  pageSize: { w: number; h: number } | null;
  onEditLine: (edit: LineEdit) => void;
  onStartEditing: () => void;
  onDeleteAnnot: () => void;
  onSaveMeta: (m: Meta) => void;
}) {
  let body: React.ReactNode;
  if (selection?.kind === "line") {
    body = (
      <LineInspector
        docId={docId}
        selection={selection}
        fonts={fonts}
        units={settings.units}
        onEdit={onEditLine}
        onStartEditing={onStartEditing}
      />
    );
  } else if (selection?.kind === "annot") {
    body = <AnnotInspector selection={selection} onDelete={onDeleteAnnot} />;
  } else if (tool === "text") {
    body = <TextToolInspector fonts={fonts} catalog={catalog} style={textStyle} onChange={onTextStyle} />;
  } else if (["highlight", "underline", "strikeout", "note", "ink", "rect"].includes(tool)) {
    body = (
      <MarkInspector
        tool={tool}
        color={markColor}
        onColor={onMarkColor}
        settings={settings}
        onSettings={onSettings}
      />
    );
  } else {
    body = <DocInspector info={info} pageSize={pageSize} units={settings.units} onSave={onSaveMeta} />;
  }
  return (
    <aside className="panel side is-inspector" aria-label="Inspector">
      <div className="side-body" style={{ paddingTop: "var(--space-200)" }}>
        {body}
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------

function LineInspector({
  docId,
  selection,
  fonts,
  units,
  onEdit,
  onStartEditing,
}: {
  docId: number;
  selection: Extract<Selection, { kind: "line" }>;
  fonts: FontSummary[] | null;
  units: Settings["units"];
  onEdit: (e: LineEdit) => void;
  onStartEditing: () => void;
}) {
  const l = selection.line;
  const font = fonts?.find((f) => f.key === l.fontKey) ?? null;
  const [stack, setStack] = useState(`"${l.family}", sans-serif`);
  const [size, setSize] = useState(l.size.toFixed(2).replace(/\.?0+$/, ""));
  const [text, setText] = useState(l.text);
  useEffect(() => {
    setSize(l.size.toFixed(2).replace(/\.?0+$/, ""));
    setText(l.text);
  }, [l]);
  useEffect(() => {
    if (!font) return;
    let live = true;
    previewStack(docId, font).then((s) => live && setStack(s));
    return () => {
      live = false;
    };
  }, [docId, font]);

  const applySize = () => {
    const v = parseFloat(size);
    if (v > 0 && Math.abs(v - l.size) > 0.001) onEdit({ size: v });
  };

  return (
    <>
      <section className="side-section">
        <span className="eyebrow">Text line</span>
        <div
          className="line-preview"
          style={{
            fontFamily: stack,
            fontWeight: l.bold ? 700 : 400,
            fontStyle: l.italic ? "italic" : undefined,
            color: rgbToHex(l.color),
          }}
        >
          {l.text}
        </div>
        {l.note && (
          <div className="notice is-warning" style={{ marginTop: 8 }}>
            <Info size={14} aria-hidden="true" />
            <span>{l.note}</span>
          </div>
        )}
        {l.editable && (
          <>
            <div style={{ marginTop: 12 }}>
              <TextField
                label="Text"
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && text !== l.text) onEdit({ text });
                }}
              />
            </div>
            <div className="inspector-actions">
              <Button appearance="primary" spacing="compact" iconBefore={Save} disabled={text === l.text} onClick={() => onEdit({ text })}>
                Apply
              </Button>
              <Button spacing="compact" iconBefore={Pencil} onClick={onStartEditing} shortcut="Enter">
                Edit on page
              </Button>
              <Button appearance="subtle" spacing="compact" iconBefore={Trash2} onClick={() => onEdit({ delete: true })} shortcut="Del">
                Delete
              </Button>
            </div>
          </>
        )}
      </section>

      <section className="side-section">
        <span className="eyebrow">Font</span>
        <dl className="props">
          <dt>Family</dt>
          <dd>{font ? fontLabel(font) : l.family}</dd>
          <dt>PostScript</dt>
          <dd className="is-mono">{l.fontName}</dd>
          {font && (
            <>
              <dt>Type</dt>
              <dd>
                {font.kind} {font.subset && <Lozenge appearance="information">subset</Lozenge>}
              </dd>
              <dt>Glyphs</dt>
              <dd>{font.glyphs.toLocaleString()}</dd>
              <dt>Installed</dt>
              <dd>{font.systemMatch ?? "no"}</dd>
            </>
          )}
        </dl>
      </section>

      {l.editable && (
        <section className="side-section">
          <span className="eyebrow">Style</span>
          <div className="row">
            <TextField
              label="Size (pt)"
              value={size}
              mono
              inputMode="decimal"
              onChange={(e) => setSize(e.target.value)}
              onBlur={applySize}
              onKeyDown={(e) => e.key === "Enter" && applySize()}
            />
            <label className="color-field">
              <span className="field-label">Colour</span>
              <input
                type="color"
                className="color-input"
                defaultValue={rgbToHex(l.color)}
                key={rgbToHex(l.color)}
                onChange={(e) => {
                  const v = e.target.value;
                  window.clearTimeout((window as unknown as { __inqColor?: number }).__inqColor);
                  (window as unknown as { __inqColor?: number }).__inqColor = window.setTimeout(
                    () => onEdit({ color: hexToRgb(v) }),
                    350,
                  );
                }}
              />
            </label>
          </div>
          <p className="is-muted" style={{ marginTop: 8, fontSize: 12 }}>
            Drawn at {l.em.toFixed(2)} pt{Math.abs(l.angle) > 0.5 ? `, rotated ${l.angle.toFixed(0)}°` : ""}.
          </p>
        </section>
      )}

      <section className="side-section">
        <span className="eyebrow">Position</span>
        <dl className="props">
          <dt>x</dt>
          <dd className="is-mono">{length(l.origin[0], units)}</dd>
          <dt>y</dt>
          <dd className="is-mono">{length(l.origin[1], units)}</dd>
          <dt>Page</dt>
          <dd>{selection.page + 1}</dd>
        </dl>
        {l.editable && (
          <p className="is-muted" style={{ marginTop: 8, fontSize: 12 }}>
            Drag the selected line on the page to move it.
          </p>
        )}
      </section>
    </>
  );
}

function AnnotInspector({ selection, onDelete }: { selection: Extract<Selection, { kind: "annot" }>; onDelete: () => void }) {
  const a = selection.annot;
  return (
    <section className="side-section">
      <span className="eyebrow">Annotation</span>
      <dl className="props" style={{ marginTop: 8 }}>
        <dt>Type</dt>
        <dd>{a.subtype}</dd>
        <dt>Page</dt>
        <dd>{selection.page + 1}</dd>
        {a.author && (
          <>
            <dt>Author</dt>
            <dd>{a.author}</dd>
          </>
        )}
        {a.color && (
          <>
            <dt>Colour</dt>
            <dd>
              <span className="comment-swatch" style={{ display: "inline-block", background: rgbToHex(a.color) }} />{" "}
              <span className="is-mono">{rgbToHex(a.color)}</span>
            </dd>
          </>
        )}
      </dl>
      {a.contents && (
        <p className="line-preview" style={{ marginTop: 12, fontSize: 13, fontFamily: "var(--font-body)" }}>
          {a.contents}
        </p>
      )}
      {a.removable && (
        <div className="inspector-actions">
          <Button appearance="danger" spacing="compact" iconBefore={Trash2} onClick={onDelete} shortcut="Del">
            Delete
          </Button>
        </div>
      )}
    </section>
  );
}

function choiceKey(c: FontChoice): string {
  if (c.kind === "document") return `doc:${c.key}`;
  if (c.kind === "system") return `sys:${c.family}`;
  return `std:${c.name}`;
}

function TextToolInspector({
  fonts,
  catalog,
  style,
  onChange,
}: {
  fonts: FontSummary[] | null;
  catalog: FontCatalog | null;
  style: TextStyle;
  onChange: (s: TextStyle) => void;
}) {
  const options = useMemo(() => {
    const out: { value: string; label: string }[] = [];
    for (const f of fonts ?? []) {
      if (f.editable) out.push({ value: `doc:${f.key}`, label: `Document · ${fontLabel(f)}${f.subset ? " (subset)" : ""}` });
    }
    for (const fam of catalog?.families ?? []) out.push({ value: `sys:${fam}`, label: `Installed · ${fam}` });
    for (const s of catalog?.standard ?? []) out.push({ value: `std:${s}`, label: `Standard · ${s}` });
    return out;
  }, [fonts, catalog]);

  const setFont = (v: string) => {
    const [kind, ...rest] = v.split(":");
    const name = rest.join(":");
    const prevSys = style.font.kind === "system" ? style.font : null;
    const font: FontChoice =
      kind === "doc"
        ? { kind: "document", key: name }
        : kind === "sys"
          ? { kind: "system", family: name, bold: prevSys?.bold ?? false, italic: prevSys?.italic ?? false }
          : { kind: "standard", name };
    onChange({ ...style, font });
  };

  const [size, setSize] = useState(String(style.size));
  useEffect(() => setSize(String(style.size)), [style.size]);

  return (
    <section className="side-section">
      <span className="eyebrow">New text</span>
      <div style={{ display: "grid", gap: 12, marginTop: 8 }}>
        <Select label="Font" value={choiceKey(style.font)} options={options.length ? options : [{ value: choiceKey(style.font), label: "Loading fonts…" }]} onChange={setFont} />
        {style.font.kind === "document" && (
          <p className="is-muted" style={{ fontSize: 12 }}>
            Written in the document’s own font. Characters its subset lacks come from the same typeface if it is installed.
          </p>
        )}
        {style.font.kind === "system" && (
          <div style={{ display: "grid", gap: 8 }}>
            <Toggle
              label="Bold"
              checked={style.font.bold}
              onChange={(v) => style.font.kind === "system" && onChange({ ...style, font: { ...style.font, bold: v } })}
            />
            <Toggle
              label="Italic"
              checked={style.font.italic}
              onChange={(v) => style.font.kind === "system" && onChange({ ...style, font: { ...style.font, italic: v } })}
            />
          </div>
        )}
        <div className="row">
          <TextField
            label="Size (pt)"
            value={size}
            mono
            inputMode="decimal"
            onChange={(e) => setSize(e.target.value)}
            onBlur={() => {
              const v = parseFloat(size);
              if (v > 0 && v < 1000) onChange({ ...style, size: v });
              else setSize(String(style.size));
            }}
          />
          <label className="color-field">
            <span className="field-label">Colour</span>
            <input type="color" className="color-input" value={style.color} onChange={(e) => onChange({ ...style, color: e.target.value })} />
          </label>
        </div>
        <p className="is-muted" style={{ fontSize: 12 }}>
          Click on the page to place text. Ctrl+Enter places it, Enter starts a new line.
        </p>
      </div>
    </section>
  );
}

function MarkInspector({
  tool,
  color,
  onColor,
  settings,
  onSettings,
}: {
  tool: Tool;
  color: string;
  onColor: (c: string) => void;
  settings: Settings;
  onSettings: (s: Settings) => void;
}) {
  const title: Record<string, string> = {
    highlight: "Highlight",
    underline: "Underline",
    strikeout: "Strike-out",
    note: "Sticky note",
    ink: "Drawing",
    rect: "Rectangle",
  };
  return (
    <section className="side-section">
      <span className="eyebrow">{title[tool]}</span>
      <div style={{ display: "grid", gap: 12, marginTop: 8 }}>
        <div>
          <span className="field-label">Colour</span>
          <div className="swatches" style={{ marginTop: 6 }}>
            {MARK_SWATCHES.map((c) => (
              <button
                key={c}
                type="button"
                className={cx("swatch", c.toLowerCase() === color.toLowerCase() && "is-selected")}
                style={{ background: c }}
                aria-label={c}
                onClick={() => onColor(c)}
              />
            ))}
            <input type="color" className="color-input" style={{ width: 28, height: 24 }} value={color} onChange={(e) => onColor(e.target.value)} aria-label="Custom colour" />
          </div>
        </div>
        {(tool === "ink" || tool === "rect") && (
          <TextField
            label="Line width (pt)"
            value={String(settings.inkWidth)}
            mono
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              if (v > 0 && v <= 40) onSettings({ ...settings, inkWidth: v });
            }}
          />
        )}
        <TextField
          label="Author"
          value={settings.author}
          placeholder="Shown on notes and highlights"
          onChange={(e) => onSettings({ ...settings, author: e.target.value })}
        />
      </div>
    </section>
  );
}

function DocInspector({
  info,
  pageSize,
  units,
  onSave,
}: {
  info: DocInfo;
  pageSize: { w: number; h: number } | null;
  units: Settings["units"];
  onSave: (m: Meta) => void;
}) {
  const [meta, setMeta] = useState(info.meta);
  useEffect(() => setMeta(info.meta), [info.meta]);
  const changed = (["title", "author", "subject", "keywords"] as const).some((k) => meta[k] !== info.meta[k]);
  return (
    <>
      <section className="side-section">
        <span className="eyebrow">Document</span>
        <dl className="props" style={{ marginTop: 8 }}>
          <dt>File</dt>
          <dd>{info.state.name}</dd>
          {info.state.path && (
            <>
              <dt>Folder</dt>
              <dd className="is-muted">{info.state.path.replace(/[\\/][^\\/]*$/, "")}</dd>
            </>
          )}
          <dt>Pages</dt>
          <dd>{info.state.pages}</dd>
          <dt>Version</dt>
          <dd>PDF {info.version}</dd>
          {info.fileSize > 0 && (
            <>
              <dt>Size</dt>
              <dd>{bytes(info.fileSize)}</dd>
            </>
          )}
          {pageSize && (
            <>
              <dt>Page</dt>
              <dd>
                {length(pageSize.w, units)} × {length(pageSize.h, units)}
                {paperName(pageSize.w, pageSize.h) && <span className="is-muted"> · {paperName(pageSize.w, pageSize.h)}</span>}
              </dd>
            </>
          )}
          {info.meta.creator && (
            <>
              <dt>Creator</dt>
              <dd>{info.meta.creator}</dd>
            </>
          )}
          {info.meta.producer && (
            <>
              <dt>Producer</dt>
              <dd>{info.meta.producer}</dd>
            </>
          )}
          {info.meta.created && (
            <>
              <dt>Created</dt>
              <dd>{pdfDate(info.meta.created)}</dd>
            </>
          )}
          {info.meta.modified && (
            <>
              <dt>Modified</dt>
              <dd>{pdfDate(info.meta.modified)}</dd>
            </>
          )}
        </dl>
      </section>
      <section className="side-section">
        <span className="eyebrow">Properties</span>
        <div style={{ display: "grid", gap: 10, marginTop: 8 }}>
          <TextField label="Title" value={meta.title} onChange={(e) => setMeta({ ...meta, title: e.target.value })} />
          <TextField label="Author" value={meta.author} onChange={(e) => setMeta({ ...meta, author: e.target.value })} />
          <TextField label="Subject" value={meta.subject} onChange={(e) => setMeta({ ...meta, subject: e.target.value })} />
          <TextField label="Keywords" value={meta.keywords} onChange={(e) => setMeta({ ...meta, keywords: e.target.value })} />
        </div>
        <div className="inspector-actions">
          <Button appearance="primary" spacing="compact" disabled={!changed} onClick={() => onSave(meta)}>
            Apply properties
          </Button>
        </div>
      </section>
    </>
  );
}
