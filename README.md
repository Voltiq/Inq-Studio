# Inq Studio

A professional PDF viewer and editor for Windows, built with Tauri 2, Rust and React in the Graphiq design
language. Edit text in the fonts the document already uses, add text, mark pages up, rearrange, insert or
extract pages, fill in and flatten forms, redact, sign and validate signatures, protect with passwords, make
scans searchable, compare documents, optimise file size and convert to PDF/A.

![Inq Studio icon](public/app-icon.svg)

## What makes it different

Most PDF editors "edit" text by painting a white box over it and typing on top in a font that only looks
similar. Inq Studio rewrites the page's own content stream:

1. Every font on the page is read into a font model (encoding, ToUnicode map, widths, embedded program, subset).
2. Your new text is re-encoded **in that font**, byte for byte the way the original producer wrote it, and put
   back into the operator that drew the line.
3. If the embedded subset lacks a character, Inq takes the glyph from the embedded program, or finds **the same
   typeface installed on your computer** and embeds just the glyphs it needs.
4. Only if the typeface is nowhere to be found are the missing characters (and only those) set in the closest
   substitute, and the app tells you so.

The result stays real text: selectable, searchable, copyable, and identical in every PDF reader.

## Develop

```bash
npm install
npm run tauri dev
```

```bash
cd src-tauri && cargo test
```

Run the engine against a real file without printing its text:

```bash
cd src-tauri && INQ_TEST_PDF="C:/path/file.pdf" cargo test real_file -- --ignored --nocapture
```

Build an installer:

```bash
npm run tauri build
```

## Layout

| Path | What |
| --- | --- |
| `src-tauri/src/fonts.rs` | Font model: encodings, ToUnicode, widths, glyph availability, encoding text |
| `src-tauri/src/interp.rs` | Content stream interpreter: text lines with exact positions |
| `src-tauri/src/edit.rs` | Text edits and new text, font fallback chain |
| `src-tauri/src/sysfonts.rs` | Installed fonts (Windows registry), subsetting and embedding |
| `src-tauri/src/cmap.rs`, `encoding.rs` | CMaps and single-byte encodings, glyph names |
| `src-tauri/src/pages.rs`, `annots.rs` | Page organisation, annotations with appearance streams |
| `src-tauri/src/store.rs` | Open documents, undo/redo, Tauri commands |
| `src-tauri/src/history.rs` | Undo as differences, appended rendering updates, which pages a change repaints |
| `src-tauri/src/security.rs` | Password protection: keep, set (AES-256 / AES-128) and remove |
| `src-tauri/src/sign.rs` | Digital signatures: validate, sign (PAdES), digital IDs — through Windows CryptoAPI |
| `src-tauri/src/ocr.rs` | OCR with the recogniser built into Windows, invisible text layer |
| `src-tauri/src/pdfa.rs` | PDF/A checks and conversion, sRGB output intent, XMP metadata |
| `src-tauri/src/optimize.rs`, `compare.rs`, `tags.rs` | Optimiser, document comparison and text export, structure tree |
| `src-tauri/src/pro.rs` | Commands of the v0.4 tools |
| `src/components/Viewer.tsx` | pdf.js rendering, overlays, inline editor |
| `src/components/ProDialogs.tsx`, `Compare.tsx` | Dialogs of the v0.4 tools, side-by-side comparison |
| `src/styles/tokens.css` | Graphiq design tokens, shared with the other Studio apps |

See [ROADMAP.md](ROADMAP.md) for what is done and what comes next.

## License

MIT
