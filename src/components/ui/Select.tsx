import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";
import { cx } from "../../lib/utils";
import { t } from "../../lib/i18n";

interface Placement {
  left: number;
  top: number;
  width: number;
  maxHeight: number;
  up: boolean;
}

/**
 * A dropdown in the Graphiq look: a button that shows the chosen option and
 * a list that opens under it (or above, when there is no room below).
 * Keyboard: arrows, Home/End, Enter or Space to choose, Esc to close, and
 * typing a few letters jumps to the option that starts with them.
 */
export function Select<T extends string>({
  label,
  value,
  options,
  onChange,
  helper,
  disabled,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  helper?: ReactNode;
  disabled?: boolean;
}) {
  const id = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [place, setPlace] = useState<Placement | null>(null);
  const typed = useRef({ text: "", at: 0 });
  const current = options.findIndex((o) => o.value === value);

  const measure = useCallback(() => {
    const t = triggerRef.current;
    if (!t) return;
    const r = t.getBoundingClientRect();
    const gap = 6;
    const below = window.innerHeight - r.bottom - gap - 8;
    const above = r.top - gap - 8;
    const wanted = Math.min(options.length * 34 + 14, 320);
    const up = below < Math.min(wanted, 160) && above > below;
    setPlace({ left: r.left, top: up ? r.top - gap : r.bottom + gap, width: r.width, maxHeight: Math.max(96, Math.min(320, up ? above : below)), up });
  }, [options.length]);

  const show = useCallback(() => {
    if (disabled) return;
    setActive(Math.max(0, current));
    measure();
    setOpen(true);
  }, [current, disabled, measure]);

  const choose = (i: number) => {
    const o = options[i];
    setOpen(false);
    if (o && o.value !== value) onChange(o.value);
  };

  // Close when something outside is pressed, and follow the trigger while the page moves.
  useEffect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      const n = e.target as Node;
      if (!triggerRef.current?.contains(n) && !listRef.current?.contains(n)) setOpen(false);
    };
    const moved = (e: Event) => {
      if (e.type === "scroll" && listRef.current?.contains(e.target as Node)) return;
      measure();
    };
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("resize", moved);
    window.addEventListener("scroll", moved, true);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("resize", moved);
      window.removeEventListener("scroll", moved, true);
    };
  }, [open, measure]);

  useLayoutEffect(() => {
    if (!open) return;
    listRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const n = options.length;
    if (!n) return;
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    // While the list is open the keys belong to it, not to a dialog around it.
    if (e.key === "Escape" || e.key === "Tab") {
      if (e.key === "Escape") e.stopPropagation();
      setOpen(false);
      return;
    }
    const move = (to: number) => {
      e.preventDefault();
      setActive(Math.max(0, Math.min(n - 1, to)));
    };
    if (e.key === "ArrowDown") return move(active + 1);
    if (e.key === "ArrowUp") return move(active - 1);
    if (e.key === "Home" || e.key === "PageUp") return move(0);
    if (e.key === "End" || e.key === "PageDown") return move(n - 1);
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      choose(active);
      return;
    }
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const now = Date.now();
      typed.current = { text: (now - typed.current.at > 700 ? "" : typed.current.text) + e.key.toLowerCase(), at: now };
      const hit = options.findIndex((o) => o.label.toLowerCase().startsWith(typed.current.text));
      if (hit >= 0) setActive(hit);
    }
  };

  return (
    <div className="field">
      <label className="field-label" id={`${id}-label`} htmlFor={id}>
        {label}
      </label>
      <button
        ref={triggerRef}
        id={id}
        type="button"
        className={cx("select-trigger", open && "is-open")}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={t("{0}-list", id)}
        aria-labelledby={t("{0}-label {1}", id, id)}
        aria-activedescendant={open ? `${id}-o${active}` : undefined}
        disabled={disabled}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={onKeyDown}
      >
        <span className="select-value">{options[current]?.label ?? ""}</span>
        <ChevronDown size={16} className="select-chevron" aria-hidden="true" />
      </button>
      {open &&
        place &&
        createPortal(
          <div
            ref={listRef}
            id={`${id}-list`}
            role="listbox"
            aria-labelledby={t("{0}-label", id)}
            className={cx("menu", "select-list", place.up && "is-up")}
            style={{ left: place.left, minWidth: place.width, maxHeight: place.maxHeight, ...(place.up ? { bottom: window.innerHeight - place.top } : { top: place.top }) }}
            // Keep the focus on the trigger, so the keyboard keeps working.
            onMouseDown={(e) => e.preventDefault()}
          >
            {options.map((o, i) => (
              <div
                key={o.value}
                id={`${id}-o${i}`}
                data-index={i}
                role="option"
                aria-selected={i === current}
                className={cx("menu-item", "select-option", i === active && "is-active", i === current && "is-chosen")}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(i)}
              >
                <span className="menu-item-icon">{i === current && <Check size={14} aria-hidden="true" />}</span>
                <span className="menu-item-label">{o.label}</span>
              </div>
            ))}
          </div>,
          document.body,
        )}
      {helper && <p className="field-message">{helper}</p>}
    </div>
  );
}
