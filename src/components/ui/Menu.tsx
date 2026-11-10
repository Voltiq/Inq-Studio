import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, type LucideIcon } from "lucide-react";
import { cx } from "../../lib/utils";

export type MenuItem =
  | {
      type?: "item";
      label: string;
      icon?: LucideIcon;
      shortcut?: string;
      danger?: boolean;
      disabled?: boolean;
      checked?: boolean;
      onSelect: () => void;
    }
  | { type: "separator" }
  | { type: "heading"; label: string };

export interface MenuRequest {
  items: MenuItem[];
  /** Pointer position for context menus. */
  x?: number;
  y?: number;
  /** Element to attach a dropdown to. */
  anchor?: HTMLElement;
  align?: "start" | "end";
}

type ActionItem = Extract<MenuItem, { onSelect: () => void }>;

function isAction(item: MenuItem): item is ActionItem {
  return (item.type === undefined || item.type === "item") && !("disabled" in item && item.disabled);
}

export function Menu({ request, onClose }: { request: MenuRequest; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const [active, setActive] = useState(() => request.items.findIndex(isAction));
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left: number;
    let top: number;
    if (request.anchor) {
      const r = request.anchor.getBoundingClientRect();
      left = request.align === "end" ? r.right - width : r.left;
      top = r.bottom + 4;
      if (top + height > vh - 8) top = r.top - height - 4;
    } else {
      left = request.x ?? 0;
      top = request.y ?? 0;
      if (left + width > vw - 8) left = left - width;
      if (top + height > vh - 8) top = Math.max(8, vh - height - 8);
    }
    setPos({
      left: Math.max(8, Math.min(left, vw - width - 8)),
      top: Math.max(8, top),
    });
    el.focus();
  }, [request]);

  useEffect(() => {
    const onDown = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) onCloseRef.current();
    };
    const onBlur = () => onCloseRef.current();
    const onScroll = (e: Event) => {
      if (!ref.current?.contains(e.target as Node)) onCloseRef.current();
    };
    document.addEventListener("pointerdown", onDown, true);
    window.addEventListener("blur", onBlur);
    window.addEventListener("resize", onBlur);
    document.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("resize", onBlur);
      document.removeEventListener("scroll", onScroll, true);
    };
  }, []);

  function move(delta: number) {
    const items = request.items;
    let i = active;
    for (let step = 0; step < items.length; step++) {
      i = (i + delta + items.length) % items.length;
      if (isAction(items[i])) {
        setActive(i);
        return;
      }
    }
  }

  function activate(i: number) {
    const item = request.items[i];
    if (!item || !isAction(item)) return;
    onClose();
    // Let the menu unmount before the action opens dialogs or moves focus.
    requestAnimationFrame(() => item.onSelect());
  }

  function onKeyDown(e: React.KeyboardEvent) {
    e.stopPropagation();
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "Home":
        e.preventDefault();
        setActive(request.items.findIndex(isAction));
        break;
      case "End": {
        e.preventDefault();
        const reversed = [...request.items].reverse().findIndex(isAction);
        if (reversed >= 0) setActive(request.items.length - 1 - reversed);
        break;
      }
      case "Enter":
      case " ":
        e.preventDefault();
        activate(active);
        break;
      case "Escape":
      case "Tab":
        e.preventDefault();
        onClose();
        break;
      default:
        if (e.key.length === 1) {
          const letter = e.key.toLowerCase();
          const start = active + 1;
          const items = request.items;
          for (let step = 0; step < items.length; step++) {
            const i = (start + step) % items.length;
            const it = items[i];
            if (isAction(it) && it.label.toLowerCase().startsWith(letter)) {
              setActive(i);
              break;
            }
          }
        }
    }
  }

  return createPortal(
    <div
      ref={ref}
      className="menu"
      role="menu"
      tabIndex={-1}
      style={pos ? { left: pos.left, top: pos.top } : { left: -9999, top: -9999 }}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      {request.items.map((item, i) => {
        if (item.type === "separator") return <div key={i} className="menu-separator" role="separator" />;
        if (item.type === "heading")
          return (
            <div key={i} className="menu-heading" role="presentation">
              {item.label}
            </div>
          );
        const Icon = item.icon;
        return (
          <div
            key={i}
            role={item.checked === undefined ? "menuitem" : "menuitemcheckbox"}
            aria-checked={item.checked}
            aria-disabled={item.disabled || undefined}
            className={cx(
              "menu-item",
              item.danger && "is-danger",
              item.disabled && "is-disabled",
              i === active && "is-active",
            )}
            onPointerMove={() => !item.disabled && setActive(i)}
            onClick={() => activate(i)}
          >
            <span className="menu-item-icon">
              {item.checked !== undefined ? (
                item.checked && <Check size={16} aria-hidden="true" />
              ) : (
                Icon && <Icon size={16} aria-hidden="true" />
              )}
            </span>
            <span className="menu-item-label">{item.label}</span>
            {item.shortcut && <span className="menu-item-shortcut">{item.shortcut}</span>}
          </div>
        );
      })}
    </div>,
    document.body,
  );
}
