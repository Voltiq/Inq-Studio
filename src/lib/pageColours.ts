/*
 * What a line of a scan or a picture looks like on the rendered page: the
 * colour behind the letters, the colour of the letters, and whether they are
 * bold. Rewriting recognised text uses it, so the new words sit on the same
 * background in the same colour, from the inline editor and the inspector alike.
 */

import type { Quad } from "./api";
import type { PageViewport } from "./pdf";

export type RGB = [number, number, number];
export interface LineLook {
  cover?: RGB;
  ink?: RGB;
  bold?: boolean;
}

type Px = [number, number, number, number];
const lum = (r: number, g: number, b: number) => r * 0.299 + g * 0.587 + b * 0.114;
const mean = (part: Px[]): RGB => [0, 1, 2].map((c) => part.reduce((s, p) => s + p[c], 0) / Math.max(1, part.length) / 255) as RGB;

/**
 * Background and ink from the pixels in a line's box. Most of a line's box is
 * background, so its colour is the middle of the pixels by brightness; the ink
 * is whichever end (darkest or brightest tenth) lies further from it, which
 * covers dark text on paper as much as white text on a coloured button.
 */
export function lookOfPixels(data: Uint8ClampedArray, width: number, height: number, emPx: number): LineLook {
  const px: Px[] = [];
  for (let i = 0; i < data.length; i += 4) px.push([data[i], data[i + 1], data[i + 2], lum(data[i], data[i + 1], data[i + 2])]);
  if (px.length < 4) return {};
  const sorted = [...px].sort((a, b) => a[3] - b[3]);
  const n = sorted.length;
  const mid = sorted.slice(Math.floor(n * 0.35), Math.max(Math.floor(n * 0.35) + 1, Math.floor(n * 0.65)));
  const back = mid.reduce((s, p) => s + p[3], 0) / mid.length;
  const dark = sorted.slice(0, Math.max(1, Math.ceil(n * 0.08)));
  const light = sorted.slice(Math.floor(n * 0.92));
  const darkL = dark.reduce((s, p) => s + p[3], 0) / dark.length;
  const lightL = light.reduce((s, p) => s + p[3], 0) / light.length;
  const inkSide = back - darkL >= lightL - back ? dark : light;
  const inkL = inkSide.reduce((s, p) => s + p[3], 0) / inkSide.length;
  const look: LineLook = { cover: mean(mid), ink: mean(inkSide) };
  // Stroke width: the typical run of ink pixels across the middle of the letters.
  if (Math.abs(inkL - back) > 40 && emPx > 6) {
    const isInk = (i: number) => Math.abs(px[i][3] - inkL) < Math.abs(px[i][3] - back);
    const runs: number[] = [];
    for (let y = Math.floor(height * 0.35); y < Math.ceil(height * 0.7); y++) {
      let run = 0;
      for (let x = 0; x <= width; x++) {
        if (x < width && isInk(y * width + x)) run++;
        else if (run) {
          runs.push(run);
          run = 0;
        }
      }
    }
    if (runs.length >= 6) {
      runs.sort((a, b) => a - b);
      // Regular sans-serif stems are about 0.09 em wide, bold ones 0.14 em and more.
      look.bold = runs[Math.floor(runs.length / 2)] >= 0.115 * emPx;
    }
  }
  return look;
}

/** The look of a line on a rendered page canvas. `em` is the line's size in points. */
export function lookOnCanvas(canvas: HTMLCanvasElement, vp: PageViewport, q: Quad, em: number): LineLook {
  const k = canvas.width / vp.width;
  const pts = q.map((p) => vp.convertToViewportPoint(p[0], p[1]) as number[]);
  const x0 = Math.max(0, Math.floor(Math.min(...pts.map((p) => p[0])) * k));
  const y0 = Math.max(0, Math.floor(Math.min(...pts.map((p) => p[1])) * k));
  const x1 = Math.min(canvas.width, Math.ceil(Math.max(...pts.map((p) => p[0])) * k));
  const y1 = Math.min(canvas.height, Math.ceil(Math.max(...pts.map((p) => p[1])) * k));
  const ctx = canvas.getContext("2d");
  if (!ctx || x1 - x0 < 2 || y1 - y0 < 2) return {};
  const { data } = ctx.getImageData(x0, y0, x1 - x0, y1 - y0);
  return lookOfPixels(data, x1 - x0, y1 - y0, em * vp.scale * k);
}

/** Each page on screen answers for the lines drawn on it. */
const samplers = new Map<string, (q: Quad, em: number) => LineLook>();

export function registerPage(docId: number, page: number, sample: (q: Quad, em: number) => LineLook): () => void {
  const key = `${docId}:${page}`;
  samplers.set(key, sample);
  return () => {
    if (samplers.get(key) === sample) samplers.delete(key);
  };
}

/** The look of a line, if its page is on screen. */
export function lookOfLine(docId: number, page: number, q: Quad, em: number): LineLook {
  return samplers.get(`${docId}:${page}`)?.(q, em) ?? {};
}
