/*
 * A picture of part of a rendered page, so that whatever is dragged (text
 * lines, text boxes, images) moves as itself under the pointer instead of as
 * an empty outline. The spot it leaves is covered in the colour around it.
 */

import type { PageViewport } from "./pdf";

export interface Ghost {
  /** The picture, as a data URL. */
  url: string;
  /** The colour around it, for covering the spot it came from. */
  cover: string;
  /** Where it was taken, in page pixels. */
  left: number;
  top: number;
  width: number;
  height: number;
}

/** A picture of the box (page pixels) on the page canvas, or null when there is nothing to take. */
export function takeGhost(canvas: HTMLCanvasElement | null, vp: PageViewport | null, box: { x0: number; y0: number; x1: number; y1: number }, pad = 1): Ghost | null {
  if (!canvas || !vp || !canvas.width) return null;
  const k = canvas.width / vp.width;
  const left = Math.max(0, box.x0 - pad);
  const top = Math.max(0, box.y0 - pad);
  const right = Math.min(vp.width, box.x1 + pad);
  const bottom = Math.min(vp.height, box.y1 + pad);
  const sw = Math.round((right - left) * k);
  const sh = Math.round((bottom - top) * k);
  if (sw < 2 || sh < 2) return null;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  try {
    const data = ctx.getImageData(Math.round(left * k), Math.round(top * k), sw, sh);
    const out = document.createElement("canvas");
    out.width = sw;
    out.height = sh;
    out.getContext("2d")?.putImageData(data, 0, 0);
    // The edge of the box is what lies around the element: its most common colour covers the old spot.
    const counts = new Map<string, number>();
    const at = (x: number, y: number) => {
      const i = (y * sw + x) * 4;
      // Rounded a little, so anti-aliasing does not split the vote.
      const c = [data.data[i], data.data[i + 1], data.data[i + 2]].map((v) => Math.round(v / 8) * 8).join(",");
      counts.set(c, (counts.get(c) ?? 0) + 1);
    };
    for (let x = 0; x < sw; x++) {
      at(x, 0);
      at(x, sh - 1);
    }
    for (let y = 0; y < sh; y++) {
      at(0, y);
      at(sw - 1, y);
    }
    const cover = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "255,255,255";
    return { url: out.toDataURL(), cover: `rgb(${cover})`, left, top, width: right - left, height: bottom - top };
  } catch {
    return null;
  }
}
