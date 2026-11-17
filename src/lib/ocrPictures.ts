/*
 * Text in pictures. On a page that already has text, recognition reads the
 * whole page, but only words that are not already text are new: lettering
 * in photos and screenshots, and lettering drawn as shapes in illustrations
 * (common in brochures, where it is often outlined). These helpers keep just
 * those words. Boxes are in the pixels of the rendered page: [left, top, right, bottom].
 */

import type { OcrLine, Quad } from "./api";

export type Box = [number, number, number, number];

/** The box around a quad, through a function that turns PDF points into rendered pixels. */
export function boxOf(quad: Quad, toPixels: (x: number, y: number) => number[]): Box {
  const pts = quad.map(([x, y]) => toPixels(x, y));
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

function overlap(a: Box, b: Box): number {
  const w = Math.min(a[2], b[2]) - Math.max(a[0], b[0]);
  const h = Math.min(a[3], b[3]) - Math.max(a[1], b[1]);
  return w > 0 && h > 0 ? w * h : 0;
}

/**
 * The recognised words that are not on text the page already has (its own
 * text, or text recognised before). Specks that are no letter or digit, which
 * the shapes of a drawing can produce, are left out, and so are lines left
 * without words.
 */
export function wordsOffText(lines: OcrLine[], taken: Box[]): OcrLine[] {
  return lines
    .map((line) => ({
      ...line,
      words: line.words.filter((w) => {
        if (!/[\p{L}\p{N}]/u.test(w.text)) return false;
        const box: Box = [w.x, w.y, w.x + w.w, w.y + w.h];
        const area = Math.max(1, w.w * w.h);
        return !taken.some((t) => overlap(box, t) > 0.4 * area);
      }),
    }))
    .filter((line) => line.words.length > 0);
}
