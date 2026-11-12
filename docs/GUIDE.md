# Inq Studio — manual

Inq Studio is a PDF viewer and editor for Windows. This manual goes through what it does, in the order most
people need it. The same text opens inside the app with `F1`. *Also in [Deutsch](ANLEITUNG.md), [Español](GUIA.md)
and [Français](MANUEL.md).*

## Installing and updating

Download the installer from the [releases page](https://github.com/Voltiq/Inq-Studio/releases/latest) and run
it. It installs for the current user and needs no administrator rights. The installer is not code-signed
yet: if SmartScreen warns, choose *More info › Run anyway*.

After installing, a PDF opens in Inq Studio with *Open with* in Explorer; make it the default there if you
like.

Inq Studio looks for a new version every time it starts. When there is one, it asks whether to install it:
**Update now**, **Later** (it asks again at the next start) or **Skip this version** (it asks again only
for the version after that). Tick *Don't ask at start* to get a short notice instead of the question; the
same switch is in Settings. *Settings › Updates* checks on demand and installs. Updates are signed, and the signature is checked before anything is installed.

## First steps

1. Open a PDF: the folder button, `Ctrl+O`, or drop files anywhere on the window.
2. Read, mark up or edit with the tools above the page.
3. Save with `Ctrl+S`, or under a new name with `Ctrl+Shift+S`.

On the first start a short **tour** points at the parts of the window. It can be started again from the
**Help** button (the question mark at the top right), which also opens this manual.

## The window

- **Title bar**: the app's icon and name, the open document, and the buttons to minimise, maximise and
  close the window. Drag it to move the window; double-click it to maximise.
- **Top bar**: open, save, undo and redo on the left; the document's name, page and badges in the middle
  (*signed*, the kind of password protection, a PDF/A claim — click a badge for its dialog); theme, **document
  tools** (the wrench), help, settings and the inspector on the right.
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
- **Drop** a line at the end or the start of another line and the two **join into one line**: the target
  is outlined while you drag, and each word keeps its own font, size and colour. Hold `Alt` to place a
  line next to another without joining (*Settings › Join lines dropped next to each other*).
- The inspector changes size, colour and font.

With **Realign text automatically** switched on in Settings (it is by default), a paragraph keeps its shape
when you change one of its lines: make a line shorter and the words that continue its sentence move up into
it; make it longer and the overflow wraps into the next lines; delete a line and the lines below it move up.
Only the lines of the same block of text move. Other blocks and the rest of the page stay where they are, so
a text that grows by a line can run into what is below it.

What you type is written **in the font the document already uses**. If that font lacks a character, Inq
Studio takes it from the same typeface installed on your computer; only if the typeface is nowhere to be found
is the missing character set in a similar font, and a notice tells you. The **Fonts** tab of the sidebar shows
for every font what an edit can do.

**Add text** (`T`): click where it should start, type, confirm with `Ctrl+Enter`.

### Text on scans

A scanned page is a picture; its words are pixels, not text. After *Recognise text* (see below) the words
can be selected, and also rewritten: double-click a recognised line with the edit tool. Because the old
words are part of the picture, Inq Studio covers the line with a patch in the colour of the paper and draws
your text over it in the colour of the ink. The new text is set in Helvetica or Arial, so it will not match
the scan's typeface exactly. Recognised text is edited one line at a time.

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
invisibly over the scan, so it can be searched, selected and copied; the picture stays as it is.

Recognition runs on your computer with the OCR languages installed in Windows. These are Windows language
packs and have nothing to do with the language of Inq Studio's interface.

- **Detect automatically** (offered when more than one language is installed) reads the first page with
  text in each installed language and keeps the one that fits.
- **Add a language to Windows**: pick a language and press *Install…*. Windows downloads the pack and asks
  for administrator rights; on a computer managed by a school or company this may not be allowed. The same
  can be done in Windows under *Settings › Time & language › Language & region*.

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

Language (English, German, Spanish, French; applies after a restart), theme, automatic realigning of text, zoom, units, your author name, how files are saved, the timestamp server,
updates, and where the log file is. Your documents stay on your computer: Inq Studio uploads nothing. The
only connections it makes are the check for a new version at start and, if you set one, the timestamp server —
which receives a hash, never the document.

## When something goes wrong

- **A PDF shows blank pages.** Update to the latest version; files from some scanners were read
  incompletely before 1.0.1. If it still happens, the file may be damaged: other readers repair silently.
- **A line cannot be edited.** The *Fonts* tab of the sidebar says for each font what an edit can do. Text
  that is part of a picture needs *Recognise text* first.
- **The language I need is missing in the OCR dialog.** Install its pack with *Add a language to Windows*.
- **Typed text is invisible on a scan.** Update to 1.0.2 or later.
- **The app crashed.** The next start shows the crash report. The log file and the reports are on your
  computer only (*Settings › Diagnostics › Show in folder*) and never contain the content of documents.
  Attach them when you report a problem at [github.com/Voltiq/Inq-Studio/issues](https://github.com/Voltiq/Inq-Studio/issues).

## Shortcuts

| | |
| --- | --- |
| `Ctrl+O` / `Ctrl+S` / `Ctrl+Shift+S` | Open / save / save as |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo |
| `Ctrl+F` / `Ctrl+P` | Find / print |
| `Ctrl+B` / `Ctrl+I` / `Ctrl+,` | Sidebar / inspector / settings |
| `F1` | This manual |
| `Ctrl+W` / `Ctrl+Tab` | Close tab / next tab |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom in / out / fit width |
| `V` `H` `E` `T` `I` | Select, hand, edit, add text, insert image |
| `M` `U` `K` `N` `D` `B` `S` `L` | Highlight, underline, strike out, note, draw, text box, stamp, links |
| `X` `C` | Redact, crop |
