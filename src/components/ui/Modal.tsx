import { useEffect, useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { cx } from "../../lib/utils";
import { IconButton } from "./Button";

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface ModalProps {
  title: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: "small" | "medium" | "large";
  /** Leave out the close button and ignore blanket clicks. */
  required?: boolean;
  /** Forms keep their input when someone clicks next to the dialog. */
  closeOnBlanket?: boolean;
  className?: string;
  titleIcon?: ReactNode;
}

export function Modal({
  title,
  onClose,
  children,
  footer,
  width = "medium",
  required,
  closeOnBlanket = true,
  className,
  titleIcon,
}: ModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const dialog = dialogRef.current;
    const initial =
      dialog?.querySelector<HTMLElement>("[data-autofocus]") ??
      dialog?.querySelector<HTMLElement>(".modal-body " + FOCUSABLE) ??
      dialog?.querySelector<HTMLElement>(FOCUSABLE);
    initial?.focus();
    if (initial instanceof HTMLInputElement) initial.select();
    return () => {
      if (previous && document.contains(previous)) previous.focus();
    };
  }, []);

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape" && !required) {
      e.stopPropagation();
      onCloseRef.current();
      return;
    }
    if (e.key !== "Tab") return;
    const items = Array.from(
      dialogRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE) ?? [],
    ).filter((el) => el.offsetParent !== null);
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  return createPortal(
    <div
      className="blanket"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !required && closeOnBlanket) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className={cx("modal", `modal-${width}`, className)}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={onKeyDown}
      >
        <header className="modal-header">
          <h2 id={titleId} className="modal-title">
            {titleIcon}
            {title}
          </h2>
          {!required && (
            <IconButton icon={X} label="Close" onClick={onClose} spacing="compact" />
          )}
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-footer">{footer}</footer>}
      </div>
    </div>,
    document.body,
  );
}
