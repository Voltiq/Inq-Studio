# Inq Studio — user guide

Inq Studio is a PDF viewer and editor for Windows. This guide goes through what it does, in the order most
people need it. *Eine deutsche Fassung steht in [ANLEITUNG.md](ANLEITUNG.md).*

## The window

- **Top bar**: open, save, undo and redo on the left; the document's name, page and badges in the middle
  (*signed*, the kind of password protection, a PDF/A claim — click a badge for its dialog); theme, **document
  tools** (the wrench), settings and the inspector on the right.
- **Sidebar** (`Ctrl+B`): page thumbnails, bookmarks, fonts, comments, history.
- **Tool bar** above the page: the tools described below, page navigation, zoom, find.
- **Inspector** (`Ctrl+I`): what is selected, the options of the active tool, and the document's properties.
- Several documents open as tabs. A PDF opened from Explorer joins the window that is already open.

Every change can be undone (`Ctrl+Z`) until you close the document; the **History** tab lists the steps and
lets you jump to any of them. Nothing is written to the file before you save (`Ctrl+S`).

## Reading

Scroll, or `PgUp` / `PgDn`. `Ctrl`+wheel zooms; `Ctrl+0` fits the width. `Ctrl+F` finds text. With the
**Select** tool (`V`) you select and copy text; the **Hand** tool (`H`) drags the page.

## Editing text

Choose **Edit text and images** (`E`). Every line gets an outline.

- **Double-click** a line to rewrite it. If the line is part of a paragraph, the whole paragraph opens and is
  re-wrapped to its column when you confirm with `Ctrl+Enter`.
- **Drag** a line to move it; it snaps to the edges and baselines of other lines (`Alt` moves freely). Arrow
  keys nudge. `Delete` removes it. `Shift`-click or drag a box to select several lines.
- The inspector changes size, colour and font.

What you type is written **in the font the document already uses**. If that font lacks a character, Inq
Studio takes it from the same typeface installed on your computer; only if the typeface is nowhere to be found
is the missing character set in a similar font, and a notice tells you. The **Fonts** tab of the sidebar shows
for every font what an edit can do.

**Add text** (`T`): click where it should start, type, confirm with `Ctrl+Enter`.

## Images

With the edit tool, click an image to select it: drag to move, drag its corner to resize, and replace, save or
delete it in the inspector. **Insert image** (`I`) places a PNG or JPEG.

## Comments and shapes

Highlight (`M`), underline (`U`) and strike out (`K`) by dragging across text. Sticky note (`N`), drawing
(`D`), rectangle, ellipse, line, arrow, polygon, text box (`B`), stamp (`S`) and links (`L`) work by clicking
or dragging on the page. With the Select tool, click one to change it in the inspector or drag it to move it.

## Forms

Fill in form fields directly on the page with the Select tool. *Document tools › Flatten form fields* turns
the filled-in values into ordinary page content.

## Pages

In the **Pages** tab: drag thumbnails to reorder, right-click for rotate, duplicate, insert a blank page,
insert pages from another PDF, extract and delete. *Document tools* adds watermarks, headers and footers,
Bates numbers, page labels, resizing to a paper size and splitting into several files. **Crop** (`C`) hides
everything outside an area.

## Redacting

**Redact** (`X`) is for content that must be gone, not just covered. Drag over it — or let *Mark all* find
every occurrence of a word — check the marks, then apply them in the inspector. The text, the pixels of
images and the drawings under each mark are removed from the file itself. After saving they cannot be
brought back.

## Scans: recognising text (OCR)

*Document tools › Recognise text*. Choose the language of the text and the pages. The words are laid
invisibly over the scan, so it can be searched, selected and copied; the picture stays as it is. Recognition
runs on your computer with the OCR languages installed in Windows (Settings › Time & language › Language &
region › your language › Language options).

## Signatures

*Document tools › Signatures* checks every signature in the document and says, in plain words:

- whether the signed content is **unchanged**, and whether anything was added to the file afterwards;
- **who signed**, according to the certificate, and whether your computer trusts the authority that issued it.

*Sign…* signs the document with a digital ID:

- **From Windows**: a certificate in your Windows certificate store, including smart cards and tokens.
- **From a file**: a `.pfx` or `.p12` file and its password.
- **Create one**: a self-signed ID, saved as a `.pfx`. It proves a document has not changed since you signed
  it, but not who you are — for that you need a certificate from a trust service.

A signature can be shown on a page (name, date, reason) or be invisible. With a **timestamp server** set in
Settings, the signature also carries the time from a timestamp authority, which keeps it checkable after
your certificate expires.

Signing saves the document. Later changes are appended to the file, so the signature stays valid and shows
that something was added after it.

## Password protection

*Document tools › Protect with a password*. A **password to open** locks the document. A **permissions
password** with restrictions (no printing, no copying …) asks reading programs to hold back; it is a request,
not a lock. A protected document you open and save keeps its protection.

## Smaller files, archiving, comparing

- **Optimise** lowers the resolution of images that have more than you need, merges duplicate fonts and
  images, and shows the size before and after.
- **PDF/A** checks the document against the archival standards PDF/A-1b, 2b and 3b, lists what does not
  conform, and converts what can be fixed. The checks cover the rules documents fail in practice; they are
  not a certified validation.
- **Compare with…** shows two documents side by side: words removed in red, words added in green, and a
  list of the changes. *Mark differences in the picture* also finds changed images and graphics.

## Accessibility

*Document tools › Accessibility* lists what screen-reader users will stumble over, sets the document
language, lets you give figures alternative text, and can draw the reading order over the page.

## Export and print

*Document tools › Export pages or text* saves pages as PNG or JPEG at a resolution you choose, or the text
as a text file. `Ctrl+P` prints.

## Settings

Language (English, German), theme, zoom, units, your author name, how files are saved, the timestamp server,
updates, and where the log file is. Your documents stay on your computer: Inq Studio uploads nothing. The
only connections it makes are the daily check for a new version and, if you set one, the timestamp server —
which receives a hash, never the document.

## Shortcuts

| | |
| --- | --- |
| `Ctrl+O` / `Ctrl+S` / `Ctrl+Shift+S` | Open / save / save as |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo |
| `Ctrl+F` / `Ctrl+P` | Find / print |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+,` | Sidebar / inspector / settings |
| `Ctrl+W` / `Ctrl+Tab` | Close tab / next tab |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom in / out / fit width |
| `V` `H` `E` `T` `I` | Select, hand, edit, add text, insert image |
| `M` `U` `K` `N` `D` `B` `S` `L` | Highlight, underline, strike out, note, draw, text box, stamp, links |
| `X` `C` | Redact, crop |
