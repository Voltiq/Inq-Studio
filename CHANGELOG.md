# Changelog

## 1.3.0 — 2026-11-12

### Added
- **Inq Studio asks about updates**: it looks for a newer version every time it starts and, when there is
  one, asks whether to install it. *Update now* installs and restarts, *Later* asks again at the next
  start, *Skip this version* waits for the one after it. Tick *Don't ask at start* (or switch off
  *Settings › Ask about updates at start*) to get a short notice instead.

### Changed
- The check for a new version runs at every start, not once a day.

## 1.2.0 — 2026-11-12

### Added
- **Text realigns itself**: when you rewrite one line of a paragraph, the words after it are wrapped again.
  A shorter line takes up the words that continue its sentence, a longer one spills into the next lines,
  and the rest of the block moves up or down with it. Deleting a line moves the lines below it up. Only
  the lines of the same block of text move. Switch it off under *Settings › Realign text automatically*.
- **Save as** has its own button next to Save.

## 1.1.0 — 2026-11-12

### Added
- **Its own title bar**: the window no longer has the Windows frame. The bar shows the app's icon and name,
  the open document, and the buttons to minimise, maximise and close. Drag it to move the window;
  double-click it to maximise.
- **A tour** on the first start: eight steps that point at the parts of the window. Start it again from the
  new **Help** button at the top right.
- **The manual inside the app**: `F1` or the Help button opens it, with a list of chapters, in the language
  of the interface.

### Changed
- The guide is now a full manual ([English](docs/GUIDE.md), [Deutsch](docs/ANLEITUNG.md)), with new chapters
  on installing and updating, first steps, text on scans, OCR languages and what to do when something goes
  wrong.
- The app's name and icon moved from the top bar into the title bar.

## 1.0.2 — 2026-11-12

### Added
- **OCR finds the language itself**: with more than one OCR language installed, "Detect automatically" reads
  the first page with text in each of them and keeps the one that fits for the rest of the document.
- **Adding an OCR language** from the OCR dialog: pick one (German is first) and Windows downloads its pack
  after asking for administrator rights. OCR languages are Windows language packs, separate from the
  language of the interface.

### Fixed
- **Editing recognised text on a scan showed nothing**: the text OCR adds is invisible, and an edit stayed
  invisible. Rewriting a recognised line now covers the old line in the colour of the paper and draws the
  new text over it in the colour of the ink. Recognised text is edited line by line.
- The chevron in the "More shapes" button sits in the middle.

## 1.0.1 — 2026-11-11

### Fixed
- **Scanned PDFs that opened as blank pages**: files written by some scanner software put white space where
  the PDF rules allow only a line end, and the page content was skipped on opening. Such files are now read
  in full.
- The close cross on document tabs sits in the middle of its hover square.

## 1.0.0 — 2026-11-11

The first release with an installer. Everything from the 0.x builds is in: editing text in the document's own
fonts, markup, forms, images, redaction, page tools, signatures, password protection, OCR, comparing,
optimising, PDF/A and accessibility. See [ROADMAP.md](ROADMAP.md) for the full list.

### Added
- **Installer** for Windows (per user, no administrator rights), with the `.pdf` file association.
- **Updates**: Inq Studio checks for a new version once a day and tells you; Settings › Updates checks on
  demand and installs. Updates are signed and checked before they install.
- **German**: the whole interface, following Windows or chosen in Settings › Language.
- **Timestamps for signatures**: with a timestamp server set in Settings, a signature carries the time from a
  timestamp authority (PAdES B-T), so it stays checkable after the certificate expires.
- **Log file and crash reports**, kept on this computer only (Settings › Diagnostics). After a crash, the next
  start shows the report.
- **One window**: opening another PDF from Explorer adds it as a tab to the window that is already open.
- **Dropdowns** in the app's own look, usable from the keyboard.

### Changed
- Switching between the light and the dark theme changes every element at once.

### For contributors
- `npm run dev` now works in a plain browser: a stand-in for the engine answers with a sample document, for
  working on the interface.
- `npm test` runs the interface tests (dropdown, page ranges, translations, a dialog against the stand-in);
  `cargo test` runs the 57 engine tests.
- `node scripts/i18n-wrap.mjs` lists the interface strings; a test fails when one has no German translation.
