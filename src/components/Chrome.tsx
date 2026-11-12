import {
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  CircleHelp,
  CircleCheck,
  FileText,
  FolderOpen,
  Hand,
  Highlighter,
  Info,
  Lock,
  Maximize,
  MessageSquarePlus,
  Minus,
  Moon,
  MousePointer2,
  PanelLeft,
  PanelRight,
  PenLine,
  Plus,
  Redo2,
  Save,
  SaveAll,
  Search,
  Settings as SettingsIcon,
  ShieldCheck,
  Square,
  Strikethrough,
  Sun,
  TextCursorInput,
  Type,
  Underline,
  Undo2,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState } from "react";
import {
  Archive,
  ChevronDown,
  Circle,
  Crop,
  EyeOff,
  ImagePlus,
  Link2,
  MoveUpRight,
  Pentagon,
  Slash,
  Stamp,
  TextQuote,
  Wrench,
} from "lucide-react";
import type { MenuRequest } from "./ui/Menu";
import { IconButton } from "./ui/Button";
import { Lozenge } from "./ui/Feedback";
import type { ShapeTool, Tool, ZoomMode } from "../lib/types";
import { cx, MOD } from "../lib/utils";
import { t, te } from "../lib/i18n";

// ---------------------------------------------------------------------------
// Top bar
// ---------------------------------------------------------------------------

export interface HudInfo {
  name: string;
  page: number;
  pages: number;
  version: string;
  dirty: boolean;
  encrypted: boolean;
  signed: boolean;
  /** "AES-256" and the like when the file is (or will be) password protected. */
  protection: string | null;
  protectionPending: boolean;
  /** "PDF/A-2b" when the document claims it. */
  pdfa: string | null;
}

export function TopBar({
  hud,
  theme,
  sidebarOpen,
  inspectorOpen,
  canSave,
  canUndo,
  canRedo,
  onToggleSidebar,
  onToggleInspector,
  onOpen,
  onSave,
  onSaveAs,
  onUndo,
  onRedo,
  onToggleTheme,
  onSettings,
  onTools,
  canTools,
  onSignatures,
  onProtect,
  onPdfa,
  onHelp,
}: {
  hud: HudInfo | null;
  theme: "light" | "dark";
  sidebarOpen: boolean;
  inspectorOpen: boolean;
  canSave: boolean;
  canUndo: boolean;
  canRedo: boolean;
  onToggleSidebar: () => void;
  onToggleInspector: () => void;
  onOpen: () => void;
  onSave: () => void;
  onSaveAs: () => void;
  onUndo: () => void;
  onRedo: () => void;
  onToggleTheme: () => void;
  onSettings: () => void;
  onTools: (anchor: HTMLElement) => void;
  canTools: boolean;
  onSignatures: () => void;
  onProtect: () => void;
  onPdfa: () => void;
  onHelp: (anchor: HTMLElement) => void;
}) {
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="topbar-start">
        <IconButton
          icon={PanelLeft}
          label={sidebarOpen ? t("Hide sidebar") : t("Show sidebar")}
          shortcut={`${MOD}+B`}
          selected={sidebarOpen}
          onClick={onToggleSidebar}
        />
        <span className="toolbar-divider" aria-hidden="true" />
        <span className="topbar-group" data-tour="file">
          <IconButton icon={FolderOpen} label={t("Open PDF")} shortcut={`${MOD}+O`} onClick={onOpen} />
          <IconButton icon={Save} label={t("Save")} shortcut={`${MOD}+S`} disabled={!canSave} onClick={onSave} />
          <IconButton icon={SaveAll} label={t("Save as…")} shortcut={`${MOD}+Shift+S`} disabled={!canSave} onClick={onSaveAs} />
          <span className="toolbar-divider" aria-hidden="true" />
          <IconButton icon={Undo2} label={t("Undo")} shortcut={`${MOD}+Z`} disabled={!canUndo} onClick={onUndo} />
          <IconButton icon={Redo2} label={t("Redo")} shortcut={`${MOD}+Y`} disabled={!canRedo} onClick={onRedo} />
        </span>
      </div>

      <div className="hud" aria-live="polite">
        {hud ? (
          <>
            <FileText size={15} className="hud-icon" aria-hidden="true" />
            <span className="hud-name" data-tooltip={hud.name} data-tooltip-overflow>
              {hud.name}
            </span>
            {hud.dirty && <span className="hud-dirty" aria-label={t("Unsaved changes")} data-tooltip={t("Unsaved changes")} />}
            <span className="hud-sep" aria-hidden="true" />
            <span className="hud-stat">
              {t("page")} <strong>{hud.page + 1}</strong> / {hud.pages}
            </span>
            <span className="hud-sep" aria-hidden="true" />
            <span className="hud-stat">{t("PDF")}{" "}{hud.version}</span>
            {hud.signed && (
              <button type="button" className="hud-chip" onClick={onSignatures} data-tooltip={t("Check the signatures. Saving appends changes, so they stay valid.")}>
                <Lozenge appearance="success" icon={ShieldCheck}>
                  {t("signed")}
                </Lozenge>
              </button>
            )}
            {hud.protection && (
              <button type="button" className="hud-chip" onClick={onProtect} data-tooltip={hud.protectionPending ? t("Password protection is written on the next save") : t("Password protected; saving keeps the protection")}>
                <Lozenge appearance={hud.protectionPending ? "warning" : "information"} icon={Lock}>
                  {hud.protection}
                </Lozenge>
              </button>
            )}
            {hud.pdfa && (
              <button type="button" className="hud-chip" onClick={onPdfa} data-tooltip={t("The document says it conforms to this archival standard")}>
                <Lozenge appearance="discovery" icon={Archive}>
                  {hud.pdfa}
                </Lozenge>
              </button>
            )}
          </>
        ) : (
          <span className="hud-idle">{t("No document open")}</span>
        )}
      </div>

      <div className="topbar-end">
        <IconButton
          icon={theme === "dark" ? Sun : Moon}
          label={theme === "dark" ? t("Light theme") : t("Dark theme")}
          onClick={onToggleTheme}
        />
        <IconButton icon={Wrench} label={t("Document tools")} disabled={!canTools} data-tour="tools" onClick={(e) => onTools(e.currentTarget)} />
        <IconButton icon={CircleHelp} label={t("Help")} shortcut="F1" data-tour="help" onClick={(e) => onHelp(e.currentTarget)} />
        <IconButton icon={SettingsIcon} label={t("Settings")} shortcut={`${MOD}+,`} onClick={onSettings} />
        <IconButton
          icon={PanelRight}
          label={inspectorOpen ? t("Hide inspector") : t("Show inspector")}
          shortcut={`${MOD}+I`}
          selected={inspectorOpen}
          onClick={onToggleInspector}
        />
      </div>
    </header>
  );
}

// ---------------------------------------------------------------------------
// Document tabs
// ---------------------------------------------------------------------------

export function DocTabs({
  docs,
  activeId,
  onSelect,
  onClose,
  onOpen,
}: {
  docs: { id: number; name: string; dirty: boolean }[];
  activeId: number | null;
  onSelect: (id: number) => void;
  onClose: (id: number) => void;
  onOpen: () => void;
}) {
  return (
    <div className="tabstrip" role="tablist" aria-label={t("Open documents")}>
      {docs.map((d) => {
        const active = d.id === activeId;
        return (
          <div
            key={d.id}
            role="tab"
            aria-selected={active}
            tabIndex={active ? 0 : -1}
            className={cx("tab", active && "is-active")}
            onClick={() => onSelect(d.id)}
            onAuxClick={(e) => e.button === 1 && onClose(d.id)}
            onKeyDown={(e) => e.key === "Enter" && onSelect(d.id)}
          >
            <FileText size={14} className="tab-icon" aria-hidden="true" />
            <span className="tab-title" data-tooltip={d.name} data-tooltip-overflow>
              {d.name}
            </span>
            {d.dirty && <span className="tab-dirty" aria-label={t("Unsaved changes")} />}
            <button
              type="button"
              className="tab-close"
              aria-label={t("Close {0}", d.name)}
              onClick={(e) => {
                e.stopPropagation();
                onClose(d.id);
              }}
            >
              <X size={12} aria-hidden="true" />
            </button>
          </div>
        );
      })}
      <IconButton icon={Plus} label={t("Open PDF")} shortcut={`${MOD}+O`} spacing="compact" onClick={onOpen} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Tool bar
// ---------------------------------------------------------------------------

interface ToolDef {
  tool: Tool;
  icon: LucideIcon;
  label: string;
  key: string;
}

const VIEW_TOOLS: ToolDef[] = [
  { tool: "select", icon: MousePointer2, label: t("Select text, fill in forms, move annotations"), key: "V" },
  { tool: "hand", icon: Hand, label: t("Hand: drag to scroll"), key: "H" },
];

const EDIT_TOOLS: ToolDef[] = [
  { tool: "edit", icon: TextCursorInput, label: t("Edit text and images"), key: "E" },
  { tool: "text", icon: Type, label: t("Add text"), key: "T" },
  { tool: "image", icon: ImagePlus, label: t("Insert image"), key: "I" },
];

const MARK_TOOLS: ToolDef[] = [
  { tool: "highlight", icon: Highlighter, label: t("Highlight"), key: "M" },
  { tool: "underline", icon: Underline, label: t("Underline"), key: "U" },
  { tool: "strikeout", icon: Strikethrough, label: t("Strike out"), key: "K" },
  { tool: "note", icon: MessageSquarePlus, label: t("Sticky note"), key: "N" },
  { tool: "ink", icon: PenLine, label: t("Draw"), key: "D" },
];

export const SHAPE_TOOLS: ToolDef[] = [
  { tool: "rect", icon: Square, label: t("Rectangle"), key: "R" },
  { tool: "ellipse", icon: Circle, label: t("Ellipse"), key: "O" },
  { tool: "line", icon: Slash, label: t("Line"), key: "/" },
  { tool: "arrow", icon: MoveUpRight, label: t("Arrow"), key: "A" },
  { tool: "polygon", icon: Pentagon, label: t("Polygon"), key: "P" },
];

const MORE_MARK_TOOLS: ToolDef[] = [
  { tool: "textbox", icon: TextQuote, label: t("Text box"), key: "B" },
  { tool: "stamp", icon: Stamp, label: t("Stamp"), key: "S" },
  { tool: "link", icon: Link2, label: t("Links"), key: "L" },
];

const PROTECT_TOOLS: ToolDef[] = [
  { tool: "redact", icon: EyeOff, label: t("Redact: remove content for good"), key: "X" },
  { tool: "crop", icon: Crop, label: t("Crop pages"), key: "C" },
];

export const TOOL_KEYS: Record<string, Tool> = Object.fromEntries(
  [...VIEW_TOOLS, ...EDIT_TOOLS, ...MARK_TOOLS, ...SHAPE_TOOLS, ...MORE_MARK_TOOLS, ...PROTECT_TOOLS].map((t) => [t.key.toLowerCase(), t.tool]),
);

export const TOOL_HINTS: Record<Tool, string> = {
  select: t("Select and copy text, fill in form fields, and click a note or shape to edit it; drag a selected one to move it."),
  hand: t("Drag to scroll the document."),
  edit: t("Click a line or image to select it (Shift-click or drag a box for more lines). Double-click to rewrite a line or a paragraph. Drag to move; drag an image's corner to resize."),
  text: t("Click on the page where the new text should start."),
  image: t("Click where the image's top-left corner should go, then choose a PNG or JPEG."),
  highlight: t("Drag across text to highlight exactly those characters, or click a line to highlight all of it."),
  underline: t("Drag across text to underline it, or click a line to underline all of it."),
  strikeout: t("Drag across text to strike it out, or click a line to strike out all of it."),
  note: t("Click where the sticky note should go."),
  ink: t("Draw on the page. Each stroke becomes an ink annotation."),
  rect: t("Drag to draw a rectangle."),
  ellipse: t("Drag to draw an ellipse."),
  line: t("Drag to draw a line."),
  arrow: t("Drag from the tail to the tip of the arrow."),
  polygon: t("Click to place corners; double-click or press Enter to finish."),
  textbox: t("Drag out a box, then type its text."),
  stamp: t("Click to place the stamp chosen in the inspector."),
  link: t("Drag over the area that should be a link, then enter a web address or page. Click an existing link to edit it."),
  redact: t("Drag over everything that must go. Marked areas are removed from the file when you apply them in the inspector."),
  crop: t("Drag the area to keep, then choose which pages to crop."),
};

function ToolGroup({ tools, value, onChange }: { tools: ToolDef[]; value: Tool; onChange: (t: Tool) => void }) {
  return (
    <div className="toolgroup" role="group">
      {tools.map((t) => (
        <IconButton
          key={t.tool}
          icon={t.icon}
          label={t.label}
          shortcut={t.key}
          spacing="compact"
          selected={value === t.tool}
          onClick={() => onChange(t.tool)}
        />
      ))}
    </div>
  );
}

/** One button for the last shape used, with a menu of the others. */
function ShapeButton({ tool, shape, onTool, onMenu }: { tool: Tool; shape: ShapeTool; onTool: (t: Tool) => void; onMenu: (m: MenuRequest) => void }) {
  const def = SHAPE_TOOLS.find((s) => s.tool === shape) ?? SHAPE_TOOLS[0];
  const active = SHAPE_TOOLS.some((s) => s.tool === tool);
  return (
    <span className="shape-button">
      <IconButton icon={def.icon} label={def.label} shortcut={def.key} spacing="compact" selected={active} onClick={() => onTool(def.tool)} />
      <button
        type="button"
        className="shape-more"
        aria-label={t("More shapes")}
        data-tooltip={t("More shapes")}
        onClick={(e) =>
          onMenu({
            anchor: e.currentTarget,
            items: SHAPE_TOOLS.map((s) => ({ label: s.label, icon: s.icon, shortcut: s.key, checked: s.tool === tool, onSelect: () => onTool(s.tool) })),
          })
        }
      >
        <ChevronDown size={12} aria-hidden="true" />
      </button>
    </span>
  );
}

export function Toolbar({
  tool,
  onTool,
  zoom,
  scale,
  onZoom,
  page,
  pages,
  onPage,
  onFind,
  findOpen,
  shape,
  onMenu,
}: {
  shape: ShapeTool;
  onMenu: (m: MenuRequest) => void;
  tool: Tool;
  onTool: (t: Tool) => void;
  zoom: ZoomMode;
  scale: number;
  onZoom: (z: ZoomMode) => void;
  page: number;
  pages: number;
  onPage: (p: number) => void;
  onFind: () => void;
  findOpen: boolean;
}) {
  const [draft, setDraft] = useState(String(page + 1));
  useEffect(() => setDraft(String(page + 1)), [page]);
  const step = (dir: 1 | -1) => {
    const steps = [0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5];
    const next =
      dir > 0 ? steps.find((s) => s > scale + 0.001) ?? 5 : [...steps].reverse().find((s) => s < scale - 0.001) ?? 0.25;
    onZoom(next);
  };
  return (
    <div className="toolbar" role="toolbar" aria-label={t("Tools")}>
      <ToolGroup tools={VIEW_TOOLS} value={tool} onChange={onTool} />
      <ToolGroup tools={EDIT_TOOLS} value={tool} onChange={onTool} />
      <div className="toolgroup" role="group">
        {MARK_TOOLS.map((t) => (
          <IconButton key={t.tool} icon={t.icon} label={t.label} shortcut={t.key} spacing="compact" selected={tool === t.tool} onClick={() => onTool(t.tool)} />
        ))}
        <ShapeButton tool={tool} shape={shape} onTool={onTool} onMenu={onMenu} />
        {MORE_MARK_TOOLS.map((t) => (
          <IconButton key={t.tool} icon={t.icon} label={t.label} shortcut={t.key} spacing="compact" selected={tool === t.tool} onClick={() => onTool(t.tool)} />
        ))}
      </div>
      <ToolGroup tools={PROTECT_TOOLS} value={tool} onChange={onTool} />
      <span className="spacer" />
      <div className="page-nav">
        <IconButton icon={ChevronLeft} label={t("Previous page")} shortcut="PgUp" spacing="compact" disabled={page <= 0} onClick={() => onPage(page - 1)} />
        <input
          className="page-input"
          aria-label={t("Page number")}
          value={draft}
          onChange={(e) => setDraft(e.target.value.replace(/\D/g, ""))}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              const n = parseInt(draft, 10);
              if (n >= 1 && n <= pages) onPage(n - 1);
              else setDraft(String(page + 1));
              (e.target as HTMLInputElement).blur();
            }
            if (e.key === "Escape") {
              setDraft(String(page + 1));
              (e.target as HTMLInputElement).blur();
            }
          }}
          onBlur={() => setDraft(String(page + 1))}
        />
        <span className="is-mono">/ {pages}</span>
        <IconButton icon={ChevronRight} label={t("Next page")} shortcut="PgDn" spacing="compact" disabled={page >= pages - 1} onClick={() => onPage(page + 1)} />
      </div>
      <span className="toolbar-divider" aria-hidden="true" />
      <IconButton icon={Minus} label={t("Zoom out")} shortcut={`${MOD}+-`} spacing="compact" onClick={() => step(-1)} />
      <button
        type="button"
        className="zoom-value"
        data-tooltip={zoom === "fit-width" ? t("Fit to page") : t("Fit to width")}
        data-tooltip-kbd={`${MOD}+0`}
        onClick={() => onZoom(zoom === "fit-width" ? "fit-page" : "fit-width")}
      >
        {Math.round(scale * 100)}%
      </button>
      <IconButton icon={Plus} label={t("Zoom in")} shortcut={`${MOD}+=`} spacing="compact" onClick={() => step(1)} />
      <IconButton
        icon={Maximize}
        label={zoom === "fit-page" ? t("Fit to width") : t("Fit whole page")}
        spacing="compact"
        selected={typeof zoom !== "number"}
        onClick={() => onZoom(zoom === "fit-page" ? "fit-width" : "fit-page")}
      />
      <span className="toolbar-divider" aria-hidden="true" />
      <IconButton icon={Search} label={t("Find")} shortcut={`${MOD}+F`} spacing="compact" selected={findOpen} onClick={onFind} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Toasts
// ---------------------------------------------------------------------------

export interface Toast {
  id: number;
  kind: "success" | "error" | "warning" | "info";
  title: string;
  body?: string;
}

const TOAST_ICONS: Record<Toast["kind"], LucideIcon> = {
  success: CircleCheck,
  error: CircleAlert,
  warning: CircleAlert,
  info: Info,
};

export function Toasts({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: number) => void }) {
  return (
    <div className="toast-stack" aria-live="polite">
      {toasts.map((toast) => {
        const Icon = TOAST_ICONS[toast.kind];
        return (
          <div key={toast.id} className={cx("toast", `toast-${toast.kind}`)} role={toast.kind === "error" ? "alert" : "status"}>
            <Icon size={16} className="toast-icon" aria-hidden="true" />
            <div className="toast-content">
              <p className="toast-title">{te(toast.title)}</p>
              {toast.body && <p className="toast-description">{te(toast.body)}</p>}
            </div>
            <IconButton icon={X} label={t("Dismiss")} spacing="compact" onClick={() => onDismiss(toast.id)} />
          </div>
        );
      })}
    </div>
  );
}
