// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { renderHook } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { parseRange } from "../components/ToolDialogs";
import { SignaturesDialog } from "../components/ProDialogs";
import { installMock, samplePdf } from "./mock";
import { usePageStamps } from "./stamps";
import { bytes, hexToRgb, plural, rgbToHex } from "./format";
import { setLanguage } from "./i18n";
import type { DocInfo } from "./api";

beforeAll(() => setLanguage("en"));
afterEach(cleanup);

describe("page ranges", () => {
  it("reads lists, ranges and open ends", () => {
    expect(parseRange("1-3, 7, 10-", 12)).toEqual([0, 1, 2, 6, 9, 10, 11]);
    expect(parseRange("-2; 5", 6)).toEqual([0, 1, 4]);
    expect(parseRange("3-1", 5)).toEqual([0, 1, 2]);
    expect(parseRange("99, x, ", 5)).toEqual([]);
  });
});

describe("formatting", () => {
  it("formats sizes, colours and counts", () => {
    expect(bytes(900)).toBe("900 B");
    expect(bytes(1536)).toBe("1.5 KB");
    expect(rgbToHex(hexToRgb("#1f9d55"))).toBe("#1f9d55");
    expect(plural(1, "page")).toBe("1 page");
    expect(plural(3, "page")).toBe("3 pages");
  });
});

describe("page stamps", () => {
  it("moves only the stamps of the pages a change touched", () => {
    const { result, rerender } = renderHook(({ revision, changed }: { revision: number; changed: number[] | null }) => usePageStamps(revision, changed, 4), {
      initialProps: { revision: 1, changed: null as number[] | null },
    });
    expect(result.current).toEqual([1, 1, 1, 1]);
    rerender({ revision: 2, changed: [2] });
    expect(result.current).toEqual([1, 1, 2, 1]);
    rerender({ revision: 3, changed: [] });
    expect(result.current).toEqual([1, 1, 2, 1]);
    rerender({ revision: 4, changed: null });
    expect(result.current).toEqual([4, 4, 4, 4]);
  });
});

describe("the stand-in engine", () => {
  it("writes a PDF whose cross-reference table points at its objects", () => {
    const text = new TextDecoder().decode(samplePdf());
    expect(text.startsWith("%PDF-1.7")).toBe(true);
    const xref = Number(/startxref\n(\d+)/.exec(text)![1]);
    expect(text.slice(xref, xref + 4)).toBe("xref");
    const offsets = [...text.slice(xref).matchAll(/(\d{10}) 00000 n/g)].map((m) => Number(m[1]));
    expect(offsets.length).toBe(7);
    offsets.forEach((o, i) => expect(text.slice(o, o + `${i + 1} 0 obj`.length)).toBe(`${i + 1} 0 obj`));
  });

  it("answers the window's commands and refuses changes", async () => {
    const calls = installMock();
    const info = await invoke<DocInfo>("doc_open", { path: "x", password: null });
    expect(info.state.pages).toBe(2);
    await expect(invoke("pages_delete", { id: 1, pages: [0] })).rejects.toMatch(/browser preview/);
    expect(calls).toEqual(["doc_open", "pages_delete"]);
  });

  it("drives a dialog: the signatures of the sample document", async () => {
    installMock();
    render(<SignaturesDialog docId={1} revision={1} onSign={() => {}} onNavigate={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText(/intact, but the signer's identity is not verified/)).toBeTruthy());
    expect(screen.getByText(/Preview Signer ·/)).toBeTruthy();
    expect(screen.getByText("Unchanged since it was signed.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign…" })).toBeTruthy();
  });
});

describe("manual", () => {
  it("reads headings, lists, tables and paragraphs", async () => {
    const { parseManual } = await import("../components/Manual");
    const blocks = parseManual("# Title\n\nSome text\nover two lines.\n\n## Chapter\n\n- one\n  more\n- two\n\n1. first\n\n| | |\n| --- | --- |\n| `F1` | Help |\n");
    expect(blocks.map((b) => b.kind)).toEqual(["heading", "paragraph", "heading", "list", "list", "table"]);
    expect(blocks[1]).toEqual({ kind: "paragraph", text: "Some text over two lines." });
    expect(blocks[3]).toEqual({ kind: "list", ordered: false, items: ["one more", "two"] });
    expect(blocks[5]).toEqual({ kind: "table", rows: [["`F1`", "Help"]] });
  });

  it("has the same chapters in every language", async () => {
    const { parseManual } = await import("../components/Manual");
    const { readFileSync } = await import("node:fs");
    const chapters = (file: string) => parseManual(readFileSync(file, "utf8")).filter((b) => b.kind === "heading" && b.level === 2).length;
    expect(chapters("docs/GUIDE.md")).toBeGreaterThan(15);
    for (const file of ["docs/ANLEITUNG.md", "docs/GUIA.md", "docs/MANUEL.md"]) expect(chapters(file), file).toBe(chapters("docs/GUIDE.md"));
  });
});
