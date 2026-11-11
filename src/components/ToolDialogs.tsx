import { useEffect, useMemo, useState } from "react";
import { FolderOpen, Plus, Trash2 } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Modal } from "./ui/Modal";
import { Button, IconButton } from "./ui/Button";
import { Segmented, Select, TextField, Toggle } from "./ui/Field";
import { api, type Bookmark, type FontCatalog, type FontSummary, type LabelRange, type Mark, type MarkStyle } from "../lib/api";
import { hexToRgb } from "../lib/format";
import { parseChoice, useFontOptions } from "./Inspector";
import { t } from "../lib/i18n";

// ---------------------------------------------------------------------------
// Page scope
// ---------------------------------------------------------------------------

type Scope = "all" | "current" | "selected" | "range";

/** "1-3, 7, 10-" → page indices. */
export function parseRange(text: string, pages: number): number[] {
  const out = new Set<number>();
  for (const part of text.split(/[,;]/)) {
    const m = /^\s*(\d+)?\s*(-\s*(\d+)?)?\s*$/.exec(part);
    if (!m || (!m[1] && !m[3])) continue;
    const a = m[1] ? parseInt(m[1], 10) : 1;
    const b = m[2] ? (m[3] ? parseInt(m[3], 10) : pages) : a;
    for (let i = Math.max(1, Math.min(a, b)); i <= Math.min(pages, Math.max(a, b)); i++) out.add(i - 1);
  }
  return [...out].sort((x, y) => x - y);
}

export function ScopePicker({
  scope,
  onScope,
  range,
  onRange,
  selected,
}: {
  scope: Scope;
  onScope: (s: Scope) => void;
  range: string;
  onRange: (r: string) => void;
  selected: number;
}) {
  return (
    <div className="row" style={{ alignItems: "flex-end" }}>
      <Select<Scope>
        label={t("Pages")}
        value={scope}
        onChange={onScope}
        options={[
          { value: "all", label: t("All pages") },
          { value: "current", label: t("Current page") },
          { value: "selected", label: selected ? t("Selected pages ({0})", selected) : t("Selected pages (none)") },
          { value: "range", label: t("Page range…") },
        ]}
      />
      {scope === "range" && <TextField label={t("Range")} value={range} placeholder="1-3, 7, 10-" onChange={(e) => onRange(e.target.value)} />}
    </div>
  );
}

export function useScope(pages: number, current: number, selected: number[]) {
  const [scope, setScope] = useState<Scope>("all");
  const [range, setRange] = useState("");
  const list = useMemo(() => {
    if (scope === "all") return Array.from({ length: pages }, (_, i) => i);
    if (scope === "current") return [current];
    if (scope === "selected") return selected;
    return parseRange(range, pages);
  }, [scope, range, pages, current, selected]);
  return { scope, setScope, range, setRange, list };
}

function StyleFields({
  style,
  onStyle,
  fonts,
  catalog,
  showOpacity,
}: {
  style: { font: string; size: string; color: string; opacity: number };
  onStyle: (s: { font: string; size: string; color: string; opacity: number }) => void;
  fonts: FontSummary[] | null;
  catalog: FontCatalog | null;
  showOpacity?: boolean;
}) {
  const options = useFontOptions(fonts, catalog);
  return (
    <>
      <Select label={t("Font")} value={style.font} options={options.length ? options : [{ value: style.font, label: "Helvetica" }]} onChange={(font) => onStyle({ ...style, font })} />
      <div className="row">
        <TextField label={t("Size (pt)")} value={style.size} mono onChange={(e) => onStyle({ ...style, size: e.target.value })} />
        <label className="color-field">
          <span className="field-label">{t("Colour")}</span>
          <input type="color" className="color-input" value={style.color} onChange={(e) => onStyle({ ...style, color: e.target.value })} />
        </label>
      </div>
      {showOpacity && (
        <label className="field">
          <span className="field-label">{t("Opacity ·")}{" "}{Math.round(style.opacity * 100)}%</span>
          <input type="range" min={5} max={100} step={5} value={Math.round(style.opacity * 100)} onChange={(e) => onStyle({ ...style, opacity: Number(e.target.value) / 100 })} />
        </label>
      )}
    </>
  );
}

function toMarkStyle(s: { font: string; size: string; color: string; opacity: number }): MarkStyle {
  return { font: parseChoice(s.font), size: Math.max(2, parseFloat(s.size) || 10), color: hexToRgb(s.color), opacity: s.opacity };
}

// ---------------------------------------------------------------------------
// Page marks
// ---------------------------------------------------------------------------

export type MarksTab = "watermark" | "header" | "bates" | "labels";

export function PageMarksDialog({
  tab: initialTab,
  docId,
  revision,
  pages,
  current,
  selected,
  fonts,
  catalog,
  onApply,
  onSaveLabels,
  onClose,
}: {
  tab: MarksTab;
  docId: number;
  revision: number;
  pages: number;
  current: number;
  selected: number[];
  fonts: FontSummary[] | null;
  catalog: FontCatalog | null;
  onApply: (pages: number[], mark: Mark) => Promise<boolean>;
  onSaveLabels: (ranges: LabelRange[]) => Promise<boolean>;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<MarksTab>(initialTab);
  const { scope, setScope, range, setRange, list } = useScope(pages, current, selected);
  const [wm, setWm] = useState({ text: "CONFIDENTIAL", angle: "45", behind: false });
  const [wmStyle, setWmStyle] = useState({ font: "std:Helvetica-Bold", size: "64", color: "#c62828", opacity: 0.25 });
  const [hf, setHf] = useState({ h0: "", h1: "", h2: "{date}", f0: "{file}", f1: "", f2: t("Page {page} of {pages}"), margin: "28", first: "1" });
  const [hfStyle, setHfStyle] = useState({ font: "std:Helvetica", size: "9", color: "#333333", opacity: 1 });
  const [bates, setBates] = useState({ prefix: "", suffix: "", start: "1", digits: "6", position: "bottom-right", margin: "20" });
  const [batesStyle, setBatesStyle] = useState({ font: "std:Helvetica-Bold", size: "9", color: "#000000", opacity: 1 });
  const [busy, setBusy] = useState(false);

  const apply = async () => {
    let mark: Mark;
    if (tab === "watermark") {
      mark = { kind: "watermark", text: wm.text, style: toMarkStyle(wmStyle), angle: parseFloat(wm.angle) || 0, behind: wm.behind };
    } else if (tab === "header") {
      mark = {
        kind: "headerFooter",
        header: [hf.h0, hf.h1, hf.h2],
        footer: [hf.f0, hf.f1, hf.f2],
        style: toMarkStyle(hfStyle),
        margin: parseFloat(hf.margin) || 28,
        firstNumber: parseInt(hf.first, 10) || 1,
      };
    } else {
      mark = {
        kind: "bates",
        prefix: bates.prefix,
        suffix: bates.suffix,
        start: Math.max(0, parseInt(bates.start, 10) || 1),
        digits: Math.min(12, Math.max(1, parseInt(bates.digits, 10) || 6)),
        position: bates.position,
        style: toMarkStyle(batesStyle),
        margin: parseFloat(bates.margin) || 20,
      };
    }
    setBusy(true);
    const ok = await onApply(list, mark);
    setBusy(false);
    if (ok) onClose();
  };

  const batesPreview = `${bates.prefix}${String(parseInt(bates.start, 10) || 1).padStart(parseInt(bates.digits, 10) || 6, "0")}${bates.suffix}`;

  return (
    <Modal
      title={t("Page marks")}
      onClose={onClose}
      width="large"
      closeOnBlanket={false}
      footer={
        tab === "labels" ? undefined : (
          <>
            <Button appearance="subtle" onClick={onClose}>
              {t("Cancel")}
            </Button>
            <Button appearance="primary" loading={busy} disabled={!list.length || (tab === "watermark" && !wm.text.trim())} onClick={apply}>
              {t("Add to")}{" "}{list.length === pages ? t("all pages") : t("{0} page{1}", list.length, list.length === 1 ? "" : "s")}
            </Button>
          </>
        )
      }
    >
      <div style={{ display: "grid", gap: 16 }}>
        <Segmented<MarksTab>
          label={t("Kind of mark")}
          value={tab}
          onChange={setTab}
          options={[
            { value: "watermark", label: t("Watermark") },
            { value: "header", label: t("Header & footer") },
            { value: "bates", label: t("Bates numbers") },
            { value: "labels", label: t("Page labels") },
          ]}
        />
        {tab === "labels" ? (
          <LabelsEditor docId={docId} revision={revision} pages={pages} onSave={onSaveLabels} />
        ) : (
          <>
            <ScopePicker scope={scope} onScope={setScope} range={range} onRange={setRange} selected={selected.length} />
            {tab === "watermark" && (
              <>
                <TextField label={t("Text")} value={wm.text} onChange={(e) => setWm({ ...wm, text: e.target.value })} />
                <StyleFields style={wmStyle} onStyle={setWmStyle} fonts={fonts} catalog={catalog} showOpacity />
                <div className="row">
                  <TextField label={t("Angle (°)")} value={wm.angle} mono onChange={(e) => setWm({ ...wm, angle: e.target.value })} />
                  <div className="field" style={{ justifyContent: "flex-end" }}>
                    <Toggle label={t("Behind the page content")} checked={wm.behind} onChange={(behind) => setWm({ ...wm, behind })} />
                  </div>
                </div>
              </>
            )}
            {tab === "header" && (
              <>
                <div className="grid-3">
                  <TextField label={t("Header left")} value={hf.h0} onChange={(e) => setHf({ ...hf, h0: e.target.value })} />
                  <TextField label={t("Header centre")} value={hf.h1} onChange={(e) => setHf({ ...hf, h1: e.target.value })} />
                  <TextField label={t("Header right")} value={hf.h2} onChange={(e) => setHf({ ...hf, h2: e.target.value })} />
                  <TextField label={t("Footer left")} value={hf.f0} onChange={(e) => setHf({ ...hf, f0: e.target.value })} />
                  <TextField label={t("Footer centre")} value={hf.f1} onChange={(e) => setHf({ ...hf, f1: e.target.value })} />
                  <TextField label={t("Footer right")} value={hf.f2} onChange={(e) => setHf({ ...hf, f2: e.target.value })} />
                </div>
                <p className="is-muted" style={{ fontSize: 12 }}>
                  {t("Placeholders:")}{" "}<code>{"{page}"}</code> {t("page number,")}{" "}<code>{"{pages}"}</code> {t("page count,")}{" "}<code>{"{date}"}</code> {t("today,")}{" "}
                  <code>{"{file}"}</code> {t("file name.")}
                </p>
                <StyleFields style={hfStyle} onStyle={setHfStyle} fonts={fonts} catalog={catalog} />
                <div className="row">
                  <TextField label={t("Margin (pt)")} value={hf.margin} mono onChange={(e) => setHf({ ...hf, margin: e.target.value })} />
                  <TextField label={t("First page number")} value={hf.first} mono onChange={(e) => setHf({ ...hf, first: e.target.value })} />
                </div>
              </>
            )}
            {tab === "bates" && (
              <>
                <div className="grid-3">
                  <TextField label={t("Prefix")} value={bates.prefix} placeholder={t("ACME-")} onChange={(e) => setBates({ ...bates, prefix: e.target.value })} />
                  <TextField label={t("Start at")} value={bates.start} mono onChange={(e) => setBates({ ...bates, start: e.target.value })} />
                  <TextField label={t("Digits")} value={bates.digits} mono onChange={(e) => setBates({ ...bates, digits: e.target.value })} />
                  <TextField label={t("Suffix")} value={bates.suffix} onChange={(e) => setBates({ ...bates, suffix: e.target.value })} />
                  <Select
                    label={t("Position")}
                    value={bates.position}
                    onChange={(position) => setBates({ ...bates, position })}
                    options={["top-left", "top-center", "top-right", "bottom-left", "bottom-center", "bottom-right"].map((v) => ({ value: v, label: v.replace("-", " ") }))}
                  />
                  <TextField label={t("Margin (pt)")} value={bates.margin} mono onChange={(e) => setBates({ ...bates, margin: e.target.value })} />
                </div>
                <p className="is-muted" style={{ fontSize: 12 }}>
                  {t("First number:")}{" "}<code>{batesPreview}</code>
                </p>
                <StyleFields style={batesStyle} onStyle={setBatesStyle} fonts={fonts} catalog={catalog} />
              </>
            )}
          </>
        )}
      </div>
    </Modal>
  );
}

const LABEL_STYLES: { value: LabelRange["style"]; label: string }[] = [
  { value: "D", label: "1, 2, 3" },
  { value: "r", label: t("i, ii, iii") },
  { value: "R", label: t("I, II, III") },
  { value: "a", label: "a, b, c" },
  { value: "A", label: "A, B, C" },
  { value: "", label: t("Prefix only") },
];

function LabelsEditor({ docId, revision, pages, onSave }: { docId: number; revision: number; pages: number; onSave: (r: LabelRange[]) => Promise<boolean> }) {
  const [rows, setRows] = useState<{ start: string; style: LabelRange["style"]; prefix: string; first: string }[] | null>(null);
  const [preview, setPreview] = useState<string[]>([]);
  useEffect(() => {
    api.labels(docId).then((l) => {
      setRows(l.ranges.map((r) => ({ start: String(r.start + 1), style: r.style, prefix: r.prefix, first: String(r.first) })));
      setPreview(l.strings);
    });
  }, [docId, revision]);
  if (!rows) return null;
  const ranges: LabelRange[] = rows
    .map((r) => ({ start: Math.max(0, (parseInt(r.start, 10) || 1) - 1), style: r.style, prefix: r.prefix, first: parseInt(r.first, 10) || 1 }))
    .filter((r) => r.start < pages);
  return (
    <div style={{ display: "grid", gap: 12 }}>
      <p className="is-muted" style={{ fontSize: 12 }}>
        {t("Page labels are the page numbers readers show, like i–iv for a preface and 1, 2, 3 after it. They do not change what is printed on the pages.")}
      </p>
      {rows.length === 0 && <p>{t("This document uses plain page numbers.")}</p>}
      {rows.map((r, i) => (
        <div key={i} className="row">
          <TextField label={t("From page")} value={r.start} mono onChange={(e) => setRows(rows.map((x, k) => (k === i ? { ...x, start: e.target.value } : x)))} />
          <Select label={t("Style")} value={r.style} options={LABEL_STYLES} onChange={(style) => setRows(rows.map((x, k) => (k === i ? { ...x, style } : x)))} />
          <TextField label={t("Prefix")} value={r.prefix} onChange={(e) => setRows(rows.map((x, k) => (k === i ? { ...x, prefix: e.target.value } : x)))} />
          <TextField label={t("Count from")} value={r.first} mono onChange={(e) => setRows(rows.map((x, k) => (k === i ? { ...x, first: e.target.value } : x)))} />
          <IconButton icon={Trash2} label={t("Remove this range")} onClick={() => setRows(rows.filter((_, k) => k !== i))} />
        </div>
      ))}
      <div className="inspector-actions" style={{ marginTop: 0 }}>
        <Button spacing="compact" iconBefore={Plus} onClick={() => setRows([...rows, { start: String(rows.length ? pages : 1), style: "D", prefix: "", first: "1" }])}>
          {t("Add range")}
        </Button>
        <span className="spacer" />
        <Button appearance="primary" spacing="compact" onClick={() => onSave(ranges)}>
          {t("Save labels")}
        </Button>
      </div>
      {preview.length > 0 && (
        <p className="is-muted" style={{ fontSize: 12 }}>
          {t("Now:")}{" "}{preview.slice(0, 14).join(", ")}
          {preview.length > 14 ? " …" : ""}
        </p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Resize pages
// ---------------------------------------------------------------------------

const SIZES: Record<string, [number, number]> = {
  A3: [842, 1191],
  A4: [595, 842],
  A5: [420, 595],
  Letter: [612, 792],
  Legal: [612, 1008],
  Tabloid: [792, 1224],
};

export function ResizeDialog({
  pages,
  current,
  selected,
  onApply,
  onClose,
}: {
  pages: number;
  current: number;
  selected: number[];
  onApply: (pages: number[], w: number, h: number) => Promise<boolean>;
  onClose: () => void;
}) {
  const { scope, setScope, range, setRange, list } = useScope(pages, current, selected);
  const [size, setSize] = useState("A4");
  const [landscape, setLandscape] = useState(false);
  const [custom, setCustom] = useState({ w: "210", h: "297" });
  let [w, h] = size === "custom" ? [(parseFloat(custom.w) / 25.4) * 72, (parseFloat(custom.h) / 25.4) * 72] : SIZES[size];
  if (landscape) [w, h] = [h, w];
  return (
    <Modal
      title={t("Resize pages")}
      onClose={onClose}
      width="medium"
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            {t("Cancel")}
          </Button>
          <Button appearance="primary" disabled={!list.length || !(w > 0 && h > 0)} onClick={async () => (await onApply(list, w, h)) && onClose()}>
            {t("Resize")}{" "}{list.length === pages ? t("all pages") : t("{0} page{1}", list.length, list.length === 1 ? "" : "s")}
          </Button>
        </>
      }
    >
      <div style={{ display: "grid", gap: 16 }}>
        <ScopePicker scope={scope} onScope={setScope} range={range} onRange={setRange} selected={selected.length} />
        <Select label={t("Paper size")} value={size} onChange={setSize} options={[...Object.keys(SIZES).map((k) => ({ value: k, label: k })), { value: "custom", label: t("Custom…") }]} />
        {size === "custom" && (
          <div className="row">
            <TextField label={t("Width (mm)")} value={custom.w} mono onChange={(e) => setCustom({ ...custom, w: e.target.value })} />
            <TextField label={t("Height (mm)")} value={custom.h} mono onChange={(e) => setCustom({ ...custom, h: e.target.value })} />
          </div>
        )}
        <Toggle label={t("Landscape")} checked={landscape} onChange={setLandscape} />
        <p className="is-muted" style={{ fontSize: 12 }}>
          {t("The content is scaled to fit and centred; annotations move with it.")}
        </p>
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Split document
// ---------------------------------------------------------------------------

type SplitMode = "every" | "selected" | "bookmarks" | "ranges";

export function SplitDialog({
  docId,
  pages,
  selected,
  onSplit,
  onClose,
}: {
  docId: number;
  pages: number;
  selected: number[];
  onSplit: (ranges: number[][], folder: string) => Promise<boolean>;
  onClose: () => void;
}) {
  const [mode, setMode] = useState<SplitMode>("every");
  const [every, setEvery] = useState("1");
  const [ranges, setRanges] = useState("");
  const [outline, setOutline] = useState<Bookmark[]>([]);
  useEffect(() => {
    api.outline(docId).then(setOutline).catch(() => {});
  }, [docId]);
  const starts = (list: number[]) => {
    const s = [...new Set([0, ...list.filter((p) => p > 0 && p < pages)])].sort((a, b) => a - b);
    return s.map((a, i) => Array.from({ length: (s[i + 1] ?? pages) - a }, (_, k) => a + k));
  };
  const parts: number[][] = useMemo(() => {
    if (mode === "every") {
      const n = Math.max(1, parseInt(every, 10) || 1);
      return starts(Array.from({ length: Math.ceil(pages / n) }, (_, i) => i * n));
    }
    if (mode === "selected") return starts(selected);
    if (mode === "bookmarks") return starts(outline.map((b) => b.page ?? 0));
    return ranges
      .split(";")
      .map((r) => parseRange(r, pages))
      .filter((r) => r.length);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, every, ranges, selected, outline, pages]);
  return (
    <Modal
      title={t("Split document")}
      onClose={onClose}
      width="medium"
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            {t("Cancel")}
          </Button>
          <Button
            appearance="primary"
            iconBefore={FolderOpen}
            disabled={parts.length < 1}
            onClick={async () => {
              const folder = await openDialog({ directory: true }).catch(() => null);
              if (typeof folder === "string" && (await onSplit(parts, folder))) onClose();
            }}
          >
            {t("Save")}{" "}{parts.length} file{parts.length === 1 ? "" : "s"}…
          </Button>
        </>
      }
    >
      <div style={{ display: "grid", gap: 16 }}>
        <Select<SplitMode>
          label={t("Split")}
          value={mode}
          onChange={setMode}
          options={[
            { value: "every", label: t("Every few pages") },
            { value: "selected", label: t("Before each selected page") },
            { value: "bookmarks", label: t("At top-level bookmarks") },
            { value: "ranges", label: t("Into page ranges") },
          ]}
        />
        {mode === "every" && <TextField label={t("Pages per file")} value={every} mono onChange={(e) => setEvery(e.target.value)} />}
        {mode === "selected" && <p className="is-muted">{selected.length ? t("{0} pages selected in the sidebar.", selected.length) : t("Select pages in the sidebar first.")}</p>}
        {mode === "bookmarks" && <p className="is-muted">{outline.length ? t("{0} top-level bookmarks.", outline.length) : t("This document has no bookmarks.")}</p>}
        {mode === "ranges" && <TextField label={t("Ranges, one file each")} value={ranges} placeholder="1-3; 4-10; 11-" onChange={(e) => setRanges(e.target.value)} />}
        <p className="is-muted" style={{ fontSize: 12 }}>
          {parts.length} file{parts.length === 1 ? "" : "s"}:{" "}
          {parts
            .slice(0, 8)
            .map((p) => (p.length === 1 ? `${p[0] + 1}` : `${p[0] + 1}–${p[p.length - 1] + 1}`))
            .join(", ")}
          {parts.length > 8 ? " …" : ""}
        </p>
      </div>
    </Modal>
  );
}
