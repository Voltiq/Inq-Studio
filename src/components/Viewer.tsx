import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Lock } from "lucide-react";
import { Button } from "./ui/Button";
import { Kbd, Spinner } from "./ui/Feedback";
import {
  api,
  type AnnotInfo,
  type FontChoice,
  type FontSummary,
  type LineEdit,
  type NewAnnot,
  type NewText,
  type PageText,
  type Point,
  type Quad,
  type TextLine,
} from "../lib/api";
import { pdfjs, type PDFDocumentProxy, type PageViewport, type SearchHit } from "../lib/pdf";
import { previewStack } from "../lib/fontPreview";
import { hexToRgb, rgbToHex } from "../lib/format";
import type { Tool, ZoomMode } from "../lib/types";
import { cx } from "../lib/utils";

export type Selection =
  | { kind: "line"; page: number; line: TextLine; revision: number; editing: boolean }
  | { kind: "annot"; page: number; annot: AnnotInfo }
  | null;

export interface TextStyle {
  font: FontChoice;
  size: number;
  color: string;
}

export interface ViewerActions {
  editLine: (page: number, line: number, revision: number, edit: LineEdit) => Promise<boolean>;
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
/** Annotations that sit on text are managed from the Comments list, so they don't block text selection. */
const TEXT_MARKUP = new Set(["Highlight", "Underline", "StrikeOut", "Squiggly", "Link", "Widget", "Popup"]);

export function Viewer({
  docId,
  revision,
  pdf,
  zoom,
  onScale,
  onZoom,
  page,
  onPage,
  navigate,
  tool,
  showLineBoxes,
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
}: {
  docId: number;
  revision: number;
  pdf: PDFDocumentProxy;
  zoom: ZoomMode;
  onScale: (scale: number) => void;
  onZoom: (z: ZoomMode) => void;
  page: number;
  onPage: (p: number) => void;
  navigate: { page: number; nonce: number } | null;
  tool: Tool;
  showLineBoxes: boolean;
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
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const pageRefs = useRef<(HTMLDivElement | null)[]>([]);
  const [sizes, setSizes] = useState<Size[]>([]);
  const [box, setBox] = useState<Size>({ w: 800, h: 600 });
  const [visible, setVisible] = useState<Set<number>>(new Set([0]));
  const anchor = useRef({ page: 0, frac: 0 });
  const [panning, setPanning] = useState(false);

  // Page sizes at scale 1, in display orientation.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const n = pdf.numPages;
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

  // Container size.
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

  // Which pages are near the viewport.
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
      { root, rootMargin: "800px 0px" },
    );
    pageRefs.current.forEach((el) => el && io.observe(el));
    return () => io.disconnect();
  }, [sizes.length, pdf]);

  // Track the current page while scrolling.
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

  // Jump to a page on request.
  useEffect(() => {
    if (!navigate) return;
    const el = pageRefs.current[navigate.page];
    const sc = scrollRef.current;
    if (el && sc) {
      sc.scrollTo({ top: el.offsetTop - PAD });
      anchor.current = { page: navigate.page, frac: 0 };
    }
  }, [navigate]);

  // Ctrl + wheel zooms.
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

  // Hand tool panning.
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
                pdf={pdf}
                index={i}
                scale={scale}
                tool={tool}
                showLineBoxes={showLineBoxes}
                fonts={fonts}
                selection={selection}
                onSelect={onSelect}
                textStyle={textStyle}
                markColor={markColor}
                inkWidth={inkWidth}
                author={author}
                hits={hits.filter((h) => h.page === i)}
                actions={actions}
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
// One page: canvas, text layer and the overlay for the active tool.
// ---------------------------------------------------------------------------

function fontStackForChoice(docId: number, choice: FontChoice, fonts: FontSummary[] | null): Promise<string> {
  if (choice.kind === "document") {
    const f = fonts?.find((x) => x.key === choice.key);
    return f ? previewStack(docId, f) : Promise.resolve("sans-serif");
  }
  if (choice.kind === "system") return Promise.resolve(`"${choice.family}", sans-serif`);
  const n = choice.name;
  if (n.startsWith("Times")) return Promise.resolve(`"Times New Roman", Times, serif`);
  if (n.startsWith("Courier")) return Promise.resolve(`"Courier New", Courier, monospace`);
  return Promise.resolve(`Helvetica, Arial, sans-serif`);
}

interface Geometry {
  left: number;
  top: number;
  width: number;
  height: number;
  angle: number;
  bbox: [number, number, number, number];
}

function lineGeometry(vp: PageViewport, q: Quad): Geometry {
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

function PageView({
  docId,
  revision,
  pdf,
  index,
  scale,
  tool,
  showLineBoxes,
  fonts,
  selection,
  onSelect,
  textStyle,
  markColor,
  inkWidth,
  author,
  hits,
  actions,
}: {
  docId: number;
  revision: number;
  pdf: PDFDocumentProxy;
  index: number;
  scale: number;
  tool: Tool;
  showLineBoxes: boolean;
  fonts: FontSummary[] | null;
  selection: Selection;
  onSelect: (s: Selection) => void;
  textStyle: TextStyle;
  markColor: string;
  inkWidth: number;
  author: string;
  hits: (SearchHit & { current: boolean })[];
  actions: ViewerActions;
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
  }, [pdf, index, scale]);

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
  }, [pdf, index, scale, tool]);

  // Text lines from the content stream, for editing and markup.
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

  // Annotations for the select tool.
  useEffect(() => {
    if (tool !== "select") return;
    let cancelled = false;
    api
      .annots(docId, index)
      .then((a) => !cancelled && setAnnots(a))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [docId, revision, index, tool]);

  // Search hit rectangles.
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
            r: [Math.min(r[0], r[2]), Math.min(r[1], r[3]), Math.abs(r[2] - r[0]), Math.abs(r[3] - r[1])] as [
              number,
              number,
              number,
              number,
            ],
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

  const lines = text?.lines ?? [];
  const geo = useMemo(() => (vp ? lines.map((l) => lineGeometry(vp, l.quad)) : []), [vp, lines]);

  const toPdf = useCallback((x: number, y: number): Point => vp!.convertToPdfPoint(x, y) as Point, [vp]);
  const local = (e: React.PointerEvent | React.MouseEvent): [number, number] => {
    const r = overlayRef.current!.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  // ----- Interaction state -----
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  const [move, setMove] = useState<{ id: number; x0: number; y0: number; dx: number; dy: number } | null>(null);
  const [ink, setInk] = useState<[number, number][] | null>(null);
  const [pendingText, setPendingText] = useState<{ x: number; y: number } | null>(null);
  const [pendingNote, setPendingNote] = useState<{ x: number; y: number } | null>(null);

  const selectedLine = selection?.kind === "line" && selection.page === index ? selection : null;

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
    if (tool === "rect" || MARKUP.includes(tool)) {
      overlayRef.current!.setPointerCapture(e.pointerId);
      setDrag({ x0: x, y0: y, x1: x, y1: y });
      return;
    }
    if (tool === "edit" && e.target === overlayRef.current) onSelect(null);
  };

  const onOverlayMove = (e: React.PointerEvent) => {
    const [x, y] = local(e);
    if (ink) setInk((p) => (p ? [...p, [x, y]] : p));
    if (drag) setDrag((d) => (d ? { ...d, x1: x, y1: y } : d));
    if (move) setMove((m) => (m ? { ...m, dx: x - m.x0, dy: y - m.y0 } : m));
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
      } else if (MARKUP.includes(tool)) {
        const tiny = r - l < 3 && b - t < 3;
        const picked = lines.filter((ln, i) => {
          const g = geo[i];
          if (!g || ln.invisible) return false;
          const [gx0, gy0, gx1, gy1] = g.bbox;
          return tiny ? l >= gx0 && l <= gx1 && t >= gy0 && t <= gy1 : gx0 < r && gx1 > l && gy0 < b && gy1 > t;
        });
        if (picked.length) {
          await actions.addAnnot(index, {
            kind: tool as "highlight" | "underline" | "strikeout",
            quads: picked.map((p) => p.quad),
            color: hexToRgb(markColor),
            author,
          });
        }
      }
    }
    if (move) {
      const m = move;
      setMove(null);
      if (Math.hypot(m.dx, m.dy) > 2 && vp && text) {
        const a = toPdf(m.x0, m.y0);
        const b = toPdf(m.x0 + m.dx, m.y0 + m.dy);
        await actions.editLine(index, m.id, text.revision, { dx: b[0] - a[0], dy: b[1] - a[1] });
      }
    }
  };

  const overlayClass = cx(
    "overlay",
    (tool === "text" || tool === "note" || tool === "ink" || tool === "rect" || MARKUP.includes(tool)) && "is-crosshair",
    tool === "text" && "is-text",
  );

  const showOverlay = tool !== "select" && tool !== "hand";

  return (
    <>
      <canvas ref={canvasRef} className="page-canvas" aria-label={`Page ${index + 1}`} />
      {!rendered && (
        <div className="page-placeholder">
          <Spinner size={20} />
        </div>
      )}
      <div ref={textRef} className="textLayer" hidden={tool !== "select"} />

      {hitRects.map((h, i) => (
        <div
          key={i}
          className={cx("search-hit", h.current && "is-current")}
          style={{ left: h.r[0], top: h.r[1], width: h.r[2], height: h.r[3] }}
        />
      ))}

      {tool === "select" && vp && (
        <div className="overlay" style={{ pointerEvents: "none" }}>
          {annots
            .filter((a) => !TEXT_MARKUP.has(a.subtype))
            .map((a) => {
              const r = vp.convertToViewportRectangle(a.rect) as number[];
              const selected = selection?.kind === "annot" && selection.page === index && selection.annot.index === a.index;
              return (
                <div
                  key={a.index}
                  className={cx("annot-box", selected && "is-selected")}
                  style={{
                    left: Math.min(r[0], r[2]),
                    top: Math.min(r[1], r[3]),
                    width: Math.abs(r[2] - r[0]),
                    height: Math.abs(r[3] - r[1]),
                    pointerEvents: "auto",
                  }}
                  data-tooltip={a.contents || a.subtype}
                  onClick={() => onSelect({ kind: "annot", page: index, annot: a })}
                />
              );
            })}
        </div>
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
          }}
        >
          {needLines &&
            lines.map((l, i) => {
              const g = geo[i];
              if (!g) return null;
              const isSel = selectedLine?.line.id === l.id;
              if (isSel && selectedLine.editing) return null;
              const moving = move?.id === l.id ? move : null;
              return (
                <div
                  key={`${text?.revision}-${l.id}`}
                  className={cx(
                    "line-box",
                    tool === "edit" && showLineBoxes && "is-outlined",
                    isSel && "is-selected",
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
                  data-tooltip={tool === "edit" ? (l.note ?? `${l.family}${l.bold ? " Bold" : ""}${l.italic ? " Italic" : ""} · ${l.em.toFixed(1)} pt`) : undefined}
                  onPointerDown={(e) => {
                    if (tool !== "edit" || e.button !== 0) return;
                    e.stopPropagation();
                    if (!isSel) {
                      onSelect({ kind: "line", page: index, line: l, revision: text!.revision, editing: false });
                      return;
                    }
                    if (!l.editable) return;
                    const [x, y] = local(e);
                    overlayRef.current!.setPointerCapture(e.pointerId);
                    setMove({ id: l.id, x0: x, y0: y, dx: 0, dy: 0 });
                  }}
                  onDoubleClick={() => {
                    if (tool === "edit" && l.editable)
                      onSelect({ kind: "line", page: index, line: l, revision: text!.revision, editing: true });
                  }}
                />
              );
            })}

          {selectedLine?.editing && vp && (
            <InlineEditor
              docId={docId}
              line={selectedLine.line}
              geometry={lineGeometry(vp, selectedLine.line.quad)}
              scale={scale}
              fonts={fonts}
              onCancel={() => onSelect({ ...selectedLine, editing: false })}
              onCommit={async (value) => {
                if (value === selectedLine.line.text) {
                  onSelect({ ...selectedLine, editing: false });
                  return;
                }
                const ok = await actions.editLine(index, selectedLine.line.id, selectedLine.revision, { text: value });
                if (!ok) onSelect({ ...selectedLine, editing: false });
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
                borderColor: markColor,
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
                const ok = await actions.addAnnot(index, {
                  kind: "note",
                  rect: [x, y, x, y],
                  color: hexToRgb(markColor),
                  contents,
                  author,
                });
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
  const [stack, setStack] = useState(`"${family}", sans-serif`);
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
  const px = line.em * scale;
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
        aria-label="Edit line"
        style={{
          left: g.left,
          top: g.top,
          minWidth: g.width,
          height: g.height,
          transform: g.angle ? `rotate(${g.angle}deg)` : undefined,
          fontFamily: stack,
          fontSize: px,
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
        <Kbd>Enter</Kbd> save in {line.family || line.fontName} <Kbd>Esc</Kbd> cancel
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
    // Focus after the pointer-down that created the editor has finished.
    const t = setTimeout(() => ref.current?.focus(), 0);
    return () => clearTimeout(t);
  }, []);
  const bold = style.font.kind === "system" ? style.font.bold : style.font.kind === "standard" && /Bold/.test(style.font.name);
  const italic =
    style.font.kind === "system" ? style.font.italic : style.font.kind === "standard" && /Italic|Oblique/.test(style.font.name);
  const finish = () => {
    if (done.current) return;
    if (!value.trim()) {
      done.current = true;
      onCancel();
      return;
    }
    done.current = true;
    onCommit(value);
  };
  return (
    <>
      <textarea
        ref={ref}
        className="inline-editor"
        value={value}
        rows={1}
        spellCheck={false}
        placeholder="Type…"
        aria-label="New text"
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
        <Kbd>Ctrl</Kbd>+<Kbd>Enter</Kbd> place <Kbd>Esc</Kbd> cancel
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
    <div
      className="note-pop"
      style={{ left: at.x, top: at.y, borderTop: `3px solid ${color}` }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <textarea
        ref={ref}
        value={value}
        placeholder="Write a note…"
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") onCancel();
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) onSave(value);
        }}
      />
      <footer>
        <Button appearance="subtle" spacing="compact" onClick={onCancel}>
          Cancel
        </Button>
        <Button appearance="primary" spacing="compact" onClick={() => onSave(value)}>
          Add note
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
