# Inq Studio — Roadmap

A professional PDF viewer and editor for Windows, built with Tauri 2, Rust and React, in the Graphiq design
language. *Inq* as in ink: what you write lands in the document's own fonts, re-encoded into its own content
streams — real, selectable, searchable text, not a patch painted over the page.

`[x]` = done, `[ ]` = to do. Work top to bottom: each milestone builds on the one before it.

---

## v0.1 — Foundation (2026-11-10)

### PDF engine (Rust)
- [x] Tauri 2 app, `com.voltiq.inq-studio`, window starts hidden and shows once themed (no white flash)
- [x] `.pdf` file association and "Open with": PDFs passed on the command line open on start
- [x] Document store: open (with password prompt), save (written next to the file, then swapped in, so a failed
      save never destroys the original), save as, close; unused objects pruned and streams compressed on save
- [x] Undo / redo with exact document snapshots (40 steps); every change bumps a revision so a stale line id can
      never edit the wrong text
- [x] **Font model** for every font a page uses: Type 1, TrueType, Type 0 / CID (Identity-H/V and embedded
      CMaps), Type 3 detection; ToUnicode CMaps read and written; WinAnsi, MacRoman and Standard encodings with
      `/Differences` glyph names; widths from `/Widths`, CID `/W` arrays and standard-14 metrics; subset
      detection and per-character "can this font draw it" checks
- [x] **Content interpreter**: CTM, `q`/`Q`, text and line matrices, `Tf Tc Tw Tz TL Ts Tr`, `Tj TJ ' "`, fill
      colour in gray / RGB / CMYK / named colour spaces → text lines with exact rotated quads
- [x] **Edit text in its own font**: rewrite, resize, recolour, move and delete lines; the new text is re-encoded
      into the original operator; spaces the font has no glyph for become `TJ` gaps
- [x] Glyphs a CID TrueType subset lacks are pulled from its embedded program (ToUnicode and widths updated)
- [x] **Same typeface from this computer**: installed fonts found through the Windows font registry, matched by
      PostScript name, then family and style; only the needed glyphs are embedded (`subsetter`) as a new
      Type 0 / Identity-H font
- [x] **Per-character substitution** when the typeface is not installed: the document font keeps every
      character it has, only the missing ones are set in Arial / Times New Roman / Courier New (by style), then
      the standard 14 — with a notice saying exactly what happened
- [x] Add text in a document font, an installed font or a standard font; multi-line; upright on rotated pages
- [x] Pages: rotate, delete, reorder (page tree flattened with inherited attributes copied down), duplicate,
      insert blank, insert pages from another PDF, extract pages to a new PDF
- [x] Annotations with their own appearance streams (look the same in every reader): highlight, underline,
      strike-out, sticky note, ink drawing, rectangle; list and delete
- [x] Document properties: title, author, subject, keywords; modification date written on save
- [x] `cargo test`: 16 tests on generated PDFs (scan, edit, move, recolour, resize, delete, add, system-font
      embedding and extension, pages, annotations, encodings, CMaps); plus opt-in real-file and demo-file runs

### App (React + Graphiq)
- [x] Graphite (dark) / Porcelain (light) / System themes from the shared Graphiq tokens, Geist + Geist Mono
- [x] App icon: a page with an ink nib (`src-tauri/icons/icon.svg`)
- [x] Welcome screen with recent files, drag and drop anywhere, multi-document tabs
- [x] pdf.js 5 rendering: double-buffered (no flicker after edits), HiDPI, only pages near the viewport, fit
      width / fit page / zoom steps, `Ctrl`+wheel, zoom keeps your place
- [x] Select tool with pdf.js text layer (select and copy), hand tool
- [x] **Edit text tool**: line outlines, inline editor drawn in the embedded font itself (loaded into the
      browser from the PDF), drag or arrow keys to move, `Delete` to remove, `Enter` to edit
- [x] Add text tool with a font picker (document fonts first, installed families, standard 14)
- [x] Markup tools with per-tool colours and author name
- [x] Sidebar: page thumbnails (multi-select, drag to reorder, context menu), outline, **font inventory**
      (samples drawn in the embedded fonts, embedded / subset / installed status, what an edit can do),
      comments list
- [x] Inspector: selected line (font, size, colour, position), annotation, tool options, document properties
- [x] Find in document with hit highlighting
- [x] Unsaved-changes guard on closing a tab or the window, toasts, settings, keyboard shortcuts

### Known limits of v0.1
- ~~Editing works line by line: longer text does not reflow into the next line yet~~ (fixed in v0.2)
- ~~A line that switches fonts mid-way reads as separate lines~~ (fixed in v0.2)
- ~~Text inside Form XObjects is shown but not listed for editing~~ (fixed in v0.2)
- Saving a password-protected PDF writes it without the password (unless saved incrementally, v0.2)
- Undo keeps whole-document snapshots, and every edit re-renders from fresh bytes (fine for everyday documents,
  heavy for very large ones)
- UI interactions are type-checked and the app was launched against a generated document; there are no
  automated UI tests yet

---

## v0.2 — Editing depth (this build, 2026-11-10)

### Engine (Rust)
- [x] **Paragraph model**: lines that stack with even leading and aligned edges become paragraphs (left, right,
      centred or justified, first-line indent kept); words broken with a hyphen at a line end are rejoined
- [x] **Reflow**: a paragraph edit re-wraps the text to its column with the fonts' own advance widths, keeps the
      alignment (justified lines are spread with `TJ` adjustments) and the leading; `\n` starts a new line
- [x] **Rich lines**: runs that continue one baseline form one line even when font, size or colour change; each
      new character takes the style of the text it replaces (case-insensitive alignment), so bold words stay bold
- [x] **Text inside Form XObjects**: followed through `Do` (matrix and resources applied), listed and edited in
      their own stream; text drawn by a form used several times is flagged
- [x] Character positions for every line (exact boundaries from the content stream, `TJ` gaps included)
- [x] Simple fonts with a CFF program grow through `/Differences`: free codes get the glyphs by name, with widths
- [x] OpenType programs (CFF or TrueType, SID- or CID-keyed) grow through their cmap; CID-keyed CFF codes are CIDs
- [x] OpenType-CFF fonts embedded end to end (tested with BrutalType and the CID-keyed GenEi Gothic)
- [x] **One growing subset per installed typeface**: TrueType faces are embedded once per document with
      `/CIDToGIDMap`, so later edits add glyphs to the same font instead of creating a new one
- [x] Edit several lines in one step (size, colour, font, move, delete), and change a line's font
- [x] Labelled undo history with jumps to any step
- [x] **Incremental save**: changes appended after the original bytes (`/Prev` chain), automatic for signed PDFs,
      so existing signatures stay valid; full rewrite otherwise
- [x] 29 tests (13 new), plus opt-in real-file runs: on a 67-page manual, 87 multi-line paragraphs and 105
      mixed-style lines were found, and a paragraph reflowed with an added sentence read back exactly

### App
- [x] **Paragraph editor**: double-click a line in a paragraph to edit the whole paragraph in place, drawn in its
      font, column width, alignment and leading; `Ctrl+Enter` reflows and saves
- [x] Multi-select: Shift/Ctrl-click lines or drag a box; move, nudge, resize, recolour, refont or delete together
- [x] **Snapping**: moved text snaps to other lines' left edges and baselines, with guides (`Alt` moves freely)
- [x] Character-level highlight, underline and strike-out with a live preview; a click still marks the whole line
- [x] History tab in the sidebar: every change by name, click to go back or forward
- [x] Inspector: spans of a mixed-style line, "Change font" for one or many lines, "Edit paragraph"
- [x] Save mode setting (automatic / always rewrite / always append), "signed" badge in the top bar
- [x] Font inventory marks fonts Inq Studio embedded from installed fonts

### Known limits of v0.2
- A reflowed paragraph is written as a new text object at the end of the page: it is drawn last, and clipping
  that applied at its old place does not carry over
- Reflow does not hyphenate, and the kerning of edited text comes from glyph widths, not the original `TJ` pairs
- The paragraph editor previews in the first line's font; the saved result keeps every word's own style
- Bare CID-keyed CFF programs carry no character map, so they cannot grow; the installed-font fallback is used
- Installed OpenType-CFF faces get a new subset when an edit needs new glyphs (TrueType faces grow in place)
- Editing text in a form drawn several times changes every copy (the line says so)
- The new on-page interactions (paragraph editor, box selection, snapping, character-level markup, history) are
  type-checked and the build was launched, but they have not been clicked through by hand or by UI tests

## v0.3 — Forms, objects and pages

- [ ] AcroForm filling (text, check box, radio, list) with appearance regeneration
- [ ] Flatten annotations and forms
- [ ] Images: insert, replace, move, resize, delete, extract
- [ ] Shapes and markup: lines, arrows, ellipses, polygons, free-text boxes, stamps
- [ ] Edit existing annotations (colour, text, position)
- [ ] True redaction: remove text, images and vector content under the redaction, not just cover it
- [ ] Watermarks, headers and footers, Bates numbering, page labels
- [ ] Bookmarks: create, rename, reorder
- [ ] Links: create and edit
- [ ] Crop and resize pages, split documents
- [ ] Print

## v0.4 — Professional

- [ ] OCR for scans (searchable invisible text layer)
- [ ] Digital signatures: validate and sign (PAdES)
- [ ] Encryption: set and remove passwords and permissions
- [ ] Optimise: image downsampling, font de-duplication, object streams
- [ ] PDF/A conversion and validation
- [ ] Compare two documents
- [ ] Export pages as PNG / JPEG, export text
- [ ] Large documents: lazy object loading, incremental rendering after edits, lighter undo (diffs, not snapshots)
- [ ] Accessibility: reading order of tagged PDFs, alt text

## v1.0 — Release

- [ ] NSIS installer, code signing, auto-update
- [ ] Log file and crash reports
- [ ] German translation
- [ ] Browser preview backend for UI work (like Atomiq Studio's mock)
- [ ] Front-end unit tests (vitest) and end-to-end UI tests
- [ ] User guide
