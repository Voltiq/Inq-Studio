import { execFileSync } from "node:child_process";
import { describe, expect, it } from "vitest";
import { de } from "./de";
import { count, setLanguage, t, te } from "./i18n";

/** Strings that are the same in German, are not words, or are written into documents as they are. */
const SAME = /^(Inq|Studio|Inq Studio|PDF|Enter|Esc|Off|Yes|Squiggly|PostScript|Type|Deutsch|English|ACME-|Aa Bb Cc 123|i, ii, iii|I, II, III|ISO 19005-|150 ppi|600 ppi|Page \{page\} of \{pages\}|http.*|en-GB.*)$/;
const technical = (k: string) => SAME.test(k) || !/[A-Za-z]{3}/.test(k.replace(/\{\d+\}/g, "")) || /^[{"]|px |sans-serif|serif$|monospace$|\.(pdf|pfx|txt)$|^(doc|sys|std):|button:not|-(helper|list|label)/.test(k);

describe("German", () => {
  it("has every interface string", () => {
    const keys: string[] = JSON.parse(execFileSync("node", ["scripts/i18n-wrap.mjs", "--keys"], { encoding: "utf8" }));
    expect(keys.length).toBeGreaterThan(500);
    const missing = keys.filter((k) => !(k in de) && !technical(k));
    expect(missing).toEqual([]);
  });

  it("keeps the placeholders of every string", () => {
    const holes = (s: string) => [...s.matchAll(/\{(\d+)\}/g)].map((m) => m[1]).sort().join();
    // A German sentence may drop an English plural ending ({1} in "{0} file{1}"), never add or lose a value.
    const wrong = Object.entries(de).filter(([en, ge]) => !holes(en).startsWith(holes(ge)) && holes(en) !== holes(ge));
    expect(wrong.map(([en]) => en)).toEqual([]);
  });

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
    setLanguage("en");
    expect(t("Open PDF")).toBe("Open PDF");
    expect(te("Delete 3 pages")).toBe("Delete 3 pages");
  });
});
