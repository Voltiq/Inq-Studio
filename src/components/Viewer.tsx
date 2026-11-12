import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Lock } from "lucide-react";
import { Button } from "./ui/Button";
import { Kbd, Spinner } from "./ui/Feedback";
import {
  api,
  type AnnotInfo,
  type Block,
  type FontChoice,
  type FontSummary,
  type LineEdit,
  type NewAnnot,
  type NewText,
  type PageText,
  type Point,
  type Quad,
  type TextLine,
  type OrderItem,
} from "../lib/api";
import { usePageStamps } from "../lib/stamps";
import { pdfjs, type PDFDocumentProxy, type PageViewport, type SearchHit } from "../lib/pdf";
import { AnnotsLayer, EXTRA_TOOLS, FieldsLayer, ImagesLayer, ShapesLayer, type ExtraActions, type ToolOptions } from "./PageExtras";
import type { FieldInfo, ImageInfo, Rect } from "../lib/api";
import { previewStack } from "../lib/fontPreview";
import { hexToRgb, rgbToHex } from "../lib/format";
import type { Tool, ZoomMode } from "../lib/types";
import { cx } from "../lib/utils";
import { t } from "../lib/i18n";

export type Selection =
  | {
      kind: "lines";
      page: number;
      /** Selected lines, all on `page`. */
      lines: TextLine[];
      revision: number;
      /** What is being edited on the page: one line, or the paragraph of the first line. */
      editing: null | "line" | "block";
      /** The paragraph of the first selected line. */
      block: Block | null;
    }
  | { kind: "annot"; page: number; annot: AnnotInfo }
  | { kind: "image"; page: number; image: ImageInfo }
  | null;

export interface TextStyle {
  font: FontChoice;
  size: number;
  color: string;
}

export interface ViewerActions extends ExtraActions {
  editLines: (page: number, lines: number[], revision: number, edit: LineEdit) => Promise<boolean>;
  editBlock: (page: number, block: number, revision: number, text: string) => Promise<boolean>;
  /** Joins a dragged line to the end of another (or, `before`, to its start); dx and dy are the drag in user space. */
  joinLines: (page: number, target: number, moved: number, before: boolean, dx: number, dy: number, revision: number) => Promise<boolean>;
  addText: (page: number, spec: NewText) => Promise<boolean>;
  addAnnot: (page: number, annot: NewAnnot) => Promise<boolean>;
}

interface Size {
  w: number;
  h: number;
}

const GAP = 16;
const PAD = 24;
const MAX_CANVAS_PIXELS = 16_000_000;
const MARKUP: Tool[] = ["highlight", "underline", "strikeout"];
const SNAP_PX = 6;
/** How close (in pixels) a dragged line's edge must come to another line's end to join it. */
const JOIN_PX = 12;

export function Viewer({
  docId,
  revision,
  changed,
  order,
  pdf,
  zoom,
  onScale,
  onZoom,
  page,
  onPage,
  navigate,
  tool,
  showLineBoxes,
  snap,
  join,
  fonts,
  selection,
  onSelect,
  textStyle,
  markColor,
  inkWidth,
  author,
  hits,
  actions,
  busy,
  fields,
  redactions,
  onRedactions,
  toolOptions,
}: {
  docId: number;
  revision: number;
  /** Pages the last change can have repainted; null means any. */
  changed: number[] | null;
  /** Reading order to draw over one page. */
  order: { page: number; items: OrderItem[] } | null;
  pdf: PDFDocumentProxy;
  zoom: ZoomMode;
  onScale: (scale: number) => void;
  onZoom: (z: ZoomMode) => void;
  page: number;
  onPage: (p: number) => void;
  navigate: { page: number; nonce: number } | null;
  tool: Tool;
  showLineBoxes: boolean;
  snap: boolean;
  /** Join a line dropped at the end or start of another into one line. */
  join: boolean;
  fonts: FontSummary[] | null;
  selection: Selection;
  onSelect: (s: Selection) => void;
  textStyle: TextStyle;
  markColor: string;
  inkWidth: number;
  author: string;
  hits: (SearchHit & { current: boolean })[];
  actions: ViewerActions;
  busy: boolean;
  fields: FieldInfo[];
  redactions: Record<number, Rect[]>;
  onRedactions: (page: number, rects: Rect[]) => void;
  toolOptions: ToolOptions;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const pageRefs = useRef<(HTMLDivElement | null)[]>([]);
  const [sizes, setSizes] = useState<Size[]>([]);
  const [box, setBox] = useState<Size>({ w: 800, h: 600 });
  const [visible, setVisible] = useState<Set<number>>(new Set([0]));
  const anchor = useRef({ page: 0, frac: 0 });
  const [panning, setPanning] = useState(false);
  const stamps = usePageStamps(revision, changed, pdf.numPages);

  // Page sizes at scale 1, in display orientation. After an edit only the
  // pages it touched are measured again.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const n = pdf.numPages;
      if (sizes.length === n && changed) {
        if (!changed.length) return;
        const out = [...sizes];
        for (const i of changed) {
          if (i >= n) continue;
          const vp = (await pdf.getPage(i + 1)).getViewport({ scale: 1 });
          if (cancelled) return;
          out[i] = { w: vp.width, h: vp.height };
        }
        setSizes(out);
        return;
      }
      const first = (await pdf.getPage(1)).getViewport({ scale: 1 });
      if (cancelled) return;
      const out: Size[] = sizes.length === n ? [...sizes] : Array.from({ length: n }, () => ({ w: first.width, h: first.height }));
      out[0] = { w: first.width, h: first.height };
      setSizes([...out]);
      for (let i = 1; i < n; i++) {
        const vp = (await pdf.getPage(i + 1)).getViewport({ scale: 1 });
        if (cancelled) return;
        out[i] = { w: vp.width, h: vp.height };
        if (i % 25 === 0 || i === n - 1) setSizes([...out]);
      }
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pdf]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const scale = useMemo(() => {
    if (typeof zoom === "number") return zoom;
    if (!sizes.length) return 1;
    const maxW = Math.max(...sizes.map((s) => s.w));
    const fitW = (box.w - PAD * 2 - 12) / maxW;
    if (zoom === "fit-width") return Math.max(0.1, Math.min(fitW, 6));
    const cur = sizes[Math.min(page, sizes.length - 1)];
    return Math.max(0.1, Math.min(fitW, (box.h - PAD * 2) / cur.h, 6));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [zoom, sizes, box]);

  useEffect(() => onScale(scale), [scale, onScale]);

  // Keep the same spot in view when the zoom changes.
  const lastScale = useRef(scale);
  useLayoutEffect(() => {
    if (lastScale.current === scale) return;
    lastScale.current = scale;
    const el = pageRefs.current[anchor.current.page];
    const sc = scrollRef.current;
    if (el && sc) sc.scrollTop = el.offsetTop + anchor.current.frac * el.offsetHeight - PAD;
  }, [scale]);

  useEffect(() => {
    const root = scrollRef.current;
    if (!root) return;
    const io = new IntersectionObserver(
      (entries) => {
        setVisible((prev) => {
          const next = new Set(prev);
          for (const e of entries) {
            const i = Number((e.target as HTMLElement).dataset.index);
            if (e.isIntersecting) next.add(i);
            else next.delete(i);
          }
          return next;
        });
      },
      { root, rootMargin: t("800px 0px") },
    );
    pageRefs.current.forEach((el) => el && io.observe(el));
    return () => io.disconnect();
  }, [sizes.length, pdf]);

  const onScroll = useCallback(() => {
    const sc = scrollRef.current;
    if (!sc) return;
    const probe = sc.scrollTop + Math.min(sc.clientHeight * 0.35, 240);
    let current = 0;
    for (let i = 0; i < pageRefs.current.length; i++) {
      const el = pageRefs.current[i];
      if (el && el.offsetTop - GAP <= probe) current = i;
      else break;
    }
    const el = pageRefs.current[current];
    if (el) anchor.current = { page: current, frac: Math.max(0, (sc.scrollTop + PAD - el.offsetTop) / el.offsetHeight) };
    if (current !== page) onPage(current);
  }, [page, onPage]);

  useEffect(() => {
    if (!navigate) return;
    const el = pageRefs.current[navigate.page];
    const sc = scrollRef.current;
    if (el && sc) {
      sc.scrollTo({ top: el.offsetTop - PAD });
      anchor.current = { page: navigate.page, frac: 0 };
    }
  }, [navigate]);

  useEffect(() => {
    const sc = scrollRef.current;
    if (!sc) return;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      const next = Math.max(0.25, Math.min(5, scale * (e.deltaY < 0 ? 1.1 : 1 / 1.1)));
      onZoom(Math.round(next * 100) / 100);
    };
    sc.addEventListener("wheel", onWheel, { passive: false });
    return () => sc.removeEventListener("wheel", onWheel);
  }, [scale, onZoom]);

  const pan = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  const onPointerDown = (e: React.PointerEvent) => {
    if (tool !== "hand" && e.button !== 1) return;
    const sc = scrollRef.current!;
    pan.current = { x: e.clientX, y: e.clientY, left: sc.scrollLeft, top: sc.scrollTop };
    sc.setPointerCapture(e.pointerId);
    setPanning(true);
    e.preventDefault();
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const p = pan.current;
    if (!p) return;
    const sc = scrollRef.current!;
    sc.scrollLeft = p.left - (e.clientX - p.x);
    sc.scrollTop = p.top - (e.clientY - p.y);
  };
  const endPan = () => {
    pan.current = null;
    setPanning(false);
  };

  return (
    <div
      ref={scrollRef}
      className={cx("viewer", tool === "hand" && "is-hand", panning && "is-panning")}
      onScroll={onScroll}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endPan}
      onPointerCancel={endPan}
      tabIndex={-1}
    >
      {busy && <div className="busy-line" aria-hidden="true" />}
      <div className="viewer-pages">
        {sizes.map((s, i) => (
          <div
            key={i}
            ref={(el) => {
              pageRefs.current[i] = el;
            }}
            data-index={i}
            className={cx("page", i === page && "is-current")}
            style={{ width: s.w * scale, height: s.h * scale, ["--scale-factor" as string]: scale }}
          >
            {visible.has(i) && (
              <PageView
                docId={docId}
                revision={revision}
                stamp={stamps[i] ?? revision}
                order={order && order.page === i ? order.items : null}
                pdf={pdf}
                index={i}
                scale={scale}
                tool={tool}
                showLineBoxes={showLineBoxes}
                snap={snap}
                join={join}
                fonts={fonts}
                selection={selection}
                onSelect={onSelect}
                textStyle={textStyle}
                markColor={markColor}
                inkWidth={inkWidth}
                author={author}
                hits={hits.filter((h) => h.page === i)}
                actions={actions}
                fields={fields}
                redactions={redactions[i] ?? []}
                onRedactions={(r) => onRedactions(i, r)}
                toolOptions={toolOptions}
              />
            )}
            <span className="page-number">{i + 1}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

function fontStackForChoice(docId: number, choice: FontChoice, fonts: FontSummary[] | null): Promise<string> {
  if (choice.kind === "document") {
    const f = fonts?.find((x) => x.key === choice.key);
    return f ? previewStack(docId, f) : Promise.resolve("sans-serif");
  }
  if (choice.kind === "system") return Promise.resolve(t("\"{0}\", sans-serif", choice.family));
  const n = choice.name;
  if (n.startsWith("Times")) return Promise.resolve(t("\"Times New Roman\", Times, serif"));
  if (n.startsWith("Courier")) return Promise.resolve(t("\"Courier New\", Courier, monospace"));
  return Promise.resolve(t("Helvetica, Arial, sans-serif"));
}

interface Geometry {
  left: number;
  top: number;
  width: number;
  height: number;
  angle: number;
  bbox: [number, number, number, number];
}

/**
 * The colour of the paper and of the ink inside a line's box on the rendered
 * page: the middle of its lighter pixels, and the mean of its darkest tenth.
 */
function paperAndInk(canvas: HTMLCanvasElement, vp: PageViewport, q: Quad): { cover?: [number, number, number]; ink?: [number, number, number] } {
  const k = canvas.width / vp.width;
  const pts = q.map((p) => vp.convertToViewportPoint(p[0], p[1]) as number[]);
  const x0 = Math.max(0, Math.floor(Math.min(...pts.map((p) => p[0])) * k));
  const y0 = Math.max(0, Math.floor(Math.min(...pts.map((p) => p[1])) * k));
  const x1 = Math.min(canvas.width, Math.ceil(Math.max(...pts.map((p) => p[0])) * k));
  const y1 = Math.min(canvas.height, Math.ceil(Math.max(...pts.map((p) => p[1])) * k));
  const ctx = canvas.getContext("2d");
  if (!ctx || x1 - x0 < 2 || y1 - y0 < 2) return {};
  const { data } = ctx.getImageData(x0, y0, x1 - x0, y1 - y0);
  const step = Math.max(1, Math.floor(data.length / 4 / 20000));
  const px: [number, number, number, number][] = [];
  for (let i = 0; i < data.length; i += 4 * step) px.push([data[i], data[i + 1], data[i + 2], data[i] * 0.299 + data[i + 1] * 0.587 + data[i + 2] * 0.114]);
  if (px.length < 4) return {};
  px.sort((a, b) => a[3] - b[3]);
  const mean = (from: number, to: number): [number, number, number] => {
    const part = px.slice(from, Math.max(from + 1, to));
    return [0, 1, 2].map((c) => part.reduce((s, p) => s + p[c], 0) / part.length / 255) as [number, number, number];
  };
  const n = px.length;
  return { cover: mean(Math.floor(n * 0.6), Math.floor(n * 0.9)), ink: mean(0, Math.ceil(n * 0.08)) };
}

function quadGeometry(vp: PageViewport, q: Quad): Geometry {
  const v = q.map((p) => vp.convertToViewportPoint(p[0], p[1]) as [number, number]);
  const [bl, br, , tl] = v;
  const width = Math.hypot(br[0] - bl[0], br[1] - bl[1]);
  const height = Math.hypot(tl[0] - bl[0], tl[1] - bl[1]);
  const angle = (Math.atan2(br[1] - bl[1], br[0] - bl[0]) * 180) / Math.PI;
  const xs = v.map((p) => p[0]);
  const ys = v.map((p) => p[1]);
  return {
    left: tl[0],
    top: tl[1],
    width: Math.max(width, 2),
    height: Math.max(height, 4),
    angle,
    bbox: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)],
  };
}

const lerp = (a: Point, b: Point, t: number): Point => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];

/** The part of a line's quad between two character boundaries. */
function partialQuad(line: TextLine, from: number, to: number): Quad {
  const total = line.chars[line.chars.length - 1] || 1;
  const a = line.chars[from] / total;
  const b = line.chars[to] / total;
  const [bl, br, tr, tl] = line.quad;
  return [lerp(bl, br, a), lerp(bl, br, b), lerp(tl, tr, b), lerp(tl, tr, a)];
}

interface Caret {
  line: number;
  at: number;
}

// ---------------------------------------------------------------------------
// One page: canvas, text layer and the overlay for the active tool.
// ---------------------------------------------------------------------------

function PageView({
  docId,
  revision,
  stamp,
  order,
  pdf,
  index,
  scale,
  tool,
  showLineBoxes,
  snap,
  join,
  fonts,
  selection,
  onSelect,
  textStyle,
  markColor,
  inkWidth,
  author,
  hits,
  actions,
  fields,
  redactions,
  onRedactions,
  toolOptions,
}: {
  docId: number;
  revision: number;
  /** The revision at which this page's picture last changed. */
  stamp: number;
  order: OrderItem[] | null;
  pdf: PDFDocumentProxy;
  index: number;
  scale: number;
  tool: Tool;
  showLineBoxes: boolean;
  snap: boolean;
  /** Join a line dropped at the end or start of another into one line. */
  join: boolean;
  fonts: FontSummary[] | null;
  selection: Selection;
  onSelect: (s: Selection) => void;
  textStyle: TextStyle;
  markColor: string;
  inkWidth: number;
  author: string;
  hits: (SearchHit & { current: boolean })[];
  actions: ViewerActions;
  fields: FieldInfo[];
  redactions: Rect[];
  onRedactions: (rects: Rect[]) => void;
  toolOptions: ToolOptions;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  const overlayRef = useRef<HTMLDivElement>(null);
  const [vp, setVp] = useState<PageViewport | null>(null);
  const [rendered, setRendered] = useState(false);
  const [text, setText] = useState<PageText | null>(null);
  const [annots, setAnnots] = useState<AnnotInfo[]>([]);
  const [hitRects, setHitRects] = useState<{ r: [number, number, number, number]; current: boolean }[]>([]);

  const needLines = tool === "edit" || MARKUP.includes(tool);

  // Render the page into an offscreen canvas, then swap it in (no flicker on edits).
  useEffect(() => {
    let cancelled = false;
    let task: { cancel: () => void; promise: Promise<void> } | null = null;
    (async () => {
      const page = await pdf.getPage(index + 1);
      if (cancelled) return;
      const view = page.getViewport({ scale });
      setVp(view);
      const dpr = window.devicePixelRatio || 1;
      const px = view.width * view.height;
      const out = Math.min(dpr, Math.sqrt(MAX_CANVAS_PIXELS / px));
      const off = document.createElement("canvas");
      off.width = Math.floor(view.width * out);
      off.height = Math.floor(view.height * out);
      task = page.render({ canvas: off, viewport: page.getViewport({ scale: scale * out }) });
      await task.promise;
      if (cancelled) return;
      const c = canvasRef.current;
      if (!c) return;
      c.width = off.width;
      c.height = off.height;
      c.getContext("2d")?.drawImage(off, 0, 0);
      setRendered(true);
    })().catch(() => {});
    return () => {
      cancelled = true;
      task?.cancel();
    };
    // The newest document is used whenever this page's stamp, place or size changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stamp, index, scale]);

  // Selectable text for the select tool.
  useEffect(() => {
    const div = textRef.current;
    if (!div || tool !== "select") return;
    let layer: InstanceType<typeof pdfjs.TextLayer> | null = null;
    let cancelled = false;
    (async () => {
      const page = await pdf.getPage(index + 1);
      if (cancelled) return;
      div.replaceChildren();
      layer = new pdfjs.TextLayer({
        textContentSource: page.streamTextContent(),
        container: div,
        viewport: page.getViewport({ scale }),
      });
      await layer.render();
    })().catch(() => {});
    return () => {
      cancelled = true;
      layer?.cancel();
      div.replaceChildren();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stamp, index, scale, tool]);

  // Lines and paragraphs from the content streams, for editing and markup.
  useEffect(() => {
    if (!needLines) return;
    let cancelled = false;
    api
      .pageText(docId, index)
      .then((t) => !cancelled && setText(t))
      .catch(() => !cancelled && setText(null));
    return () => {
      cancelled = true;
    };
  }, [docId, revision, index, needLines]);

  useEffect(() => {
    if (tool !== "select" && tool !== "link") return;
    let cancelled = false;
    api
      .annots(docId, index)
      .then((a) => !cancelled && setAnnots(a))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [docId, revision, index, tool]);

  useEffect(() => {
    if (!hits.length || !vp) {
      setHitRects([]);
      return;
    }
    let cancelled = false;
    (async () => {
      const page = await pdf.getPage(index + 1);
      const tc = await page.getTextContent();
      if (cancelled) return;
      const rects = hits.flatMap((h) => {
        const item = tc.items[h.item];
        if (!item || !("str" in item) || !item.str.length) return [];
        const [a, b, , , e, f] = item.transform as number[];
        const size = Math.hypot(a, b) || item.height || 10;
        const x0 = e + (item.width * h.start) / item.str.length;
        const x1 = e + (item.width * (h.start + h.match.length)) / item.str.length;
        const r = vp.convertToViewportRectangle([x0, f - size * 0.22, x1, f + size * 0.82]) as number[];
        return [
          {
            r: [Math.min(r[0], r[2]), Math.min(r[1], r[3]), Math.abs(r[2] - r[0]), Math.abs(r[3] - r[1])] as [number, number, number, number],
            current: h.current,
          },
        ];
      });
      setHitRects(rects);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [hits, vp, pdf, index]);

  const lines = useMemo(() => text?.lines ?? [], [text]);
  const geo = useMemo(() => (vp ? lines.map((l) => quadGeometry(vp, l.quad)) : []), [vp, lines]);

  const toPdf = useCallback((x: number, y: number): Point => vp!.convertToPdfPoint(x, y) as Point, [vp]);
  const local = (e: React.PointerEvent | React.MouseEvent): [number, number] => {
    const r = overlayRef.current!.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  const sel = selection?.kind === "lines" && selection.page === index ? selection : null;
  const selectedIds = useMemo(() => new Set(sel?.lines.map((l) => l.id) ?? []), [sel]);
  // A selection made against an older scan of this page no longer matches its lines.
  const stale = sel && text && sel.revision !== text.revision;

  const selectLines = useCallback(
    (ids: number[], editing: null | "line" | "block" = null) => {
      if (!text || !ids.length) {
        onSelect(null);
        return;
      }
      const picked = ids.map((i) => text.lines[i]).filter(Boolean);
      const block = text.blocks[picked[0].block] ?? null;
      onSelect({ kind: "lines", page: index, lines: picked, revision: text.revision, editing, block });
    },
    [text, index, onSelect],
  );

  // ----- Interaction state -----
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number; add: boolean } | null>(null);
  const [move, setMove] = useState<{ x0: number; y0: number; dx: number; dy: number; free: boolean } | null>(null);
  const [guides, setGuides] = useState<{ x: number | null; y: number | null }>({ x: null, y: null });
  /** Where a moved line can snap: other lines' left edges and baselines, and the ends it can join. */
  const snapTargets = useRef<{ xs: number[]; ys: number[]; left: number; base: number; joins: { id: number; before: boolean; x: number; base: number }[] } | null>(null);
  /** The line the dragged one will join when dropped here. */
  const [joinTo, setJoinTo] = useState<{ id: number; before: boolean } | null>(null);
  const [ink, setInk] = useState<[number, number][] | null>(null);
  const [mark, setMark] = useState<{ a: Caret; b: Caret; x0: number; y0: number } | null>(null);
  const [pendingText, setPendingText] = useState<{ x: number; y: number } | null>(null);
  const [pendingNote, setPendingNote] = useState<{ x: number; y: number } | null>(null);

  /** The character boundary nearest a point, on the line under it (or the closest line). */
  const caretAt = useCallback(
    (x: number, y: number): Caret | null => {
      if (!vp) return null;
      let best = -1;
      let bestD = 28;
      geo.forEach((g, i) => {
        if (lines[i].invisible) return;
        const [x0, y0, x1, y1] = g.bbox;
        const dx = x < x0 ? x0 - x : x > x1 ? x - x1 : 0;
        const dy = y < y0 ? y0 - y : y > y1 ? y - y1 : 0;
        const d = Math.hypot(dx, dy * 2);
        if (d < bestD) {
          bestD = d;
          best = i;
        }
      });
      if (best < 0) return null;
      const l = lines[best];
      const p = toPdf(x, y);
      const a = (l.angle * Math.PI) / 180;
      const s = (p[0] - l.origin[0]) * Math.cos(a) + (p[1] - l.origin[1]) * Math.sin(a);
      let at = 0;
      let d = Infinity;
      l.chars.forEach((c, i) => {
        if (Math.abs(c - s) < d) {
          d = Math.abs(c - s);
          at = i;
        }
      });
      return { line: best, at };
    },
    [vp, geo, lines, toPdf],
  );

  /** Quads covering the text between two carets, line by line. */
  const markQuads = useCallback(
    (a: Caret, b: Caret): Quad[] => {
      const [s, e] = a.line < b.line || (a.line === b.line && a.at <= b.at) ? [a, b] : [b, a];
      const out: Quad[] = [];
      for (let li = s.line; li <= e.line; li++) {
        const l = lines[li];
        if (!l || l.invisible) continue;
        const n = l.chars.length - 1;
        let from = li === s.line ? s.at : 0;
        let to = li === e.line ? e.at : n;
        const chars = [...l.text];
        while (from < to && chars[from] === " ") from++;
        while (to > from && chars[to - 1] === " ") to--;
        if (to > from) out.push(partialQuad(l, from, to));
      }
      return out;
    },
    [lines],
  );

  const onOverlayDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || !vp) return;
    const [x, y] = local(e);
    if (tool === "text") {
      if (pendingText) return;
      setPendingText({ x, y });
      e.preventDefault();
      return;
    }
    if (tool === "note") {
      if (!pendingNote) setPendingNote({ x, y });
      return;
    }
    if (tool === "ink") {
      overlayRef.current!.setPointerCapture(e.pointerId);
      setInk([[x, y]]);
      return;
    }
    if (MARKUP.includes(tool)) {
      overlayRef.current!.setPointerCapture(e.pointerId);
      const c = caretAt(x, y);
      if (c) setMark({ a: c, b: c, x0: x, y0: y });
      return;
    }
    if (tool === "rect" || (tool === "edit" && e.target === overlayRef.current)) {
      overlayRef.current!.setPointerCapture(e.pointerId);
      setDrag({ x0: x, y0: y, x1: x, y1: y, add: e.shiftKey || e.ctrlKey });
    }
  };

  const onOverlayMove = (e: React.PointerEvent) => {
    const [x, y] = local(e);
    if (ink) setInk((p) => (p ? [...p, [x, y]] : p));
    if (drag) setDrag((d) => (d ? { ...d, x1: x, y1: y } : d));
    if (mark) {
      const c = caretAt(x, y);
      if (c) setMark((m) => (m ? { ...m, b: c } : m));
    }
    if (move) {
      let dx = x - move.x0;
      let dy = y - move.y0;
      const t = snapTargets.current;
      let gx: number | null = null;
      let gy: number | null = null;
      // Dropped at the end or the start of another line, the dragged line joins it.
      const j = t && join && !e.altKey ? t.joins.find((c) => Math.abs(t.left + dx - c.x) < JOIN_PX && Math.abs(t.base + dy - c.base) < JOIN_PX) : undefined;
      if (t && j) {
        dx = j.x - t.left;
        dy = j.base - t.base;
        gy = j.base;
      } else if (t && snap && !e.altKey) {
        const nx = t.xs.find((v) => Math.abs(t.left + dx - v) < SNAP_PX);
        if (nx !== undefined) {
          dx = nx - t.left;
          gx = nx;
        }
        const ny = t.ys.find((v) => Math.abs(t.base + dy - v) < SNAP_PX);
        if (ny !== undefined) {
          dy = ny - t.base;
          gy = ny;
        }
      }
      setGuides({ x: gx, y: gy });
      setJoinTo(j ? { id: j.id, before: j.before } : null);
      setMove({ ...move, dx, dy });
    }
  };

  const onOverlayUp = async () => {
    if (ink && vp) {
      const pts = ink;
      setInk(null);
      if (pts.length > 1) {
        await actions.addAnnot(index, {
          kind: "ink",
          paths: [pts.map(([x, y]) => toPdf(x, y))],
          color: hexToRgb(markColor),
          width: inkWidth,
          author,
        });
      }
    }
    if (mark && vp) {
      const m = mark;
      setMark(null);
      let quads = markQuads(m.a, m.b);
      if (m.a.line === m.b.line && m.a.at === m.b.at) {
        // A click marks the whole line under it.
        const l = lines[m.a.line];
        const [x0, y0, x1, y1] = geo[m.a.line].bbox;
        quads = l && m.x0 >= x0 - 2 && m.x0 <= x1 + 2 && m.y0 >= y0 - 2 && m.y0 <= y1 + 2 ? [l.quad] : [];
      }
      if (quads.length) {
        await actions.addAnnot(index, { kind: tool as "highlight" | "underline" | "strikeout", quads, color: hexToRgb(markColor), author });
      }
    }
    if (drag && vp) {
      const d = drag;
      setDrag(null);
      const [l, t, r, b] = [Math.min(d.x0, d.x1), Math.min(d.y0, d.y1), Math.max(d.x0, d.x1), Math.max(d.y0, d.y1)];
      if (tool === "rect") {
        if (r - l > 3 && b - t > 3) {
          const p0 = toPdf(l, b);
          const p1 = toPdf(r, t);
          await actions.addAnnot(index, {
            kind: "rect",
            rect: [Math.min(p0[0], p1[0]), Math.min(p0[1], p1[1]), Math.max(p0[0], p1[0]), Math.max(p0[1], p1[1])],
            color: hexToRgb(markColor),
            width: Math.max(1, inkWidth),
            author,
          });
        }
      } else if (tool === "edit") {
        if (r - l < 3 && b - t < 3) {
          if (!d.add) onSelect(null);
        } else {
          const hit = geo
            .map((g, i) => [g, i] as const)
            .filter(([g]) => g.bbox[0] < r && g.bbox[2] > l && g.bbox[1] < b && g.bbox[3] > t)
            .map(([, i]) => i);
          const keep = d.add && sel && !stale ? sel.lines.map((x) => x.id) : [];
          selectLines([...new Set([...keep, ...hit])]);
        }
      }
    }
    if (move) {
      const m = move;
      const j = joinTo;
      setMove(null);
      setJoinTo(null);
      setGuides({ x: null, y: null });
      snapTargets.current = null;
      if (Math.hypot(m.dx, m.dy) > 1 && vp && text && sel) {
        const a = toPdf(m.x0, m.y0);
        const b = toPdf(m.x0 + m.dx, m.y0 + m.dy);
        if (j && sel.lines.length === 1) await actions.joinLines(index, j.id, sel.lines[0].id, j.before, b[0] - a[0], b[1] - a[1], text.revision);
        else await actions.editLines(index, sel.lines.map((l) => l.id), text.revision, { dx: b[0] - a[0], dy: b[1] - a[1] });
      }
    }
  };

  /** Edges and baselines of other lines to snap a moved line to. */
  const startMove = (l: TextLine, x: number, y: number, free: boolean) => {
    if (!vp) return;
    const own = geo[l.id];
    const base = (vp.convertToViewportPoint(l.origin[0], l.origin[1]) as number[])[1];
    const xs: number[] = [];
    const ys: number[] = [];
    const joins: { id: number; before: boolean; x: number; base: number }[] = [];
    // One line on its own can join another: after its end, or in front of its start, a space apart.
    const single = selectedIds.size === 1 && l.editable && !l.invisible;
    const width = own.bbox[2] - own.bbox[0];
    lines.forEach((o, i) => {
      if (selectedIds.has(o.id) || Math.abs(geo[i].angle) > 0.5 || o.invisible) return;
      const y = (vp.convertToViewportPoint(o.origin[0], o.origin[1]) as number[])[1];
      xs.push(geo[i].bbox[0]);
      ys.push(y);
      if (single && o.editable) {
        const gap = 0.28 * Math.max(o.em, l.em) * vp.scale;
        joins.push({ id: o.id, before: false, x: geo[i].bbox[2] + gap, base: y });
        joins.push({ id: o.id, before: true, x: geo[i].bbox[0] - gap - width, base: y });
      }
    });
    snapTargets.current = Math.abs(own.angle) > 0.5 ? null : { xs, ys, left: own.bbox[0], base, joins };
    setMove({ x0: x, y0: y, dx: 0, dy: 0, free });
  };

  const overlayClass = cx(
    "overlay",
    (tool === "text" || tool === "note" || tool === "ink" || tool === "rect" || MARKUP.includes(tool)) && "is-crosshair",
    (tool === "text" || MARKUP.includes(tool)) && "is-text",
  );
  const showOverlay = tool !== "select" && tool !== "hand" && !EXTRA_TOOLS.includes(tool);
  const markPreview = mark ? markQuads(mark.a, mark.b) : [];
  const editingBlock = sel?.editing === "block" && sel.block && !stale ? sel.block : null;
  const editingLine = sel?.editing === "line" && !stale ? sel.lines[0] : null;
  const hiddenLines = new Set(editingBlock ? editingBlock.lines : editingLine ? [editingLine.id] : []);

  return (
    <>
      <canvas ref={canvasRef} className="page-canvas" aria-label={t("Page {0}", index + 1)} />
      {vp &&
        order?.map((o) => {
          const r = vp.convertToViewportRectangle(o.bbox) as number[];
          const [x, y] = [Math.min(r[0], r[2]), Math.min(r[1], r[3])];
          return (
            <span key={o.n} className="order-box" style={{ left: x, top: y, width: Math.abs(r[2] - r[0]), height: Math.abs(r[3] - r[1]) }} data-tooltip={o.text || undefined}>
              <span className="order-n">
                {o.n}
                {o.role && <em>{o.role}</em>}
              </span>
            </span>
          );
        })}
      {!rendered && (
        <div className="page-placeholder">
          <Spinner size={20} />
        </div>
      )}
      <div ref={textRef} className="textLayer" hidden={tool !== "select"} />

      {hitRects.map((h, i) => (
        <div key={i} className={cx("search-hit", h.current && "is-current")} style={{ left: h.r[0], top: h.r[1], width: h.r[2], height: h.r[3] }} />
      ))}

      {(tool === "select" || tool === "link") && vp && (
        <AnnotsLayer vp={vp} page={index} annots={annots} selection={selection} onSelect={onSelect} actions={actions} showLinks={tool === "link"} />
      )}
      {tool === "select" && vp && fields.length > 0 && <FieldsLayer vp={vp} page={index} fields={fields} actions={actions} />}
      {vp && (
        <ShapesLayer
          vp={vp}
          page={index}
          tool={tool}
          color={markColor}
          width={inkWidth}
          author={author}
          options={toolOptions}
          redactions={redactions}
          onRedactions={onRedactions}
          actions={actions}
        />
      )}

      {showOverlay && vp && (
        <div
          ref={overlayRef}
          className={overlayClass}
          onPointerDown={onOverlayDown}
          onPointerMove={onOverlayMove}
          onPointerUp={onOverlayUp}
          onPointerCancel={() => {
            setInk(null);
            setDrag(null);
            setMove(null);
            setMark(null);
            setGuides({ x: null, y: null });
          }}
        >
          {tool === "edit" && (
            <ImagesLayer vp={vp} page={index} docId={docId} revision={revision} selection={selection} onSelect={onSelect} actions={actions} />
          )}
          {tool === "edit" &&
            lines.map((l, i) => {
              const g = geo[i];
              if (!g || hiddenLines.has(l.id)) return null;
              const isSel = selectedIds.has(l.id) && !stale;
              const moving = isSel && move ? move : null;
              const inBlock = sel && !stale && sel.block && sel.lines.length === 1 && sel.block.lines.length > 1 && sel.block.lines.includes(l.id) && !isSel;
              const styles = l.spans.length > 1 ? t(" · {0} styles", l.spans.length) : "";
              return (
                <div
                  key={`${text?.revision}-${l.id}`}
                  className={cx(
                    "line-box",
                    showLineBoxes && "is-outlined",
                    isSel && "is-selected",
                    inBlock && "is-in-block",
                    l.invisible && "is-invisible",
                    !l.editable && "is-locked",
                    moving && "is-dragging",
                  )}
                  style={{
                    left: g.left + (moving?.dx ?? 0),
                    top: g.top + (moving?.dy ?? 0),
                    width: g.width,
                    height: g.height,
                    transform: g.angle ? `rotate(${g.angle}deg)` : undefined,
                  }}
                  data-tooltip={l.note ?? t("{0}{1}{2} · {3} pt{4}", l.family, l.bold ? " Bold" : "", l.italic ? " Italic" : "", l.em.toFixed(1), styles)}
                  onPointerDown={(e) => {
                    if (e.button !== 0 || !text) return;
                    e.stopPropagation();
                    if (e.shiftKey || e.ctrlKey || e.metaKey) {
                      const ids = sel && !stale ? sel.lines.map((x) => x.id) : [];
                      selectLines(ids.includes(l.id) ? ids.filter((x) => x !== l.id) : [...ids, l.id]);
                      return;
                    }
                    if (!isSel) {
                      selectLines([l.id]);
                      return;
                    }
                    if (!l.editable) return;
                    const [x, y] = local(e);
                    overlayRef.current!.setPointerCapture(e.pointerId);
                    startMove(l, x, y, e.altKey);
                  }}
                  onDoubleClick={() => {
                    if (!l.editable || !text) return;
                    const block = text.blocks[l.block];
                    selectLines([l.id], block && block.lines.length > 1 ? "block" : "line");
                  }}
                />
              );
            })}

          {MARKUP.includes(tool) &&
            markPreview.map((q, i) => {
              const g = quadGeometry(vp, q);
              return (
                <div
                  key={i}
                  className="mark-preview"
                  style={{
                    left: g.left,
                    top: g.top,
                    width: g.width,
                    height: g.height,
                    transform: g.angle ? `rotate(${g.angle}deg)` : undefined,
                    background: tool === "highlight" ? markColor : undefined,
                    borderBottom: tool === "underline" ? `2px solid ${markColor}` : undefined,
                    backgroundImage: tool === "strikeout" ? `linear-gradient(transparent 52%, ${markColor} 52%, ${markColor} 60%, transparent 60%)` : undefined,
                  }}
                />
              );
            })}

          {joinTo && geo[joinTo.id] && (
            <div
              className="join-target"
              style={{
                left: geo[joinTo.id].bbox[0] - 3,
                top: geo[joinTo.id].bbox[1] - 3,
                width: geo[joinTo.id].bbox[2] - geo[joinTo.id].bbox[0] + 6,
                height: geo[joinTo.id].bbox[3] - geo[joinTo.id].bbox[1] + 6,
              }}
            >
              <span className="join-badge">{t("Joins into one line")}</span>
            </div>
          )}
          {guides.x !== null && <div className="snap-guide is-vertical" style={{ left: guides.x }} />}
          {guides.y !== null && <div className="snap-guide is-horizontal" style={{ top: guides.y }} />}

          {editingLine && (
            <InlineEditor
              docId={docId}
              line={editingLine}
              geometry={quadGeometry(vp, editingLine.quad)}
              scale={scale}
              fonts={fonts}
              onCancel={() => sel && onSelect({ ...sel, editing: null })}
              onCommit={async (value) => {
                if (!sel) return;
                if (value === editingLine.text) {
                  onSelect({ ...sel, editing: null });
                  return;
                }
                // On a scan the old words are pixels: the engine covers them in the colour of the paper.
                const colours = editingLine.invisible && vp && canvasRef.current ? paperAndInk(canvasRef.current, vp, editingLine.quad) : {};
                const ok = await actions.editLines(index, [editingLine.id], sel.revision, { text: value, ...colours });
                if (!ok) onSelect({ ...sel, editing: null });
              }}
            />
          )}

          {editingBlock && text && (
            <BlockEditor
              docId={docId}
              block={editingBlock}
              first={text.lines[editingBlock.lines[0]]}
              geometry={quadGeometry(vp, editingBlock.quad)}
              scale={scale}
              fonts={fonts}
              onCancel={() => sel && onSelect({ ...sel, editing: null })}
              onCommit={async (value) => {
                if (!sel) return;
                if (value === editingBlock.text) {
                  onSelect({ ...sel, editing: null });
                  return;
                }
                const ok = await actions.editBlock(index, editingBlock.id, sel.revision, value);
                if (!ok) onSelect({ ...sel, editing: null });
              }}
            />
          )}

          {drag && (
            <div
              className="drag-rect"
              style={{
                left: Math.min(drag.x0, drag.x1),
                top: Math.min(drag.y0, drag.y1),
                width: Math.abs(drag.x1 - drag.x0),
                height: Math.abs(drag.y1 - drag.y0),
                borderColor: tool === "rect" ? markColor : undefined,
              }}
            />
          )}

          {ink && (
            <svg className="ink-layer">
              <polyline
                points={ink.map((p) => p.join(",")).join(" ")}
                fill="none"
                stroke={markColor}
                strokeWidth={inkWidth * scale}
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          )}

          {pendingText && (
            <NewTextEditor
              docId={docId}
              at={pendingText}
              style={textStyle}
              scale={scale}
              fonts={fonts}
              onCancel={() => setPendingText(null)}
              onCommit={async (value) => {
                const size = textStyle.size;
                const baseline = pendingText.y + size * scale * 0.8;
                const [x, y] = toPdf(pendingText.x, baseline);
                const ok = await actions.addText(index, {
                  x,
                  y,
                  text: value,
                  font: textStyle.font,
                  size,
                  color: hexToRgb(textStyle.color),
                  lineHeight: 1.2,
                });
                if (ok) setPendingText(null);
              }}
            />
          )}

          {pendingNote && (
            <NotePopover
              at={pendingNote}
              color={markColor}
              onCancel={() => setPendingNote(null)}
              onSave={async (contents) => {
                const [x, y] = toPdf(pendingNote.x, pendingNote.y);
                const ok = await actions.addAnnot(index, { kind: "note", rect: [x, y, x, y], color: hexToRgb(markColor), contents, author });
                if (ok) setPendingNote(null);
              }}
            />
          )}
        </div>
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Editors drawn over the page
// ---------------------------------------------------------------------------

function useFontStack(docId: number, fontKey: string, fonts: FontSummary[] | null, family: string): string {
  const [stack, setStack] = useState(t("\"{0}\", sans-serif", family));
  useEffect(() => {
    const f = fonts?.find((x) => x.key === fontKey);
    if (!f) return;
    let live = true;
    previewStack(docId, f).then((s) => live && setStack(s));
    return () => {
      live = false;
    };
  }, [docId, fontKey, fonts]);
  return stack;
}

function InlineEditor({
  docId,
  line,
  geometry: g,
  scale,
  fonts,
  onCommit,
  onCancel,
}: {
  docId: number;
  line: TextLine;
  geometry: Geometry;
  scale: number;
  fonts: FontSummary[] | null;
  onCommit: (value: string) => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(line.text);
  const done = useRef(false);
  const stack = useFontStack(docId, line.fontKey, fonts, line.family);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const commit = () => {
    if (done.current) return;
    done.current = true;
    onCommit(value);
  };
  return (
    <>
      <input
        ref={ref}
        className="inline-editor"
        value={value}
        spellCheck={false}
        aria-label={t("Edit line")}
        style={{
          left: g.left,
          top: g.top,
          minWidth: g.width,
          height: g.height,
          transform: g.angle ? `rotate(${g.angle}deg)` : undefined,
          fontFamily: stack,
          fontSize: line.em * scale,
          fontWeight: line.bold ? 700 : 400,
          fontStyle: line.italic ? "italic" : "normal",
          lineHeight: `${g.height}px`,
          color: rgbToHex(line.color),
        }}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            done.current = true;
            onCancel();
          }
        }}
        onBlur={commit}
        onPointerDown={(e) => e.stopPropagation()}
      />
      <div className="inline-hint" style={{ left: g.left, top: g.top + g.height + 10 }}>
        <Kbd>{t("Enter")}</Kbd> {t("save in")}{" "}{line.family || line.fontName}
        {line.spans.length > 1 ? t(" (styles kept)") : ""} <Kbd>{t("Esc")}</Kbd> cancel
      </div>
    </>
  );
}

function BlockEditor({
  docId,
  block,
  first,
  geometry: g,
  scale,
  fonts,
  onCommit,
  onCancel,
}: {
  docId: number;
  block: Block;
  first: TextLine;
  geometry: Geometry;
  scale: number;
  fonts: FontSummary[] | null;
  onCommit: (value: string) => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [value, setValue] = useState(block.text);
  const done = useRef(false);
  const stack = useFontStack(docId, first.fontKey, fonts, first.family);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }, []);
  const commit = () => {
    if (done.current) return;
    done.current = true;
    onCommit(value);
  };
  return (
    <>
      <textarea
        ref={ref}
        className="inline-editor block-editor"
        value={value}
        spellCheck={false}
        aria-label={t("Edit paragraph")}
        style={{
          left: g.left,
          top: g.top,
          width: block.width * scale + 4,
          minHeight: g.height,
          transform: g.angle ? `rotate(${g.angle}deg)` : undefined,
          fontFamily: stack,
          fontSize: first.em * scale,
          fontWeight: first.bold ? 700 : 400,
          fontStyle: first.italic ? "italic" : "normal",
          lineHeight: `${block.leading * scale}px`,
          textAlign: block.align,
          color: rgbToHex(first.color),
        }}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            commit();
          }
          if (e.key === "Escape") {
            done.current = true;
            onCancel();
          }
        }}
        onBlur={commit}
        onPointerDown={(e) => e.stopPropagation()}
      />
      <div className="inline-hint" style={{ left: g.left, top: g.top - 32 }}>
        {t("Paragraph ·")}{" "}{block.lines.length} {t("lines ·")}{" "}{block.align} <Kbd>{t("Ctrl")}</Kbd>+<Kbd>{t("Enter")}</Kbd> {t("reflow and save")}{" "}<Kbd>{t("Esc")}</Kbd> cancel
      </div>
    </>
  );
}

function NewTextEditor({
  docId,
  at,
  style,
  scale,
  fonts,
  onCommit,
  onCancel,
}: {
  docId: number;
  at: { x: number; y: number };
  style: TextStyle;
  scale: number;
  fonts: FontSummary[] | null;
  onCommit: (value: string) => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [value, setValue] = useState("");
  const [stack, setStack] = useState("sans-serif");
  const done = useRef(false);
  useEffect(() => {
    let live = true;
    fontStackForChoice(docId, style.font, fonts).then((s) => live && setStack(s));
    return () => {
      live = false;
    };
  }, [docId, style.font, fonts]);
  useEffect(() => {
    const t = setTimeout(() => ref.current?.focus(), 0);
    return () => clearTimeout(t);
  }, []);
  const bold = style.font.kind === "system" ? style.font.bold : style.font.kind === "standard" && /Bold/.test(style.font.name);
  const italic = style.font.kind === "system" ? style.font.italic : style.font.kind === "standard" && /Italic|Oblique/.test(style.font.name);
  const finish = () => {
    if (done.current) return;
    done.current = true;
    if (!value.trim()) onCancel();
    else onCommit(value);
  };
  return (
    <>
      <textarea
        ref={ref}
        className="inline-editor"
        value={value}
        rows={1}
        spellCheck={false}
        placeholder={t("Type…")}
        aria-label={t("New text")}
        style={{
          left: at.x,
          top: at.y,
          minWidth: 40,
          fontFamily: stack,
          fontSize: style.size * scale,
          lineHeight: 1.2,
          fontWeight: bold ? 700 : 400,
          fontStyle: italic ? "italic" : "normal",
          color: style.color,
        }}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) finish();
          if (e.key === "Escape") {
            done.current = true;
            onCancel();
          }
        }}
        onBlur={finish}
        onPointerDown={(e) => e.stopPropagation()}
      />
      <div className="inline-hint" style={{ left: at.x, top: at.y - 30 }}>
        <Kbd>{t("Ctrl")}</Kbd>+<Kbd>{t("Enter")}</Kbd> place <Kbd>{t("Esc")}</Kbd> cancel
      </div>
    </>
  );
}

function NotePopover({
  at,
  color,
  onSave,
  onCancel,
}: {
  at: { x: number; y: number };
  color: string;
  onSave: (text: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState("");
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const t = setTimeout(() => ref.current?.focus(), 0);
    return () => clearTimeout(t);
  }, []);
  return (
    <div className="note-pop" style={{ left: at.x, top: at.y, borderTop: `3px solid ${color}` }} onPointerDown={(e) => e.stopPropagation()}>
      <textarea
        ref={ref}
        value={value}
        placeholder={t("Write a note…")}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") onCancel();
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) onSave(value);
        }}
      />
      <footer>
        <Button appearance="subtle" spacing="compact" onClick={onCancel}>
          {t("Cancel")}
        </Button>
        <Button appearance="primary" spacing="compact" onClick={() => onSave(value)}>
          {t("Add note")}
        </Button>
      </footer>
    </div>
  );
}

export function LockedNotice({ text }: { text: string }) {
  return (
    <div className="notice is-warning">
      <Lock size={14} aria-hidden="true" />
      <span>{text}</span>
    </div>
  );
}
