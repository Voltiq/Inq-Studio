import type { PDFDocumentProxy } from "./pdf";

/** Canvases above this many pixels fail to allocate in the webview. */
const MAX_PIXELS = 64_000_000;

/**
 * Renders a page (index from 0) on white at `scale` pixels per point.
 * The canvas is the page's viewport at that scale, in whole pixels.
 */
export async function renderPage(pdf: PDFDocumentProxy, index: number, scale: number): Promise<HTMLCanvasElement> {
  const page = await pdf.getPage(index + 1);
  const base = page.getViewport({ scale: 1 });
  const fit = Math.min(scale, Math.sqrt(MAX_PIXELS / (base.width * base.height)));
  const vp = page.getViewport({ scale: fit });
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.floor(vp.width));
  canvas.height = Math.max(1, Math.floor(vp.height));
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("The page could not be drawn.");
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  await page.render({ canvas, viewport: vp, intent: "print" }).promise;
  return canvas;
}

/** The canvas as one grey byte per pixel, row by row. */
export function grayPixels(canvas: HTMLCanvasElement): Uint8Array {
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("The page could not be read.");
  const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
  const out = new Uint8Array(canvas.width * canvas.height);
  for (let i = 0, p = 0; i < out.length; i++, p += 4) {
    out[i] = (data[p] * 77 + data[p + 1] * 150 + data[p + 2] * 29) >> 8;
  }
  return out;
}

/**
 * Marks where two renderings of a page differ: returns a canvas the size of
 * `a` that is transparent where the pages agree and tinted where they do
 * not, and the share of pixels that differ.
 */
export function pixelDiff(a: HTMLCanvasElement, b: HTMLCanvasElement): { mask: HTMLCanvasElement; share: number } {
  const w = Math.min(a.width, b.width);
  const h = Math.min(a.height, b.height);
  const mask = document.createElement("canvas");
  mask.width = a.width;
  mask.height = a.height;
  const ctx = mask.getContext("2d")!;
  const da = a.getContext("2d")!.getImageData(0, 0, w, h).data;
  const db = b.getContext("2d")!.getImageData(0, 0, w, h).data;
  const out = ctx.createImageData(w, h);
  let differing = 0;
  for (let p = 0; p < da.length; p += 4) {
    const d = Math.abs(da[p] - db[p]) + Math.abs(da[p + 1] - db[p + 1]) + Math.abs(da[p + 2] - db[p + 2]);
    if (d > 96) {
      differing++;
      out.data[p] = 230;
      out.data[p + 1] = 60;
      out.data[p + 2] = 200;
      out.data[p + 3] = 150;
    }
  }
  ctx.putImageData(out, 0, 0);
  // Pages of different sizes differ everywhere outside the common area.
  const extra = a.width * a.height - w * h;
  return { mask, share: (differing + Math.max(0, extra)) / Math.max(1, a.width * a.height) };
}
