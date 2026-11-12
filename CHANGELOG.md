# Changelog

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
