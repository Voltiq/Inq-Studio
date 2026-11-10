import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ChevronDown, ChevronUp, Search, X } from "lucide-react";
import { DocTabs, TOOL_HINTS, TOOL_KEYS, Toasts, Toolbar, TopBar, type Toast } from "./components/Chrome";
import { Welcome } from "./components/Welcome";
import { Viewer, type Selection, type TextStyle, type ViewerActions } from "./components/Viewer";
import { Sidebar, type PageAction, type SideTab } from "./components/Sidebar";
import { Inspector } from "./components/Inspector";
import { PasswordDialog, SettingsDialog, UnsavedDialog } from "./components/Dialogs";
import { TooltipLayer } from "./components/ui/Tooltip";
import { IconButton } from "./components/ui/Button";
import { TextField } from "./components/ui/Field";
import { Menu, type MenuRequest } from "./components/ui/Menu";
import {
  api,
  errorText,
  type DocInfo,
  type DocState,
  type EditResult,
  type FontCatalog,
  type FontSummary,
  type LineEdit,
  type Meta,
} from "./lib/api";
import { loadPdf, search, type PDFDocumentProxy, type SearchHit } from "./lib/pdf";
import { forgetDocFonts } from "./lib/fontPreview";
import { forgetRecent, loadRecent, loadSettings, rememberRecent, saveSettings, type RecentFile, type Settings } from "./lib/storage";
import { applyTheme, watchSystemTheme, type Theme } from "./lib/theme";
import { length, paperName, plural } from "./lib/format";
import type { Tool, ZoomMode } from "./lib/types";
import { isEditableTarget } from "./lib/utils";

interface DocView {
  info: DocInfo;
  pdf: PDFDocumentProxy | null;
  zoom: ZoomMode;
  page: number;
  selectedPages: number[];
  fonts: FontSummary[] | null;
  fontsRevision: number;
}

const PDF_FILTER = [{ name: "PDF documents", extensions: ["pdf"] }];

function initialZoom(s: Settings): ZoomMode {
  return s.defaultZoom === "100" ? 1 : s.defaultZoom;
}

export default function App() {
  const [settings, setSettingsState] = useState<Settings>(loadSettings);
  const [theme, setTheme] = useState<Theme>(() => (document.documentElement.dataset.theme as Theme) ?? "dark");
  const [docs, setDocs] = useState<DocView[]>([]);
  const [activeId, setActiveId] = useState<number | null>(null);
  const [tool, setTool] = useState<Tool>("select");
  const [selection, setSelection] = useState<Selection>(null);
  const [sideTab, setSideTab] = useState<SideTab>("pages");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [scale, setScale] = useState(1);
  const [navigate, setNavigate] = useState<{ page: number; nonce: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [menu, setMenu] = useState<MenuRequest | null>(null);
  const [recent, setRecent] = useState<RecentFile[]>(loadRecent);
  const [showSettings, setShowSettings] = useState(false);
  const [password, setPassword] = useState<{ path: string; wrong: boolean } | null>(null);
  const [unsaved, setUnsaved] = useState<{ ids: number[]; then: () => void } | null>(null);
  const [dropping, setDropping] = useState(false);
  const [catalog, setCatalog] = useState<FontCatalog | null>(null);
  const [textStyle, setTextStyle] = useState<TextStyle>({
    font: { kind: "standard", name: "Helvetica" },
    size: 12,
    color: "#111318",
  });
  const textStyleTouched = useRef(false);
  const [findOpen, setFindOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [hitIndex, setHitIndex] = useState(0);
  const [searching, setSearching] = useState(false);
  const [pageSize, setPageSize] = useState<{ w: number; h: number } | null>(null);

  const active = docs.find((d) => d.info.state.id === activeId) ?? null;
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const docsRef = useRef(docs);
  docsRef.current = docs;

  // ----- helpers -----

  const toast = useCallback((kind: Toast["kind"], title: string, body?: string) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-3), { id, kind, title, body }]);
    window.setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), kind === "error" ? 9000 : kind === "warning" ? 8000 : 4000);
  }, []);

  const setSettings = useCallback((s: Settings) => {
    setSettingsState(s);
    saveSettings(s);
  }, []);

  const updateDoc = useCallback((id: number, fn: (d: DocView) => DocView) => {
    setDocs((list) => list.map((d) => (d.info.state.id === id ? fn(d) : d)));
  }, []);

  // ----- theme & window -----

  useEffect(() => {
    let live = true;
    applyTheme(settings.theme).then((t) => {
      if (!live) return;
      setTheme(t);
      getCurrentWindow().show().catch(() => {});
    });
    if (settings.theme !== "system") return () => void (live = false);
    const stop = watchSystemTheme(() => applyTheme("system").then(setTheme));
    return () => {
      live = false;
      stop();
    };
  }, [settings.theme]);

  useEffect(() => {
    const title = active ? `${active.info.state.dirty ? "• " : ""}${active.info.state.name} — Inq Studio` : "Inq Studio";
    document.title = title;
    getCurrentWindow().setTitle(title).catch(() => {});
  }, [active?.info.state.name, active?.info.state.dirty, active]);

  useEffect(() => {
    api.systemFonts().then(setCatalog).catch(() => {});
  }, []);

  // ----- opening -----

  const openPath = useCallback(
    async (path: string, pw?: string) => {
      const existing = docsRef.current.find((d) => d.info.state.path?.toLowerCase() === path.toLowerCase());
      if (existing) {
        setActiveId(existing.info.state.id);
        return;
      }
      setBusy(true);
      try {
        const info = await api.open(path, pw);
        const pdf = await loadPdf(await api.bytes(info.state.id));
        const view: DocView = {
          info,
          pdf,
          zoom: initialZoom(settings),
          page: 0,
          selectedPages: [],
          fonts: null,
          fontsRevision: -1,
        };
        setDocs((list) => [...list, view]);
        setActiveId(info.state.id);
        setSelection(null);
        setPassword(null);
        setRecent(rememberRecent({ path, name: info.state.name, pages: info.state.pages }));
      } catch (e) {
        const msg = errorText(e);
        if (msg === "PASSWORD_REQUIRED") setPassword({ path, wrong: !!pw });
        else {
          toast("error", "Could not open the file", msg);
          if (/not find|cannot find|No such file|os error 2/i.test(msg)) setRecent(forgetRecent(path));
        }
      } finally {
        setBusy(false);
      }
    },
    [settings, toast],
  );

  const openPicker = useCallback(async () => {
    const picked = await openDialog({ multiple: true, filters: PDF_FILTER }).catch(() => null);
    const paths = picked == null ? [] : Array.isArray(picked) ? picked : [picked];
    for (const p of paths) await openPath(p);
  }, [openPath]);

  // Files from "Open with" and drag and drop.
  useEffect(() => {
    api
      .startupFiles()
      .then(async (files) => {
        for (const f of files) await openPath(f);
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent(async (event) => {
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") setDropping(true);
      else if (p.type === "leave") setDropping(false);
      else if (p.type === "drop") {
        setDropping(false);
        const pdfs = p.paths.filter((x) => x.toLowerCase().endsWith(".pdf"));
        if (!pdfs.length) toast("warning", "Only PDF files can be opened");
        for (const f of pdfs) await openPath(f);
      }
    });
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, [openPath, toast]);

  // ----- reloading after changes -----

  const refresh = useCallback(
    async (id: number, state: DocState) => {
      const pdf = await loadPdf(await api.bytes(id));
      let old: PDFDocumentProxy | null = null;
      updateDoc(id, (d) => {
        old = d.pdf;
        return {
          ...d,
          pdf,
          info: { ...d.info, state },
          page: Math.min(d.page, state.pages - 1),
          selectedPages: d.selectedPages.filter((p) => p < state.pages),
        };
      });
      // Let pages still rendering from the old copy finish or cancel first.
      window.setTimeout(() => (old as PDFDocumentProxy | null)?.destroy(), 2500);
    },
    [updateDoc],
  );

  const run = useCallback(
    async (id: number, fn: () => Promise<DocState | EditResult>, success?: string): Promise<boolean> => {
      setBusy(true);
      try {
        const r = await fn();
        const state = "state" in r ? r.state : r;
        const notice = "notice" in r ? r.notice : null;
        await refresh(id, state);
        if (notice) {
          const kind = /instead|substitute|not installed/i.test(notice) ? "warning" : "info";
          toast(kind, kind === "warning" ? "Font substituted" : "Done", notice);
        } else if (success) toast("success", success);
        return true;
      } catch (e) {
        toast("error", "That didn’t work", errorText(e));
        return false;
      } finally {
        setBusy(false);
      }
    },
    [refresh, toast],
  );

  // Fonts of the active document, refreshed after every change.
  useEffect(() => {
    if (!active || active.fontsRevision === active.info.state.revision) return;
    const id = active.info.state.id;
    const rev = active.info.state.revision;
    let live = true;
    api
      .fonts(id)
      .then((fonts) => live && updateDoc(id, (d) => ({ ...d, fonts, fontsRevision: rev })))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [active?.info.state.id, active?.info.state.revision, active, updateDoc]);

  // Default "Add text" font: the document's most used editable font.
  useEffect(() => {
    if (textStyleTouched.current || !active?.fonts?.length) return;
    const best = [...active.fonts].filter((f) => f.editable).sort((a, b) => b.pages.length - a.pages.length)[0];
    if (best) setTextStyle((s) => ({ ...s, font: { kind: "document", key: best.key } }));
  }, [active?.fonts]);

  // Current page size for the inspector and status bar.
  useEffect(() => {
    if (!active?.pdf) {
      setPageSize(null);
      return;
    }
    let live = true;
    active.pdf
      .getPage(active.page + 1)
      .then((p) => {
        const vp = p.getViewport({ scale: 1 });
        if (live) setPageSize({ w: vp.width, h: vp.height });
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [active?.pdf, active?.page]);

  // ----- document commands -----

  const id = active?.info.state.id ?? null;

  const goTo = useCallback(
    (p: number) => {
      if (!active) return;
      const page = Math.max(0, Math.min(active.info.state.pages - 1, p));
      updateDoc(active.info.state.id, (d) => ({ ...d, page }));
      setNavigate({ page, nonce: Date.now() });
    },
    [active, updateDoc],
  );

  const saveAs = useCallback(
    async (docId: number): Promise<boolean> => {
      const d = docsRef.current.find((x) => x.info.state.id === docId);
      if (!d) return false;
      const path = await saveDialog({ defaultPath: d.info.state.path ?? d.info.state.name, filters: PDF_FILTER }).catch(() => null);
      if (!path) return false;
      try {
        const state = await api.save(docId, path, settingsRef.current.saveMode);
        updateDoc(docId, (x) => ({ ...x, info: { ...x.info, state } }));
        setRecent(rememberRecent({ path, name: state.name, pages: state.pages }));
        toast("success", "Saved", state.name);
        return true;
      } catch (e) {
        toast("error", "Saving failed", errorText(e));
        return false;
      }
    },
    [toast, updateDoc],
  );

  const save = useCallback(
    async (docId: number): Promise<boolean> => {
      const d = docsRef.current.find((x) => x.info.state.id === docId);
      if (!d) return false;
      if (!d.info.state.path) return saveAs(docId);
      try {
        const mode = settingsRef.current.saveMode;
        const state = await api.save(docId, undefined, mode);
        updateDoc(docId, (x) => ({ ...x, info: { ...x.info, state } }));
        const incremental = mode === "incremental" || (mode === "auto" && d.info.state.signed);
        toast("success", "Saved", incremental ? `${state.name}: changes appended, signatures kept` : state.name);
        return true;
      } catch (e) {
        toast("error", "Saving failed", errorText(e));
        return false;
      }
    },
    [saveAs, toast, updateDoc],
  );

  const closeNow = useCallback(
    (docId: number) => {
      const d = docsRef.current.find((x) => x.info.state.id === docId);
      api.close(docId).catch(() => {});
      d?.pdf?.destroy();
      forgetDocFonts(docId);
      setDocs((list) => {
        const i = list.findIndex((x) => x.info.state.id === docId);
        const next = list.filter((x) => x.info.state.id !== docId);
        setActiveId((cur) => (cur === docId ? (next[Math.min(i, next.length - 1)]?.info.state.id ?? null) : cur));
        return next;
      });
      setSelection(null);
    },
    [],
  );

  const closeDoc = useCallback(
    (docId: number) => {
      const d = docsRef.current.find((x) => x.info.state.id === docId);
      if (d?.info.state.dirty) setUnsaved({ ids: [docId], then: () => closeNow(docId) });
      else closeNow(docId);
    },
    [closeNow],
  );

  // Ask before the window closes with unsaved work.
  useEffect(() => {
    const win = getCurrentWindow();
    const un = win.onCloseRequested((event) => {
      const dirty = docsRef.current.filter((d) => d.info.state.dirty).map((d) => d.info.state.id);
      if (!dirty.length) return;
      event.preventDefault();
      setUnsaved({ ids: dirty, then: () => win.destroy() });
    });
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  const undo = useCallback(() => {
    if (id == null || !active?.info.state.canUndo) return;
    setSelection(null);
    run(id, () => api.undo(id));
  }, [id, active, run]);

  const redo = useCallback(() => {
    if (id == null || !active?.info.state.canRedo) return;
    setSelection(null);
    run(id, () => api.redo(id));
  }, [id, active, run]);

  const actions: ViewerActions = useMemo(
    () => ({
      editLines: async (page, lines, revision, edit) => {
        if (id == null) return false;
        const ok = await run(id, () => api.editLines(id, page, revision, lines, edit));
        if (ok) setSelection(null);
        return ok;
      },
      editBlock: async (page, block, revision, text) => {
        if (id == null) return false;
        const ok = await run(id, () => api.editBlock(id, page, revision, block, text));
        if (ok) setSelection(null);
        return ok;
      },
      addText: async (page, spec) => (id == null ? false : run(id, () => api.addText(id, page, spec))),
      addAnnot: async (page, annot) => (id == null ? false : run(id, () => api.addAnnot(id, page, annot))),
    }),
    [id, run],
  );

  const editSelected = useCallback(
    (edit: LineEdit) => {
      if (selection?.kind !== "lines") return;
      actions.editLines(
        selection.page,
        selection.lines.map((l) => l.id),
        selection.revision,
        edit,
      );
    },
    [selection, actions],
  );

  const deleteSelectedAnnot = useCallback(() => {
    if (id == null || selection?.kind !== "annot") return;
    const { page, annot } = selection;
    setSelection(null);
    run(id, () => api.deleteAnnot(id, page, annot.index), "Annotation deleted");
  }, [id, selection, run]);

  const pageAction = useCallback(
    async (action: PageAction, pages: number[]) => {
      if (id == null || !active) return;
      const sorted = [...pages].sort((a, b) => a - b);
      const last = sorted[sorted.length - 1];
      setSelection(null);
      switch (action) {
        case "rotate-left":
        case "rotate-right":
          await run(id, () => api.rotate(id, sorted, action === "rotate-left" ? -90 : 90));
          break;
        case "duplicate":
          await run(id, () => api.duplicate(id, sorted[0]), "Page duplicated");
          break;
        case "blank-before":
          await run(id, () => api.insertBlank(id, sorted[0]), "Blank page inserted");
          break;
        case "blank-after":
          await run(id, () => api.insertBlank(id, last + 1), "Blank page inserted");
          break;
        case "insert-file": {
          const picked = await openDialog({ multiple: false, filters: PDF_FILTER }).catch(() => null);
          if (typeof picked === "string") await run(id, () => api.insertFile(id, picked, last + 1));
          break;
        }
        case "extract": {
          const base = active.info.state.name.replace(/\.pdf$/i, "");
          const label = sorted.length === 1 ? `page ${sorted[0] + 1}` : `pages ${sorted[0] + 1}-${last + 1}`;
          const path = await saveDialog({ defaultPath: `${base} (${label}).pdf`, filters: PDF_FILTER }).catch(() => null);
          if (!path) break;
          try {
            await api.extract(id, sorted, path);
            toast("success", `Extracted ${plural(sorted.length, "page")}`, path);
          } catch (e) {
            toast("error", "Extracting failed", errorText(e));
          }
          break;
        }
        case "delete":
          if (await run(id, () => api.deletePages(id, sorted), `Deleted ${plural(sorted.length, "page")}`)) {
            updateDoc(id, (d) => ({ ...d, selectedPages: [] }));
          }
          break;
      }
    },
    [id, active, run, toast, updateDoc],
  );

  const setZoom = useCallback((z: ZoomMode) => id != null && updateDoc(id, (d) => ({ ...d, zoom: z })), [id, updateDoc]);

  // ----- find -----

  useEffect(() => {
    if (!findOpen || !active?.pdf || !query.trim()) {
      setHits([]);
      return;
    }
    const signal = { cancelled: false };
    const t = window.setTimeout(async () => {
      setSearching(true);
      const all = await search(active.pdf!, query.trim(), (partial) => !signal.cancelled && setHits(partial), signal);
      if (signal.cancelled) return;
      setHits(all);
      setSearching(false);
      const first = all.findIndex((h) => h.page >= active.page);
      setHitIndex(first >= 0 ? first : 0);
    }, 220);
    return () => {
      signal.cancelled = true;
      window.clearTimeout(t);
      setSearching(false);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, findOpen, active?.pdf]);

  const stepHit = (dir: 1 | -1) => {
    if (!hits.length) return;
    const next = (hitIndex + dir + hits.length) % hits.length;
    setHitIndex(next);
    goTo(hits[next].page);
  };

  const viewerHits = useMemo(() => hits.map((h, i) => ({ ...h, current: i === hitIndex })), [hits, hitIndex]);

  // ----- keyboard -----

  const findRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
      if (mod) {
        const handled = () => e.preventDefault();
        if (k === "o") return handled(), openPicker();
        if (k === "s" && id != null) return handled(), e.shiftKey ? saveAs(id) : save(id);
        if (k === "w" && id != null) return handled(), closeDoc(id);
        if (k === "z" && !isEditableTarget(e.target)) return handled(), e.shiftKey ? redo() : undo();
        if (k === "y" && !isEditableTarget(e.target)) return handled(), redo();
        if (k === "f" && active) {
          handled();
          setFindOpen(true);
          window.setTimeout(() => findRef.current?.select(), 0);
          return;
        }
        if (k === "b") return handled(), setSidebarOpen((v) => !v);
        if (k === "i") return handled(), setInspectorOpen((v) => !v);
        if (k === ",") return handled(), setShowSettings(true);
        if ((k === "=" || k === "+") && active) return handled(), setZoom(Math.min(5, Math.round(scale * 1.2 * 100) / 100));
        if (k === "-" && active) return handled(), setZoom(Math.max(0.25, Math.round((scale / 1.2) * 100) / 100));
        if (k === "0" && active) return handled(), setZoom("fit-width");
        if (k === "tab" && docs.length > 1) {
          handled();
          const i = docs.findIndex((d) => d.info.state.id === id);
          const n = (i + (e.shiftKey ? -1 : 1) + docs.length) % docs.length;
          setActiveId(docs[n].info.state.id);
          setSelection(null);
        }
        return;
      }
      if (isEditableTarget(e.target) || e.altKey) return;
      if (e.key === "Escape") {
        if (selection) setSelection(null);
        else if (findOpen) setFindOpen(false);
        else setTool("select");
        return;
      }
      if (!active) return;
      if (TOOL_KEYS[k] && !e.shiftKey) {
        setTool(TOOL_KEYS[k]);
        setSelection(null);
        return;
      }
      if (e.key === "Delete" || e.key === "Backspace") {
        if (selection?.kind === "lines" && selection.lines.every((l) => l.editable)) editSelected({ delete: true });
        else if (selection?.kind === "annot" && selection.annot.removable) deleteSelectedAnnot();
        return;
      }
      if (e.key === "Enter" && selection?.kind === "lines" && selection.lines.length === 1 && selection.lines[0].editable && !selection.editing) {
        e.preventDefault();
        const para = selection.block && selection.block.lines.length > 1;
        setSelection({ ...selection, editing: para ? "block" : "line" });
        return;
      }
      if (selection?.kind === "lines" && selection.lines.every((l) => l.editable) && e.key.startsWith("Arrow")) {
        e.preventDefault();
        const step = e.shiftKey ? 10 : 1;
        const d = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] }[e.key];
        if (d) editSelected({ dx: d[0], dy: d[1] });
        return;
      }
      if (e.key === "PageDown") return e.preventDefault(), goTo(active.page + 1);
      if (e.key === "PageUp") return e.preventDefault(), goTo(active.page - 1);
      if (e.key === "Home") return e.preventDefault(), goTo(0);
      if (e.key === "End") return e.preventDefault(), goTo(active.info.state.pages - 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    id,
    active,
    docs,
    selection,
    findOpen,
    scale,
    openPicker,
    save,
    saveAs,
    closeDoc,
    undo,
    redo,
    goTo,
    setZoom,
    editSelected,
    deleteSelectedAnnot,
  ]);

  // Selection belongs to one tool and one document.
  useEffect(() => setSelection(null), [activeId]);

  const markColor = settings.toolColors[tool] ?? "#ffd43b";
  const activeFontKey = selection?.kind === "lines" ? selection.lines[0].fontKey : null;

  // ----- render -----

  return (
    <div className="app">
      <TopBar
        hud={
          active
            ? {
                name: active.info.state.name,
                page: active.page,
                pages: active.info.state.pages,
                version: active.info.version,
                dirty: active.info.state.dirty,
                encrypted: active.info.wasEncrypted,
                signed: active.info.state.signed,
              }
            : null
        }
        theme={theme}
        sidebarOpen={sidebarOpen}
        inspectorOpen={inspectorOpen}
        canSave={!!active}
        canUndo={!!active?.info.state.canUndo}
        canRedo={!!active?.info.state.canRedo}
        onToggleSidebar={() => setSidebarOpen((v) => !v)}
        onToggleInspector={() => setInspectorOpen((v) => !v)}
        onOpen={openPicker}
        onSave={() => id != null && save(id)}
        onUndo={undo}
        onRedo={redo}
        onToggleTheme={() => setSettings({ ...settings, theme: theme === "dark" ? "light" : "dark" })}
        onSettings={() => setShowSettings(true)}
      />

      <main className="workbench" style={{ gridTemplateColumns: `${sidebarOpen && active?.pdf ? "auto " : ""}minmax(0, 1fr)${inspectorOpen && active ? " auto" : ""}` }}>
        {sidebarOpen && active?.pdf && (
          <Sidebar
            tab={sideTab}
            onTab={setSideTab}
            docId={active.info.state.id}
            revision={active.info.state.revision}
            pdf={active.pdf}
            page={active.page}
            selected={active.selectedPages}
            onSelectPages={(pages) => updateDoc(active.info.state.id, (d) => ({ ...d, selectedPages: pages }))}
            onNavigate={goTo}
            onReorder={(order) => run(active.info.state.id, () => api.reorder(active.info.state.id, order), "Pages reordered")}
            onPageAction={pageAction}
            onMenu={setMenu}
            fonts={active.fonts}
            activeFontKey={activeFontKey}
            selection={selection}
            onSelect={setSelection}
            onDeleteAnnot={(page, index) => run(active.info.state.id, () => api.deleteAnnot(active.info.state.id, page, index), "Annotation deleted")}
            history={active.info.state.history}
            future={active.info.state.future}
            onHistoryGo={(steps) => {
              setSelection(null);
              run(active.info.state.id, () => api.historyGo(active.info.state.id, steps));
            }}
          />
        )}

        <section className="panel stage" aria-label="Document">
          {docs.length > 0 && (
            <DocTabs
              docs={docs.map((d) => ({ id: d.info.state.id, name: d.info.state.name, dirty: d.info.state.dirty }))}
              activeId={activeId}
              onSelect={setActiveId}
              onClose={closeDoc}
              onOpen={openPicker}
            />
          )}
          {active?.pdf ? (
            <>
              <Toolbar
                tool={tool}
                onTool={(t) => {
                  setTool(t);
                  setSelection(null);
                }}
                zoom={active.zoom}
                scale={scale}
                onZoom={setZoom}
                page={active.page}
                pages={active.info.state.pages}
                onPage={goTo}
                findOpen={findOpen}
                onFind={() => {
                  setFindOpen((v) => !v);
                  window.setTimeout(() => findRef.current?.select(), 0);
                }}
              />
              {findOpen && (
                <div className="findbar">
                  <TextField
                    ref={findRef}
                    compact
                    icon={Search}
                    placeholder="Find in document"
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") stepHit(e.shiftKey ? -1 : 1);
                      if (e.key === "Escape") setFindOpen(false);
                    }}
                    aria-label="Find in document"
                  />
                  <span className="find-count">
                    {query.trim()
                      ? hits.length
                        ? `${hitIndex + 1} of ${hits.length}${searching ? "…" : ""}`
                        : searching
                          ? "Searching…"
                          : "No matches"
                      : ""}
                  </span>
                  <IconButton icon={ChevronUp} label="Previous match" shortcut="Shift+Enter" spacing="compact" disabled={!hits.length} onClick={() => stepHit(-1)} />
                  <IconButton icon={ChevronDown} label="Next match" shortcut="Enter" spacing="compact" disabled={!hits.length} onClick={() => stepHit(1)} />
                  <span className="spacer" />
                  <IconButton icon={X} label="Close find" shortcut="Esc" spacing="compact" onClick={() => setFindOpen(false)} />
                </div>
              )}
              <Viewer
                key={active.info.state.id}
                docId={active.info.state.id}
                revision={active.info.state.revision}
                pdf={active.pdf}
                zoom={active.zoom}
                onScale={setScale}
                onZoom={setZoom}
                page={active.page}
                onPage={(p) => updateDoc(active.info.state.id, (d) => ({ ...d, page: p }))}
                navigate={navigate}
                tool={tool}
                showLineBoxes={settings.showLineBoxes}
                snap={settings.snap}
                fonts={active.fonts}
                selection={selection}
                onSelect={setSelection}
                textStyle={textStyle}
                markColor={markColor}
                inkWidth={settings.inkWidth}
                author={settings.author}
                hits={findOpen ? viewerHits : []}
                actions={actions}
                busy={busy}
              />
              <footer className="statusbar">
                <span className="status-item is-mono">{Math.round(scale * 100)}%</span>
                {pageSize && (
                  <span className="status-item">
                    {length(pageSize.w, settings.units)} × {length(pageSize.h, settings.units)}
                    {paperName(pageSize.w, pageSize.h) && <span className="is-muted">{paperName(pageSize.w, pageSize.h)}</span>}
                  </span>
                )}
                {active.fonts && <span className="status-item">{plural(active.fonts.length, "font")}</span>}
                <span className="status-hint">{TOOL_HINTS[tool]}</span>
              </footer>
            </>
          ) : (
            <Welcome
              recent={recent}
              onOpen={openPicker}
              onOpenPath={(p) => openPath(p)}
              onForget={(p) => setRecent(forgetRecent(p))}
            />
          )}
        </section>

        {inspectorOpen && active && (
          <Inspector
            docId={active.info.state.id}
            info={active.info}
            selection={selection}
            tool={tool}
            fonts={active.fonts}
            catalog={catalog}
            textStyle={textStyle}
            onTextStyle={(s) => {
              textStyleTouched.current = true;
              setTextStyle(s);
            }}
            markColor={markColor}
            onMarkColor={(c) => setSettings({ ...settings, toolColors: { ...settings.toolColors, [tool]: c } })}
            settings={settings}
            onSettings={setSettings}
            pageSize={pageSize}
            onEditLine={editSelected}
            onStartEditing={(what) => selection?.kind === "lines" && setSelection({ ...selection, editing: what })}
            onDeleteAnnot={deleteSelectedAnnot}
            onSaveMeta={(m: Meta) => run(active.info.state.id, () => api.setMeta(active.info.state.id, m), "Properties updated").then(async () => {
              const info = await api.info(active.info.state.id);
              updateDoc(active.info.state.id, (d) => ({ ...d, info }));
            })}
          />
        )}
      </main>

      {menu && <Menu request={menu} onClose={() => setMenu(null)} />}
      {showSettings && <SettingsDialog settings={settings} onChange={setSettings} onClose={() => setShowSettings(false)} />}
      {password && (
        <PasswordDialog
          name={password.path.split(/[\\/]/).pop() ?? password.path}
          wrong={password.wrong}
          onClose={() => setPassword(null)}
          onSubmit={(pw) => openPath(password.path, pw)}
        />
      )}
      {unsaved && (
        <UnsavedDialog
          names={unsaved.ids.map((i) => docs.find((d) => d.info.state.id === i)?.info.state.name ?? "Document")}
          onCancel={() => setUnsaved(null)}
          onDiscard={() => {
            const u = unsaved;
            setUnsaved(null);
            u.then();
          }}
          onSave={async () => {
            const u = unsaved;
            setUnsaved(null);
            for (const i of u.ids) if (!(await save(i))) return;
            u.then();
          }}
        />
      )}
      {dropping && (
        <div className="drop-veil">
          <div className="drop-veil-card">Drop PDFs to open them</div>
        </div>
      )}
      <Toasts toasts={toasts} onDismiss={(tid) => setToasts((t) => t.filter((x) => x.id !== tid))} />
      <TooltipLayer />
    </div>
  );
}
