/*
 * Painting letters out of a picture. Covering recognised text with one flat
 * colour shows as a block on gradients and photos; instead, inside the box,
 * only the letter pixels are replaced, by blending the background around
 * them. Everything else stays the page's own pixels, so the patch has no
 * edges.
 */

export type RGB = [number, number, number];

/**
 * Paints the letters out of RGBA pixels (in place) and returns how many were
 * painted. `ink` is the colour of the letters (0–255). The background behind
 * each pixel is taken from the left and right edges of its row, which lie
 * outside the letters, so gradients across and down carry through.
 */
export function paintOutLetters(data: Uint8ClampedArray, w: number, h: number, ink: RGB): number {
  if (w < 3 || h < 3) return 0;
  const at = (x: number, y: number) => (y * w + x) * 4;
  const edge = (y: number, x0: number, x1: number): RGB => {
    const out: RGB = [0, 0, 0];
    let n = 0;
    for (let x = x0; x <= x1; x++) {
      const i = at(x, y);
      out[0] += data[i];
      out[1] += data[i + 1];
      out[2] += data[i + 2];
      n++;
    }
    return out.map((v) => v / n) as RGB;
  };
  const band = Math.max(1, Math.min(2, Math.floor(w / 10)));
  const mask = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) {
    const left = edge(y, 0, band - 1);
    const right = edge(y, w - band, w - 1);
    for (let x = 0; x < w; x++) {
      const f = w > 1 ? x / (w - 1) : 0;
      const back = [0, 1, 2].map((c) => left[c] + (right[c] - left[c]) * f);
      const i = at(x, y);
      const d = [0, 1, 2].map((c) => ink[c] - back[c]);
      const span = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
      const p = [0, 1, 2].map((c) => data[i + c] - back[c]);
      // How far the pixel leans from the background towards the ink (anti-aliased edges included).
      const lean = span > 400 ? (p[0] * d[0] + p[1] * d[1] + p[2] * d[2]) / span : Math.hypot(p[0], p[1], p[2]) / 60;
      if (lean > 0.18) mask[y * w + x] = 1;
    }
  }
  // One pixel more all round, for the softest edges.
  const grown = mask.slice();
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      if (!mask[y * w + x]) continue;
      for (let dy = -1; dy <= 1; dy++)
        for (let dx = -1; dx <= 1; dx++) {
          const xx = x + dx;
          const yy = y + dy;
          if (xx >= 0 && yy >= 0 && xx < w && yy < h) grown[yy * w + xx] = 1;
        }
    }
  // Each letter pixel: the nearest background to the left and right, and above and below, blended.
  const src = data.slice();
  let painted = 0;
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      if (!grown[y * w + x]) continue;
      let l = x - 1;
      while (l >= 0 && grown[y * w + l]) l--;
      let r = x + 1;
      while (r < w && grown[y * w + r]) r++;
      let u = y - 1;
      while (u >= 0 && grown[u * w + x]) u--;
      let b = y + 1;
      while (b < h && grown[b * w + x]) b++;
      const across = blend(src, at, l >= 0 ? [l, y] : null, r < w ? [r, y] : null, x - l, r - x);
      const down = blend(src, at, u >= 0 ? [x, u] : null, b < h ? [x, b] : null, y - u, b - y);
      // The shorter gap is the better guess.
      const wa = across ? 1 / Math.max(1, (l >= 0 && r < w ? r - l : 2 * w)) : 0;
      const wd = down ? 1 / Math.max(1, (u >= 0 && b < h ? b - u : 2 * h)) : 0;
      if (!wa && !wd) continue;
      const i = at(x, y);
      for (let c = 0; c < 3; c++) data[i + c] = ((across?.[c] ?? 0) * wa + (down?.[c] ?? 0) * wd) / (wa + wd);
      data[i + 3] = 255;
      painted++;
    }
  return painted;
}

function blend(src: Uint8ClampedArray, at: (x: number, y: number) => number, a: [number, number] | null, b: [number, number] | null, da: number, db: number): RGB | null {
  if (!a && !b) return null;
  const pa = a ? at(a[0], a[1]) : -1;
  const pb = b ? at(b[0], b[1]) : -1;
  if (!a) return [src[pb], src[pb + 1], src[pb + 2]];
  if (!b) return [src[pa], src[pa + 1], src[pa + 2]];
  const t = da / (da + db);
  return [0, 1, 2].map((c) => src[pa + c] * (1 - t) + src[pb + c] * t) as RGB;
}
