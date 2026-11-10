import { useEffect, useMemo, useRef, useState } from "react";
import {
  BookOpen,
  ChevronRight,
  Copy,
  FileDown,
  FilePlus2,
  LayoutGrid,
  MessageSquare,
  RotateCcw,
  RotateCw,
  Trash2,
  Type,
  FileInput,
  History,
  CircleDot,
} from "lucide-react";
import { IconButton } from "./ui/Button";
import { EmptyState, Lozenge, Spinner } from "./ui/Feedback";
import { Segmented } from "./ui/Field";
import type { MenuRequest } from "./ui/Menu";
import { api, type AnnotInfo, type FontSummary } from "../lib/api";
import { destPage, outline, type OutlineItem, type PDFDocumentProxy } from "../lib/pdf";
import { previewStack } from "../lib/fontPreview";
import { fontLabel, plural, rgbToHex } from "../lib/format";
import type { Selection } from "./Viewer";
import { cx } from "../lib/utils";

export type SideTab = "pages" | "outline" | "fonts" | "comments" | "history";

export type PageAction = "rotate-left" | "rotate-right" | "duplicate" | "blank-before" | "blank-after" | "insert-file" | "extract" | "delete";

export function Sidebar({
  tab,
  onTab,
  docId,
  revision,
  pdf,
  page,
  selected,
  onSelectPages,
  onNavigate,
  onReorder,
  onPageAction,
  onMenu,
  fonts,
  activeFontKey,
  selection,
  onSelect,
  onDeleteAnnot,
  history,
  future,
  onHistoryGo,
}: {
  tab: SideTab;
  onTab: (t: SideTab) => void;
  docId: number;
  revision: number;
  pdf: PDFDocumentProxy;
  page: number;
  selected: number[];
  onSelectPages: (pages: number[]) => void;
  onNavigate: (page: number) => void;
  onReorder: (order: number[]) => void;
  onPageAction: (action: PageAction, pages: number[]) => void;
  onMenu: (m: MenuRequest) => void;
  fonts: FontSummary[] | null;
  activeFontKey: string | null;
  selection: Selection;
  onSelect: (s: Selection) => void;
  onDeleteAnnot: (page: number, index: number) => void;
  history: string[];
  future: string[];
  onHistoryGo: (steps: number) => void;
}) {
  return (
    <aside className="panel side" aria-label="Document sidebar">
      <div className="side-head">
        <Segmented<SideTab>
          label="Sidebar"
          value={tab}
          onChange={onTab}
          options={[
            { value: "pages", label: "", icon: LayoutGrid },
            { value: "outline", label: "", icon: BookOpen },
            { value: "fonts", label: "", icon: Type },
            { value: "comments", label: "", icon: MessageSquare },
            { value: "history", label: "", icon: History },
          ]}
        />
      </div>
      {tab === "pages" && (
        <PagesPanel
          pdf={pdf}
          page={page}
          selected={selected}
          onSelectPages={onSelectPages}
          onNavigate={onNavigate}
          onReorder={onReorder}
          onPageAction={onPageAction}
          onMenu={onMenu}
        />
      )}
      {tab === "outline" && <OutlinePanel pdf={pdf} onNavigate={onNavigate} />}
      {tab === "fonts" && <FontsPanel docId={docId} fonts={fonts} activeKey={activeFontKey} onNavigate={onNavigate} />}
      {tab === "comments" && (
        <CommentsPanel
          docId={docId}
          revision={revision}
          pages={pdf.numPages}
          selection={selection}
          onSelect={onSelect}
          onNavigate={onNavigate}
          onDelete={onDeleteAnnot}
        />
      )}
      {tab === "history" && <HistoryPanel history={history} future={future} onGo={onHistoryGo} />}
    </aside>
  );
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

function HistoryPanel({ history, future, onGo }: { history: string[]; future: string[]; onGo: (steps: number) => void }) {
  const current = history.length;
  const rows = [
    { label: "Opened", step: 0 },
    ...history.map((label, i) => ({ label, step: i + 1 })),
    ...future.map((label, i) => ({ label, step: current + i + 1 })),
  ];
  return (
    <div className="side-body">
      <span className="eyebrow" style={{ display: "block", marginBottom: 8 }}>
        {history.length ? plural(history.length, "change") : "No changes yet"}
      </span>
      <div className="history">
        {rows.map((r) => (
          <button
            key={r.step}
            type="button"
            className={cx("history-row", r.step === current && "is-current", r.step > current && "is-undone")}
            onClick={() => r.step !== current && onGo(r.step - current)}
            data-tooltip={r.step === current ? "Current state" : r.step < current ? "Go back to this step" : "Redo up to this step"}
          >
            <CircleDot size={12} aria-hidden="true" className="history-dot" />
            <span className="history-label">{r.label}</span>
          </button>
        ))}
      </div>
      <p className="is-muted" style={{ fontSize: 12, marginTop: 12 }}>
        Click a step to go back to it. Steps after it stay available until you make a new change.
      </p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

function Thumb({ pdf, index, width }: { pdf: PDFDocumentProxy; index: number; width: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const holder = useRef<HTMLDivElement>(null);
  const [ratio, setRatio] = useState(1.414);
  const [shown, setShown] = useState(false);

  useEffect(() => {
    const el = holder.current;
    if (!el) return;
    const io = new IntersectionObserver(([e]) => e.isIntersecting && setShown(true), { rootMargin: "300px" });
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    if (!shown) return;
    let cancelled = false;
    let task: { cancel: () => void; promise: Promise<void> } | null = null;
    (async () => {
      const page = await pdf.getPage(index + 1);
      if (cancelled) return;
      const base = page.getViewport({ scale: 1 });
      setRatio(base.height / base.width);
      const dpr = window.devicePixelRatio || 1;
      const vp = page.getViewport({ scale: (width / base.width) * dpr });
      const off = document.createElement("canvas");
      off.width = Math.floor(vp.width);
      off.height = Math.floor(vp.height);
      task = page.render({ canvas: off, viewport: vp });
      await task.promise;
      const c = ref.current;
      if (cancelled || !c) return;
      c.width = off.width;
      c.height = off.height;
      c.getContext("2d")?.drawImage(off, 0, 0);
    })().catch(() => {});
    return () => {
      cancelled = true;
      task?.cancel();
    };
  }, [pdf, index, width, shown]);

  return (
    <div ref={holder} className="thumb-paper" style={{ aspectRatio: `1 / ${ratio}` }}>
      <canvas ref={ref} />
    </div>
  );
}

function PagesPanel({
  pdf,
  page,
  selected,
  onSelectPages,
  onNavigate,
  onReorder,
  onPageAction,
  onMenu,
}: {
  pdf: PDFDocumentProxy;
  page: number;
  selected: number[];
  onSelectPages: (pages: number[]) => void;
  onNavigate: (page: number) => void;
  onReorder: (order: number[]) => void;
  onPageAction: (action: PageAction, pages: number[]) => void;
  onMenu: (m: MenuRequest) => void;
}) {
  const n = pdf.numPages;
  const [dragging, setDragging] = useState<number[] | null>(null);
  const [drop, setDrop] = useState<{ index: number; after: boolean } | null>(null);
  const anchor = useRef(page);
  const listRef = useRef<HTMLDivElement>(null);
  const targets = selected.length ? selected : [page];

  useEffect(() => {
    listRef.current?.querySelector(`[data-thumb="${page}"]`)?.scrollIntoView({ block: "nearest" });
  }, [page]);

  const click = (i: number, e: React.MouseEvent) => {
    if (e.shiftKey) {
      const [a, b] = [Math.min(anchor.current, i), Math.max(anchor.current, i)];
      onSelectPages(Array.from({ length: b - a + 1 }, (_, k) => a + k));
    } else if (e.ctrlKey || e.metaKey) {
      onSelectPages(selected.includes(i) ? selected.filter((p) => p !== i) : [...selected, i].sort((x, y) => x - y));
      anchor.current = i;
    } else {
      onSelectPages([i]);
      anchor.current = i;
      onNavigate(i);
    }
  };

  const menu = (i: number, e: React.MouseEvent) => {
    e.preventDefault();
    const pages = selected.includes(i) ? selected : [i];
    if (!selected.includes(i)) onSelectPages([i]);
    const label = pages.length > 1 ? `${pages.length} pages` : `page ${i + 1}`;
    onMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { type: "heading", label: label.charAt(0).toUpperCase() + label.slice(1) },
        { label: "Rotate right", icon: RotateCw, onSelect: () => onPageAction("rotate-right", pages) },
        { label: "Rotate left", icon: RotateCcw, onSelect: () => onPageAction("rotate-left", pages) },
        { label: "Duplicate", icon: Copy, disabled: pages.length > 1, onSelect: () => onPageAction("duplicate", pages) },
        { type: "separator" },
        { label: "Insert blank page before", icon: FilePlus2, onSelect: () => onPageAction("blank-before", pages) },
        { label: "Insert blank page after", icon: FilePlus2, onSelect: () => onPageAction("blank-after", pages) },
        { label: "Insert pages from PDF…", icon: FileInput, onSelect: () => onPageAction("insert-file", pages) },
        { label: "Extract to new PDF…", icon: FileDown, onSelect: () => onPageAction("extract", pages) },
        { type: "separator" },
        { label: `Delete ${label}`, icon: Trash2, danger: true, shortcut: "Del", onSelect: () => onPageAction("delete", pages) },
      ],
    });
  };

  const onDrop = () => {
    if (!dragging || !drop) return;
    const moving = [...dragging].sort((a, b) => a - b);
    const rest = Array.from({ length: n }, (_, i) => i).filter((i) => !moving.includes(i));
    const target = drop.index + (drop.after ? 1 : 0);
    const insertAt = rest.filter((i) => i < target).length;
    const order = [...rest.slice(0, insertAt), ...moving, ...rest.slice(insertAt)];
    setDragging(null);
    setDrop(null);
    if (order.some((v, i) => v !== i)) onReorder(order);
  };

  return (
    <>
      <div className="side-head" style={{ paddingTop: 0 }}>
        <span className="eyebrow" style={{ flex: 1 }}>
          {selected.length > 1 ? `${selected.length} selected` : plural(n, "page")}
        </span>
        <IconButton icon={RotateCcw} label="Rotate left" spacing="compact" onClick={() => onPageAction("rotate-left", targets)} />
        <IconButton icon={RotateCw} label="Rotate right" spacing="compact" onClick={() => onPageAction("rotate-right", targets)} />
        <IconButton icon={FilePlus2} label="Insert blank page after" spacing="compact" onClick={() => onPageAction("blank-after", targets)} />
        <IconButton icon={FileInput} label="Insert pages from PDF…" spacing="compact" onClick={() => onPageAction("insert-file", targets)} />
        <IconButton icon={Trash2} label="Delete pages" spacing="compact" onClick={() => onPageAction("delete", targets)} />
      </div>
      <div className="side-body" ref={listRef}>
        <div className={cx("thumbs", n === 1 && "is-single")} onDragOver={(e) => e.preventDefault()} onDrop={onDrop}>
          {Array.from({ length: n }, (_, i) => (
            <div
              key={i}
              data-thumb={i}
              className={cx(
                "thumb",
                selected.includes(i) && "is-selected",
                i === page && "is-current",
                drop?.index === i && (drop.after ? "is-drop-after" : "is-drop-before"),
              )}
              draggable
              onDragStart={(e) => {
                const pages = selected.includes(i) ? selected : [i];
                setDragging(pages);
                e.dataTransfer.effectAllowed = "move";
                e.dataTransfer.setData("text/plain", pages.join(","));
              }}
              onDragOver={(e) => {
                e.preventDefault();
                const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                setDrop({ index: i, after: e.clientX > r.left + r.width / 2 });
              }}
              onDragEnd={() => {
                setDragging(null);
                setDrop(null);
              }}
              onClick={(e) => click(i, e)}
              onContextMenu={(e) => menu(i, e)}
            >
              <Thumb pdf={pdf} index={i} width={n === 1 ? 180 : 112} />
              <span className="thumb-label">{i + 1}</span>
            </div>
          ))}
        </div>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Outline
// ---------------------------------------------------------------------------

function OutlineNode({ item, pdf, onNavigate, depth }: { item: OutlineItem; pdf: PDFDocumentProxy; onNavigate: (p: number) => void; depth: number }) {
  const [open, setOpen] = useState(depth < 1);
  const has = item.items.length > 0;
  return (
    <div>
      <button
        type="button"
        className="outline-item"
        onClick={async () => {
          const p = await destPage(pdf, item.dest);
          if (p != null) onNavigate(p);
        }}
        style={{ fontWeight: item.bold ? 600 : undefined, fontStyle: item.italic ? "italic" : undefined }}
      >
        <span
          role={has ? "button" : undefined}
          aria-label={has ? (open ? "Collapse" : "Expand") : undefined}
          onClick={(e) => {
            if (!has) return;
            e.stopPropagation();
            setOpen(!open);
          }}
          style={{ visibility: has ? "visible" : "hidden", display: "flex" }}
        >
          <ChevronRight size={14} className={cx("chev", open && "is-open")} />
        </span>
        <span>{item.title}</span>
      </button>
      {has && open && (
        <div className="outline-children">
          {item.items.map((c, i) => (
            <OutlineNode key={i} item={c} pdf={pdf} onNavigate={onNavigate} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

function OutlinePanel({ pdf, onNavigate }: { pdf: PDFDocumentProxy; onNavigate: (p: number) => void }) {
  const [items, setItems] = useState<OutlineItem[] | null>(null);
  useEffect(() => {
    let live = true;
    outline(pdf)
      .then((o) => live && setItems(o))
      .catch(() => live && setItems([]));
    return () => {
      live = false;
    };
  }, [pdf]);
  return (
    <div className="side-body">
      {items == null ? (
        <Spinner />
      ) : items.length === 0 ? (
        <EmptyState icon={BookOpen} title="No outline" compact>
          This document has no bookmarks.
        </EmptyState>
      ) : (
        items.map((it, i) => <OutlineNode key={i} item={it} pdf={pdf} onNavigate={onNavigate} depth={0} />)
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

function editability(f: FontSummary): string {
  if (!f.editable) return "Drawn with PDF operators (Type 3); not editable yet.";
  if (f.added) return `Embedded from ${f.systemMatch ?? "an installed font"} by Inq Studio; it grows as edits need more characters.`;
  if (!f.embedded) return "Not embedded: readers draw it with their own copy. Edits use the same encoding.";
  if (f.subset) {
    const extra = f.extendable
      ? "more come from its embedded program"
      : f.systemMatch
        ? `others come from ${f.systemMatch} installed here`
        : "others are set in a substitute font";
    return `Subset with ${plural(f.glyphs, "character")}; ${extra}.`;
  }
  return "Fully embedded: edits can use any character it has.";
}

function FontCard({ docId, f, active, onNavigate }: { docId: number; f: FontSummary; active: boolean; onNavigate: (p: number) => void }) {
  const [stack, setStack] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    previewStack(docId, f).then((s) => live && setStack(s));
    return () => {
      live = false;
    };
  }, [docId, f]);
  const sample = useMemo(() => {
    const letters = [...f.charset].filter((c) => /\p{L}|\p{N}/u.test(c)).join("");
    return letters.length > 6 ? letters.slice(0, 40) : "Aa Bb Cc 123";
  }, [f.charset]);
  return (
    <div className={cx("font-card", active && "is-active")}>
      <div className="font-head">
        <span className="font-name">{fontLabel(f)}</span>
        <Lozenge>{f.kind}</Lozenge>
      </div>
      <div className="font-ps" data-tooltip={f.rawName} data-tooltip-overflow>
        {f.rawName}
      </div>
      <div
        className="font-sample"
        style={{
          fontFamily: stack ?? undefined,
          fontWeight: f.bold ? 700 : 400,
          fontStyle: f.italic ? "italic" : undefined,
        }}
        data-tooltip={f.previewable ? "Drawn with the font embedded in this PDF" : "Approximated with an installed font"}
      >
        {sample}
      </div>
      <div className="font-tags">
        {f.embedded ? <Lozenge appearance="success">embedded</Lozenge> : <Lozenge appearance="warning">not embedded</Lozenge>}
        {f.subset && <Lozenge appearance="information">subset</Lozenge>}
        {f.systemMatch && <Lozenge appearance="discovery">installed</Lozenge>}
        {f.added && <Lozenge appearance="information">added by Inq</Lozenge>}
        {!f.hasToUnicode && f.embedded && <Lozenge appearance="warning">no unicode map</Lozenge>}
      </div>
      <p className="font-meta" style={{ display: "block" }}>
        {editability(f)}
      </p>
      <dl className="font-meta">
        <dt>Encoding</dt>
        <dd>{f.encoding || "—"}</dd>
        <dt>Pages</dt>
        <dd>
          {f.pages.slice(0, 8).map((p, i) => (
            <span key={p}>
              {i > 0 && ", "}
              <a href="#" onClick={(e) => (e.preventDefault(), onNavigate(p))} style={{ color: "var(--link)" }}>
                {p + 1}
              </a>
            </span>
          ))}
          {f.pages.length > 8 && ` +${f.pages.length - 8}`}
        </dd>
      </dl>
    </div>
  );
}

function FontsPanel({
  docId,
  fonts,
  activeKey,
  onNavigate,
}: {
  docId: number;
  fonts: FontSummary[] | null;
  activeKey: string | null;
  onNavigate: (p: number) => void;
}) {
  return (
    <div className="side-body">
      {fonts == null ? (
        <Spinner />
      ) : fonts.length === 0 ? (
        <EmptyState icon={Type} title="No fonts" compact>
          This document has no text drawn with fonts, so it may be a scan.
        </EmptyState>
      ) : (
        <>
          <span className="eyebrow" style={{ display: "block", marginBottom: 8 }}>
            {plural(fonts.length, "font")}
          </span>
          {fonts.map((f) => (
            <FontCard key={f.key} docId={docId} f={f} active={f.key === activeKey} onNavigate={onNavigate} />
          ))}
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Comments
// ---------------------------------------------------------------------------

const ANNOT_NAMES: Record<string, string> = {
  Text: "Note",
  Highlight: "Highlight",
  Underline: "Underline",
  StrikeOut: "Strike-out",
  Squiggly: "Squiggly",
  Ink: "Drawing",
  Square: "Rectangle",
  Circle: "Ellipse",
  FreeText: "Text box",
  Line: "Line",
  Stamp: "Stamp",
  Link: "Link",
  Widget: "Form field",
};

function CommentsPanel({
  docId,
  revision,
  pages,
  selection,
  onSelect,
  onNavigate,
  onDelete,
}: {
  docId: number;
  revision: number;
  pages: number;
  selection: Selection;
  onSelect: (s: Selection) => void;
  onNavigate: (p: number) => void;
  onDelete: (page: number, index: number) => void;
}) {
  const [items, setItems] = useState<{ page: number; a: AnnotInfo }[] | null>(null);
  useEffect(() => {
    let live = true;
    (async () => {
      const out: { page: number; a: AnnotInfo }[] = [];
      for (let p = 0; p < Math.min(pages, 500); p++) {
        const list = await api.annots(docId, p);
        for (const a of list) if (a.subtype !== "Link" && a.subtype !== "Widget") out.push({ page: p, a });
      }
      if (live) setItems(out);
    })().catch(() => live && setItems([]));
    return () => {
      live = false;
    };
  }, [docId, revision, pages]);

  return (
    <div className="side-body">
      {items == null ? (
        <Spinner />
      ) : items.length === 0 ? (
        <EmptyState icon={MessageSquare} title="No comments" compact>
          Highlights, notes and drawings you add show up here.
        </EmptyState>
      ) : (
        items.map(({ page, a }) => {
          const sel = selection?.kind === "annot" && selection.page === page && selection.annot.index === a.index;
          return (
            <div
              key={`${page}-${a.index}`}
              className={cx("comment", sel && "is-selected")}
              onClick={() => {
                onNavigate(page);
                onSelect({ kind: "annot", page, annot: a });
              }}
            >
              <span className="comment-swatch" style={{ background: a.color ? rgbToHex(a.color) : "var(--surface-4)" }} />
              <div style={{ minWidth: 0 }}>
                <div className="comment-title">
                  {ANNOT_NAMES[a.subtype] ?? a.subtype} <span className="is-muted">· p. {page + 1}</span>
                </div>
                {a.contents && <div className="comment-body">{a.contents}</div>}
                {a.author && <div className="is-muted" style={{ fontSize: 11 }}>{a.author}</div>}
              </div>
              {a.removable && (
                <IconButton
                  icon={Trash2}
                  label="Delete"
                  spacing="compact"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(page, a.index);
                  }}
                />
              )}
            </div>
          );
        })
      )}
    </div>
  );
}
