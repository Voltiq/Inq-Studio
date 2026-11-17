/*
 * Text in pictures. On a page that already has text, recognition reads the
 * whole page, but only the words inside an image are new: everything else is
 * text the page already holds. These helpers keep just those words.
 * Boxes are in the pixels of the rendered page: [left, top, right, bottom].
 */

import type { OcrLine, Quad } from "./api";

export type Box = [number, number, number, number];

/** Pictures smaller than this (in rendered pixels) hold no readable text. */
const MIN_SIDE = 24;

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
 * The recognised words that lie inside a picture and not on text the page
 * already has (its own text, or text recognised before). Lines left without
 * words are dropped.
 */
export function wordsInPictures(lines: OcrLine[], pictures: Box[], taken: Box[]): OcrLine[] {
  const big = pictures.filter((p) => p[2] - p[0] >= MIN_SIDE && p[3] - p[1] >= MIN_SIDE);
  if (!big.length) return [];
  return lines
    .map((line) => ({
      ...line,
      words: line.words.filter((w) => {
        const box: Box = [w.x, w.y, w.x + w.w, w.y + w.h];
        const cx = w.x + w.w / 2;
        const cy = w.y + w.h / 2;
        const inside = big.some((p) => cx >= p[0] && cx <= p[2] && cy >= p[1] && cy <= p[3]);
        const area = Math.max(1, w.w * w.h);
        const covered = taken.some((t) => overlap(box, t) > 0.4 * area);
        return inside && !covered;
      }),
    }))
    .filter((line) => line.words.length > 0);
}
