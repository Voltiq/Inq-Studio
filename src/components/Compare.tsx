import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeftRight, Minus, Plus, X } from "lucide-react";
import { Button, IconButton } from "./ui/Button";
import { Toggle } from "./ui/Field";
import { Spinner } from "./ui/Feedback";
import type { Comparison, PagePair, Quad } from "../lib/api";
import type { PDFDocumentProxy } from "../lib/pdf";
import { pixelDiff, renderPage } from "../lib/raster";
import { plural } from "../lib/format";
import { cx } from "../lib/utils";
import { t } from "../lib/i18n";

export interface CompareSide {
  name: string;
  pdf: PDFDocumentProxy;
}

/** One page of one document with the differing words marked. */
function Sheet({
  pdf,
  page,
  quads,
  kind,
  width,
  other,
  otherPage,
  pixels,
  onShare,
}: {
  pdf: PDFDocumentProxy;
  page: number | null;
  quads: Quad[];
  kind: "removed" | "added";
  width: number;
  other: PDFDocumentProxy;
  otherPage: number | null;
  pixels: boolean;
  onShare?: (share: number) => void;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState<{ w: number; h: number; scale: number; convert: (x: number, y: number) => number[] } | null>(null);
  useEffect(() => {
    if (page == null) return;
    let live = true;
    (async () => {
      const p = await pdf.getPage(page + 1);
      const base = p.getViewport({ scale: 1 });
      const scale = width / base.width;
      const vp = p.getViewport({ scale });
      const dpr = Math.min(2, window.devicePixelRatio || 1);
      const canvas = await renderPage(pdf, page, scale * dpr);
      if (!live || !ref.current) return;
      const c = ref.current;
      c.width = canvas.width;
      c.height = canvas.height;
      const ctx = c.getContext("2d")!;
      ctx.drawImage(canvas, 0, 0);
      if (pixels && otherPage != null) {
        const twin = await renderPage(other, otherPage, scale * dpr);
        if (!live) return;
        const { mask, share } = pixelDiff(canvas, twin);
        ctx.drawImage(mask, 0, 0);
        onShare?.(share);
      }
      setSize({ w: vp.width, h: vp.height, scale, convert: (x, y) => vp.convertToViewportPoint(x, y) as number[] });
    })().catch(() => {});
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pdf, page, width, pixels, other, otherPage]);
  if (page == null) {
    return (
      <div className="compare-sheet is-absent" style={{ width }}>
        <span>{kind === "removed" ? t("Not in this document") : t("Not in this document")}</span>
      </div>
    );
  }
  return (
    <div className="compare-sheet" style={{ width, minHeight: size?.h ?? width * 1.3 }}>
      <canvas ref={ref} style={{ width: "100%", display: "block" }} />
      {size &&
        quads.map((q, i) => {
          const pts = q.map((p) => size.convert(p[0], p[1]));
          const xs = pts.map((p) => p[0]);
          const ys = pts.map((p) => p[1]);
          return (
            <span
              key={i}
              className={cx("compare-mark", `is-${kind}`)}
              style={{ left: Math.min(...xs) - 1, top: Math.min(...ys) - 1, width: Math.max(...xs) - Math.min(...xs) + 2, height: Math.max(...ys) - Math.min(...ys) + 2 }}
            />
          );
        })}
      <span className="page-number">{page + 1}</span>
    </div>
  );
}

function Row({ pair, a, b, width, pixels, active, onVisible }: { pair: PagePair; a: CompareSide; b: CompareSide; width: number; pixels: boolean; active: boolean; onVisible: (el: HTMLDivElement | null) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [seen, setSeen] = useState(false);
  const [share, setShare] = useState<number | null>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver((e) => e.some((x) => x.isIntersecting) && setSeen(true), { rootMargin: t("600px 0px") });
    io.observe(el);
    return () => io.disconnect();
  }, []);
  useEffect(() => onVisible(ref.current), [onVisible]);
  const changed = pair.wordsAdded + pair.wordsRemoved > 0 || pair.a == null || pair.b == null;
  return (
    <div ref={ref} className={cx("compare-row", active && "is-active")}>
      <div className="compare-row-head">
        <span>
          {pair.a != null ? t("Page {0}", pair.a + 1) : "—"} <ArrowLeftRight size={12} aria-hidden="true" /> {pair.b != null ? t("Page {0}", pair.b + 1) : "—"}
        </span>
        <span className={cx("compare-row-note", changed && "is-changed")}>
          {pair.a == null
            ? t("page added")
            : pair.b == null
              ? t("page removed")
              : changed
                ? t("{0} removed, {1} added", plural(pair.wordsRemoved, "word"), pair.wordsAdded)
                : share != null && share > 0.0005
                  ? t("same text, {0} % of the picture differs", (share * 100).toFixed(share < 0.01 ? 2 : 1))
                  : t("same text")}
        </span>
      </div>
      <div className="compare-pair">
        {seen ? (
          <>
            <Sheet pdf={a.pdf} page={pair.a} quads={pair.removed} kind="removed" width={width} other={b.pdf} otherPage={pair.b} pixels={pixels} onShare={setShare} />
            <Sheet pdf={b.pdf} page={pair.b} quads={pair.added} kind="added" width={width} other={a.pdf} otherPage={pair.a} pixels={pixels} />
          </>
        ) : (
          <div style={{ height: width * 1.3 }} />
        )}
      </div>
    </div>
  );
}

/** Two documents side by side with their differences marked. */
export function CompareView({ a, b, result, onSwap, onClose }: { a: CompareSide; b: CompareSide; result: Comparison | null; onSwap: () => void; onClose: () => void }) {
  const [width, setWidth] = useState(520);
  const [pixels, setPixels] = useState(false);
  const [onlyChanged, setOnlyChanged] = useState(false);
  const [active, setActive] = useState<number | null>(null);
  const rows = useRef(new Map<number, HTMLDivElement>());
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);
  const shown = useMemo(
    () => (result?.pairs ?? []).map((p, i) => ({ p, i })).filter(({ p }) => !onlyChanged || p.wordsAdded + p.wordsRemoved > 0 || p.a == null || p.b == null),
    [result, onlyChanged],
  );
  const go = (pair: number) => {
    setActive(pair);
    rows.current.get(pair)?.scrollIntoView({ block: "start", behavior: "smooth" });
  };
  return (
    <div className="compare" role="dialog" aria-label={t("Compare documents")}>
      <header className="compare-bar">
        <strong className="compare-name is-removed" data-tooltip={a.name} data-tooltip-overflow>
          {a.name}
        </strong>
        <IconButton icon={ArrowLeftRight} label={t("Swap sides")} spacing="compact" onClick={onSwap} />
        <strong className="compare-name is-added" data-tooltip={b.name} data-tooltip-overflow>
          {b.name}
        </strong>
        <span className="spacer" />
        <Toggle label={t("Only changed pages")} checked={onlyChanged} onChange={setOnlyChanged} />
        <Toggle label={t("Mark differences in the picture")} checked={pixels} onChange={setPixels} />
        <IconButton icon={Minus} label={t("Smaller")} spacing="compact" onClick={() => setWidth((w) => Math.max(280, w - 80))} />
        <IconButton icon={Plus} label={t("Larger")} spacing="compact" onClick={() => setWidth((w) => Math.min(1100, w + 80))} />
        <Button iconBefore={X} onClick={onClose}>
          {t("Close")}
        </Button>
      </header>
      <div className="compare-body">
        <aside className="compare-list">
          {!result ? (
            <Spinner label={t("Comparing")} />
          ) : (
            <>
              <p className="compare-summary">
                {result.sameText
                  ? t("The text of both documents is the same.")
                  : t("{0} removed, {1} added{2}.", plural(result.wordsRemoved, "word"), result.wordsAdded, result.pagesAdded + result.pagesRemoved ? `; ${plural(result.pagesAdded, "page")} added, ${result.pagesRemoved} removed` : "")}
              </p>
              {result.sameText && <p className="is-muted">{t("Turn on “Mark differences in the picture” to compare images and graphics.")}</p>}
              {result.changes.map((c, i) => (
                <button key={i} type="button" className={cx("compare-change", active === c.pair && "is-active")} onClick={() => go(c.pair)}>
                  <span className={cx("compare-kind", `is-${c.kind}`)}>{c.kind}</span>
                  {c.before && <span className="compare-before">{c.before}</span>}
                  {c.after && <span className="compare-after">{c.after}</span>}
                </button>
              ))}
            </>
          )}
        </aside>
        <div className="compare-pages">
          {shown.map(({ p, i }) => (
            <Row
              key={`${i}-${p.a}-${p.b}`}
              pair={p}
              a={a}
              b={b}
              width={width}
              pixels={pixels}
              active={active === i}
              onVisible={(el) => {
                if (el) rows.current.set(i, el);
                else rows.current.delete(i);
              }}
            />
          ))}
          {result && !shown.length && <p className="is-muted" style={{ padding: 24 }}>{t("No page differs in its text.")}</p>}
        </div>
      </div>
    </div>
  );
}
