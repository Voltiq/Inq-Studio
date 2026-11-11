import { useEffect, useMemo, useState } from "react";
import { Download, EyeOff, ImageUp, Info, Pencil, Pilcrow, Save, Trash2 } from "lucide-react";
import type { ToolOptions } from "./PageExtras";
import { Button } from "./ui/Button";
import { Lozenge } from "./ui/Feedback";
import { Select, TextField, Toggle } from "./ui/Field";
import type { AnnotChange, DocInfo, FontCatalog, FontChoice, FontSummary, LineEdit, Meta } from "../lib/api";
import { previewStack } from "../lib/fontPreview";
import { bytes, fontLabel, length, paperName, pdfDate, rgbToHex, hexToRgb } from "../lib/format";
import type { Settings } from "../lib/storage";
import type { Tool } from "../lib/types";
import type { Selection, TextStyle } from "./Viewer";
import { cx } from "../lib/utils";
import { t } from "../lib/i18n";

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
  onUpdateAnnot,
  onReplaceImage,
  onExtractImage,
  onDeleteImage,
  toolOptions,
  onToolOptions,
  redactCount,
  redactFill,
  onRedactFill,
  onRedactFind,
  onRedactApply,
  onRedactClear,
  onUncrop,
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
  onStartEditing: (what: "line" | "block") => void;
  onDeleteAnnot: () => void;
  onSaveMeta: (m: Meta) => void;
  onUpdateAnnot: (c: AnnotChange) => void;
  onReplaceImage: () => void;
  onExtractImage: () => void;
  onDeleteImage: () => void;
  toolOptions: ToolOptions;
  onToolOptions: (o: ToolOptions) => void;
  redactCount: number;
  redactFill: "black" | "white" | "none";
  onRedactFill: (f: "black" | "white" | "none") => void;
  onRedactFind: (query: string) => void;
  onRedactApply: () => void;
  onRedactClear: () => void;
  onUncrop: (scope: "page" | "all") => void;
}) {
  let body: React.ReactNode;
  if (selection?.kind === "lines") {
    body = (
      <LinesInspector
        docId={docId}
        selection={selection}
        fonts={fonts}
        catalog={catalog}
        units={settings.units}
        onEdit={onEditLine}
        onStartEditing={onStartEditing}
      />
    );
  } else if (selection?.kind === "annot") {
    body = <AnnotInspector selection={selection} onDelete={onDeleteAnnot} onUpdate={onUpdateAnnot} />;
  } else if (selection?.kind === "image") {
    body = (
      <ImageInspector
        selection={selection}
        units={settings.units}
        onReplace={onReplaceImage}
        onExtract={onExtractImage}
        onDelete={onDeleteImage}
      />
    );
  } else if (tool === "redact") {
    body = <RedactInspector count={redactCount} fill={redactFill} onFill={onRedactFill} onFind={onRedactFind} onApply={onRedactApply} onClear={onRedactClear} />;
  } else if (tool === "crop") {
    body = <CropInspector onUncrop={onUncrop} />;
  } else if (tool === "text") {
    body = <TextToolInspector fonts={fonts} catalog={catalog} style={textStyle} onChange={onTextStyle} />;
  } else if (["highlight", "underline", "strikeout", "note", "ink", "rect", "ellipse", "line", "arrow", "polygon", "textbox", "stamp"].includes(tool)) {
    body = (
      <>
        <ToolOptionsInspector tool={tool} options={toolOptions} onOptions={onToolOptions} />
        <MarkInspector
        tool={tool}
        color={markColor}
        onColor={onMarkColor}
        settings={settings}
        onSettings={onSettings}
      />
      </>
    );
  } else {
    body = <DocInspector info={info} pageSize={pageSize} units={settings.units} onSave={onSaveMeta} />;
  }
  return (
    <aside className="panel side is-inspector" aria-label={t("Inspector")}>
      <div className="side-body" style={{ paddingTop: "var(--space-200)" }}>
        {body}
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------

export function useFontOptions(fonts: FontSummary[] | null, catalog: FontCatalog | null) {
  return useMemo(() => {
    const out: { value: string; label: string }[] = [];
    for (const f of fonts ?? []) {
      if (f.editable) out.push({ value: `doc:${f.key}`, label: t("Document · {0}{1}", fontLabel(f), f.subset ? " (subset)" : "") });
    }
    for (const fam of catalog?.families ?? []) out.push({ value: `sys:${fam}`, label: t("Installed · {0}", fam) });
    for (const s of catalog?.standard ?? []) out.push({ value: `std:${s}`, label: t("Standard · {0}", s) });
    return out;
  }, [fonts, catalog]);
}

export function parseChoice(v: string, bold = false, italic = false): FontChoice {
  const [kind, ...rest] = v.split(":");
  const name = rest.join(":");
  if (kind === "doc") return { kind: "document", key: name };
  if (kind === "sys") return { kind: "system", family: name, bold, italic };
  return { kind: "standard", name };
}

function LinesInspector({
  docId,
  selection,
  fonts,
  catalog,
  units,
  onEdit,
  onStartEditing,
}: {
  docId: number;
  selection: Extract<Selection, { kind: "lines" }>;
  fonts: FontSummary[] | null;
  catalog: FontCatalog | null;
  units: Settings["units"];
  onEdit: (e: LineEdit) => void;
  onStartEditing: (what: "line" | "block") => void;
}) {
  const many = selection.lines.length > 1;
  const l = selection.lines[0];
  const editable = selection.lines.every((x) => x.editable);
  const font = fonts?.find((f) => f.key === l.fontKey) ?? null;
  const block = selection.block;
  const inParagraph = !many && block && block.lines.length > 1;
  const [stack, setStack] = useState(t("\"{0}\", sans-serif", l.family));
  const sizeText = (v: number) => v.toFixed(2).replace(/\.?0+$/, "");
  const [size, setSize] = useState(sizeText(l.size));
  const [text, setText] = useState(l.text);
  const options = useFontOptions(fonts, catalog);
  const [fontPick, setFontPick] = useState("");
  const [bold, setBold] = useState(false);
  const [italic, setItalic] = useState(false);
  useEffect(() => {
    setSize(sizeText(l.size));
    setText(l.text);
    setFontPick("");
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
        <span className="eyebrow">{many ? t("{0} lines", selection.lines.length) : inParagraph ? t("Line in a paragraph") : t("Text line")}</span>
        {!many ? (
          <div
            className="line-preview"
            style={{ fontFamily: stack, fontWeight: l.bold ? 700 : 400, fontStyle: l.italic ? "italic" : undefined, color: rgbToHex(l.color) }}
          >
            {l.text}
          </div>
        ) : (
          <div className="line-preview" style={{ fontSize: 13, fontFamily: "var(--font-body)" }}>
            {selection.lines.slice(0, 6).map((x) => (
              <div key={x.id} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {x.text}
              </div>
            ))}
            {selection.lines.length > 6 && <div className="is-muted">and {selection.lines.length - 6} more</div>}
          </div>
        )}
        {l.note && !many && (
          <div className="notice is-warning" style={{ marginTop: 8 }}>
            <Info size={14} aria-hidden="true" />
            <span>{l.note}</span>
          </div>
        )}
        {editable && !many && (
          <div style={{ marginTop: 12 }}>
            <TextField
              label={t("Text")}
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && text !== l.text) onEdit({ text });
              }}
            />
          </div>
        )}
        {editable && (
          <div className="inspector-actions">
            {!many && (
              <Button appearance="primary" spacing="compact" iconBefore={Save} disabled={text === l.text} onClick={() => onEdit({ text })}>
                {t("Apply")}
              </Button>
            )}
            {!many && (
              <Button spacing="compact" iconBefore={Pencil} onClick={() => onStartEditing("line")}>
                {t("Edit line")}
              </Button>
            )}
            {inParagraph && (
              <Button spacing="compact" iconBefore={Pilcrow} onClick={() => onStartEditing("block")} shortcut="Enter">
                {t("Edit paragraph")}
              </Button>
            )}
            <Button appearance="subtle" spacing="compact" iconBefore={Trash2} onClick={() => onEdit({ delete: true })} shortcut="Del">
              {t("Delete")}
            </Button>
          </div>
        )}
      </section>

      {!many && (
        <section className="side-section">
          <span className="eyebrow">{t("Fonts in this line")}</span>
          {l.spans.length > 1 ? (
            <div className="spans">
              {l.spans.map((s, i) => (
                <div key={i} className="span-row">
                  <span className="span-swatch" style={{ background: rgbToHex(s.color) }} />
                  <span className="span-text">{[...l.text].slice(s.start, s.end).join("")}</span>
                  <span className="is-muted">
                    {s.family}
                    {s.bold ? t(" Bold") : ""}
                    {s.italic ? t(" Italic") : ""} · {s.em.toFixed(1)}
                  </span>
                </div>
              ))}
              <p className="is-muted" style={{ fontSize: 12, marginTop: 6 }}>
                {t("Edits keep each word’s style: new text takes the style of the text it replaces.")}
              </p>
            </div>
          ) : (
            <dl className="props" style={{ marginTop: 8 }}>
              <dt>{t("Family")}</dt>
              <dd>{font ? fontLabel(font) : l.family}</dd>
              <dt>{t("PostScript")}</dt>
              <dd className="is-mono">{l.fontName}</dd>
              {font && (
                <>
                  <dt>{t("Type")}</dt>
                  <dd>
                    {font.kind} {font.subset && <Lozenge appearance="information">subset</Lozenge>}
                  </dd>
                  <dt>{t("Glyphs")}</dt>
                  <dd>{font.glyphs.toLocaleString()}</dd>
                  <dt>{t("Installed")}</dt>
                  <dd>{font.systemMatch ?? "no"}</dd>
                </>
              )}
            </dl>
          )}
        </section>
      )}

      {editable && (
        <section className="side-section">
          <span className="eyebrow">{t("Style")}</span>
          <div style={{ display: "grid", gap: 12, marginTop: 8 }}>
            <div className="row">
              <TextField
                label={many ? t("Size of first (pt)") : t("Size (pt)")}
                value={size}
                mono
                inputMode="decimal"
                onChange={(e) => setSize(e.target.value)}
                onBlur={applySize}
                onKeyDown={(e) => e.key === "Enter" && applySize()}
              />
              <label className="color-field">
                <span className="field-label">{t("Colour")}</span>
                <input
                  type="color"
                  className="color-input"
                  defaultValue={rgbToHex(l.color)}
                  key={`${l.id}-${rgbToHex(l.color)}`}
                  onChange={(e) => {
                    const v = e.target.value;
                    window.clearTimeout(colorTimer.current);
                    colorTimer.current = window.setTimeout(() => onEdit({ color: hexToRgb(v) }), 350);
                  }}
                />
              </label>
            </div>
            <Select
              label={t("Change font")}
              value={fontPick}
              options={[{ value: "", label: many ? t("Keep each line’s font") : t("Keep its font") }, ...options]}
              onChange={setFontPick}
            />
            {fontPick.startsWith("sys:") && (
              <div style={{ display: "flex", gap: 16 }}>
                <Toggle label={t("Bold")} checked={bold} onChange={setBold} />
                <Toggle label={t("Italic")} checked={italic} onChange={setItalic} />
              </div>
            )}
            {fontPick && (
              <div className="inspector-actions" style={{ marginTop: 0 }}>
                <Button appearance="primary" spacing="compact" onClick={() => onEdit({ font: parseChoice(fontPick, bold, italic) })}>
                  {t("Apply font")}
                </Button>
              </div>
            )}
          </div>
          {!many && (
            <p className="is-muted" style={{ marginTop: 8, fontSize: 12 }}>
              {t("Drawn at")}{" "}{l.em.toFixed(2)} pt{Math.abs(l.angle) > 0.5 ? t(", rotated {0}°", l.angle.toFixed(0)) : ""}.
            </p>
          )}
        </section>
      )}

      {!many && (
        <section className="side-section">
          <span className="eyebrow">{t("Position")}</span>
          <dl className="props" style={{ marginTop: 8 }}>
            <dt>x</dt>
            <dd className="is-mono">{length(l.origin[0], units)}</dd>
            <dt>y</dt>
            <dd className="is-mono">{length(l.origin[1], units)}</dd>
            <dt>{t("Page")}</dt>
            <dd>{selection.page + 1}</dd>
            {inParagraph && block && (
              <>
                <dt>{t("Paragraph")}</dt>
                <dd>
                  {block.lines.length} {t("lines,")}{" "}{block.align}, {length(block.width, units)} wide
                </dd>
              </>
            )}
          </dl>
        </section>
      )}
      {editable && (
        <p className="is-muted" style={{ marginTop: 12, fontSize: 12 }}>
          {t("Drag selected lines to move them; they snap to nearby edges and baselines (hold Alt to move freely). Arrow keys nudge by 1 pt, Shift+arrow by 10 pt. Shift-click or drag a box to select more lines.")}
        </p>
      )}
    </>
  );
}

const colorTimer = { current: 0 };

function Swatches({ value, onChange, allowNone }: { value: string | null; onChange: (c: string | null) => void; allowNone?: boolean }) {
  return (
    <div className="swatches" style={{ marginTop: 6 }}>
      {allowNone && (
        <button
          type="button"
          className={cx("swatch", "is-none", value === null && "is-selected")}
          aria-label={t("No fill")}
          data-tooltip={t("No fill")}
          onClick={() => onChange(null)}
        />
      )}
      {MARK_SWATCHES.map((c) => (
        <button
          key={c}
          type="button"
          className={cx("swatch", value !== null && c.toLowerCase() === value.toLowerCase() && "is-selected")}
          style={{ background: c }}
          aria-label={c}
          onClick={() => onChange(c)}
        />
      ))}
      <input
        type="color"
        className="color-input"
        style={{ width: 28, height: 24 }}
        value={value ?? "#ffffff"}
        onChange={(e) => onChange(e.target.value)}
        aria-label={t("Custom colour")}
      />
    </div>
  );
}

const ANNOT_LABELS: Record<string, string> = {
  Text: t("Sticky note"),
  Highlight: t("Highlight"),
  Underline: t("Underline"),
  StrikeOut: "Strike-out",
  Ink: t("Drawing"),
  Square: t("Rectangle"),
  Circle: t("Ellipse"),
  Line: t("Line"),
  Polygon: t("Polygon"),
  FreeText: t("Text box"),
  Stamp: t("Stamp"),
  Link: "Link",
};

function AnnotInspector({
  selection,
  onDelete,
  onUpdate,
}: {
  selection: Extract<Selection, { kind: "annot" }>;
  onDelete: () => void;
  onUpdate: (c: AnnotChange) => void;
}) {
  const a = selection.annot;
  const [contents, setContents] = useState(a.contents);
  const [target, setTarget] = useState(a.target ?? "");
  useEffect(() => {
    setContents(a.contents);
    setTarget(a.target ?? "");
  }, [a]);
  const isLink = a.subtype === "Link";
  const fillable = ["Square", "Circle", "Polygon", "FreeText"].includes(a.subtype);
  const stroked = ["Ink", "Square", "Circle", "Line", "Polygon"].includes(a.subtype);
  const asPage = /^\s*(page\s*)?(\d+)\s*$/i.exec(target);
  return (
    <>
      <section className="side-section">
        <span className="eyebrow">{ANNOT_LABELS[a.subtype] ?? a.subtype}</span>
        <dl className="props" style={{ marginTop: 8 }}>
          <dt>{t("Page")}</dt>
          <dd>{selection.page + 1}</dd>
          {a.author && (
            <>
              <dt>{t("Author")}</dt>
              <dd>{a.author}</dd>
            </>
          )}
          {isLink && (
            <>
              <dt>{t("Goes to")}</dt>
              <dd className="is-mono">{a.target ?? "—"}</dd>
            </>
          )}
        </dl>
        {!a.editable && !isLink && (
          <p className="is-muted" style={{ marginTop: 8, fontSize: 12 }}>
            {t("Made by another program: it can be moved and deleted, but its look cannot be redrawn here.")}
          </p>
        )}
      </section>

      {isLink ? (
        <section className="side-section">
          <TextField label={t("Web address or page")} value={target} onChange={(e) => setTarget(e.target.value)} placeholder={t("https://… or 12")} />
          <div className="inspector-actions">
            <Button
              appearance="primary"
              spacing="compact"
              disabled={!target.trim() || target === a.target}
              onClick={() =>
                asPage
                  ? onUpdate({ page: parseInt(asPage[2], 10) - 1 })
                  : onUpdate({ uri: /^[a-z][a-z0-9+.-]*:/i.test(target.trim()) ? target.trim() : `https://${target.trim()}` })
              }
            >
              {t("Change target")}
            </Button>
          </div>
        </section>
      ) : (
        a.editable && (
          <section className="side-section">
            <span className="eyebrow">{t("Look")}</span>
            <div style={{ display: "grid", gap: 12, marginTop: 8 }}>
              <div>
                <span className="field-label">{t("Colour")}</span>
                <Swatches value={a.color ? rgbToHex(a.color) : "#000000"} onChange={(c) => c && onUpdate({ color: hexToRgb(c) })} />
              </div>
              {fillable && (
                <div>
                  <span className="field-label">{t("Fill")}</span>
                  <Swatches value={a.fill ? rgbToHex(a.fill) : null} allowNone onChange={(c) => onUpdate({ fill: c ? hexToRgb(c) : null })} />
                </div>
              )}
              {stroked && (
                <TextField
                  label={t("Line width (pt)")}
                  defaultValue={String(a.width)}
                  key={`w-${a.index}-${a.width}`}
                  mono
                  onBlur={(e) => {
                    const v = parseFloat(e.target.value);
                    if (v > 0 && v <= 40 && v !== a.width) onUpdate({ width: v });
                  }}
                />
              )}
              <label className="field">
                <span className="field-label">{t("Opacity ·")}{" "}{Math.round(a.opacity * 100)}%</span>
                <input
                  type="range"
                  min={10}
                  max={100}
                  step={5}
                  defaultValue={Math.round(a.opacity * 100)}
                  key={`o-${a.index}-${a.opacity}`}
                  onPointerUp={(e) => onUpdate({ opacity: Number((e.target as HTMLInputElement).value) / 100 })}
                  onKeyUp={(e) => onUpdate({ opacity: Number((e.target as HTMLInputElement).value) / 100 })}
                />
              </label>
            </div>
          </section>
        )
      )}

      {!isLink && (
        <section className="side-section">
          <label className="field">
            <span className="field-label">{a.subtype === "FreeText" ? "Text" : a.subtype === "Stamp" ? t("Label") : t("Comment")}</span>
            <textarea className="text-area" value={contents} onChange={(e) => setContents(e.target.value)} rows={4} />
          </label>
          <div className="inspector-actions">
            <Button appearance="primary" spacing="compact" disabled={contents === a.contents} onClick={() => onUpdate({ contents })}>
              {t("Apply")}
            </Button>
          </div>
        </section>
      )}

      {a.removable && (
        <div className="inspector-actions">
          <Button appearance="danger" spacing="compact" iconBefore={Trash2} onClick={onDelete} shortcut="Del">
            {t("Delete")}
          </Button>
        </div>
      )}
      {a.editable && (
        <p className="is-muted" style={{ marginTop: 12, fontSize: 12 }}>
          {t("Drag it on the page to move it.")}
        </p>
      )}
    </>
  );
}

function ImageInspector({
  selection,
  units,
  onReplace,
  onExtract,
  onDelete,
}: {
  selection: Extract<Selection, { kind: "image" }>;
  units: Settings["units"];
  onReplace: () => void;
  onExtract: () => void;
  onDelete: () => void;
}) {
  const im = selection.image;
  const [x0, y0, x1, y1] = im.bbox;
  return (
    <section className="side-section">
      <span className="eyebrow">{t("Image")}</span>
      <dl className="props" style={{ marginTop: 8 }}>
        <dt>{t("Pixels")}</dt>
        <dd className="is-mono">
          {im.width} × {im.height}
        </dd>
        <dt>{t("Shown at")}</dt>
        <dd>
          {length(x1 - x0, units)} × {length(y1 - y0, units)}
        </dd>
        <dt>{t("Resolution")}</dt>
        <dd>
          {Math.round(im.dpi)} dpi {im.dpi < 150 && <Lozenge appearance="warning">{t("low for print")}</Lozenge>}
        </dd>
        <dt>{t("Stored as")}</dt>
        <dd>
          {im.filter === "DCTDecode" ? "JPEG" : im.filter || "uncompressed"} · {im.colorSpace || "—"} · {im.bits} bit
          {im.hasAlpha ? t(" · transparency") : ""}
        </dd>
        <dt>{t("Page")}</dt>
        <dd>{selection.page + 1}</dd>
      </dl>
      <div className="inspector-actions">
        <Button spacing="compact" iconBefore={ImageUp} onClick={onReplace}>
          {t("Replace…")}
        </Button>
        <Button spacing="compact" iconBefore={Download} onClick={onExtract} disabled={im.inline}>
          {t("Save image…")}
        </Button>
        <Button appearance="subtle" spacing="compact" iconBefore={Trash2} onClick={onDelete} shortcut="Del">
          {t("Delete")}
        </Button>
      </div>
      <p className="is-muted" style={{ marginTop: 12, fontSize: 12 }}>
        {t("Drag the image to move it, or its corner to resize it (hold Shift to change its proportions).")}
      </p>
    </section>
  );
}

export const STAMPS = [t("Approved"), t("Draft"), t("Confidential"), t("Final"), t("Reviewed"), t("Not approved"), t("Void"), t("For comment")];

function ToolOptionsInspector({
  tool,
  options,
  onOptions,
}: {
  tool: Tool;
  options: ToolOptions;
  onOptions: (o: ToolOptions) => void;
}) {
  if (tool === "stamp") {
    return (
      <section className="side-section">
        <Select label={t("Stamp")} value={options.stampLabel} options={STAMPS.map((s) => ({ value: s, label: s }))} onChange={(stampLabel) => onOptions({ ...options, stampLabel })} />
      </section>
    );
  }
  if (["rect", "ellipse", "polygon", "textbox"].includes(tool)) {
    return (
      <section className="side-section">
        <span className="field-label">{t("Fill")}</span>
        <Swatches value={options.fill} allowNone onChange={(fill) => onOptions({ ...options, fill })} />
        {tool === "textbox" && (
          <div style={{ marginTop: 12 }}>
            <TextField
              label={t("Text size (pt)")}
              defaultValue={String(options.textSize)}
              mono
              onBlur={(e) => {
                const v = parseFloat(e.target.value);
                if (v >= 4 && v <= 96) onOptions({ ...options, textSize: v });
              }}
            />
          </div>
        )}
      </section>
    );
  }
  return null;
}

function RedactInspector({
  count,
  fill,
  onFill,
  onFind,
  onApply,
  onClear,
}: {
  count: number;
  fill: "black" | "white" | "none";
  onFill: (f: "black" | "white" | "none") => void;
  onFind: (query: string) => void;
  onApply: () => void;
  onClear: () => void;
}) {
  const [query, setQuery] = useState("");
  const [confirm, setConfirm] = useState(false);
  useEffect(() => setConfirm(false), [count]);
  return (
    <>
      <section className="side-section">
        <span className="eyebrow">{t("Redaction")}</span>
        <p style={{ marginTop: 8 }}>
          {count === 0 ? t("No areas marked yet. Drag over the page to mark what must go.") : t("{0} area{1} marked.", count, count === 1 ? "" : "s")}
        </p>
        <div className="notice" style={{ marginTop: 10 }}>
          <Info size={14} aria-hidden="true" />
          <span>
            {t("Applying removes the text, image pixels, drawings and annotations under each mark from the file itself, not just from view.")}
          </span>
        </div>
      </section>
      <section className="side-section">
        <span className="eyebrow">{t("Mark every occurrence")}</span>
        <div className="row" style={{ marginTop: 8 }}>
          <TextField
            label={t("Text")}
            value={query}
            placeholder={t("A name, number…")}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && query.trim() && onFind(query.trim())}
          />
          <Button spacing="compact" disabled={!query.trim()} onClick={() => onFind(query.trim())}>
            {t("Mark all")}
          </Button>
        </div>
      </section>
      <section className="side-section">
        <Select
          label={t("Cover removed areas with")}
          value={fill}
          onChange={onFill}
          options={[
            { value: "black", label: t("A black box") },
            { value: "white", label: t("A white box") },
            { value: "none", label: t("Nothing (leave blank)") },
          ]}
        />
        <div className="inspector-actions">
          {!confirm ? (
            <Button appearance="danger" spacing="compact" iconBefore={EyeOff} disabled={count === 0} onClick={() => setConfirm(true)}>
              {t("Apply redactions…")}
            </Button>
          ) : (
            <>
              <Button appearance="danger" spacing="compact" onClick={onApply}>
                {t("Remove for good")}
              </Button>
              <Button appearance="subtle" spacing="compact" onClick={() => setConfirm(false)}>
                {t("Cancel")}
              </Button>
            </>
          )}
          <Button appearance="subtle" spacing="compact" disabled={count === 0} onClick={onClear}>
            {t("Clear marks")}
          </Button>
        </div>
        {confirm && (
          <p className="is-muted" style={{ marginTop: 8, fontSize: 12 }}>
            {t("Undo still works until you save; after saving, the removed content cannot be recovered from the file.")}
          </p>
        )}
      </section>
    </>
  );
}

function CropInspector({ onUncrop }: { onUncrop: (scope: "page" | "all") => void }) {
  return (
    <section className="side-section">
      <span className="eyebrow">{t("Crop")}</span>
      <p style={{ marginTop: 8 }}>{t("Drag the area to keep on the page. Cropping hides the rest; it is not deleted.")}</p>
      <div className="inspector-actions">
        <Button spacing="compact" onClick={() => onUncrop("page")}>
          {t("Remove crop from this page")}
        </Button>
        <Button appearance="subtle" spacing="compact" onClick={() => onUncrop("all")}>
          {t("From all pages")}
        </Button>
      </div>
    </section>
  );
}

function choiceKey(c: FontChoice): string {
  if (c.kind === "document") return t("doc:{0}", c.key);
  if (c.kind === "system") return t("sys:{0}", c.family);
  return t("std:{0}", c.name);
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
  const options = useFontOptions(fonts, catalog);

  const setFont = (v: string) => {
    const prevSys = style.font.kind === "system" ? style.font : null;
    onChange({ ...style, font: parseChoice(v, prevSys?.bold ?? false, prevSys?.italic ?? false) });
  };

  const [size, setSize] = useState(String(style.size));
  useEffect(() => setSize(String(style.size)), [style.size]);

  return (
    <section className="side-section">
      <span className="eyebrow">{t("New text")}</span>
      <div style={{ display: "grid", gap: 12, marginTop: 8 }}>
        <Select label={t("Font")} value={choiceKey(style.font)} options={options.length ? options : [{ value: choiceKey(style.font), label: t("Loading fonts…") }]} onChange={setFont} />
        {style.font.kind === "document" && (
          <p className="is-muted" style={{ fontSize: 12 }}>
            {t("Written in the document’s own font. Characters its subset lacks come from the same typeface if it is installed.")}
          </p>
        )}
        {style.font.kind === "system" && (
          <div style={{ display: "grid", gap: 8 }}>
            <Toggle
              label={t("Bold")}
              checked={style.font.bold}
              onChange={(v) => style.font.kind === "system" && onChange({ ...style, font: { ...style.font, bold: v } })}
            />
            <Toggle
              label={t("Italic")}
              checked={style.font.italic}
              onChange={(v) => style.font.kind === "system" && onChange({ ...style, font: { ...style.font, italic: v } })}
            />
          </div>
        )}
        <div className="row">
          <TextField
            label={t("Size (pt)")}
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
            <span className="field-label">{t("Colour")}</span>
            <input type="color" className="color-input" value={style.color} onChange={(e) => onChange({ ...style, color: e.target.value })} />
          </label>
        </div>
        <p className="is-muted" style={{ fontSize: 12 }}>
          {t("Click on the page to place text. Ctrl+Enter places it, Enter starts a new line.")}
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
    highlight: t("Highlight"),
    underline: t("Underline"),
    strikeout: "Strike-out",
    note: t("Sticky note"),
    ink: t("Drawing"),
    rect: t("Rectangle"),
    ellipse: t("Ellipse"),
    line: t("Line"),
    arrow: t("Arrow"),
    polygon: t("Polygon"),
    textbox: t("Text box"),
    stamp: t("Stamp"),
  };
  return (
    <section className="side-section">
      <span className="eyebrow">{title[tool]}</span>
      <div style={{ display: "grid", gap: 12, marginTop: 8 }}>
        <div>
          <span className="field-label">{t("Colour")}</span>
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
            <input type="color" className="color-input" style={{ width: 28, height: 24 }} value={color} onChange={(e) => onColor(e.target.value)} aria-label={t("Custom colour")} />
          </div>
        </div>
        {["ink", "rect", "ellipse", "line", "arrow", "polygon"].includes(tool) && (
          <TextField
            label={t("Line width (pt)")}
            value={String(settings.inkWidth)}
            mono
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              if (v > 0 && v <= 40) onSettings({ ...settings, inkWidth: v });
            }}
          />
        )}
        <TextField
          label={t("Author")}
          value={settings.author}
          placeholder={t("Shown on notes and highlights")}
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
        <span className="eyebrow">{t("Document")}</span>
        <dl className="props" style={{ marginTop: 8 }}>
          <dt>{t("File")}</dt>
          <dd>{info.state.name}</dd>
          {info.state.path && (
            <>
              <dt>{t("Folder")}</dt>
              <dd className="is-muted">{info.state.path.replace(/[\\/][^\\/]*$/, "")}</dd>
            </>
          )}
          <dt>{t("Pages")}</dt>
          <dd>{info.state.pages}</dd>
          <dt>{t("Version")}</dt>
          <dd>{t("PDF")}{" "}{info.version}</dd>
          {info.fileSize > 0 && (
            <>
              <dt>{t("Size")}</dt>
              <dd>{bytes(info.fileSize)}</dd>
            </>
          )}
          {pageSize && (
            <>
              <dt>{t("Page")}</dt>
              <dd>
                {length(pageSize.w, units)} × {length(pageSize.h, units)}
                {paperName(pageSize.w, pageSize.h) && <span className="is-muted"> · {paperName(pageSize.w, pageSize.h)}</span>}
              </dd>
            </>
          )}
          {info.meta.creator && (
            <>
              <dt>{t("Creator")}</dt>
              <dd>{info.meta.creator}</dd>
            </>
          )}
          {info.meta.producer && (
            <>
              <dt>{t("Producer")}</dt>
              <dd>{info.meta.producer}</dd>
            </>
          )}
          {info.meta.created && (
            <>
              <dt>{t("Created")}</dt>
              <dd>{pdfDate(info.meta.created)}</dd>
            </>
          )}
          {info.meta.modified && (
            <>
              <dt>{t("Modified")}</dt>
              <dd>{pdfDate(info.meta.modified)}</dd>
            </>
          )}
        </dl>
      </section>
      <section className="side-section">
        <span className="eyebrow">{t("Properties")}</span>
        <div style={{ display: "grid", gap: 10, marginTop: 8 }}>
          <TextField label={t("Title")} value={meta.title} onChange={(e) => setMeta({ ...meta, title: e.target.value })} />
          <TextField label={t("Author")} value={meta.author} onChange={(e) => setMeta({ ...meta, author: e.target.value })} />
          <TextField label={t("Subject")} value={meta.subject} onChange={(e) => setMeta({ ...meta, subject: e.target.value })} />
          <TextField label={t("Keywords")} value={meta.keywords} onChange={(e) => setMeta({ ...meta, keywords: e.target.value })} />
        </div>
        <div className="inspector-actions">
          <Button appearance="primary" spacing="compact" disabled={!changed} onClick={() => onSave(meta)}>
            {t("Apply properties")}
          </Button>
        </div>
      </section>
    </>
  );
}
