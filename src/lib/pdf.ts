import * as pdfjs from "pdfjs-dist";
import type { PDFDocumentProxy, PDFPageProxy, PageViewport } from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export type { PDFDocumentProxy, PDFPageProxy, PageViewport };
export { pdfjs };

/** Opens PDF bytes for rendering. The buffer is handed to the worker. */
export function loadPdf(bytes: ArrayBuffer): Promise<PDFDocumentProxy> {
  return pdfjs.getDocument({
    data: new Uint8Array(bytes),
    // Character maps, the standard 14 fonts and image decoders, copied next to the app by vite.config.ts.
    cMapUrl: "/pdfjs/cmaps/",
    cMapPacked: true,
    standardFontDataUrl: "/pdfjs/standard_fonts/",
    wasmUrl: "/pdfjs/wasm/",
    enableXfa: false,
  }).promise;
}

export interface OutlineItem {
  title: string;
  bold: boolean;
  italic: boolean;
  dest: unknown;
  url: string | null;
  items: OutlineItem[];
}

export async function outline(pdf: PDFDocumentProxy): Promise<OutlineItem[]> {
  const raw = (await pdf.getOutline()) ?? [];
  const map = (items: typeof raw): OutlineItem[] =>
    items.map((i) => ({
      title: i.title,
      bold: !!i.bold,
      italic: !!i.italic,
      dest: i.dest,
      url: i.url ?? null,
      items: map(i.items ?? []),
    }));
  return map(raw);
}

/** The page index an outline destination points at, if any. */
export async function destPage(pdf: PDFDocumentProxy, dest: unknown): Promise<number | null> {
  try {
    const explicit = typeof dest === "string" ? await pdf.getDestination(dest) : (dest as unknown[] | null);
    if (!explicit || !explicit.length) return null;
    const ref = explicit[0];
    if (typeof ref === "number") return ref;
    return await pdf.getPageIndex(ref as Parameters<PDFDocumentProxy["getPageIndex"]>[0]);
  } catch {
    return null;
  }
}

export interface SearchHit {
  page: number;
  /** Index of the text item within the page's text content. */
  item: number;
  /** Where the match starts within the item's string. */
  start: number;
  before: string;
  match: string;
  after: string;
}

const textCache = new WeakMap<PDFDocumentProxy, Map<number, Promise<string[]>>>();

export function pageStrings(pdf: PDFDocumentProxy, page: number): Promise<string[]> {
  let m = textCache.get(pdf);
  if (!m) {
    m = new Map();
    textCache.set(pdf, m);
  }
  let p = m.get(page);
  if (!p) {
    p = pdf
      .getPage(page + 1)
      .then((pg) => pg.getTextContent())
      .then((tc) => tc.items.map((i) => ("str" in i ? i.str : "")));
    m.set(page, p);
  }
  return p;
}

/** Case-insensitive search across all pages, item by item. */
export async function search(
  pdf: PDFDocumentProxy,
  query: string,
  onProgress?: (hits: SearchHit[]) => void,
  signal?: { cancelled: boolean },
): Promise<SearchHit[]> {
  const q = query.toLocaleLowerCase();
  const hits: SearchHit[] = [];
  if (!q) return hits;
  for (let p = 0; p < pdf.numPages; p++) {
    if (signal?.cancelled) break;
    const items = await pageStrings(pdf, p);
    items.forEach((s, i) => {
      const lower = s.toLocaleLowerCase();
      let at = lower.indexOf(q);
      while (at >= 0) {
        hits.push({
          page: p,
          item: i,
          start: at,
          before: s.slice(Math.max(0, at - 28), at),
          match: s.slice(at, at + q.length),
          after: s.slice(at + q.length, at + q.length + 40),
        });
        at = lower.indexOf(q, at + q.length);
      }
    });
    if (p % 8 === 7) onProgress?.([...hits]);
  }
  return hits;
}

/** Documents above this size are read in ranges instead of being copied whole. */
const RANGE_THRESHOLD = 24 * 1024 * 1024;

/**
 * Opens a large document without copying it: the reader asks the engine for
 * the byte ranges it needs. The engine's bytes only grow by appended
 * updates, so ranges of one generation stay valid while pages render.
 */
function loadByRange(read: (begin: number, end: number) => Promise<ArrayBuffer>, length: number): Promise<PDFDocumentProxy> {
  class Transport extends pdfjs.PDFDataRangeTransport {
    requestDataRange(begin: number, end: number) {
      read(begin, end)
        .then((buf) => this.onDataRange(begin, new Uint8Array(buf)))
        .catch(() => this.onDataRange(begin, null));
    }
  }
  return pdfjs.getDocument({
    range: new Transport(length, null),
    length,
    rangeChunkSize: 1 << 20,
    disableAutoFetch: true,
    disableStream: true,
    cMapUrl: "/pdfjs/cmaps/",
    cMapPacked: true,
    standardFontDataUrl: "/pdfjs/standard_fonts/",
    wasmUrl: "/pdfjs/wasm/",
    enableXfa: false,
  }).promise;
}

/** Opens the current rendering bytes of an open document. */
export async function loadDocument(
  info: () => Promise<{ generation: number; length: number }>,
  range: (generation: number, begin: number, end: number) => Promise<ArrayBuffer>,
  whole: () => Promise<ArrayBuffer>,
): Promise<PDFDocumentProxy> {
  try {
    const { generation, length } = await info();
    if (length >= RANGE_THRESHOLD) return await loadByRange((b, e) => range(generation, b, e), length);
  } catch {
    // Fall back to the whole document.
  }
  return loadPdf(await whole());
}
