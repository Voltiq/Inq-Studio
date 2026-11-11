import {
  ChevronLeft,
  ChevronRight,
  CircleAlert,
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
  onUndo,
  onRedo,
  onToggleTheme,
  onSettings,
  onTools,
  canTools,
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
  onUndo: () => void;
  onRedo: () => void;
  onToggleTheme: () => void;
  onSettings: () => void;
  onTools: (anchor: HTMLElement) => void;
  canTools: boolean;
}) {
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="topbar-start">
        <IconButton
          icon={PanelLeft}
          label={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
          shortcut={`${MOD}+B`}
          selected={sidebarOpen}
          onClick={onToggleSidebar}
        />
        <div className="brand">
          <img className="brand-glyph" src="/app-icon.svg" alt="" />
          <span className="brand-name">
            Inq <span className="brand-studio">Studio</span>
          </span>
        </div>
        <IconButton icon={FolderOpen} label="Open PDF" shortcut={`${MOD}+O`} onClick={onOpen} />
        <IconButton icon={Save} label="Save" shortcut={`${MOD}+S`} disabled={!canSave} onClick={onSave} />
        <span className="toolbar-divider" aria-hidden="true" />
        <IconButton icon={Undo2} label="Undo" shortcut={`${MOD}+Z`} disabled={!canUndo} onClick={onUndo} />
        <IconButton icon={Redo2} label="Redo" shortcut={`${MOD}+Y`} disabled={!canRedo} onClick={onRedo} />
      </div>

      <div className="hud" aria-live="polite">
        {hud ? (
          <>
            <FileText size={15} className="hud-icon" aria-hidden="true" />
            <span className="hud-name" data-tooltip={hud.name} data-tooltip-overflow>
              {hud.name}
            </span>
            {hud.dirty && <span className="hud-dirty" aria-label="Unsaved changes" data-tooltip="Unsaved changes" />}
            <span className="hud-sep" aria-hidden="true" />
            <span className="hud-stat">
              page <strong>{hud.page + 1}</strong> / {hud.pages}
            </span>
            <span className="hud-sep" aria-hidden="true" />
            <span className="hud-stat">PDF {hud.version}</span>
            {hud.signed && (
              <Lozenge appearance="success" icon={ShieldCheck} title="Saving appends changes, so existing signatures stay valid">
                signed
              </Lozenge>
            )}
            {hud.encrypted && (
              <Lozenge appearance="warning" icon={Lock} title="Saved copies are written without the password">
                was protected
              </Lozenge>
            )}
          </>
        ) : (
          <span className="hud-idle">No document open</span>
        )}
      </div>

      <div className="topbar-end">
        <IconButton
          icon={theme === "dark" ? Sun : Moon}
          label={theme === "dark" ? "Light theme" : "Dark theme"}
          onClick={onToggleTheme}
        />
        <IconButton icon={Wrench} label="Document tools" disabled={!canTools} onClick={(e) => onTools(e.currentTarget)} />
        <IconButton icon={SettingsIcon} label="Settings" shortcut={`${MOD}+,`} onClick={onSettings} />
        <IconButton
          icon={PanelRight}
          label={inspectorOpen ? "Hide inspector" : "Show inspector"}
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
    <div className="tabstrip" role="tablist" aria-label="Open documents">
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
            {d.dirty && <span className="tab-dirty" aria-label="Unsaved changes" />}
            <button
              type="button"
              className="tab-close"
              aria-label={`Close ${d.name}`}
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
      <IconButton icon={Plus} label="Open PDF" shortcut={`${MOD}+O`} spacing="compact" onClick={onOpen} />
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
  { tool: "select", icon: MousePointer2, label: "Select text, fill in forms, move annotations", key: "V" },
  { tool: "hand", icon: Hand, label: "Hand: drag to scroll", key: "H" },
];

const EDIT_TOOLS: ToolDef[] = [
  { tool: "edit", icon: TextCursorInput, label: "Edit text and images", key: "E" },
  { tool: "text", icon: Type, label: "Add text", key: "T" },
  { tool: "image", icon: ImagePlus, label: "Insert image", key: "I" },
];

const MARK_TOOLS: ToolDef[] = [
  { tool: "highlight", icon: Highlighter, label: "Highlight", key: "M" },
  { tool: "underline", icon: Underline, label: "Underline", key: "U" },
  { tool: "strikeout", icon: Strikethrough, label: "Strike out", key: "K" },
  { tool: "note", icon: MessageSquarePlus, label: "Sticky note", key: "N" },
  { tool: "ink", icon: PenLine, label: "Draw", key: "D" },
];

export const SHAPE_TOOLS: ToolDef[] = [
  { tool: "rect", icon: Square, label: "Rectangle", key: "R" },
  { tool: "ellipse", icon: Circle, label: "Ellipse", key: "O" },
  { tool: "line", icon: Slash, label: "Line", key: "/" },
  { tool: "arrow", icon: MoveUpRight, label: "Arrow", key: "A" },
  { tool: "polygon", icon: Pentagon, label: "Polygon", key: "P" },
];

const MORE_MARK_TOOLS: ToolDef[] = [
  { tool: "textbox", icon: TextQuote, label: "Text box", key: "B" },
  { tool: "stamp", icon: Stamp, label: "Stamp", key: "S" },
  { tool: "link", icon: Link2, label: "Links", key: "L" },
];

const PROTECT_TOOLS: ToolDef[] = [
  { tool: "redact", icon: EyeOff, label: "Redact: remove content for good", key: "X" },
  { tool: "crop", icon: Crop, label: "Crop pages", key: "C" },
];

export const TOOL_KEYS: Record<string, Tool> = Object.fromEntries(
  [...VIEW_TOOLS, ...EDIT_TOOLS, ...MARK_TOOLS, ...SHAPE_TOOLS, ...MORE_MARK_TOOLS, ...PROTECT_TOOLS].map((t) => [t.key.toLowerCase(), t.tool]),
);

export const TOOL_HINTS: Record<Tool, string> = {
  select: "Select and copy text, fill in form fields, and click a note or shape to edit it; drag a selected one to move it.",
  hand: "Drag to scroll the document.",
  edit: "Click a line or image to select it (Shift-click or drag a box for more lines). Double-click to rewrite a line or a paragraph. Drag to move; drag an image's corner to resize.",
  text: "Click on the page where the new text should start.",
  image: "Click where the image's top-left corner should go, then choose a PNG or JPEG.",
  highlight: "Drag across text to highlight exactly those characters, or click a line to highlight all of it.",
  underline: "Drag across text to underline it, or click a line to underline all of it.",
  strikeout: "Drag across text to strike it out, or click a line to strike out all of it.",
  note: "Click where the sticky note should go.",
  ink: "Draw on the page. Each stroke becomes an ink annotation.",
  rect: "Drag to draw a rectangle.",
  ellipse: "Drag to draw an ellipse.",
  line: "Drag to draw a line.",
  arrow: "Drag from the tail to the tip of the arrow.",
  polygon: "Click to place corners; double-click or press Enter to finish.",
  textbox: "Drag out a box, then type its text.",
  stamp: "Click to place the stamp chosen in the inspector.",
  link: "Drag over the area that should be a link, then enter a web address or page. Click an existing link to edit it.",
  redact: "Drag over everything that must go. Marked areas are removed from the file when you apply them in the inspector.",
  crop: "Drag the area to keep, then choose which pages to crop.",
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
        aria-label="More shapes"
        data-tooltip="More shapes"
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
    <div className="toolbar" role="toolbar" aria-label="Tools">
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
        <IconButton icon={ChevronLeft} label="Previous page" shortcut="PgUp" spacing="compact" disabled={page <= 0} onClick={() => onPage(page - 1)} />
        <input
          className="page-input"
          aria-label="Page number"
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
        <IconButton icon={ChevronRight} label="Next page" shortcut="PgDn" spacing="compact" disabled={page >= pages - 1} onClick={() => onPage(page + 1)} />
      </div>
      <span className="toolbar-divider" aria-hidden="true" />
      <IconButton icon={Minus} label="Zoom out" shortcut={`${MOD}+-`} spacing="compact" onClick={() => step(-1)} />
      <button
        type="button"
        className="zoom-value"
        data-tooltip={zoom === "fit-width" ? "Fit to page" : "Fit to width"}
        data-tooltip-kbd={`${MOD}+0`}
        onClick={() => onZoom(zoom === "fit-width" ? "fit-page" : "fit-width")}
      >
        {Math.round(scale * 100)}%
      </button>
      <IconButton icon={Plus} label="Zoom in" shortcut={`${MOD}+=`} spacing="compact" onClick={() => step(1)} />
      <IconButton
        icon={Maximize}
        label={zoom === "fit-page" ? "Fit to width" : "Fit whole page"}
        spacing="compact"
        selected={typeof zoom !== "number"}
        onClick={() => onZoom(zoom === "fit-page" ? "fit-width" : "fit-page")}
      />
      <span className="toolbar-divider" aria-hidden="true" />
      <IconButton icon={Search} label="Find" shortcut={`${MOD}+F`} spacing="compact" selected={findOpen} onClick={onFind} />
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
      {toasts.map((t) => {
        const Icon = TOAST_ICONS[t.kind];
        return (
          <div key={t.id} className={cx("toast", `toast-${t.kind}`)} role={t.kind === "error" ? "alert" : "status"}>
            <Icon size={16} className="toast-icon" aria-hidden="true" />
            <div className="toast-content">
              <p className="toast-title">{t.title}</p>
              {t.body && <p className="toast-description">{t.body}</p>}
            </div>
            <IconButton icon={X} label="Dismiss" spacing="compact" onClick={() => onDismiss(t.id)} />
          </div>
        );
      })}
    </div>
  );
}
