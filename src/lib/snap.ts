/*
 * Snapping for anything dragged on a page: text lines, text boxes and other
 * annotations, images. The dragged box snaps its left edge, centre or right
 * edge to those of the nearest elements (and of the page), and its top,
 * middle, bottom or baseline likewise. Guides show every element that lines
 * up with the snapped position. All values are in page pixels, y downwards.
 */

export interface SnapBox {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  /** Baseline of text, when the box holds text. */
  base?: number;
}

export interface Guide {
  /** "x": a vertical line at `at`; "y": a horizontal one. */
  axis: "x" | "y";
  at: number;
  from: number;
  to: number;
}

export interface Snapped {
  dx: number;
  dy: number;
  guides: Guide[];
}

export interface SnapOptions {
  /** How close (in pixels) an edge must come to snap. */
  threshold?: number;
  /** How many of the nearest elements are snap targets. */
  nearest?: number;
  /** The page, whose edges and centre are always targets. */
  page?: { width: number; height: number };
}

const xs = (b: SnapBox) => [b.x0, (b.x0 + b.x1) / 2, b.x1];
const ys = (b: SnapBox) => [b.y0, (b.y0 + b.y1) / 2, b.y1];

/** The gap between two boxes, 0 when they overlap. */
function gap(a: SnapBox, b: SnapBox): number {
  const gx = Math.max(0, b.x0 - a.x1, a.x0 - b.x1);
  const gy = Math.max(0, b.y0 - a.y1, a.y0 - b.y1);
  return Math.hypot(gx, gy);
}

/** The targets worth snapping to: the nearest few elements, and the page. */
export function nearestTargets(moving: SnapBox, targets: SnapBox[], nearest = 10, page?: { width: number; height: number }): SnapBox[] {
  const near = [...targets].sort((a, b) => gap(moving, a) - gap(moving, b)).slice(0, nearest);
  return page ? [...near, { x0: 0, y0: 0, x1: page.width, y1: page.height }] : near;
}

/**
 * Where a box dragged by (dx, dy) lands once snapped, and the guides to draw.
 * Baselines snap before any other vertical edge, so text sits on text.
 */
export function snapMove(moving: SnapBox, dx: number, dy: number, targets: SnapBox[], opts: SnapOptions = {}): Snapped {
  const threshold = opts.threshold ?? 6;
  const moved: SnapBox = { x0: moving.x0 + dx, y0: moving.y0 + dy, x1: moving.x1 + dx, y1: moving.y1 + dy, base: moving.base != null ? moving.base + dy : undefined };
  const pool = nearestTargets(moved, targets, opts.nearest ?? 10, opts.page);

  let bestX: { d: number; to: number } | null = null;
  let bestY: { d: number; to: number; base: boolean } | null = null;
  for (const t of pool) {
    for (const a of xs(moved))
      for (const b of xs(t)) {
        const d = b - a;
        if (Math.abs(d) <= threshold && (!bestX || Math.abs(d) < Math.abs(bestX.d))) bestX = { d, to: b };
      }
    if (moved.base != null && t.base != null) {
      const d = t.base - moved.base;
      // A baseline wins over an edge that is only a little closer.
      if (Math.abs(d) <= threshold && (!bestY || !bestY.base || Math.abs(d) < Math.abs(bestY.d))) bestY = { d, to: t.base, base: true };
    }
    for (const a of ys(moved))
      for (const b of ys(t)) {
        const d = b - a;
        if (Math.abs(d) <= threshold && (!bestY || (!bestY.base && Math.abs(d) < Math.abs(bestY.d)) || (bestY.base && Math.abs(d) + 2 < Math.abs(bestY.d)))) bestY = { d, to: b, base: false };
      }
  }

  const fx = bestX?.d ?? 0;
  const fy = bestY?.d ?? 0;
  const final: SnapBox = { x0: moved.x0 + fx, y0: moved.y0 + fy, x1: moved.x1 + fx, y1: moved.y1 + fy, base: moved.base != null ? moved.base + fy : undefined };
  const guides: Guide[] = [];
  const near = (a: number, b: number) => Math.abs(a - b) < 0.5;
  if (bestX) {
    // The guide runs along every target that shares the line.
    const along = pool.filter((t) => xs(t).some((v) => near(v, bestX!.to)));
    const from = Math.min(final.y0, ...along.map((t) => t.y0));
    const to = Math.max(final.y1, ...along.map((t) => t.y1));
    guides.push({ axis: "x", at: bestX.to, from, to });
  }
  if (bestY) {
    const along = pool.filter((t) => (bestY!.base ? t.base != null && near(t.base, bestY!.to) : ys(t).some((v) => near(v, bestY!.to))));
    const from = Math.min(final.x0, ...along.map((t) => t.x0));
    const to = Math.max(final.x1, ...along.map((t) => t.x1));
    guides.push({ axis: "y", at: bestY.to, from, to });
  }
  return { dx: dx + fx, dy: dy + fy, guides };
}

/**
 * Snapping for a resize: the edges that move (right and bottom here, as the
 * handle drags them) snap to the edges and centres of the targets.
 */
export function snapResize(box: SnapBox, dw: number, dh: number, targets: SnapBox[], opts: SnapOptions = {}): Snapped {
  const threshold = opts.threshold ?? 6;
  const right = box.x1 + dw;
  const bottom = box.y1 + dh;
  const moved: SnapBox = { x0: box.x0, y0: box.y0, x1: right, y1: bottom };
  const pool = nearestTargets(moved, targets, opts.nearest ?? 10, opts.page);
  let bx: number | null = null;
  let by: number | null = null;
  for (const t of pool) {
    for (const v of xs(t)) if (Math.abs(v - right) <= threshold && (bx == null || Math.abs(v - right) < Math.abs(bx - right))) bx = v;
    for (const v of ys(t)) if (Math.abs(v - bottom) <= threshold && (by == null || Math.abs(v - bottom) < Math.abs(by - bottom))) by = v;
  }
  const guides: Guide[] = [];
  if (bx != null) guides.push({ axis: "x", at: bx, from: Math.min(box.y0, ...pool.filter((t) => xs(t).some((v) => Math.abs(v - bx!) < 0.5)).map((t) => t.y0)), to: Math.max(by ?? bottom, ...pool.filter((t) => xs(t).some((v) => Math.abs(v - bx!) < 0.5)).map((t) => t.y1)) });
  if (by != null) guides.push({ axis: "y", at: by, from: Math.min(box.x0, ...pool.filter((t) => ys(t).some((v) => Math.abs(v - by!) < 0.5)).map((t) => t.x0)), to: Math.max(bx ?? right, ...pool.filter((t) => ys(t).some((v) => Math.abs(v - by!) < 0.5)).map((t) => t.x1)) });
  return { dx: (bx ?? right) - box.x1, dy: (by ?? bottom) - box.y1, guides };
}
