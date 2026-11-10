import { api, type FontSummary } from "./api";

/**
 * CSS font stacks that draw text the way the document's font does.
 *
 * When the embedded program is a TrueType/OpenType font that maps Unicode,
 * the browser loads it directly, so the inline editor shows the exact
 * glyphs the PDF will use. Otherwise the stack names the family (which is
 * often installed) with a generic fallback of the same kind.
 */

const loaded = new Map<string, Promise<string>>();

function fallbackStack(f: Pick<FontSummary, "family" | "serif" | "fixed">): string {
  const generic = f.fixed ? "monospace" : f.serif ? "serif" : "sans-serif";
  return `"${f.family.replace(/"/g, "")}", ${generic}`;
}

export function previewStack(docId: number, f: FontSummary): Promise<string> {
  const id = `${docId}:${f.key}`;
  let p = loaded.get(id);
  if (!p) {
    p = (async () => {
      if (!f.previewable) return fallbackStack(f);
      try {
        const bytes = await api.fontProgram(docId, f.key);
        if (!bytes.byteLength) return fallbackStack(f);
        const name = `inq-${docId}-${f.key.replace(/\W/g, "_")}`;
        const face = new FontFace(name, bytes);
        await face.load();
        document.fonts.add(face);
        return `"${name}", ${fallbackStack(f)}`;
      } catch {
        return fallbackStack(f);
      }
    })();
    loaded.set(id, p);
  }
  return p;
}

export function forgetDocFonts(docId: number) {
  for (const k of [...loaded.keys()]) if (k.startsWith(`${docId}:`)) loaded.delete(k);
}
