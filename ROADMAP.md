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
- ~~Saving a password-protected PDF writes it without the password~~ (fixed in v0.4)
- ~~Undo keeps whole-document snapshots, and every edit re-renders from fresh bytes~~ (fixed in v0.4)
- UI interactions are type-checked and the app was launched against a generated document; there are no
  automated UI tests yet

---

## v0.2 — Editing depth (2026-11-10)

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

## v0.3 — Forms, objects and pages (2026-11-11)

### Engine (Rust)
- [x] **Forms**: the AcroForm field tree read with inherited attributes (text, check box, radio, combo, list,
      button, signature); filling regenerates each widget's appearance in the field's own font (`/DA`, `/DR`),
      with auto-size, multi-line wrapping and alignment; characters the form font lacks come from Arial
- [x] **Flatten** form fields, annotations or both: appearances drawn into the page as Form XObjects with the
      spec's BBox-to-Rect mapping; links are kept
- [x] **Images**: list placements (inline images included), insert PNG/JPEG (JPEGs embedded as they are,
      transparency as a soft mask), move and resize (orientation kept), replace (fitted into the same box),
      delete, extract (JPEG as stored, everything else as PNG); decoding for RGB, Gray, CMYK, ICC, Indexed and
      1–16-bit images
- [x] **Shapes and markup**: ellipse, line, arrow, polygon, text box, stamp, link, alongside the v0.1 kinds; every
      appearance is generated from the annotation's own data
- [x] **Edit existing annotations**: colour, fill, text, opacity, line width, position, link target; the
      appearance is redrawn
- [x] **True redaction**: glyphs under a box are removed from their text operators and replaced by an equal gap
      (the rest of the line does not move); images fully covered are removed, partly covered ones get the
      covered pixels blacked out in a new copy; drawings fully covered are removed; annotations touching a box
      are deleted; optional cover box; find-and-mark every occurrence of a text
- [x] **Watermarks, headers and footers, Bates numbers** in any document, installed or standard font, upright on
      rotated pages, with `{page}`, `{pages}`, `{date}`, `{file}` placeholders; watermarks see-through and
      optionally behind the content
- [x] **Page labels**: read, written, and computed as readers show them (i, ii … A-1 …)
- [x] **Bookmarks**: read (named destinations included) and rewritten as a fresh outline tree
- [x] **Crop** pages, **resize** pages to a paper size (content and annotations scaled and centred), **split**
      into files by ranges
- [x] 42 tests (13 new): every annotation kind, editing, flattening with filled-in values readable as page text,
      the full image lifecycle, glyph-exact redaction, partial image redaction, marks on rotated pages, labels,
      bookmarks, crop, resize, split

### App
- [x] Form fields are filled in right on the page with the Select tool (text, check boxes, radio buttons, lists)
- [x] Select tool: click a note, shape, stamp or text box to edit it in the inspector; drag it to move it
- [x] Edit tool: click an image to select it, drag to move, drag the corner to resize; replace, save or delete it
- [x] New tools: insert image, shapes menu (rectangle, ellipse, line, arrow, polygon), text box, stamp, links,
      redact, crop
- [x] Redaction workflow: marked areas shown hatched, "Mark all" for a word or name, a cover choice, and a
      two-step confirmation before applying
- [x] Document tools menu: watermark, header and footer, Bates numbers, page labels, resize pages, split
      document, flatten, print
- [x] Editable bookmarks in the sidebar (add for the current page, rename, delete, reorder, indent, retarget)
- [x] Page labels shown under thumbnails
- [x] **Print** through the system dialog (pages rendered at 200 dpi, each at its own paper size)

### Known limits of v0.3
- ~~Signature fields are listed but cannot be signed yet~~ (done in v0.4)
- Redaction removes drawings only when they lie entirely under a box; a partly covered drawing stays (the cover
  box hides it on screen, but it remains in the file)
- Redacting text inside a form used several times removes it from every copy
- Printing goes through rendered page images, so very large documents take a while to prepare
- Links on rotated pages are placed by their rectangle only (no quad points)
- The new on-page interactions are type-checked, the build was launched against a generated document and two
  layout bugs found there were fixed, but the tools have not been clicked through by hand or by UI tests

## v0.4 — Professional (this build, 2026-11-11)

### Engine (Rust)
- [x] **OCR for scans**: text is recognised by the recogniser built into Windows (`Windows.Media.Ocr`, offline, with
      the installed language packs) and laid over the page as invisible text, every word stretched to where it
      was seen; upright on rotated pages; standard font for Latin text, an installed font for other scripts
- [x] **Signatures, validate**: every signature and document timestamp is checked against the file on disk:
      the signed byte range is hashed and the CMS signature verified (`adbe.pkcs7.detached`,
      `ETSI.CAdES.detached`, `adbe.pkcs7.sha1`, `ETSI.RFC3161`), the signer's certificate is shown and its chain
      checked against the authorities Windows trusts, and content appended after a signature is reported as such
      (revision n of m)
- [x] **Signatures, sign (PAdES baseline)**: `ETSI.CAdES.detached`, SHA-256, signing certificate bound into the
      signed attributes, the certificate chain included; visible signature (name, date, reason, location) or
      invisible; into an existing empty field or a new one; a second signature is appended so the first stays
      valid; certificates from the Windows store (smart cards and tokens included) or a `.pfx` file
- [x] **Digital IDs**: create a self-signed ID (RSA 3072) as a password-protected `.pfx`; nothing is installed
- [x] **Encryption**: a protected file keeps its protection on save, with both passwords, although only one was
      typed (the file key and password entries are written back); set new protection with an open password, a
      permissions password and individual permissions (AES-256 or AES-128); remove it; incremental saves of a
      protected file encrypt what they append
- [x] **Optimise**: images above a chosen resolution are resampled (photographs to JPEG, the rest lossless,
      transparency kept, only when the result is smaller); identical fonts, images and streams are merged;
      thumbnails and private application data dropped; object streams and a cross-reference stream on save
- [x] **PDF/A-1b / 2b / 3b check**: some 25 rules that documents fail in practice, each with its ISO 19005 clause:
      encryption, file identifier, XMP metadata and its claim, fonts not embedded (also in annotation
      appearances; invisible text exempt), output intent and device colours, scripts and actions, external
      streams, LZW, JPEG 2000 / transparency / layers (A-1), image hints, PostScript, transfer functions, glyph
      lists, attachments, XFA, annotation kinds, flags and appearances
- [x] **PDF/A conversion**: XMP written from the document information (and kept in step on every later save),
      sRGB output intent (profile generated in code), file identifier, scripts and reader-dependent actions
      removed, annotation flags set, LZW recompressed, XFA removed, and fonts that are not embedded taken from
      the fonts installed on this computer
- [x] **Compare two documents**: pages are paired by their text (inserted and removed pages found), words
      aligned, and every difference returned with its place on its page
- [x] **Export text**: paragraphs in the order the pages draw them
- [x] **Lighter undo**: a history step keeps only the objects a change touched, not a copy of the document;
      200 steps within a memory budget
- [x] **Appended rendering**: the bytes the window renders from grow by one small incremental update per change
      instead of being serialised afresh, and can be read in ranges, so a large document is never copied whole
- [x] **Changed pages**: every change reports which pages it can have repainted (following references up from
      the touched objects); the others keep their canvas
- [x] **Accessibility**: the structure tree read with role mapping, marked content located on its page (text and
      bounds per MCID), reading order per page, alternative text / actual text / title of an element, document
      language, "show title instead of file name"; a list of what assistive technology will stumble over
- [x] 56 tests (14 new): undo as differences, appended updates read back identically (also after a
      cross-reference stream), changed pages, encryption round trips with both passwords in both strengths,
      optimising, comparing, text export, the structure tree, the ICC profile, PDF/A check–convert–check, the OCR
      text layer, the Windows recogniser, and signing with a freshly made digital ID: valid, still valid after an
      appended edit, two signatures, and a changed byte caught

### App
- [x] Document tools menu: Recognise text, Signatures, Sign, Protect, Optimise, PDF/A, Accessibility, Compare,
      Export
- [x] **Signatures dialog**: one card per signature with a plain verdict, what was checked and what was not
- [x] **Sign dialog**: digital ID from Windows, from a file, or newly created; reason, location, visible or not,
      page and corner; sign in place or as a copy
- [x] **Protect dialog**: passwords, permissions, strength; "remove protection"
- [x] **Optimise dialog** with the size before and after
- [x] **PDF/A dialog**: standard, findings with clause and whether converting fixes them, convert
- [x] **Accessibility dialog**: issues, language, title display, structure tree, alternative text for figures,
      "next figure without text", reading order drawn over the page
- [x] **Compare view**: both documents side by side, removed words red, added words green, a list of changes
      to jump to, optional marking of differences in the picture (images and graphics)
- [x] **Export dialog**: pages as PNG or JPEG at a chosen resolution, or text
- [x] **OCR dialog**: language, pages, "skip pages that already have text", progress
- [x] Only the pages a change touched are rendered again, in the viewer and the thumbnails
- [x] Documents of 24 MB and more are read by the renderer in ranges
- [x] Top bar: protection and PDF/A badges; the "signed" badge opens the signatures

### Known limits of v0.4
- **Lazy object loading is not done**: the PDF library reads every object when a file is opened. Opening a very
  large file is as slow and as memory-hungry as before; what improved is everything after opening (moved to v1.0)
- Signatures: no timestamp from a timestamp authority yet (PAdES B-B, not B-T) and no long-term validation
  data; revocation is not checked online; `adbe.x509.rsa_sha1` signatures cannot be checked; certification
  signatures are recognised but their permitted-changes rules are not evaluated; a protected document cannot
  be signed
- The visible signature is placed in a page corner at a fixed size (no drawing it by hand yet)
- PDF/A: not a certified validator. Not checked: glyph widths against the font program, ICC profile contents,
  the XMP schema rules of PDF/A-1, PDF/A level a and u. Not converted: CMYK device colours (no CMYK profile is
  supplied), transparency for PDF/A-1, fonts that are neither embedded nor installed, composite fonts without a
  program. Embedding takes the whole font file
- OCR quality is that of the Windows recogniser; skewed scans are not straightened; the layer is placed word by
  word on a baseline estimated from the line's box; Windows only
- Optimise: JPEG 2000, JBIG2 and CCITT images and one-bit scans are left as they are; no font subsetting
- Compare is by words and, optionally, pixels: moved text shows as removed and added; no comparison of
  annotations or form values
- Accessibility: tags can be read and their texts edited, but not created, moved or retyped; content tagged
  inside reusable forms is not located
- Encryption: RC4-protected files keep RC4 when saved unchanged; certificate-based encryption is not supported
- The new dialogs and the compare view are type-checked and the build was launched (see below); they have not
  been clicked through by hand or by UI tests

## v1.0 — Release

- [ ] Large documents: lazy object loading (needs a different parser front end)
- [ ] Signatures: timestamps (PAdES B-T), long-term validation, placing a visible signature by drawing
- [ ] NSIS installer, code signing, auto-update
- [ ] Log file and crash reports
- [ ] German translation
- [ ] Browser preview backend for UI work (like Atomiq Studio's mock)
- [ ] Front-end unit tests (vitest) and end-to-end UI tests
- [ ] User guide
