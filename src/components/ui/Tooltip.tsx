import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Shortcut } from "./Feedback";

interface Tip {
  text: string;
  kbd: string | null;
  rect: DOMRect;
}

/**
 * One tooltip for the whole app. Any element with `data-tooltip` gets a
 * tooltip; `data-tooltip-kbd` adds a shortcut hint and
 * `data-tooltip-overflow` only shows it when the text is cut off.
 */
export function TooltipLayer() {
  const [tip, setTip] = useState<Tip | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);

  useEffect(() => {
    let timer: number | undefined;
    let current: HTMLElement | null = null;

    const hide = () => {
      window.clearTimeout(timer);
      current = null;
      setTip(null);
    };

    const show = (el: HTMLElement, delay: number) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        if (!document.contains(el) || current !== el) return;
        const text = el.getAttribute("data-tooltip");
        if (!text) return;
        if (el.hasAttribute("data-tooltip-overflow") && el.scrollWidth <= el.clientWidth) return;
        setTip({ text, kbd: el.getAttribute("data-tooltip-kbd"), rect: el.getBoundingClientRect() });
      }, delay);
    };

    const onOver = (e: PointerEvent) => {
      const el = (e.target as Element | null)?.closest?.("[data-tooltip]") as HTMLElement | null;
      if (el === current) return;
      setTip(null);
      current = el;
      if (el) show(el, 500);
      else window.clearTimeout(timer);
    };

    const onFocus = (e: FocusEvent) => {
      const el = (e.target as Element | null)?.closest?.("[data-tooltip]") as HTMLElement | null;
      if (el && el.matches(":focus-visible")) {
        current = el;
        show(el, 300);
      }
    };

    document.addEventListener("pointerover", onOver);
    document.addEventListener("pointerdown", hide, true);
    document.addEventListener("keydown", hide, true);
    document.addEventListener("scroll", hide, true);
    document.addEventListener("focusin", onFocus);
    document.addEventListener("focusout", hide);
    window.addEventListener("blur", hide);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("pointerover", onOver);
      document.removeEventListener("pointerdown", hide, true);
      document.removeEventListener("keydown", hide, true);
      document.removeEventListener("scroll", hide, true);
      document.removeEventListener("focusin", onFocus);
      document.removeEventListener("focusout", hide);
      window.removeEventListener("blur", hide);
    };
  }, []);

  useLayoutEffect(() => {
    if (!tip || !ref.current) {
      setPos(null);
      return;
    }
    const { width, height } = ref.current.getBoundingClientRect();
    const r = tip.rect;
    let top = r.bottom + 6;
    if (top + height > window.innerHeight - 8) top = r.top - height - 6;
    const left = Math.max(8, Math.min(r.left + r.width / 2 - width / 2, window.innerWidth - width - 8));
    setPos({ left, top });
  }, [tip]);

  if (!tip) return null;
  return createPortal(
    <div
      ref={ref}
      className="tooltip"
      role="tooltip"
      style={pos ? { left: pos.left, top: pos.top } : { left: -9999, top: -9999 }}
    >
      <span>{tip.text}</span>
      {tip.kbd && <Shortcut keys={tip.kbd} />}
    </div>,
    document.body,
  );
}
