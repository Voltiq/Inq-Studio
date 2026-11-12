import { execFileSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { de } from "./de";
import { es } from "./es";
import { fr } from "./fr";
import { count, setLanguage, t, te } from "./i18n";

/** Strings that are the same in German, are not words, or are written into documents as they are. */
const SAME = /^(Inq|Studio|Inq Studio|PDF|Enter|Esc|Off|Yes|Squiggly|PostScript|Type|Deutsch|English|Español|Français|ACME-|Aa Bb Cc 123|i, ii, iii|I, II, III|ISO 19005-|150 ppi|600 ppi|Page \{page\} of \{pages\}|http.*|en-GB.*)$/;
const technical = (k: string) => SAME.test(k) || !/[A-Za-z]{3}/.test(k.replace(/\{\d+\}/g, "")) || /^[{"]|px |sans-serif|serif$|monospace$|\.(pdf|pfx|txt)$|^(doc|sys|std):|button:not|-(helper|list|label)/.test(k);

describe.each([
  ["German", de],
  ["Spanish", es],
  ["French", fr],
])("%s", (_name, table) => {
  it("has every interface string", () => {
    const keys: string[] = JSON.parse(execFileSync("node", ["scripts/i18n-wrap.mjs", "--keys"], { encoding: "utf8" }));
    expect(keys.length).toBeGreaterThan(500);
    const missing = keys.filter((k) => !(k in table) && !technical(k));
    expect(missing).toEqual([]);
  });

  it("keeps the placeholders of every string", () => {
    // Each value once: a plural ending ({1} in "{0} file{1}") may be used twice or dropped.
    const holes = (s: string) => [...new Set([...s.matchAll(/\{(\d+)\}/g)].map((m) => m[1]))].sort().join();
    const wrong = Object.entries(table).filter(([en, own]) => !holes(en).startsWith(holes(own)) && holes(en) !== holes(own));
    expect(wrong.map(([en]) => en)).toEqual([]);
  });

  it("has the same strings as the German table", () => {
    expect(Object.keys(table).filter((k) => !(k in de))).toEqual([]);
    expect(Object.keys(de).filter((k) => !(k in table))).toEqual([]);
  });
});

describe("translation", () => {
  it("translates, fills in and falls back", () => {
    setLanguage("de");
    expect(t("Open PDF")).toBe("PDF öffnen");
    expect(t("Page {0}", 3)).toBe("Seite 3");
    expect(t("Not in the table {0}", "x")).toBe("Not in the table x");
    expect(count(1, "page")).toBe("1 Seite");
    expect(count(4, "page")).toBe("4 Seiten");
    // Engine text: numbers and quoted names are put back in.
    expect(te("Delete 3 pages")).toBe("3 Seiten löschen");
    expect(te("Edit text “Hello”")).toBe("Text bearbeiten “Hello”");
    expect(te("Something the table does not know")).toBe("Something the table does not know");
    setLanguage("es");
    expect(t("Open PDF")).toBe("Abrir PDF");
    expect(count(4, "page")).toBe("4 páginas");
    expect(te("Delete 3 pages")).toBe("Eliminar 3 páginas");
    setLanguage("fr");
    expect(t("Open PDF")).toBe("Ouvrir un PDF");
    expect(te("Edit text “Hello”")).toBe("Modifier le texte “Hello”");
    setLanguage("en");
    expect(t("Open PDF")).toBe("Open PDF");
    expect(te("Delete 3 pages")).toBe("Delete 3 pages");
  });
});
