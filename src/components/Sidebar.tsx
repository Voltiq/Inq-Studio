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
  ArrowDown,
  ArrowUp,
  BookmarkPlus,
  Crosshair,
  IndentDecrease,
  IndentIncrease,
} from "lucide-react";
import { IconButton } from "./ui/Button";
import { EmptyState, Lozenge, Spinner } from "./ui/Feedback";
import { Segmented } from "./ui/Field";
import type { MenuRequest } from "./ui/Menu";
import { api, type AnnotInfo, type Bookmark, type FontSummary } from "../lib/api";
import { type PDFDocumentProxy } from "../lib/pdf";
import { usePageStamps } from "../lib/stamps";
import { previewStack } from "../lib/fontPreview";
import { fontLabel, plural, rgbToHex } from "../lib/format";
import type { Selection } from "./Viewer";
import { cx } from "../lib/utils";
import { t, te } from "../lib/i18n";

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
  onSaveOutline,
  labels,
  changed,
}: {
  /** Pages the last change can have repainted; null means any. */
  changed: number[] | null;
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
  onSaveOutline: (items: Bookmark[]) => Promise<void>;
  labels: string[] | null;
}) {
  const stamps = usePageStamps(revision, changed, pdf.numPages);
  return (
    <aside className="panel side" aria-label={t("Document sidebar")}>
      <div className="side-head">
        <Segmented<SideTab>
          label={t("Sidebar")}
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
          stamps={stamps}
          labels={labels}
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
      {tab === "outline" && <BookmarksPanel docId={docId} revision={revision} page={page} onNavigate={onNavigate} onSave={onSaveOutline} />}
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
    { label: t("Opened"), step: 0 },
    ...history.map((label, i) => ({ label, step: i + 1 })),
    ...future.map((label, i) => ({ label, step: current + i + 1 })),
  ];
  return (
    <div className="side-body">
      <span className="eyebrow" style={{ display: "block", marginBottom: 8 }}>
        {history.length ? plural(history.length, "change") : t("No changes yet")}
      </span>
      <div className="history">
        {rows.map((r) => (
          <button
            key={r.step}
            type="button"
            className={cx("history-row", r.step === current && "is-current", r.step > current && "is-undone")}
            onClick={() => r.step !== current && onGo(r.step - current)}
            data-tooltip={r.step === current ? t("Current state") : r.step < current ? t("Go back to this step") : t("Redo up to this step")}
          >
            <CircleDot size={12} aria-hidden="true" className="history-dot" />
            <span className="history-label">{te(r.label)}</span>
          </button>
        ))}
      </div>
      <p className="is-muted" style={{ fontSize: 12, marginTop: 12 }}>
        {t("Click a step to go back to it. Steps after it stay available until you make a new change.")}
      </p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

function Thumb({ pdf, index, width, stamp }: { pdf: PDFDocumentProxy; index: number; width: number; stamp: number }) {
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stamp, index, width, shown]);

  return (
    <div ref={holder} className="thumb-paper" style={{ aspectRatio: `1 / ${ratio}` }}>
      <canvas ref={ref} />
    </div>
  );
}

function PagesPanel({
  stamps,
  labels,
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
  labels: string[] | null;
  stamps: number[];
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
    const label = pages.length > 1 ? t("{0} pages", pages.length) : t("page {0}", i + 1);
    onMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        { type: "heading", label: label.charAt(0).toUpperCase() + label.slice(1) },
        { label: t("Rotate right"), icon: RotateCw, onSelect: () => onPageAction("rotate-right", pages) },
        { label: t("Rotate left"), icon: RotateCcw, onSelect: () => onPageAction("rotate-left", pages) },
        { label: t("Duplicate"), icon: Copy, disabled: pages.length > 1, onSelect: () => onPageAction("duplicate", pages) },
        { type: "separator" },
        { label: t("Insert blank page before"), icon: FilePlus2, onSelect: () => onPageAction("blank-before", pages) },
        { label: t("Insert blank page after"), icon: FilePlus2, onSelect: () => onPageAction("blank-after", pages) },
        { label: t("Insert pages from PDF…"), icon: FileInput, onSelect: () => onPageAction("insert-file", pages) },
        { label: t("Extract to new PDF…"), icon: FileDown, onSelect: () => onPageAction("extract", pages) },
        { type: "separator" },
        { label: t("Delete {0}", label), icon: Trash2, danger: true, shortcut: "Del", onSelect: () => onPageAction("delete", pages) },
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
          {selected.length > 1 ? t("{0} selected", selected.length) : plural(n, "page")}
        </span>
        <IconButton icon={RotateCcw} label={t("Rotate left")} spacing="compact" onClick={() => onPageAction("rotate-left", targets)} />
        <IconButton icon={RotateCw} label={t("Rotate right")} spacing="compact" onClick={() => onPageAction("rotate-right", targets)} />
        <IconButton icon={FilePlus2} label={t("Insert blank page after")} spacing="compact" onClick={() => onPageAction("blank-after", targets)} />
        <IconButton icon={FileInput} label={t("Insert pages from PDF…")} spacing="compact" onClick={() => onPageAction("insert-file", targets)} />
        <IconButton icon={Trash2} label={t("Delete pages")} spacing="compact" onClick={() => onPageAction("delete", targets)} />
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
              <Thumb pdf={pdf} index={i} width={n === 1 ? 180 : 112} stamp={stamps[i] ?? 0} />
              <span className="thumb-label">
                {labels && labels[i] && labels[i] !== String(i + 1) ? (
                  <>
                    {labels[i]} <span className="is-muted">({i + 1})</span>
                  </>
                ) : (
                  i + 1
                )}
              </span>
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

type Path = number[];

function at(items: Bookmark[], path: Path): Bookmark | null {
  let list = items;
  let node: Bookmark | null = null;
  for (const i of path) {
    node = list[i] ?? null;
    if (!node) return null;
    list = node.children;
  }
  return node;
}

function clone(items: Bookmark[]): Bookmark[] {
  return JSON.parse(JSON.stringify(items));
}

/** The sibling list holding `path`, and its index there. */
function parentOf(items: Bookmark[], path: Path): [Bookmark[], number] {
  let list = items;
  for (const i of path.slice(0, -1)) list = list[i].children;
  return [list, path[path.length - 1]];
}

function BookmarkRow({
  item,
  path,
  selected,
  editing,
  onSelect,
  onNavigate,
  onRename,
  onToggle,
}: {
  item: Bookmark;
  path: Path;
  selected: Path | null;
  editing: Path | null;
  onSelect: (p: Path) => void;
  onNavigate: (page: number) => void;
  onRename: (p: Path, title: string | null) => void;
  onToggle: (p: Path) => void;
}) {
  const same = (a: Path | null) => !!a && a.length === path.length && a.every((v, i) => v === path[i]);
  const [draft, setDraft] = useState(item.title);
  useEffect(() => setDraft(item.title), [item.title]);
  const has = item.children.length > 0;
  return (
    <div>
      <div
        className={cx("outline-item", "bookmark-row", same(selected) && "is-selected")}
        role="treeitem"
        aria-selected={same(selected)}
        tabIndex={0}
        onClick={() => {
          onSelect(path);
          if (item.page != null) onNavigate(item.page);
        }}
        onDoubleClick={() => onRename(path, item.title)}
      >
        <span
          role={has ? "button" : undefined}
          aria-label={has ? (item.open ? t("Collapse") : t("Expand")) : undefined}
          onClick={(e) => {
            if (!has) return;
            e.stopPropagation();
            onToggle(path);
          }}
          style={{ visibility: has ? "visible" : "hidden", display: "flex" }}
        >
          <ChevronRight size={14} className={cx("chev", item.open && "is-open")} />
        </span>
        {same(editing) ? (
          <input
            className="bookmark-input"
            autoFocus
            value={draft}
            onClick={(e) => e.stopPropagation()}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") onRename(path, draft.trim() ? draft : null);
              if (e.key === "Escape") onRename(path, null);
            }}
            onBlur={() => onRename(path, draft.trim() && draft !== item.title ? draft : null)}
          />
        ) : (
          <span className="bookmark-title">{item.title || t("Untitled")}</span>
        )}
        {item.page != null && <span className="bookmark-page">{item.page + 1}</span>}
      </div>
      {has && item.open && (
        <div className="outline-children" role="group">
          {item.children.map((c, i) => (
            <BookmarkRow
              key={i}
              item={c}
              path={[...path, i]}
              selected={selected}
              editing={editing}
              onSelect={onSelect}
              onNavigate={onNavigate}
              onRename={onRename}
              onToggle={onToggle}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function BookmarksPanel({
  docId,
  revision,
  page,
  onNavigate,
  onSave,
}: {
  docId: number;
  revision: number;
  page: number;
  onNavigate: (p: number) => void;
  onSave: (items: Bookmark[]) => Promise<void>;
}) {
  const [items, setItems] = useState<Bookmark[] | null>(null);
  const [selected, setSelected] = useState<Path | null>(null);
  const [editing, setEditing] = useState<Path | null>(null);
  useEffect(() => {
    let live = true;
    api
      .outline(docId)
      .then((o) => live && setItems(o))
      .catch(() => live && setItems([]));
    return () => {
      live = false;
    };
  }, [docId, revision]);

  const commit = (next: Bookmark[], sel: Path | null = selected) => {
    setItems(next);
    setSelected(sel);
    onSave(next);
  };

  const add = () => {
    const next = clone(items ?? []);
    const bm: Bookmark = { title: t("Page {0}", page + 1), page, open: false, children: [] };
    let path: Path;
    if (selected && at(next, selected)) {
      const [list, i] = parentOf(next, selected);
      list.splice(i + 1, 0, bm);
      path = [...selected.slice(0, -1), i + 1];
    } else {
      next.push(bm);
      path = [next.length - 1];
    }
    setItems(next);
    setSelected(path);
    setEditing(path);
  };

  const remove = () => {
    if (!selected || !items) return;
    const next = clone(items);
    const [list, i] = parentOf(next, selected);
    list.splice(i, 1);
    commit(next, null);
  };

  const move = (dir: -1 | 1) => {
    if (!selected || !items) return;
    const next = clone(items);
    const [list, i] = parentOf(next, selected);
    const j = i + dir;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    commit(next, [...selected.slice(0, -1), j]);
  };

  const indent = () => {
    if (!selected || !items) return;
    const next = clone(items);
    const [list, i] = parentOf(next, selected);
    if (i === 0) return;
    const [node] = list.splice(i, 1);
    const prev = list[i - 1];
    prev.children.push(node);
    prev.open = true;
    commit(next, [...selected.slice(0, -1), i - 1, prev.children.length - 1]);
  };

  const outdent = () => {
    if (!selected || selected.length < 2 || !items) return;
    const next = clone(items);
    const [list, i] = parentOf(next, selected);
    const [node] = list.splice(i, 1);
    const parentPath = selected.slice(0, -1);
    const [plist, pi] = parentOf(next, parentPath);
    plist.splice(pi + 1, 0, node);
    commit(next, [...parentPath.slice(0, -1), pi + 1]);
  };

  const setTarget = () => {
    if (!selected || !items) return;
    const next = clone(items);
    const node = at(next, selected);
    if (!node) return;
    node.page = page;
    node.top = null;
    commit(next);
  };

  const sel = selected && items ? at(items, selected) : null;
  return (
    <>
      <div className="side-head" style={{ paddingTop: 0 }}>
        <span className="eyebrow" style={{ flex: 1 }}>
          {t("Bookmarks")}
        </span>
        <IconButton icon={BookmarkPlus} label={t("Add bookmark to page {0}", page + 1)} spacing="compact" onClick={add} />
        <IconButton icon={ArrowUp} label={t("Move up")} spacing="compact" disabled={!sel} onClick={() => move(-1)} />
        <IconButton icon={ArrowDown} label={t("Move down")} spacing="compact" disabled={!sel} onClick={() => move(1)} />
        <IconButton icon={IndentIncrease} label={t("Make it part of the bookmark above")} spacing="compact" disabled={!sel} onClick={indent} />
        <IconButton icon={IndentDecrease} label={t("Move it out one level")} spacing="compact" disabled={!sel || (selected?.length ?? 0) < 2} onClick={outdent} />
        <IconButton icon={Crosshair} label={t("Point it at page {0}", page + 1)} spacing="compact" disabled={!sel} onClick={setTarget} />
        <IconButton icon={Trash2} label={t("Delete bookmark")} spacing="compact" disabled={!sel} onClick={remove} />
      </div>
      <div className="side-body" role="tree" aria-label={t("Bookmarks")}>
        {items == null ? (
          <Spinner />
        ) : items.length === 0 ? (
          <EmptyState icon={BookOpen} title={t("No bookmarks")} compact>
            {t("Add one for the current page; double-click a bookmark to rename it.")}
          </EmptyState>
        ) : (
          items.map((it, i) => (
            <BookmarkRow
              key={i}
              item={it}
              path={[i]}
              selected={selected}
              editing={editing}
              onSelect={setSelected}
              onNavigate={onNavigate}
              onToggle={(p) => {
                const next = clone(items);
                const n = at(next, p);
                if (n) n.open = !n.open;
                setItems(next);
              }}
              onRename={(p, title) => {
                setEditing(null);
                if (title == null) {
                  // A new bookmark keeps its default name; just save it.
                  onSave(items);
                  return;
                }
                const next = clone(items);
                const n = at(next, p);
                if (n) n.title = title;
                commit(next, p);
              }}
            />
          ))
        )}
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

function editability(f: FontSummary): string {
  if (!f.editable) return t("Drawn with PDF operators (Type 3); not editable yet.");
  if (f.added) return t("Embedded from {0} by Inq Studio; it grows as edits need more characters.", f.systemMatch ?? "an installed font");
  if (!f.embedded) return t("Not embedded: readers draw it with their own copy. Edits use the same encoding.");
  if (f.subset) {
    const extra = f.extendable
      ? t("more come from its embedded program")
      : f.systemMatch
        ? t("others come from {0} installed here", f.systemMatch)
        : t("others are set in a substitute font");
    return t("Subset with {0}; {1}.", plural(f.glyphs, "character"), extra);
  }
  return t("Fully embedded: edits can use any character it has.");
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
    return letters.length > 6 ? letters.slice(0, 40) : t("Aa Bb Cc 123");
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
        data-tooltip={f.previewable ? t("Drawn with the font embedded in this PDF") : t("Approximated with an installed font")}
      >
        {sample}
      </div>
      <div className="font-tags">
        {f.embedded ? <Lozenge appearance="success">embedded</Lozenge> : <Lozenge appearance="warning">{t("not embedded")}</Lozenge>}
        {f.subset && <Lozenge appearance="information">subset</Lozenge>}
        {f.systemMatch && <Lozenge appearance="discovery">installed</Lozenge>}
        {f.added && <Lozenge appearance="information">{t("added by Inq")}</Lozenge>}
        {!f.hasToUnicode && f.embedded && <Lozenge appearance="warning">{t("no unicode map")}</Lozenge>}
      </div>
      <p className="font-meta" style={{ display: "block" }}>
        {editability(f)}
      </p>
      <dl className="font-meta">
        <dt>{t("Encoding")}</dt>
        <dd>{f.encoding || "—"}</dd>
        <dt>{t("Pages")}</dt>
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
        <EmptyState icon={Type} title={t("No fonts")} compact>
          {t("This document has no text drawn with fonts, so it may be a scan.")}
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
  Text: t("Note"),
  Highlight: t("Highlight"),
  Underline: t("Underline"),
  StrikeOut: "Strike-out",
  Squiggly: t("Squiggly"),
  Ink: t("Drawing"),
  Square: t("Rectangle"),
  Circle: t("Ellipse"),
  FreeText: t("Text box"),
  Line: t("Line"),
  Stamp: t("Stamp"),
  Link: "Link",
  Widget: t("Form field"),
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
        <EmptyState icon={MessageSquare} title={t("No comments")} compact>
          {t("Highlights, notes and drawings you add show up here.")}
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
                  label={t("Delete")}
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
