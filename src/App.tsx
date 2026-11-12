import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { findUpdateQuietly } from "./lib/updates";
import { BookOpen, ChevronDown, ChevronUp, Route, Search, X } from "lucide-react";
import { DocTabs, SHAPE_TOOLS, TOOL_HINTS, TOOL_KEYS, Toasts, Toolbar, TopBar, type Toast } from "./components/Chrome";
import { PageMarksDialog, ResizeDialog, SplitDialog, type MarksTab } from "./components/ToolDialogs";
import type { ToolOptions } from "./components/PageExtras";
import { printDocument } from "./lib/print";
import { Accessibility, Archive, Droplets, FileOutput, GitCompareArrows, Hash, Layers, Lock, PanelTop, PenLine, Printer, Scaling, ScanText, Scissors, ShieldCheck, Tags, Zap } from "lucide-react";
import { AccessibilityDialog, ExportDialog, OcrDialog, OptimizeDialog, PdfaDialog, ProtectDialog, SignaturesDialog, SignDialog } from "./components/ProDialogs";
import { CompareView } from "./components/Compare";
import { Welcome } from "./components/Welcome";
import { TitleBar } from "./components/TitleBar";
import { Tour, tourSeen } from "./components/Tour";
import { ManualDialog } from "./components/Manual";
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
  type FieldInfo,
  type Rect,
  type Comparison,
  type OrderItem,
} from "./lib/api";
import { loadDocument, search, type PDFDocumentProxy, type SearchHit } from "./lib/pdf";
import { forgetDocFonts } from "./lib/fontPreview";
import { forgetRecent, loadRecent, loadSettings, rememberRecent, saveSettings, type RecentFile, type Settings } from "./lib/storage";
import { applyTheme, watchSystemTheme, type Theme } from "./lib/theme";
import { length, paperName, plural } from "./lib/format";
import type { ShapeTool, Tool, ZoomMode } from "./lib/types";
import { isEditableTarget, MOD } from "./lib/utils";
import { t } from "./lib/i18n";

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

/** The document as the engine currently has it, ready to render. */
function openRendering(id: number): Promise<PDFDocumentProxy> {
  return loadDocument(
    () => api.renderInfo(id),
    (generation, begin, end) => api.range(id, generation, begin, end),
    () => api.bytes(id),
  );
}

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
  const [showManual, setShowManual] = useState(false);
  /** The tour runs by itself on the first start. */
  const [touring, setTouring] = useState(() => !tourSeen());
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
  const [shape, setShape] = useState<ShapeTool>("rect");
  const [toolOptions, setToolOptions] = useState<ToolOptions>({ stampLabel: t("Approved"), fill: null, textSize: 12 });
  /** Redaction marks waiting to be applied: document → page → rectangles. */
  const [redactions, setRedactions] = useState<Record<number, Record<number, Rect[]>>>({});
  const [redactFill, setRedactFill] = useState<"black" | "white" | "none">("black");
  const [fields, setFields] = useState<FieldInfo[]>([]);
  const [labels, setLabels] = useState<string[] | null>(null);
  const [dialog, setDialog] = useState<
    | null
    | { kind: "marks"; tab: MarksTab }
    | { kind: "resize" | "split" | "protect" | "optimize" | "export" | "ocr" | "pdfa" | "access" | "signatures" }
    | { kind: "sign"; field: string | null }
  >(null);
  const [comparing, setComparing] = useState<{ a: number; b: number; result: Comparison | null } | null>(null);
  /** The reading order drawn over the current page, when switched on. */
  const [showOrder, setShowOrder] = useState(false);
  const [order, setOrder] = useState<{ page: number; items: OrderItem[] } | null>(null);
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
    // The page and everything React draws switch in the same frame; the title bar follows.
    if (settings.theme !== "system") setTheme(settings.theme);
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
    const title = active ? t("{0}{1} — Inq Studio", active.info.state.dirty ? "• " : "", active.info.state.name) : t("Inq Studio");
    document.title = title;
    getCurrentWindow().setTitle(title).catch(() => {});
  }, [active?.info.state.name, active?.info.state.dirty, active]);

  useEffect(() => {
    api.systemFonts().then(setCatalog).catch(() => {});
  }, []);

  // ----- opening -----

  const openPath = useCallback(
    async (path: string, pw?: string): Promise<number | null> => {
      const existing = docsRef.current.find((d) => d.info.state.path?.toLowerCase() === path.toLowerCase());
      if (existing) {
        setActiveId(existing.info.state.id);
        return existing.info.state.id;
      }
      setBusy(true);
      try {
        const info = await api.open(path, pw);
        const pdf = await openRendering(info.state.id);
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
        return info.state.id;
      } catch (e) {
        const msg = errorText(e);
        if (msg === "PASSWORD_REQUIRED") setPassword({ path, wrong: !!pw });
        else {
          toast("error", t("Could not open the file"), msg);
          if (/not find|cannot find|No such file|os error 2/i.test(msg)) setRecent(forgetRecent(path));
        }
      } finally {
        setBusy(false);
      }
      return null;
    },
    [settings, toast],
  );

  const openPicker = useCallback(async () => {
    const picked = await openDialog({ multiple: true, filters: PDF_FILTER }).catch(() => null);
    const paths = picked == null ? [] : Array.isArray(picked) ? picked : [picked];
    for (const p of paths) await openPath(p);
  }, [openPath]);

  // Files from "Open with" and drag and drop.
  const started = useRef(false);
  useEffect(() => {
    // Development mode runs effects twice; the files must open once.
    if (started.current) return;
    started.current = true;
    api
      .startupFiles()
      .then(async (files) => {
        for (const f of files) await openPath(f);
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // A second start (double-click on another PDF) hands its files to this window.
  useEffect(() => {
    const un = listen<string[]>("open-files", async (event) => {
      for (const f of event.payload) await openPath(f);
    });
    return () => {
      un.then((f) => f()).catch(() => {});
    };
  }, [openPath]);

  // Once at start: a newer version, and a crash report from last time.
  useEffect(() => {
    findUpdateQuietly().then((update) => {
      if (update) toast("info", t("Inq Studio {0} is available", update.version), t("Install it from Settings › Updates."));
    });
    api
      .logInfo()
      .then((l) => {
        if (!l.newCrashes.length) return;
        toast("warning", t("Inq Studio closed unexpectedly last time"), t("A crash report was saved; it opens in its folder now. Sending it along with a bug report helps to fix the cause."));
        revealItemInDir(l.newCrashes[l.newCrashes.length - 1]).catch(() => {});
        api.crashesSeen().catch(() => {});
      })
      .catch(() => {});
    const onError = (e: ErrorEvent) => api.log("error", `window: ${e.message} (${e.filename}:${e.lineno})`);
    const onRejection = (e: PromiseRejectionEvent) => api.log("error", `unhandled: ${errorText(e.reason)}`);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
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
        if (!pdfs.length) toast("warning", t("Only PDF files can be opened"));
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
      const pdf = await openRendering(id);
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
          toast(kind, kind === "warning" ? t("Font substituted") : t("Done"), notice);
        } else if (success) toast("success", success);
        return true;
      } catch (e) {
        toast("error", t("That didn’t work"), errorText(e));
        api.log("warn", `command failed: ${errorText(e)}`);
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

  // Form fields and page labels of the active document, refreshed after every change.
  useEffect(() => {
    if (!active) {
      setFields([]);
      setLabels(null);
      return;
    }
    const docId = active.info.state.id;
    let live = true;
    api
      .fields(docId)
      .then((f) => live && setFields(f))
      .catch(() => live && setFields([]));
    api
      .labels(docId)
      .then((l) => live && setLabels(l.strings))
      .catch(() => live && setLabels(null));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active?.info.state.id, active?.info.state.revision]);

  const chooseTool = useCallback((t: Tool) => {
    setTool(t);
    setSelection(null);
    if (SHAPE_TOOLS.some((s) => s.tool === t)) setShape(t as ShapeTool);
  }, []);

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
        api.info(docId).then((info) => updateDoc(docId, (x) => ({ ...x, info }))).catch(() => {});
        setRecent(rememberRecent({ path, name: state.name, pages: state.pages }));
        toast("success", t("Saved"), state.name);
        return true;
      } catch (e) {
        toast("error", t("Saving failed"), errorText(e));
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
        api.info(docId).then((info) => updateDoc(docId, (x) => ({ ...x, info }))).catch(() => {});
        const incremental = mode === "incremental" || (mode === "auto" && d.info.state.signed);
        toast("success", t("Saved"), incremental ? t("{0}: changes appended, signatures kept", state.name) : state.name);
        return true;
      } catch (e) {
        toast("error", t("Saving failed"), errorText(e));
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
        // Rewriting or deleting a line keeps its paragraph in shape, when that is switched on.
        const whole = settingsRef.current.reflow && (edit.text != null || edit.delete) ? { ...edit, reflow: true } : edit;
        const ok = await run(id, () => api.editLines(id, page, revision, lines, whole));
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
      updateAnnot: async (page, index, change) => {
        if (id == null) return false;
        const ok = await run(id, () => api.updateAnnot(id, page, index, change));
        if (ok) {
          const list = await api.annots(id, page).catch(() => []);
          const a = list.find((x) => x.index === index);
          setSelection((s) => (s?.kind === "annot" && s.page === page && s.annot.index === index ? (a ? { kind: "annot", page, annot: a } : null) : s));
        }
        return ok;
      },
      setField: async (name, value) => (id == null ? false : run(id, () => api.setField(id, name, value))),
      placeImage: async (page, index, bbox) => {
        if (id == null) return false;
        const ok = await run(id, () => api.placeImage(id, page, index, bbox));
        if (ok) setSelection(null);
        return ok;
      },
      insertImage: async (page, at) => {
        if (id == null) return false;
        const path = await openDialog({ multiple: false, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg"] }] }).catch(() => null);
        if (typeof path !== "string") return false;
        return run(id, () => api.insertImage(id, page, path, at));
      },
      crop: async (page, area, scope) => {
        if (id == null) return false;
        const d = docsRef.current.find((x) => x.info.state.id === id);
        const all = Array.from({ length: d?.info.state.pages ?? 0 }, (_, i) => i);
        const pages = scope === "page" ? [page] : scope === "all" ? all : d?.selectedPages.length ? d.selectedPages : [page];
        return run(id, () => api.crop(id, pages, area), t("Cropped {0}", pages.length === 1 ? "the page" : `${pages.length} pages`));
      },
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

  const deleteImage = useCallback(() => {
    if (id == null || selection?.kind !== "image") return;
    const { page, image } = selection;
    setSelection(null);
    run(id, () => api.deleteImage(id, page, image.index), t("Image deleted"));
  }, [id, selection, run]);

  const replaceImage = useCallback(async () => {
    if (id == null || selection?.kind !== "image") return;
    const { page, image } = selection;
    const path = await openDialog({ multiple: false, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg"] }] }).catch(() => null);
    if (typeof path !== "string") return;
    setSelection(null);
    run(id, () => api.replaceImage(id, page, image.index, path), t("Image replaced"));
  }, [id, selection, run]);

  const extractImage = useCallback(async () => {
    if (id == null || selection?.kind !== "image" || !active) return;
    const { page, image } = selection;
    const base = active.info.state.name.replace(/\.pdf$/i, "");
    const ext = image.filter === "DCTDecode" && !image.hasAlpha ? "jpg" : "png";
    const path = await saveDialog({ defaultPath: t("{0} p{1} image {2}.{3}", base, page + 1, image.index + 1, ext) }).catch(() => null);
    if (!path) return;
    try {
      const written = await api.extractImage(id, page, image.index, path);
      toast("success", t("Image saved"), written);
    } catch (e) {
      toast("error", t("Saving the image failed"), errorText(e));
    }
  }, [id, selection, active, toast]);

  const docRedactions = id != null ? (redactions[id] ?? {}) : {};
  const redactCount = Object.values(docRedactions).reduce((n, r) => n + r.length, 0);

  const markForRedaction = useCallback(
    async (query: string) => {
      if (id == null) return;
      try {
        const found = await api.find(id, query);
        if (!found.length) {
          toast("info", t("Not found"), t("“{0}” does not appear in the text of this document.", query));
          return;
        }
        setRedactions((all) => {
          const cur = { ...(all[id] ?? {}) };
          let n = 0;
          for (const f of found) {
            const rects = f.quads.map((q) => {
              const xs = q.map((p) => p[0]);
              const ys = q.map((p) => p[1]);
              return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] as Rect;
            });
            n += rects.length;
            cur[f.page] = [...(cur[f.page] ?? []), ...rects];
          }
          toast("success", t("Marked {0} occurrence{1}", n, n === 1 ? "" : "s"), t("Check the marks, then apply them in the inspector."));
          return { ...all, [id]: cur };
        });
      } catch (e) {
        toast("error", t("Search failed"), errorText(e));
      }
    },
    [id, toast],
  );

  const applyRedactions = useCallback(async () => {
    if (id == null) return;
    const marks = Object.entries(redactions[id] ?? {})
      .filter(([, r]) => r.length)
      .map(([page, rects]) => ({ page: Number(page), rects }));
    if (!marks.length) return;
    const fill: [number, number, number] | null = redactFill === "black" ? [0, 0, 0] : redactFill === "white" ? [1, 1, 1] : null;
    const ok = await run(id, () => api.redact(id, marks, fill));
    if (ok) setRedactions((all) => ({ ...all, [id]: {} }));
  }, [id, redactions, redactFill, run]);

  /** Document facts that are not part of the state every command returns. */
  const reloadInfo = useCallback(
    async (docId: number) => {
      const info = await api.info(docId).catch(() => null);
      if (info) updateDoc(docId, (d) => ({ ...d, info }));
    },
    [updateDoc],
  );

  const startCompare = useCallback(
    async (other: number | "file") => {
      if (id == null) return;
      let b: number | null = other === "file" ? null : other;
      if (other === "file") {
        const picked = await openDialog({ multiple: false, filters: PDF_FILTER }).catch(() => null);
        if (typeof picked !== "string") return;
        b = await openPath(picked);
        // Stay on the document the comparison started from.
        setActiveId(id);
      }
      if (b == null || b === id) return;
      setComparing({ a: id, b, result: null });
      try {
        const result = await api.compare(id, b);
        setComparing((c) => (c && c.a === id && c.b === b ? { ...c, result } : c));
      } catch (e) {
        setComparing(null);
        toast("error", t("Comparing failed"), errorText(e));
      }
    },
    [id, openPath, toast],
  );

  const signDocument = useCallback(
    async (identity: Parameters<typeof api.sign>[1], options: Parameters<typeof api.sign>[2], saveAsCopy: boolean): Promise<boolean> => {
      if (id == null) return false;
      const d = docsRef.current.find((x) => x.info.state.id === id);
      if (!d) return false;
      let path: string | null = null;
      if (saveAsCopy || !d.info.state.path) {
        const base = (d.info.state.path ?? d.info.state.name).replace(/\.pdf$/i, "");
        path = await saveDialog({ defaultPath: t("{0} (signed).pdf", base), filters: PDF_FILTER }).catch(() => null);
        if (!path) return false;
      }
      setSelection(null);
      const ok = await run(id, () => api.sign(id, identity, { ...options, timestampUrl: settingsRef.current.timestampUrl }, path), t("Signed and saved"));
      if (ok) {
        await reloadInfo(id);
        if (path) setRecent(rememberRecent({ path, name: path.split(/[\\/]/).pop() ?? path, pages: d.info.state.pages }));
      }
      return ok;
    },
    [id, run, reloadInfo],
  );

  // The reading order of the page in view, while it is switched on.
  useEffect(() => {
    if (!showOrder || !active) {
      setOrder(null);
      return;
    }
    let live = true;
    const page = active.page;
    api
      .readingOrder(active.info.state.id, page)
      .then((items) => live && setOrder({ page, items }))
      .catch(() => live && setOrder(null));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showOrder, active?.info.state.id, active?.info.state.revision, active?.page]);

  const print = useCallback(async () => {
    if (!active?.pdf) return;
    setBusy(true);
    try {
      await printDocument(active.pdf);
    } catch (e) {
      toast("error", t("Printing failed"), errorText(e));
    } finally {
      setBusy(false);
    }
  }, [active, toast]);

  const flatten = useCallback(
    (scope: "forms" | "annotations" | "all") => {
      if (id == null) return;
      setSelection(null);
      run(id, () => api.flatten(id, scope));
    },
    [id, run],
  );

  const deleteSelectedAnnot = useCallback(() => {
    if (id == null || selection?.kind !== "annot") return;
    const { page, annot } = selection;
    setSelection(null);
    run(id, () => api.deleteAnnot(id, page, annot.index), t("Annotation deleted"));
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
          await run(id, () => api.duplicate(id, sorted[0]), t("Page duplicated"));
          break;
        case "blank-before":
          await run(id, () => api.insertBlank(id, sorted[0]), t("Blank page inserted"));
          break;
        case "blank-after":
          await run(id, () => api.insertBlank(id, last + 1), t("Blank page inserted"));
          break;
        case "insert-file": {
          const picked = await openDialog({ multiple: false, filters: PDF_FILTER }).catch(() => null);
          if (typeof picked === "string") await run(id, () => api.insertFile(id, picked, last + 1));
          break;
        }
        case "extract": {
          const base = active.info.state.name.replace(/\.pdf$/i, "");
          const label = sorted.length === 1 ? t("page {0}", sorted[0] + 1) : t("pages {0}-{1}", sorted[0] + 1, last + 1);
          const path = await saveDialog({ defaultPath: t("{0} ({1}).pdf", base, label), filters: PDF_FILTER }).catch(() => null);
          if (!path) break;
          try {
            await api.extract(id, sorted, path);
            toast("success", t("Extracted {0}", plural(sorted.length, "page")), path);
          } catch (e) {
            toast("error", t("Extracting failed"), errorText(e));
          }
          break;
        }
        case "delete":
          if (await run(id, () => api.deletePages(id, sorted), t("Deleted {0}", plural(sorted.length, "page")))) {
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
      if (e.key === "F1") {
        e.preventDefault();
        setShowManual(true);
        return;
      }
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
        if (k === "p" && active) return handled(), print();
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
        if (comparing) return;
        if (selection) setSelection(null);
        else if (findOpen) setFindOpen(false);
        else setTool("select");
        return;
      }
      if (!active) return;
      if (TOOL_KEYS[k] && !e.shiftKey) {
        chooseTool(TOOL_KEYS[k]);
        return;
      }
      if (e.key === "Delete" || e.key === "Backspace") {
        if (selection?.kind === "lines" && selection.lines.every((l) => l.editable)) editSelected({ delete: true });
        else if (selection?.kind === "annot" && selection.annot.removable) deleteSelectedAnnot();
        else if (selection?.kind === "image") deleteImage();
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
    deleteImage,
    chooseTool,
    print,
  ]);

  // Selection belongs to one tool and one document.
  useEffect(() => setSelection(null), [activeId]);

  const markColor = settings.toolColors[tool] ?? "#ffd43b";
  const activeFontKey = selection?.kind === "lines" ? selection.lines[0].fontKey : null;

  // ----- render -----

  return (
    <div className="app">
      <TitleBar document={active?.info.state.name ?? null} dirty={!!active?.info.state.dirty} />
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
                protection: active.info.security.protected ? active.info.security.method : null,
                protectionPending: active.info.security.pending,
                pdfa: active.info.pdfa,
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
        onSaveAs={() => id != null && saveAs(id)}
        onUndo={undo}
        onRedo={redo}
        onToggleTheme={() => setSettings({ ...settings, theme: theme === "dark" ? "light" : "dark" })}
        onSettings={() => setShowSettings(true)}
        onHelp={(anchor) =>
          setMenu({
            anchor,
            items: [
              { label: t("Manual"), icon: BookOpen, shortcut: "F1", onSelect: () => setShowManual(true) },
              { label: t("Take the tour"), icon: Route, onSelect: () => setTouring(true) },
            ],
          })
        }
        canTools={!!active?.pdf}
        onSignatures={() => setDialog({ kind: "signatures" })}
        onProtect={() => setDialog({ kind: "protect" })}
        onPdfa={() => setDialog({ kind: "pdfa" })}
        onTools={(anchor) =>
          setMenu({
            anchor,
            align: "end",
            items: [
              { type: "heading", label: t("Page marks") },
              { label: t("Watermark…"), icon: Droplets, onSelect: () => setDialog({ kind: "marks", tab: "watermark" }) },
              { label: t("Header & footer…"), icon: PanelTop, onSelect: () => setDialog({ kind: "marks", tab: "header" }) },
              { label: t("Bates numbers…"), icon: Hash, onSelect: () => setDialog({ kind: "marks", tab: "bates" }) },
              { label: t("Page labels…"), icon: Tags, onSelect: () => setDialog({ kind: "marks", tab: "labels" }) },
              { type: "separator" },
              { label: t("Resize pages…"), icon: Scaling, onSelect: () => setDialog({ kind: "resize" }) },
              { label: t("Split document…"), icon: Scissors, onSelect: () => setDialog({ kind: "split" }) },
              { type: "separator" },
              { label: t("Flatten form fields"), icon: Layers, disabled: !fields.length, onSelect: () => flatten("forms") },
              { label: t("Flatten annotations"), icon: Layers, onSelect: () => flatten("annotations") },
              { label: t("Flatten everything"), icon: Layers, onSelect: () => flatten("all") },
              { type: "heading", label: t("Professional") },
              { label: t("Recognise text (OCR)…"), icon: ScanText, onSelect: () => setDialog({ kind: "ocr" }) },
              { label: t("Signatures…"), icon: ShieldCheck, onSelect: () => setDialog({ kind: "signatures" }) },
              { label: t("Sign…"), icon: PenLine, onSelect: () => setDialog({ kind: "sign", field: null }) },
              { label: t("Protect with a password…"), icon: Lock, onSelect: () => setDialog({ kind: "protect" }) },
              { label: t("Optimise…"), icon: Zap, onSelect: () => setDialog({ kind: "optimize" }) },
              { label: "PDF/A…", icon: Archive, onSelect: () => setDialog({ kind: "pdfa" }) },
              { label: t("Accessibility…"), icon: Accessibility, onSelect: () => setDialog({ kind: "access" }) },
              { type: "separator" },
              ...docs
                .filter((d) => d.info.state.id !== id && d.pdf)
                .map((d) => ({ label: t("Compare with {0}", d.info.state.name), icon: GitCompareArrows, onSelect: () => void startCompare(d.info.state.id) })),
              { label: t("Compare with a file…"), icon: GitCompareArrows, onSelect: () => void startCompare("file") },
              { type: "separator" },
              { label: t("Export pages or text…"), icon: FileOutput, onSelect: () => setDialog({ kind: "export" }) },
              { label: t("Print…"), icon: Printer, shortcut: `${MOD}+P`, onSelect: () => print() },
            ],
          })
        }
      />

      <main className="workbench" style={{ gridTemplateColumns: `${sidebarOpen && active?.pdf ? "auto " : ""}minmax(0, 1fr)${inspectorOpen && active ? " auto" : ""}` }}>
        {sidebarOpen && active?.pdf && (
          <Sidebar
            tab={sideTab}
            onTab={setSideTab}
            docId={active.info.state.id}
            revision={active.info.state.revision}
            changed={active.info.state.changed}
            pdf={active.pdf}
            page={active.page}
            selected={active.selectedPages}
            onSelectPages={(pages) => updateDoc(active.info.state.id, (d) => ({ ...d, selectedPages: pages }))}
            onNavigate={goTo}
            onReorder={(order) => run(active.info.state.id, () => api.reorder(active.info.state.id, order), t("Pages reordered"))}
            onPageAction={pageAction}
            onMenu={setMenu}
            fonts={active.fonts}
            activeFontKey={activeFontKey}
            selection={selection}
            onSelect={setSelection}
            onDeleteAnnot={(page, index) => run(active.info.state.id, () => api.deleteAnnot(active.info.state.id, page, index), t("Annotation deleted"))}
            history={active.info.state.history}
            future={active.info.state.future}
            labels={labels}
            onSaveOutline={async (items) => {
              await run(active.info.state.id, () => api.setOutline(active.info.state.id, items));
            }}
            onHistoryGo={(steps) => {
              setSelection(null);
              run(active.info.state.id, () => api.historyGo(active.info.state.id, steps));
            }}
          />
        )}

        <section className="panel stage" aria-label={t("Document")}>
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
                shape={shape}
                onMenu={setMenu}
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
                    placeholder={t("Find in document")}
                    value={query}
                    onChange={(e) => setQuery(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") stepHit(e.shiftKey ? -1 : 1);
                      if (e.key === "Escape") setFindOpen(false);
                    }}
                    aria-label={t("Find in document")}
                  />
                  <span className="find-count">
                    {query.trim()
                      ? hits.length
                        ? t("{0} of {1}{2}", hitIndex + 1, hits.length, searching ? "…" : "")
                        : searching
                          ? t("Searching…")
                          : t("No matches")
                      : ""}
                  </span>
                  <IconButton icon={ChevronUp} label={t("Previous match")} shortcut="Shift+Enter" spacing="compact" disabled={!hits.length} onClick={() => stepHit(-1)} />
                  <IconButton icon={ChevronDown} label={t("Next match")} shortcut="Enter" spacing="compact" disabled={!hits.length} onClick={() => stepHit(1)} />
                  <span className="spacer" />
                  <IconButton icon={X} label={t("Close find")} shortcut="Esc" spacing="compact" onClick={() => setFindOpen(false)} />
                </div>
              )}
              <Viewer
                key={active.info.state.id}
                docId={active.info.state.id}
                revision={active.info.state.revision}
                changed={active.info.state.changed}
                order={order}
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
                fields={fields}
                redactions={redactions[active.info.state.id] ?? {}}
                onRedactions={(page, rects) =>
                  setRedactions((all) => ({ ...all, [active.info.state.id]: { ...(all[active.info.state.id] ?? {}), [page]: rects } }))
                }
                toolOptions={toolOptions}
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
                {showOrder && (
                  <button type="button" className="status-item status-button" onClick={() => setShowOrder(false)} data-tooltip={t("Hide the reading order")}>
                    {t("reading order shown")}
                  </button>
                )}
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
            onUpdateAnnot={(c) => selection?.kind === "annot" && actions.updateAnnot(selection.page, selection.annot.index, c)}
            onReplaceImage={replaceImage}
            onExtractImage={extractImage}
            onDeleteImage={deleteImage}
            toolOptions={toolOptions}
            onToolOptions={setToolOptions}
            redactCount={redactCount}
            redactFill={redactFill}
            onRedactFill={setRedactFill}
            onRedactFind={markForRedaction}
            onRedactApply={applyRedactions}
            onRedactClear={() => setRedactions((all) => ({ ...all, [active.info.state.id]: {} }))}
            onUncrop={(scope) => {
              const all = Array.from({ length: active.info.state.pages }, (_, i) => i);
              run(active.info.state.id, () => api.crop(active.info.state.id, scope === "page" ? [active.page] : all, null), t("Crop removed"));
            }}
            onSaveMeta={(m: Meta) => run(active.info.state.id, () => api.setMeta(active.info.state.id, m), t("Properties updated")).then(async () => {
              const info = await api.info(active.info.state.id);
              updateDoc(active.info.state.id, (d) => ({ ...d, info }));
            })}
          />
        )}
      </main>

      {menu && <Menu request={menu} onClose={() => setMenu(null)} />}
      {dialog?.kind === "marks" && active && (
        <PageMarksDialog
          tab={dialog.tab}
          docId={active.info.state.id}
          revision={active.info.state.revision}
          pages={active.info.state.pages}
          current={active.page}
          selected={active.selectedPages}
          fonts={active.fonts}
          catalog={catalog}
          onApply={(pages, mark) => run(active.info.state.id, () => api.pageMarks(active.info.state.id, pages, mark), t("Marks added"))}
          onSaveLabels={(ranges) => run(active.info.state.id, () => api.setLabels(active.info.state.id, ranges), t("Page labels saved"))}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "resize" && active && (
        <ResizeDialog
          pages={active.info.state.pages}
          current={active.page}
          selected={active.selectedPages}
          onApply={(pages, w, h) => run(active.info.state.id, () => api.resizePages(active.info.state.id, pages, w, h), t("Pages resized"))}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "split" && active && (
        <SplitDialog
          docId={active.info.state.id}
          pages={active.info.state.pages}
          selected={active.selectedPages}
          onSplit={async (ranges, folder) => {
            try {
              const files = await api.split(active.info.state.id, ranges, folder);
              toast("success", t("Saved {0} file{1}", files.length, files.length === 1 ? "" : "s"), folder);
              return true;
            } catch (e) {
              toast("error", t("Splitting failed"), errorText(e));
              return false;
            }
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "protect" && active && (
        <ProtectDialog
          info={active.info}
          onClose={() => setDialog(null)}
          onApply={async (req) => {
            try {
              const info = await api.protect(active.info.state.id, req);
              updateDoc(active.info.state.id, (d) => ({ ...d, info }));
              toast("success", req ? t("Protection is written on the next save") : t("Protection is removed on the next save"));
              return true;
            } catch (e) {
              toast("error", t("That didn’t work"), errorText(e));
              return false;
            }
          }}
        />
      )}
      {dialog?.kind === "optimize" && active && (
        <OptimizeDialog
          info={active.info}
          onClose={() => setDialog(null)}
          onRun={async (options) => {
            const docId = active.info.state.id;
            let report = null;
            setSelection(null);
            await run(docId, async () => {
              const r = await api.optimize(docId, options);
              report = r.report;
              return r.state;
            });
            await reloadInfo(docId);
            return report;
          }}
        />
      )}
      {dialog?.kind === "export" && active?.pdf && (
        <ExportDialog
          docId={active.info.state.id}
          pdf={active.pdf}
          name={active.info.state.name}
          pages={active.info.state.pages}
          current={active.page}
          selected={active.selectedPages}
          onDone={toast}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "ocr" && active?.pdf && (
        <OcrDialog
          docId={active.info.state.id}
          pdf={active.pdf}
          pages={active.info.state.pages}
          current={active.page}
          selected={active.selectedPages}
          onApply={(pages, lang) => run(active.info.state.id, () => api.ocrApply(active.info.state.id, pages, lang))}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "pdfa" && active && (
        <PdfaDialog
          docId={active.info.state.id}
          revision={active.info.state.revision}
          claimed={active.info.pdfa}
          onClose={() => setDialog(null)}
          onConvert={async (part) => {
            const docId = active.info.state.id;
            let out = null;
            setSelection(null);
            await run(docId, async () => {
              const r = await api.pdfaConvert(docId, part);
              out = { done: r.done, report: r.report };
              return r.state;
            });
            await reloadInfo(docId);
            return out;
          }}
        />
      )}
      {dialog?.kind === "access" && active && (
        <AccessibilityDialog
          docId={active.info.state.id}
          revision={active.info.state.revision}
          page={active.page}
          run={(fn, success) => run(active.info.state.id, fn, success)}
          orderShown={showOrder}
          onShowOrder={setShowOrder}
          onNavigate={goTo}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "signatures" && active && (
        <SignaturesDialog
          docId={active.info.state.id}
          revision={active.info.state.revision}
          onSign={(field) => setDialog({ kind: "sign", field })}
          onNavigate={goTo}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "sign" && active && (
        <SignDialog
          pages={active.info.state.pages}
          current={active.page}
          pageSize={pageSize}
          field={dialog.field}
          signed={active.info.state.signed}
          hasPath={!!active.info.state.path}
          onSign={signDocument}
          onClose={() => setDialog(null)}
        />
      )}
      {comparing &&
        (() => {
          const a = docs.find((d) => d.info.state.id === comparing.a);
          const b = docs.find((d) => d.info.state.id === comparing.b);
          if (!a?.pdf || !b?.pdf) return null;
          return (
            <CompareView
              a={{ name: a.info.state.name, pdf: a.pdf }}
              b={{ name: b.info.state.name, pdf: b.pdf }}
              result={comparing.result}
              onSwap={() => {
                const swapped = { a: comparing.b, b: comparing.a, result: null };
                setComparing(swapped);
                api
                  .compare(swapped.a, swapped.b)
                  .then((result) => setComparing((c) => (c && c.a === swapped.a && c.b === swapped.b ? { ...c, result } : c)))
                  .catch(() => setComparing(null));
              }}
              onClose={() => setComparing(null)}
            />
          );
        })()}
      {showManual && <ManualDialog onClose={() => setShowManual(false)} />}
      {touring && !showManual && !showSettings && !password && !unsaved && <Tour onClose={() => setTouring(false)} />}
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
          <div className="drop-veil-card">{t("Drop PDFs to open them")}</div>
        </div>
      )}
      <Toasts toasts={toasts} onDismiss={(tid) => setToasts((t) => t.filter((x) => x.id !== tid))} />
      <TooltipLayer />
    </div>
  );
}
