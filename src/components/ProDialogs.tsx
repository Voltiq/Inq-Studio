import { useEffect, useMemo, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { BadgeCheck, CircleAlert, CircleCheck, FileKey, FolderOpen, Image as ImageIcon, PenLine, ShieldAlert, ShieldCheck, ShieldQuestion, Stamp } from "lucide-react";
import { Modal } from "./ui/Modal";
import { Button } from "./ui/Button";
import { Checkbox, RadioList, Segmented, Select, TextField, Toggle } from "./ui/Field";
import { ProgressBar, SectionMessage, Spinner } from "./ui/Feedback";
import { ScopePicker, useScope } from "./ToolDialogs";
import {
  api,
  errorText,
  type Allowed,
  type DocInfo,
  type DocState,
  type EditResult,
  type Identity,
  type IdentityRef,
  type OcrPage,
  type OptimizeOptions,
  type OptimizeReport,
  type OrderItem,
  type PdfaReport,
  type ProtectRequest,
  type Rect,
  type SigInfo,
  type Signatures,
  type SignOptions,
  type TagInfo,
  type TagNode,
} from "../lib/api";
import type { PDFDocumentProxy } from "../lib/pdf";
import { grayPixels, renderPage } from "../lib/raster";
import { bytes, plural } from "../lib/format";
import { cx } from "../lib/utils";

/** Runs a change on the document and reloads it; resolves to whether it worked. */
export type Run = (fn: () => Promise<DocState | EditResult>, success?: string) => Promise<boolean>;

const stack = { display: "grid", gap: 16 } as const;
const note = { fontSize: 12 } as const;

function when(iso: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

const PERMISSIONS: { key: keyof Allowed; label: string }[] = [
  { key: "print", label: "Print" },
  { key: "printHigh", label: "Print at full quality" },
  { key: "copy", label: "Copy text and images" },
  { key: "modify", label: "Change the content" },
  { key: "assemble", label: "Insert, delete and rotate pages" },
  { key: "annotate", label: "Comment and mark up" },
  { key: "fill", label: "Fill in form fields" },
];

export function ProtectDialog({ info, onApply, onClose }: { info: DocInfo; onApply: (req: ProtectRequest | null) => Promise<boolean>; onClose: () => void }) {
  const sec = info.security;
  const [open, setOpen] = useState("");
  const [open2, setOpen2] = useState("");
  const [owner, setOwner] = useState("");
  const [restrict, setRestrict] = useState(sec.protected && PERMISSIONS.some((p) => !sec.allowed[p.key]));
  const [allowed, setAllowed] = useState<Allowed>(sec.allowed);
  const [aes256, setAes256] = useState(sec.method !== "AES-128");
  const [busy, setBusy] = useState(false);
  const mismatch = open !== open2;
  const nothing = !open && !(restrict && owner);
  const problem = mismatch
    ? "The two open passwords differ."
    : restrict && !owner && !open
      ? "Restrictions need a permissions password."
      : restrict && owner && owner === open
        ? "Use a different permissions password; with the same one, everybody who can open the file has every right."
        : null;
  const apply = async (req: ProtectRequest | null) => {
    setBusy(true);
    const ok = await onApply(req);
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <Modal
      title="Protect with a password"
      onClose={onClose}
      width="medium"
      closeOnBlanket={false}
      footer={
        <>
          {sec.protected && (
            <span className="modal-footer-start">
              <Button appearance="subtle" disabled={busy} onClick={() => apply(null)}>
                Remove protection
              </Button>
            </span>
          )}
          <Button appearance="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button
            appearance="primary"
            loading={busy}
            disabled={nothing || !!problem}
            onClick={() => apply({ openPassword: open, ownerPassword: restrict ? owner : "", allowed: restrict ? allowed : { ...allowed, print: true, printHigh: true, modify: true, copy: true, annotate: true, fill: true, assemble: true }, aes256 })}
          >
            Protect
          </Button>
        </>
      }
    >
      <div style={stack}>
        {sec.protected && (
          <SectionMessage appearance="information" title={sec.pending ? `Will be protected with ${sec.method} on the next save` : `Protected with ${sec.method}`}>
            {sec.pending
              ? "Save the document to write the protection."
              : "Saving keeps this protection and both of its passwords. Enter new passwords below to replace it."}
          </SectionMessage>
        )}
        <div className="row">
          <TextField label="Password to open" type="password" value={open} autoComplete="new-password" onChange={(e) => setOpen(e.target.value)} />
          <TextField label="Repeat" type="password" value={open2} autoComplete="new-password" error={mismatch && open2 ? "Does not match" : null} onChange={(e) => setOpen2(e.target.value)} />
        </div>
        <p className="is-muted" style={note}>
          Leave empty for a document that anyone can open but that restricts what they may do. There is no way to recover a forgotten password.
        </p>
        <Toggle label="Restrict what readers may do" checked={restrict} onChange={setRestrict} />
        {restrict && (
          <>
            <TextField
              label="Permissions password"
              type="password"
              value={owner}
              autoComplete="new-password"
              helper="Whoever knows it can change or remove the restrictions."
              onChange={(e) => setOwner(e.target.value)}
            />
            <div className="check-grid">
              {PERMISSIONS.map((p) => (
                <Checkbox
                  key={p.key}
                  label={p.label}
                  checked={allowed[p.key]}
                  disabled={p.key === "printHigh" && !allowed.print}
                  onChange={(v) => setAllowed({ ...allowed, [p.key]: v })}
                />
              ))}
            </div>
            <p className="is-muted" style={note}>
              Restrictions are a request to the reading program, not a lock: programs that ignore them exist. Screen readers are always let in.
            </p>
          </>
        )}
        <Select
          label="Encryption"
          value={aes256 ? "256" : "128"}
          onChange={(v) => setAes256(v === "256")}
          options={[
            { value: "256", label: "AES-256 (Acrobat X and later)" },
            { value: "128", label: "AES-128 (Acrobat 7 and later)" },
          ]}
        />
        {problem && <SectionMessage appearance="warning">{problem}</SectionMessage>}
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Optimise
// ---------------------------------------------------------------------------

export function OptimizeDialog({ info, onRun, onClose }: { info: DocInfo; onRun: (o: OptimizeOptions) => Promise<OptimizeReport | null>; onClose: () => void }) {
  const [images, setImages] = useState(true);
  const [dpi, setDpi] = useState("150");
  const [quality, setQuality] = useState(75);
  const [merge, setMerge] = useState(true);
  const [extras, setExtras] = useState(true);
  const [compact, setCompact] = useState(true);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<OptimizeReport | null>(null);
  const run = async () => {
    setBusy(true);
    const r = await onRun({ imageDpi: images ? Math.max(36, parseFloat(dpi) || 150) : 0, jpegQuality: quality, mergeDuplicates: merge, discardExtras: extras, compact });
    setBusy(false);
    setReport(r);
  };
  const saved = report ? report.sizeBefore - report.sizeAfter : 0;
  return (
    <Modal
      title="Optimise"
      onClose={onClose}
      width="medium"
      closeOnBlanket={false}
      footer={
        report ? (
          <Button appearance="primary" onClick={onClose}>
            Done
          </Button>
        ) : (
          <>
            <Button appearance="subtle" onClick={onClose}>
              Cancel
            </Button>
            <Button appearance="primary" loading={busy} onClick={run}>
              Optimise
            </Button>
          </>
        )
      }
    >
      {report ? (
        <div style={stack}>
          <SectionMessage appearance={saved > 0 ? "success" : "information"} title={saved > 0 ? `${bytes(saved)} smaller (${Math.round((saved / Math.max(1, report.sizeBefore)) * 100)} %)` : "Already as small as these settings make it"}>
            {bytes(report.sizeBefore)} → {bytes(report.sizeAfter)} once saved. Undo brings the original back until you save.
          </SectionMessage>
          <dl className="props">
            <dt>Images resampled</dt>
            <dd>
              {report.imagesResampled} of {report.imagesResampled + report.imagesKept}
              {report.imageBytesBefore > 0 && ` · ${bytes(report.imageBytesBefore)} → ${bytes(report.imageBytesAfter)}`}
            </dd>
            <dt>Duplicates merged</dt>
            <dd>
              {report.duplicatesMerged}
              {report.fontsMerged > 0 && ` (${plural(report.fontsMerged, "font object")})`}
            </dd>
            <dt>Extras removed</dt>
            <dd>{report.extrasRemoved}</dd>
          </dl>
        </div>
      ) : (
        <div style={stack}>
          {info.state.signed && (
            <SectionMessage appearance="warning" title="This document is signed">
              Optimising rewrites the file, which invalidates its signatures.
            </SectionMessage>
          )}
          <Toggle label="Reduce image resolution" description="Images drawn at a higher resolution are resampled; photographs are stored as JPEG." checked={images} onChange={setImages} />
          {images && (
            <>
              <Select
                label="Resolution"
                value={dpi}
                onChange={setDpi}
                options={[
                  { value: "96", label: "96 ppi — screen" },
                  { value: "150", label: "150 ppi — e-mail and office printing" },
                  { value: "220", label: "220 ppi — good printing" },
                  { value: "300", label: "300 ppi — print production" },
                ]}
              />
              <label className="field">
                <span className="field-label">JPEG quality · {quality}</span>
                <input type="range" min={30} max={95} step={5} value={quality} onChange={(e) => setQuality(Number(e.target.value))} />
              </label>
            </>
          )}
          <Toggle label="Merge duplicate fonts and images" description="A font or picture embedded once per page is kept once." checked={merge} onChange={setMerge} />
          <Toggle label="Discard thumbnails and private application data" checked={extras} onChange={setExtras} />
          <Toggle label="Compact file structure" description="Object streams (PDF 1.5). Every current reader opens them; very old ones do not." checked={compact} onChange={setCompact} />
          {busy && <ProgressBar indeterminate label="Optimising" />}
        </div>
      )}
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

type ExportFormat = "png" | "jpeg" | "text";

export function ExportDialog({
  docId,
  pdf,
  name,
  pages,
  current,
  selected,
  onDone,
  onClose,
}: {
  docId: number;
  pdf: PDFDocumentProxy;
  name: string;
  pages: number;
  current: number;
  selected: number[];
  onDone: (kind: "success" | "error", title: string, body?: string) => void;
  onClose: () => void;
}) {
  const { scope, setScope, range, setRange, list } = useScope(pages, current, selected);
  const [format, setFormat] = useState<ExportFormat>("png");
  const [dpi, setDpi] = useState("150");
  const [quality, setQuality] = useState(90);
  const [progress, setProgress] = useState<number | null>(null);
  const base = name.replace(/\.pdf$/i, "");
  const run = async () => {
    try {
      if (format === "text") {
        const path = await saveDialog({ defaultPath: `${base}.txt`, filters: [{ name: "Text", extensions: ["txt"] }] });
        if (!path) return;
        setProgress(0);
        const n = await api.exportText(docId, list, path);
        onDone("success", n ? "Text exported" : "No text found", n ? path : "These pages hold no text. Recognise text (OCR) first if they are scans.");
        onClose();
        return;
      }
      const folder = await openDialog({ directory: true });
      if (typeof folder !== "string") return;
      const ext = format === "png" ? "png" : "jpg";
      const digits = String(pages).length;
      for (let i = 0; i < list.length; i++) {
        setProgress(i / list.length);
        const canvas = await renderPage(pdf, list[i], (parseFloat(dpi) || 150) / 72);
        const blob: Blob | null = await new Promise((r) => canvas.toBlob(r, format === "png" ? "image/png" : "image/jpeg", quality / 100));
        if (!blob) throw new Error(`Page ${list[i] + 1} could not be rendered.`);
        const file = `${folder}\\${base} p${String(list[i] + 1).padStart(digits, "0")}.${ext}`;
        await api.writeFile(file, new Uint8Array(await blob.arrayBuffer()));
      }
      onDone("success", `Exported ${plural(list.length, "page")}`, folder);
      onClose();
    } catch (e) {
      onDone("error", "Export failed", errorText(e));
    } finally {
      setProgress(null);
    }
  };
  return (
    <Modal
      title="Export"
      onClose={onClose}
      width="medium"
      closeOnBlanket={false}
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button appearance="primary" iconBefore={format === "text" ? undefined : FolderOpen} loading={progress != null} disabled={!list.length} onClick={run}>
            Export {plural(list.length, "page")}…
          </Button>
        </>
      }
    >
      <div style={stack}>
        <Segmented<ExportFormat>
          label="Format"
          value={format}
          onChange={setFormat}
          options={[
            { value: "png", label: "PNG images" },
            { value: "jpeg", label: "JPEG images" },
            { value: "text", label: "Plain text" },
          ]}
        />
        <ScopePicker scope={scope} onScope={setScope} range={range} onRange={setRange} selected={selected.length} />
        {format !== "text" && (
          <Select
            label="Resolution"
            value={dpi}
            onChange={setDpi}
            options={[
              { value: "72", label: "72 ppi — as on screen" },
              { value: "150", label: "150 ppi" },
              { value: "300", label: "300 ppi — print" },
              { value: "600", label: "600 ppi" },
            ]}
          />
        )}
        {format === "jpeg" && (
          <label className="field">
            <span className="field-label">Quality · {quality}</span>
            <input type="range" min={40} max={100} step={5} value={quality} onChange={(e) => setQuality(Number(e.target.value))} />
          </label>
        )}
        <p className="is-muted" style={note}>
          {format === "text"
            ? "Paragraphs in the order the pages draw them, one file, pages separated by a form feed."
            : "One file per page, named after the document and the page number. Annotations are included as shown."}
        </p>
        {progress != null && <ProgressBar value={progress} label="Exporting" />}
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// OCR
// ---------------------------------------------------------------------------

export function OcrDialog({
  docId,
  pdf,
  pages,
  current,
  selected,
  onApply,
  onClose,
}: {
  docId: number;
  pdf: PDFDocumentProxy;
  pages: number;
  current: number;
  selected: number[];
  onApply: (pages: OcrPage[], lang: string) => Promise<boolean>;
  onClose: () => void;
}) {
  const { scope, setScope, range, setRange, list } = useScope(pages, current, selected);
  const [setup, setSetup] = useState<{ languages: { tag: string; name: string }[]; maxDimension: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lang, setLang] = useState("");
  const [skip, setSkip] = useState(true);
  const [progress, setProgress] = useState<{ done: number; total: number; words: number } | null>(null);
  const [cancel] = useState({ now: false });
  useEffect(() => {
    api
      .ocrSetup()
      .then((s) => {
        setSetup(s);
        const mine = navigator.language.toLowerCase();
        const best = s.languages.find((l) => l.tag.toLowerCase() === mine) ?? s.languages.find((l) => l.tag.slice(0, 2).toLowerCase() === mine.slice(0, 2)) ?? s.languages[0];
        setLang(best?.tag ?? "");
      })
      .catch((e) => setError(errorText(e)));
    return () => {
      cancel.now = true;
    };
  }, [cancel]);
  const run = async () => {
    if (!setup) return;
    const out: OcrPage[] = [];
    let words = 0;
    let skipped = 0;
    setError(null);
    try {
      for (let i = 0; i < list.length; i++) {
        if (cancel.now) return;
        setProgress({ done: i, total: list.length, words });
        if (skip) {
          const text = await api.pageText(docId, list[i]);
          if (text.lines.length) {
            skipped++;
            continue;
          }
        }
        // 300 ppi reads small print reliably; very large pages are scaled to what Windows accepts.
        const page = await pdf.getPage(list[i] + 1);
        const base = page.getViewport({ scale: 1 });
        const scale = Math.min(300 / 72, (setup.maxDimension - 8) / Math.max(base.width, base.height));
        const canvas = await renderPage(pdf, list[i], scale);
        const found = await api.ocrRecognize(grayPixels(canvas), canvas.width, canvas.height, lang);
        if (found.lines.length) {
          // The canvas is the viewport at this scale, rounded down to whole pixels.
          const vp = page.getViewport({ scale });
          out.push({ page: list[i], transform: vp.transform, lines: found.lines });
          words += found.lines.reduce((n, l) => n + l.words.length, 0);
        }
      }
      setProgress({ done: list.length, total: list.length, words });
      if (!out.length) {
        setError(skipped === list.length ? "Every page in this range already has text. Turn off “Skip pages that already have text” to recognise them again." : "No text was recognised on these pages.");
        setProgress(null);
        return;
      }
      if (await onApply(out, lang)) onClose();
      else setProgress(null);
    } catch (e) {
      setError(errorText(e));
      setProgress(null);
    }
  };
  return (
    <Modal
      title="Recognise text (OCR)"
      onClose={onClose}
      width="medium"
      closeOnBlanket={false}
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            Cancel
          </Button>
          <Button appearance="primary" loading={progress != null} disabled={!list.length || !setup?.languages.length} onClick={run}>
            Recognise {plural(list.length, "page")}
          </Button>
        </>
      }
    >
      <div style={stack}>
        <p className="is-muted">
          Scanned pages are pictures of text. Recognition lays the words invisibly over the picture, so they can be searched, selected and copied. The scan itself stays as it is.
        </p>
        {!setup && !error && <Spinner label="Asking Windows for its OCR languages" />}
        {setup && !setup.languages.length && (
          <SectionMessage appearance="warning" title="No OCR language is installed">
            Add one in Windows under Settings › Time &amp; language › Language &amp; region › your language › Language options › Optical character recognition.
          </SectionMessage>
        )}
        {setup && setup.languages.length > 0 && (
          <>
            <Select label="Language of the text" value={lang} onChange={setLang} options={setup.languages.map((l) => ({ value: l.tag, label: l.name }))} helper="Languages come from the OCR packs installed in Windows; recognition runs on this computer." />
            <ScopePicker scope={scope} onScope={setScope} range={range} onRange={setRange} selected={selected.length} />
            <Toggle label="Skip pages that already have text" checked={skip} onChange={setSkip} />
          </>
        )}
        {progress && (
          <>
            <ProgressBar value={progress.done / Math.max(1, progress.total)} label="Recognising" />
            <p className="is-muted" style={note}>
              Page {Math.min(progress.done + 1, progress.total)} of {progress.total} · {plural(progress.words, "word")} so far
            </p>
          </>
        )}
        {error && <SectionMessage appearance="warning">{error}</SectionMessage>}
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// PDF/A
// ---------------------------------------------------------------------------

export function PdfaDialog({
  docId,
  revision,
  claimed,
  onConvert,
  onClose,
}: {
  docId: number;
  revision: number;
  claimed: string | null;
  onConvert: (part: number) => Promise<{ done: string[]; report: PdfaReport } | null>;
  onClose: () => void;
}) {
  const [part, setPart] = useState(claimed?.startsWith("PDF/A-1") ? "1" : claimed?.startsWith("PDF/A-3") ? "3" : "2");
  const [report, setReport] = useState<PdfaReport | null>(null);
  const [done, setDone] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setReport(null);
    api
      .pdfaCheck(docId, Number(part))
      .then((r) => live && setReport(r))
      .catch((e) => live && setError(errorText(e)));
    return () => {
      live = false;
    };
  }, [docId, revision, part]);
  const fixable = report?.issues.filter((i) => i.fixable).length ?? 0;
  const convert = async () => {
    setBusy(true);
    const r = await onConvert(Number(part));
    setBusy(false);
    if (r) {
      setDone(r.done);
      setReport(r.report);
    }
  };
  return (
    <Modal
      title="PDF/A — archiving"
      onClose={onClose}
      width="large"
      closeOnBlanket={false}
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            Close
          </Button>
          <Button appearance="primary" loading={busy} disabled={!report || (report.passed && report.claimed === report.target)} onClick={convert}>
            Convert to {report?.target ?? "PDF/A"}
          </Button>
        </>
      }
    >
      <div style={stack}>
        <Segmented
          label="Standard"
          value={part}
          onChange={(p) => {
            setDone(null);
            setPart(p);
          }}
          options={[
            { value: "1", label: "PDF/A-1b" },
            { value: "2", label: "PDF/A-2b" },
            { value: "3", label: "PDF/A-3b" },
          ]}
        />
        <p className="is-muted" style={note}>
          {part === "1" ? "The oldest and strictest: PDF 1.4, no transparency, no layers, no attachments." : part === "2" ? "The usual choice: allows transparency, layers and JPEG 2000." : "Like PDF/A-2, and any file may be attached (used for e-invoices)."}
        </p>
        {error && <SectionMessage appearance="danger">{error}</SectionMessage>}
        {!report && !error && <Spinner label="Checking" />}
        {report && (
          <>
            <SectionMessage
              appearance={report.passed ? "success" : fixable === report.issues.length ? "information" : "warning"}
              title={
                report.passed
                  ? `Passes all ${report.checked} checks for ${report.target}`
                  : `${plural(report.issues.length, "rule")} of ${report.checked} not met${fixable ? ` · converting fixes ${fixable}` : ""}`
              }
            >
              {report.claimed ? `The document says it is ${report.claimed}. ` : "The document does not claim to be PDF/A. "}
              These checks cover the rules documents fail in practice; they are not a certified validation (glyph-level font checks and colour management details are left out).
            </SectionMessage>
            {done && done.length > 0 && (
              <SectionMessage appearance="success" title="Converted — save to keep it">
                <ul className="plain-list">
                  {done.map((d) => (
                    <li key={d}>{d}</li>
                  ))}
                </ul>
              </SectionMessage>
            )}
            {report.issues.length > 0 && (
              <ul className="issue-list">
                {report.issues.map((i) => (
                  <li key={i.id} className="issue">
                    {i.fixable ? <CircleCheck size={15} className="issue-icon is-fixable" aria-hidden="true" /> : <CircleAlert size={15} className="issue-icon" aria-hidden="true" />}
                    <div>
                      <p className="issue-title">
                        {i.message}
                        {i.count > 1 && <span className="is-muted"> ×{i.count}</span>}
                      </p>
                      {i.detail && <p className="issue-detail">{i.detail}</p>}
                      <p className="issue-detail">
                        ISO 19005-{part}, {i.clause} · {i.fixable ? "fixed by converting" : "cannot be fixed automatically"}
                      </p>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </div>
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Accessibility
// ---------------------------------------------------------------------------

function TagRow({ node, depth, selected, onSelect }: { node: TagNode; depth: number; selected: string | null; onSelect: (n: TagNode) => void }) {
  const [open, setOpen] = useState(depth < 2);
  const figure = node.role === "Figure" || node.role === "Formula";
  const missing = figure && !node.alt.trim() && !node.actualText.trim();
  return (
    <>
      <button
        type="button"
        className={cx("tag-row", selected != null && selected === node.id && "is-selected")}
        style={{ paddingLeft: 8 + depth * 14 }}
        onClick={() => {
          onSelect(node);
          if (node.children.length) setOpen((v) => !v);
        }}
      >
        <span className="tag-caret">{node.children.length ? (open ? "▾" : "▸") : ""}</span>
        <span className={cx("tag-role", missing && "is-missing")}>{node.role || "?"}</span>
        <span className="tag-text">{figure ? node.alt || node.actualText || "no alternative text" : node.text}</span>
        {node.page != null && <span className="tag-page">{node.page + 1}</span>}
      </button>
      {open && node.children.map((c, i) => <TagRow key={c.id || i} node={c} depth={depth + 1} selected={selected} onSelect={onSelect} />)}
    </>
  );
}

export function AccessibilityDialog({
  docId,
  revision,
  page,
  run,
  orderShown,
  onShowOrder,
  onNavigate,
  onClose,
}: {
  docId: number;
  revision: number;
  page: number;
  run: Run;
  orderShown: boolean;
  onShowOrder: (show: boolean) => void;
  onNavigate: (page: number) => void;
  onClose: () => void;
}) {
  const [info, setInfo] = useState<TagInfo | null>(null);
  const [lang, setLang] = useState("");
  const [picked, setPicked] = useState<TagNode | null>(null);
  const [alt, setAlt] = useState("");
  const [order, setOrder] = useState<OrderItem[]>([]);
  useEffect(() => {
    let live = true;
    api.tags(docId).then((t) => {
      if (!live) return;
      setInfo(t);
      setLang(t.lang);
    });
    api
      .readingOrder(docId, page)
      .then((o) => live && setOrder(o))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [docId, revision, page]);
  const firstMissing = useMemo(() => {
    const find = (nodes: TagNode[]): TagNode | null => {
      for (const n of nodes) {
        if ((n.role === "Figure" || n.role === "Formula") && !n.alt.trim() && !n.actualText.trim() && n.id) return n;
        const c = find(n.children);
        if (c) return c;
      }
      return null;
    };
    return info ? find(info.nodes) : null;
  }, [info]);
  const pick = (n: TagNode) => {
    setPicked(n);
    setAlt(n.alt);
    if (n.page != null) onNavigate(n.page);
  };
  return (
    <Modal title="Accessibility" onClose={onClose} width="large" closeOnBlanket={false} footer={<Button onClick={onClose}>Close</Button>}>
      {!info ? (
        <Spinner label="Reading the structure" />
      ) : (
        <div style={stack}>
          {info.issues.length ? (
            <SectionMessage appearance="warning" title={`${plural(info.issues.length, "thing")} to look at`}>
              <ul className="plain-list">
                {info.issues.map((i) => (
                  <li key={i}>{i}</li>
                ))}
              </ul>
            </SectionMessage>
          ) : (
            <SectionMessage appearance="success" title="Tagged, with a language, a title and alternative text for every figure" />
          )}
          <div className="row">
            <TextField label="Document language" value={lang} placeholder="en-GB, de, fr-CH …" mono onChange={(e) => setLang(e.target.value)} />
            <Button disabled={lang.trim() === info.lang} onClick={() => run(() => api.setTagDocument(docId, lang.trim(), null), "Language set")}>
              Set
            </Button>
          </div>
          <Toggle
            label="Show the title instead of the file name"
            description={info.title ? `“${info.title}” — change it under Document properties in the inspector.` : "The document has no title yet; set one under Document properties in the inspector."}
            checked={info.displayTitle}
            onChange={(v) => run(() => api.setTagDocument(docId, null, v))}
          />
          <Toggle
            label={`Show the reading order on page ${page + 1}`}
            description={info.hasTree ? `${plural(order.length, "tagged piece")} of content, numbered in the order a screen reader speaks them.` : "Without tags, readers follow the order the page draws its paragraphs."}
            checked={orderShown}
            onChange={onShowOrder}
          />
          {info.hasTree && (
            <>
              <div className="tag-head">
                <span className="eyebrow">
                  Structure · {plural(info.elements, "element")}
                  {info.truncated ? " (first part shown)" : ""}
                </span>
                {firstMissing && (
                  <Button spacing="compact" iconBefore={ImageIcon} onClick={() => pick(firstMissing)}>
                    Next figure without text
                  </Button>
                )}
              </div>
              <div className="tag-tree">
                {info.nodes.map((n, i) => (
                  <TagRow key={n.id || i} node={n} depth={0} selected={picked?.id ?? null} onSelect={pick} />
                ))}
              </div>
              {picked && (picked.role === "Figure" || picked.role === "Formula") && (
                <div className="row">
                  <TextField
                    label={`Alternative text for this ${picked.role.toLowerCase()}${picked.page != null ? ` on page ${picked.page + 1}` : ""}`}
                    value={alt}
                    placeholder="What the picture shows, in a sentence"
                    onChange={(e) => setAlt(e.target.value)}
                  />
                  <Button
                    appearance="primary"
                    disabled={!picked.id || alt === picked.alt}
                    onClick={async () => {
                      if (await run(() => api.setTagElement(docId, picked.id, { alt }), "Alternative text saved")) setPicked({ ...picked, alt });
                    }}
                  >
                    Save
                  </Button>
                </div>
              )}
            </>
          )}
        </div>
      )}
    </Modal>
  );
}

// ---------------------------------------------------------------------------
// Signatures
// ---------------------------------------------------------------------------

function verdict(s: SigInfo): { tone: "success" | "warning" | "danger"; icon: typeof ShieldCheck; title: string } {
  if (s.intact === false) return { tone: "danger", icon: ShieldAlert, title: "Invalid: the document was changed, or the signature is damaged" };
  if (s.intact == null) return { tone: "warning", icon: ShieldQuestion, title: "Could not be checked" };
  if (s.trust === "revoked") return { tone: "danger", icon: ShieldAlert, title: "The signer's certificate was revoked" };
  const what = s.kind === "timestamp" ? "Timestamp" : s.kind === "certification" ? "Certification" : "Signature";
  if (s.trust === "trusted") return { tone: "success", icon: ShieldCheck, title: `${what} is valid` };
  return { tone: "warning", icon: ShieldQuestion, title: `${what} is intact, but the signer's identity is not verified` };
}

export function SignaturesDialog({
  docId,
  revision,
  onSign,
  onNavigate,
  onClose,
}: {
  docId: number;
  revision: number;
  onSign: (field: string | null) => void;
  onNavigate: (page: number) => void;
  onClose: () => void;
}) {
  const [data, setData] = useState<Signatures | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    api
      .signatures(docId)
      .then((d) => live && setData(d))
      .catch((e) => live && setError(errorText(e)));
    return () => {
      live = false;
    };
  }, [docId, revision]);
  return (
    <Modal
      title="Signatures"
      onClose={onClose}
      width="large"
      closeOnBlanket={false}
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            Close
          </Button>
          <Button appearance="primary" iconBefore={PenLine} onClick={() => onSign(null)}>
            Sign…
          </Button>
        </>
      }
    >
      <div style={stack}>
        {error && <SectionMessage appearance="danger">{error}</SectionMessage>}
        {!data && !error && <Spinner label="Checking signatures" />}
        {data && data.unsaved && data.signatures.length > 0 && (
          <SectionMessage appearance="information">The signatures are checked against the saved file; your unsaved changes are not part of it yet.</SectionMessage>
        )}
        {data && !data.signatures.length && <p className="is-muted">This document is not signed.</p>}
        {data?.signatures.map((s) => {
          const v = verdict(s);
          return (
            <section key={s.field} className={cx("sig-card", `is-${v.tone}`)}>
              <header className="sig-head">
                <v.icon size={18} aria-hidden="true" />
                <div>
                  <p className="sig-title">{v.title}</p>
                  <p className="sig-sub">
                    {s.signer || "Unknown signer"}
                    {s.time && ` · ${when(s.time)}`}
                  </p>
                </div>
                <span className="spacer" />
                {s.page != null && (
                  <Button spacing="compact" appearance="subtle" onClick={() => onNavigate(s.page!)}>
                    Page {s.page + 1}
                  </Button>
                )}
              </header>
              <dl className="props">
                <dt>Document</dt>
                <dd>
                  {s.intact === false
                    ? s.problem
                    : s.intact == null
                      ? s.problem
                      : s.coversWhole
                        ? "Unchanged since it was signed."
                        : `Unchanged in the signed part (revision ${s.revision} of ${s.revisions}); content was added to the file afterwards.`}
                </dd>
                <dt>Signer</dt>
                <dd>
                  {s.trustDetail || "—"}
                  {s.issuer && ` Issued by ${s.issuer}.`}
                </dd>
                {s.subject && (
                  <>
                    <dt>Certificate</dt>
                    <dd>
                      {s.subject}
                      {s.validTo && ` · valid ${when(s.validFrom)} – ${when(s.validTo)}`}
                    </dd>
                  </>
                )}
                {s.reason && (
                  <>
                    <dt>Reason</dt>
                    <dd>{s.reason}</dd>
                  </>
                )}
                {s.location && (
                  <>
                    <dt>Location</dt>
                    <dd>{s.location}</dd>
                  </>
                )}
                <dt>Details</dt>
                <dd>
                  {[s.field, s.subFilter, s.digest, s.timestamped ? "timestamped by an authority" : "time from the signer's clock"].filter(Boolean).join(" · ")}
                  {s.digest === "SHA-1" || s.digest === "MD5" ? " — this digest is no longer considered secure" : ""}
                </dd>
              </dl>
            </section>
          );
        })}
        {data && data.emptyFields.length > 0 && (
          <div>
            <p className="eyebrow">Empty signature fields</p>
            <div className="inspector-actions">
              {data.emptyFields.map((f) => (
                <Button key={f} spacing="compact" iconBefore={PenLine} onClick={() => onSign(f)}>
                  Sign {f}
                </Button>
              ))}
            </div>
          </div>
        )}
      </div>
    </Modal>
  );
}

type Corner = "bottom-right" | "bottom-left" | "top-right" | "top-left";

export function SignDialog({
  pages,
  current,
  pageSize,
  field,
  signed,
  hasPath,
  onSign,
  onClose,
}: {
  pages: number;
  current: number;
  /** Size of the current page in points, as displayed. */
  pageSize: { w: number; h: number } | null;
  field: string | null;
  signed: boolean;
  hasPath: boolean;
  onSign: (identity: IdentityRef, options: SignOptions, saveAs: boolean) => Promise<boolean>;
  onClose: () => void;
}) {
  const [source, setSource] = useState<"store" | "file" | "new">("store");
  const [ids, setIds] = useState<Identity[] | null>(null);
  const [thumb, setThumb] = useState("");
  const [path, setPath] = useState("");
  const [password, setPassword] = useState("");
  const [fileId, setFileId] = useState<Identity | null>(null);
  const [fileError, setFileError] = useState<string | null>(null);
  const [fresh, setFresh] = useState({ name: "", email: "", organisation: "", country: "", years: "5", password: "", password2: "" });
  const [reason, setReason] = useState("");
  const [location, setLocation] = useState("");
  const [visible, setVisible] = useState(!field);
  const [page, setPage] = useState(String(current + 1));
  const [corner, setCorner] = useState<Corner>("bottom-right");
  const [copy, setCopy] = useState(!hasPath);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .identities()
      .then((list) => {
        setIds(list);
        const first = list.find((i) => !i.expired);
        if (first) setThumb(first.thumbprint);
        else setSource("file");
      })
      .catch(() => {
        setIds([]);
        setSource("file");
      });
  }, []);

  const unlock = async () => {
    setFileError(null);
    setFileId(null);
    try {
      setFileId(await api.fileIdentity(path, password));
    } catch (e) {
      setFileError(errorText(e));
    }
  };
  const create = async () => {
    setError(null);
    const target = await saveDialog({ defaultPath: `${fresh.name || "Digital ID"}.pfx`, filters: [{ name: "Digital ID", extensions: ["pfx"] }] });
    if (!target) return;
    setBusy(true);
    try {
      const made = await api.createIdentity({ name: fresh.name, email: fresh.email, organisation: fresh.organisation, country: fresh.country, years: parseInt(fresh.years, 10) || 5, path: target, password: fresh.password });
      setPath(target);
      setPassword(fresh.password);
      setFileId(made);
      setSource("file");
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  const chosen = source === "store" ? ids?.find((i) => i.thumbprint === thumb) ?? null : source === "file" ? fileId : null;
  const rect = (): Rect | null => {
    if (!visible || field || !pageSize) return null;
    const [w, h, m] = [200, 64, 36];
    const x = corner.endsWith("right") ? pageSize.w - m - w : m;
    const y = corner.startsWith("top") ? pageSize.h - m - h : m;
    return [x, y, x + w, y + h];
  };
  const sign = async () => {
    if (!chosen) return;
    setBusy(true);
    setError(null);
    const identity: IdentityRef = source === "store" ? { kind: "store", thumbprint: thumb } : { kind: "file", path, password };
    const p = Math.min(pages, Math.max(1, parseInt(page, 10) || 1)) - 1;
    const ok = await onSign(identity, { reason, location, contact: "", page: p, rect: rect(), field }, copy);
    setBusy(false);
    if (ok) onClose();
  };
  const freshProblem = !fresh.name.trim() ? "Enter your name." : fresh.password.length < 4 ? "Choose a password of at least four characters." : fresh.password !== fresh.password2 ? "The passwords differ." : null;

  return (
    <Modal
      title={field ? `Sign ${field}` : "Sign document"}
      onClose={onClose}
      width="medium"
      closeOnBlanket={false}
      footer={
        <>
          <Button appearance="subtle" onClick={onClose}>
            Cancel
          </Button>
          {source === "new" ? (
            <Button appearance="primary" loading={busy} disabled={!!freshProblem} onClick={create}>
              Create digital ID…
            </Button>
          ) : (
            <Button appearance="primary" iconBefore={PenLine} loading={busy} disabled={!chosen || chosen.expired} onClick={sign}>
              {copy ? "Sign and save as…" : "Sign and save"}
            </Button>
          )}
        </>
      }
    >
      <div style={stack}>
        <Segmented
          label="Digital ID"
          value={source}
          onChange={setSource}
          options={[
            { value: "store", label: "From Windows", icon: BadgeCheck },
            { value: "file", label: "From a file", icon: FileKey },
            { value: "new", label: "Create one", icon: Stamp },
          ]}
        />
        {source === "store" &&
          (ids == null ? (
            <Spinner label="Reading the Windows certificate store" />
          ) : ids.length ? (
            <RadioList
              label="Certificate"
              value={thumb}
              onChange={setThumb}
              options={ids.map((i) => ({
                value: i.thumbprint,
                label: i.name,
                description: `${i.selfSigned ? "Self-signed" : `Issued by ${i.issuer}`} · ${i.expired ? "expired" : `valid until ${when(i.validTo)}`}`,
              }))}
            />
          ) : (
            <p className="is-muted">Windows holds no certificate with a private key for you. Use a .pfx file, plug in your smart card or token, or create a digital ID.</p>
          ))}
        {source === "file" && (
          <>
            <div className="row">
              <TextField label="Digital ID file (.pfx, .p12)" value={path} readOnly placeholder="Choose a file" />
              <Button
                iconBefore={FolderOpen}
                onClick={async () => {
                  const p = await openDialog({ multiple: false, filters: [{ name: "Digital ID", extensions: ["pfx", "p12"] }] });
                  if (typeof p === "string") {
                    setPath(p);
                    setFileId(null);
                  }
                }}
              >
                Browse
              </Button>
            </div>
            <div className="row">
              <TextField label="Password" type="password" value={password} error={fileError} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === "Enter" && path && unlock()} />
              <Button disabled={!path} onClick={unlock}>
                Unlock
              </Button>
            </div>
            {fileId && (
              <SectionMessage appearance={fileId.expired ? "warning" : "success"} title={fileId.name}>
                {fileId.selfSigned ? "Self-signed" : `Issued by ${fileId.issuer}`} · {fileId.expired ? "expired" : `valid until ${when(fileId.validTo)}`}
              </SectionMessage>
            )}
          </>
        )}
        {source === "new" && (
          <>
            <SectionMessage appearance="information" title="A self-signed digital ID">
              It proves that a document has not changed since you signed it. It does not prove who you are: readers show it as unverified until the recipient chooses to trust it. For that, use a certificate from a trust service.
            </SectionMessage>
            <div className="row">
              <TextField label="Name" value={fresh.name} onChange={(e) => setFresh({ ...fresh, name: e.target.value })} />
              <TextField label="E-mail" value={fresh.email} onChange={(e) => setFresh({ ...fresh, email: e.target.value })} />
            </div>
            <div className="row">
              <TextField label="Organisation" value={fresh.organisation} onChange={(e) => setFresh({ ...fresh, organisation: e.target.value })} />
              <TextField label="Country (2 letters)" value={fresh.country} mono maxLength={2} onChange={(e) => setFresh({ ...fresh, country: e.target.value })} />
              <TextField label="Valid (years)" value={fresh.years} mono onChange={(e) => setFresh({ ...fresh, years: e.target.value.replace(/\D/g, "") })} />
            </div>
            <div className="row">
              <TextField label="Password for the file" type="password" autoComplete="new-password" value={fresh.password} onChange={(e) => setFresh({ ...fresh, password: e.target.value })} />
              <TextField label="Repeat" type="password" autoComplete="new-password" value={fresh.password2} onChange={(e) => setFresh({ ...fresh, password2: e.target.value })} />
            </div>
            <p className="is-muted" style={note}>
              The ID is saved as a .pfx file you choose; keep it and its password safe. Nothing is installed in Windows.
            </p>
          </>
        )}
        {source !== "new" && (
          <>
            <div className="row">
              <TextField label="Reason (optional)" value={reason} placeholder="I approve this document" onChange={(e) => setReason(e.target.value)} />
              <TextField label="Location (optional)" value={location} onChange={(e) => setLocation(e.target.value)} />
            </div>
            {!field && (
              <>
                <Toggle label="Show the signature on a page" description="Your name, the date and the reason in a box. Without it the signature is invisible but just as valid." checked={visible} onChange={setVisible} />
                {visible && (
                  <div className="row">
                    <TextField label="Page" value={page} mono onChange={(e) => setPage(e.target.value.replace(/\D/g, ""))} />
                    <Select<Corner>
                      label="Position"
                      value={corner}
                      onChange={setCorner}
                      options={[
                        { value: "bottom-right", label: "Bottom right" },
                        { value: "bottom-left", label: "Bottom left" },
                        { value: "top-right", label: "Top right" },
                        { value: "top-left", label: "Top left" },
                      ]}
                    />
                  </div>
                )}
              </>
            )}
            <Toggle label="Save the signed document as a copy" checked={copy} disabled={!hasPath} onChange={setCopy} />
            <p className="is-muted" style={note}>
              {signed
                ? "This document is signed already: the new signature is appended, so the existing ones stay valid."
                : "Signing saves the document (PAdES, SHA-256). Changes made afterwards are appended and show up as such when the signature is checked."}
            </p>
          </>
        )}
        {source === "new" && freshProblem && fresh.name && <p className="is-muted">{freshProblem}</p>}
        {error && <SectionMessage appearance="danger">{error}</SectionMessage>}
      </div>
    </Modal>
  );
}
