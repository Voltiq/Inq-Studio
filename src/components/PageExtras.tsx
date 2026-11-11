import { useEffect, useMemo, useRef, useState } from "react";
import type { AnnotChange, AnnotInfo, FieldInfo, ImageInfo, NewAnnot, Point, Rect } from "../lib/api";
import { api } from "../lib/api";
import type { PageViewport } from "../lib/pdf";
import { hexToRgb } from "../lib/format";
import type { Tool } from "../lib/types";
import { cx } from "../lib/utils";
import { Button } from "./ui/Button";
import { Kbd } from "./ui/Feedback";
import type { Selection } from "./Viewer";

export interface ExtraActions {
  addAnnot: (page: number, annot: NewAnnot) => Promise<boolean>;
  updateAnnot: (page: number, index: number, change: AnnotChange) => Promise<boolean>;
  setField: (name: string, value: string) => Promise<boolean>;
  placeImage: (page: number, index: number, bbox: Rect) => Promise<boolean>;
  insertImage: (page: number, at: Point) => Promise<boolean>;
  crop: (page: number, area: Rect, scope: "page" | "all" | "selected") => Promise<boolean>;
}

export interface ToolOptions {
  stampLabel: string;
  fill: string | null;
  textSize: number;
}

/** Tools whose clicks and drags this layer handles. */
export const EXTRA_TOOLS: Tool[] = ["image", "ellipse", "line", "arrow", "polygon", "textbox", "stamp", "link", "redact", "crop"];

type View = { left: number; top: number; width: number; height: number };

function viewRect(vp: PageViewport, r: Rect): View {
  const v = vp.convertToViewportRectangle(r) as number[];
  return { left: Math.min(v[0], v[2]), top: Math.min(v[1], v[3]), width: Math.abs(v[2] - v[0]), height: Math.abs(v[3] - v[1]) };
}

function pdfRect(vp: PageViewport, x0: number, y0: number, x1: number, y1: number): Rect {
  const a = vp.convertToPdfPoint(x0, y0) as number[];
  const b = vp.convertToPdfPoint(x1, y1) as number[];
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
}

// ---------------------------------------------------------------------------
// Form fields (select tool)
// ---------------------------------------------------------------------------

export function FieldsLayer({ vp, page, fields, actions }: { vp: PageViewport; page: number; fields: FieldInfo[]; actions: ExtraActions }) {
  const here = fields.flatMap((f) => f.widgets.map((w, wi) => ({ f, w, wi }))).filter((x) => x.w.page === page);
  if (!here.length) return null;
  return (
    <div className="overlay fields-layer" style={{ pointerEvents: "none" }}>
      {here.map(({ f, w, wi }) => {
        const r = viewRect(vp, w.rect);
        const style = { ...r, fontSize: Math.max(9, Math.min(r.height * 0.62, 16)) };
        const key = `${f.name}-${wi}`;
        const tip = `${f.name}${f.required ? " (required)" : ""}${f.readonly ? " (read-only)" : ""}`;
        if (f.kind === "text") return <TextFieldInput key={key} field={f} style={style} tip={tip} onCommit={(v) => actions.setField(f.name, v)} />;
        if (f.kind === "checkbox" || f.kind === "radio") {
          return (
            <button
              key={key}
              type="button"
              className={cx("field-widget", "is-toggle", w.checked && "is-checked", f.kind === "radio" && "is-radio")}
              style={style}
              data-tooltip={tip}
              disabled={f.readonly}
              aria-pressed={w.checked}
              aria-label={f.name}
              onClick={() => actions.setField(f.name, f.kind === "checkbox" ? (w.checked ? "Off" : w.onState ?? "Yes") : w.onState ?? "")}
            />
          );
        }
        if (f.kind === "combo" || f.kind === "list") {
          return (
            <select
              key={key}
              className="field-widget"
              style={style}
              value={f.value}
              disabled={f.readonly}
              aria-label={f.name}
              data-tooltip={tip}
              onChange={(e) => actions.setField(f.name, e.target.value)}
            >
              {!f.options.includes(f.value) && <option value={f.value}>{f.value}</option>}
              {f.options.map((o) => (
                <option key={o} value={o}>
                  {o}
                </option>
              ))}
            </select>
          );
        }
        return <div key={key} className="field-widget is-static" style={style} data-tooltip={`${tip} · ${f.kind}`} />;
      })}
    </div>
  );
}

function TextFieldInput({ field, style, tip, onCommit }: { field: FieldInfo; style: React.CSSProperties; tip: string; onCommit: (v: string) => void }) {
  const [value, setValue] = useState(field.value);
  useEffect(() => setValue(field.value), [field.value]);
  const commit = () => value !== field.value && onCommit(value);
  const common = {
    className: "field-widget",
    style,
    value,
    disabled: field.readonly,
    maxLength: field.maxLen ?? undefined,
    "aria-label": field.name,
    "data-tooltip": tip,
    spellCheck: false,
    onBlur: commit,
  };
  return field.multiline ? (
    <textarea {...common} onChange={(e) => setValue(e.target.value)} onKeyDown={(e) => e.key === "Escape" && setValue(field.value)} />
  ) : (
    <input
      {...common}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") setValue(field.value);
      }}
    />
  );
}

// ---------------------------------------------------------------------------
// Annotations (select tool): select and drag
// ---------------------------------------------------------------------------

const TEXT_MARKUP = new Set(["Highlight", "Underline", "StrikeOut", "Squiggly", "Link", "Widget", "Popup"]);

export function AnnotsLayer({
  vp,
  page,
  annots,
  selection,
  onSelect,
  actions,
  showLinks,
}: {
  vp: PageViewport;
  page: number;
  annots: AnnotInfo[];
  selection: Selection;
  onSelect: (s: Selection) => void;
  actions: ExtraActions;
  showLinks: boolean;
}) {
  const [drag, setDrag] = useState<{ index: number; x0: number; y0: number; dx: number; dy: number } | null>(null);
  const shown = annots.filter((a) => (showLinks ? a.subtype === "Link" : !TEXT_MARKUP.has(a.subtype)));
  return (
    <div className="overlay" style={{ pointerEvents: "none" }}>
      {shown.map((a) => {
        const r = viewRect(vp, a.rect);
        const selected = selection?.kind === "annot" && selection.page === page && selection.annot.index === a.index;
        const d = drag?.index === a.index ? drag : null;
        return (
          <div
            key={a.index}
            className={cx("annot-box", selected && "is-selected", a.subtype === "Link" && "is-link")}
            style={{ ...r, left: r.left + (d?.dx ?? 0), top: r.top + (d?.dy ?? 0), pointerEvents: "auto", cursor: selected && a.editable ? "move" : "pointer" }}
            data-tooltip={a.subtype === "Link" ? a.target ?? "Link" : a.contents || a.subtype}
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              e.stopPropagation();
              if (!selected) {
                onSelect({ kind: "annot", page, annot: a });
                return;
              }
              if (!a.editable) return;
              (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
              setDrag({ index: a.index, x0: e.clientX, y0: e.clientY, dx: 0, dy: 0 });
            }}
            onPointerMove={(e) => drag && setDrag({ ...drag, dx: e.clientX - drag.x0, dy: e.clientY - drag.y0 })}
            onPointerUp={async () => {
              if (!drag) return;
              const m = drag;
              setDrag(null);
              if (Math.hypot(m.dx, m.dy) < 2) return;
              const p0 = vp.convertToPdfPoint(0, 0) as number[];
              const p1 = vp.convertToPdfPoint(m.dx, m.dy) as number[];
              await actions.updateAnnot(page, a.index, { dx: p1[0] - p0[0], dy: p1[1] - p0[1] });
            }}
          />
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Images (edit tool): select, move, resize
// ---------------------------------------------------------------------------

export function ImagesLayer({
  vp,
  page,
  docId,
  revision,
  selection,
  onSelect,
  actions,
}: {
  vp: PageViewport;
  page: number;
  docId: number;
  revision: number;
  selection: Selection;
  onSelect: (s: Selection) => void;
  actions: ExtraActions;
}) {
  const [images, setImages] = useState<ImageInfo[]>([]);
  const [drag, setDrag] = useState<{ index: number; mode: "move" | "resize"; x0: number; y0: number; dx: number; dy: number; keep: boolean } | null>(null);
  useEffect(() => {
    let live = true;
    api
      .images(docId, page)
      .then((l) => live && setImages(l))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [docId, page, revision]);
  if (!images.length) return null;
  return (
    <div className="overlay" style={{ pointerEvents: "none" }}>
      {images.map((im) => {
        const r = viewRect(vp, im.bbox);
        const selected = selection?.kind === "image" && selection.page === page && selection.image.index === im.index;
        const d = drag?.index === im.index ? drag : null;
        let { left, top, width, height } = r;
        if (d?.mode === "move") {
          left += d.dx;
          top += d.dy;
        } else if (d?.mode === "resize") {
          width = Math.max(8, width + d.dx);
          height = d.keep ? (width * r.height) / r.width : Math.max(8, height + d.dy);
        }
        const start = (e: React.PointerEvent, mode: "move" | "resize") => {
          e.stopPropagation();
          (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
          setDrag({ index: im.index, mode, x0: e.clientX, y0: e.clientY, dx: 0, dy: 0, keep: !e.shiftKey });
        };
        const finish = async () => {
          if (!d) return;
          setDrag(null);
          if (Math.hypot(d.dx, d.dy) < 2) return;
          await actions.placeImage(page, im.index, pdfRect(vp, left, top, left + width, top + height));
        };
        return (
          <div
            key={im.index}
            className={cx("image-box", selected && "is-selected")}
            style={{ left, top, width, height, pointerEvents: "auto" }}
            data-tooltip={`${im.width} × ${im.height} px · ${Math.round(im.dpi)} dpi`}
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              if (!selected) {
                e.stopPropagation();
                onSelect({ kind: "image", page, image: im });
                return;
              }
              start(e, "move");
            }}
            onPointerMove={(e) => drag && setDrag({ ...drag, dx: e.clientX - drag.x0, dy: e.clientY - drag.y0, keep: !e.shiftKey })}
            onPointerUp={finish}
          >
            {selected && (
              <span
                className="resize-handle"
                aria-label="Resize"
                onPointerDown={(e) => start(e, "resize")}
                onPointerMove={(e) => drag && setDrag({ ...drag, dx: e.clientX - drag.x0, dy: e.clientY - drag.y0, keep: !e.shiftKey })}
                onPointerUp={finish}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shapes, text boxes, stamps, links, redaction and crop
// ---------------------------------------------------------------------------

export function ShapesLayer({
  vp,
  page,
  tool,
  color,
  width,
  author,
  options,
  redactions,
  onRedactions,
  actions,
}: {
  vp: PageViewport;
  page: number;
  tool: Tool;
  color: string;
  width: number;
  author: string;
  options: ToolOptions;
  redactions: Rect[];
  onRedactions: (rects: Rect[]) => void;
  actions: ExtraActions;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const [poly, setPoly] = useState<[number, number][]>([]);
  const [hover, setHover] = useState<[number, number] | null>(null);
  const [pending, setPending] = useState<{ kind: "link" | "textbox" | "crop"; box: [number, number, number, number] } | null>(null);
  const active = EXTRA_TOOLS.includes(tool);
  const rgb = hexToRgb(color);
  const fill = options.fill ? hexToRgb(options.fill) : undefined;
  const toPdf = (x: number, y: number): Point => vp.convertToPdfPoint(x, y) as Point;
  const local = (e: React.PointerEvent | React.MouseEvent): [number, number] => {
    const r = ref.current!.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  useEffect(() => {
    setPoly([]);
    setPending(null);
  }, [tool]);

  const finishPolygon = async () => {
    if (poly.length >= 3) await actions.addAnnot(page, { kind: "polygon", points: poly.map(([x, y]) => toPdf(x, y)), color: rgb, fill, width, author });
    setPoly([]);
  };

  useEffect(() => {
    if (tool !== "polygon") return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter") finishPolygon();
      if (e.key === "Escape") setPoly([]);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const redactViews = useMemo(() => redactions.map((r) => viewRect(vp, r)), [redactions, vp]);

  const onDown = async (e: React.PointerEvent) => {
    if (e.button !== 0 || pending) return;
    const [x, y] = local(e);
    if (tool === "image") {
      await actions.insertImage(page, toPdf(x, y));
      return;
    }
    if (tool === "stamp") {
      const w = 170;
      const h = 52;
      const r = pdfRect(vp, x - w / 2, y - h / 2, x + w / 2, y + h / 2);
      await actions.addAnnot(page, { kind: "stamp", rect: r, label: options.stampLabel, color: rgb, author });
      return;
    }
    if (tool === "polygon") {
      setPoly((p) => [...p, [x, y]]);
      return;
    }
    ref.current!.setPointerCapture(e.pointerId);
    setDrag({ x0: x, y0: y, x1: x, y1: y });
  };

  const onUp = async () => {
    if (!drag) return;
    const d = drag;
    setDrag(null);
    const box: [number, number, number, number] = [Math.min(d.x0, d.x1), Math.min(d.y0, d.y1), Math.max(d.x0, d.x1), Math.max(d.y0, d.y1)];
    const big = box[2] - box[0] > 4 && box[3] - box[1] > 4;
    if (tool === "line" || tool === "arrow") {
      if (Math.hypot(d.x1 - d.x0, d.y1 - d.y0) > 4)
        await actions.addAnnot(page, { kind: tool, points: [toPdf(d.x0, d.y0), toPdf(d.x1, d.y1)], color: rgb, width, author });
      return;
    }
    if (!big) return;
    const r = pdfRect(vp, ...box);
    if (tool === "ellipse") await actions.addAnnot(page, { kind: "ellipse", rect: r, color: rgb, fill, width, author });
    else if (tool === "redact") onRedactions([...redactions, r]);
    else if (tool === "link" || tool === "textbox" || tool === "crop") setPending({ kind: tool, box });
  };

  return (
    <>
      {/* Redaction marks show whatever the tool, until applied. */}
      {redactViews.length > 0 && (
        <div className="overlay" style={{ pointerEvents: "none" }}>
          {redactViews.map((r, i) => (
            <div key={i} className="redact-mark" style={r}>
              {tool === "redact" && (
                <button
                  type="button"
                  className="redact-remove"
                  style={{ pointerEvents: "auto" }}
                  aria-label="Remove this mark"
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={() => onRedactions(redactions.filter((_, k) => k !== i))}
                >
                  ×
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      {active && (
        <div
          ref={ref}
          className="overlay is-crosshair"
          onPointerDown={onDown}
          onPointerMove={(e) => {
            const [x, y] = local(e);
            if (drag) setDrag({ ...drag, x1: x, y1: y });
            if (tool === "polygon") setHover([x, y]);
          }}
          onPointerUp={onUp}
          onDoubleClick={() => tool === "polygon" && finishPolygon()}
        >
          {drag && (tool === "line" || tool === "arrow") && (
            <svg className="ink-layer">
              <line x1={drag.x0} y1={drag.y0} x2={drag.x1} y2={drag.y1} stroke={color} strokeWidth={Math.max(1, width)} />
            </svg>
          )}
          {drag && tool !== "line" && tool !== "arrow" && (
            <div
              className={cx("drag-rect", tool === "ellipse" && "is-ellipse", tool === "redact" && "is-redact", tool === "crop" && "is-crop")}
              style={{
                left: Math.min(drag.x0, drag.x1),
                top: Math.min(drag.y0, drag.y1),
                width: Math.abs(drag.x1 - drag.x0),
                height: Math.abs(drag.y1 - drag.y0),
                borderColor: tool === "ellipse" ? color : undefined,
              }}
            />
          )}
          {tool === "polygon" && poly.length > 0 && (
            <svg className="ink-layer">
              <polyline
                points={[...poly, ...(hover ? [hover] : [])].map((p) => p.join(",")).join(" ")}
                fill={options.fill ?? "none"}
                fillOpacity={0.3}
                stroke={color}
                strokeWidth={Math.max(1, width)}
              />
            </svg>
          )}
          {tool === "polygon" && poly.length > 0 && (
            <div className="inline-hint" style={{ left: poly[0][0], top: poly[0][1] - 30 }}>
              {poly.length} points · double-click or <Kbd>Enter</Kbd> to finish · <Kbd>Esc</Kbd> cancel
            </div>
          )}
          {pending && (
            <PendingBox
              vp={vp}
              pending={pending}
              onCancel={() => setPending(null)}
              onLink={async (uri, target) => {
                const ok = await actions.addAnnot(page, { kind: "link", rect: pdfRect(vp, ...pending.box), uri: uri || undefined, page: target ?? undefined, color: rgb });
                if (ok) setPending(null);
              }}
              onText={async (text) => {
                const ok = await actions.addAnnot(page, {
                  kind: "textbox",
                  rect: pdfRect(vp, ...pending.box),
                  contents: text,
                  size: options.textSize,
                  color: rgb,
                  fill,
                  author,
                });
                if (ok) setPending(null);
              }}
              onCrop={async (scope) => {
                const ok = await actions.crop(page, pdfRect(vp, ...pending.box), scope);
                if (ok) setPending(null);
              }}
            />
          )}
        </div>
      )}
    </>
  );
}

function PendingBox({
  vp,
  pending,
  onCancel,
  onLink,
  onText,
  onCrop,
}: {
  vp: PageViewport;
  pending: { kind: "link" | "textbox" | "crop"; box: [number, number, number, number] };
  onCancel: () => void;
  onLink: (uri: string, page: number | null) => void;
  onText: (text: string) => void;
  onCrop: (scope: "page" | "all" | "selected") => void;
}) {
  const [b0, b1, b2, b3] = pending.box;
  const [value, setValue] = useState("");
  const inputRef = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  useEffect(() => {
    const t = setTimeout(() => inputRef.current?.focus(), 0);
    return () => clearTimeout(t);
  }, []);
  void vp;
  const box = <div className={cx("drag-rect", pending.kind === "crop" && "is-crop")} style={{ left: b0, top: b1, width: b2 - b0, height: b3 - b1 }} />;
  const at = { left: b0, top: b3 + 8 };
  if (pending.kind === "crop") {
    return (
      <>
        {box}
        <div className="note-pop" style={at} onPointerDown={(e) => e.stopPropagation()}>
          <p style={{ marginBottom: 8 }}>Crop to this area:</p>
          <footer style={{ flexWrap: "wrap" }}>
            <Button appearance="subtle" spacing="compact" onClick={onCancel}>
              Cancel
            </Button>
            <Button spacing="compact" onClick={() => onCrop("selected")}>
              Selected pages
            </Button>
            <Button spacing="compact" onClick={() => onCrop("all")}>
              All pages
            </Button>
            <Button appearance="primary" spacing="compact" onClick={() => onCrop("page")}>
              This page
            </Button>
          </footer>
        </div>
      </>
    );
  }
  if (pending.kind === "textbox") {
    return (
      <>
        {box}
        <div className="note-pop" style={at} onPointerDown={(e) => e.stopPropagation()}>
          <textarea
            ref={inputRef}
            value={value}
            placeholder="Text for the box…"
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Escape") onCancel();
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && value.trim()) onText(value);
            }}
          />
          <footer>
            <Button appearance="subtle" spacing="compact" onClick={onCancel}>
              Cancel
            </Button>
            <Button appearance="primary" spacing="compact" disabled={!value.trim()} onClick={() => onText(value)}>
              Add text box
            </Button>
          </footer>
        </div>
      </>
    );
  }
  const asPage = /^\s*(page\s*)?(\d+)\s*$/i.exec(value);
  const submit = () => {
    if (asPage) onLink("", parseInt(asPage[2], 10) - 1);
    else if (value.trim()) onLink(/^[a-z][a-z0-9+.-]*:/i.test(value.trim()) ? value.trim() : `https://${value.trim()}`, null);
  };
  return (
    <>
      {box}
      <div className="note-pop" style={at} onPointerDown={(e) => e.stopPropagation()}>
        <input
          ref={inputRef}
          className="text-input"
          value={value}
          placeholder="Web address, or a page number"
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter") submit();
            if (e.key === "Escape") onCancel();
          }}
        />
        <footer>
          <Button appearance="subtle" spacing="compact" onClick={onCancel}>
            Cancel
          </Button>
          <Button appearance="primary" spacing="compact" disabled={!value.trim()} onClick={submit}>
            {asPage ? `Link to page ${asPage[2]}` : "Add link"}
          </Button>
        </footer>
      </div>
    </>
  );
}
