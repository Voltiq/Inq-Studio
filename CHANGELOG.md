# Changelog

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
