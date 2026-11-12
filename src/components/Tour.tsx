import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "./ui/Button";
import { t } from "../lib/i18n";

/*
 * The tour: a handful of cards, each pointing at one part of the window.
 * A step whose part is not on screen (the tool bar without a document)
 * shows its card in the middle instead.
 */

interface Step {
  /** CSS selector of the part the card points at; none for a card in the middle. */
  target?: string;
  title: string;
  body: string;
}

const STEPS: Step[] = [
  {
    title: t("Welcome to Inq Studio"),
    body: t("Eight short steps show where everything is. You can leave at any time with Esc and start the tour again from the Help button."),
  },
  {
    target: '[data-tour="file"]',
    title: t("Open and save"),
    body: t("Open a PDF here, or drop files anywhere on the window. Nothing is written to the file until you save, and every change can be undone until you close the document."),
  },
  {
    target: ".hud",
    title: t("The document at a glance"),
    body: t("Name, page and PDF version. Badges appear when the document is signed, password protected or claims PDF/A; click one for the details."),
  },
  {
    target: ".toolbar",
    title: t("The tools"),
    body: t("Select and read, edit text in the document's own fonts, add text and images, mark up, draw shapes, redact and crop. Each tool has a one-letter shortcut, shown when you rest the pointer on it. Page navigation, zoom and find are on the right."),
  },
  {
    target: ".side:not(.is-inspector)",
    title: t("The sidebar"),
    body: t("Page thumbnails to reorder, rotate, insert and delete pages; bookmarks; the fonts of the document; comments; and the history of your changes, where you can jump back to any step."),
  },
  {
    target: ".side.is-inspector",
    title: t("The inspector"),
    body: t("Shows what is selected and the options of the active tool: font, size and colour of text, the look of a shape, the document's properties."),
  },
  {
    target: '[data-tour="tools"]',
    title: t("Document tools"),
    body: t("Everything that works on the whole document: recognise text in scans (OCR), sign and check signatures, protect with a password, optimise, PDF/A, compare, watermarks, export and print."),
  },
  {
    target: '[data-tour="help"]',
    title: t("Help is here"),
    body: t("The manual explains every tool and opens with F1. This tour can be started again from the same menu. Settings are next to it."),
  },
];

const SEEN_KEY = "inq.tour";

export function tourSeen(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) === "done";
  } catch {
    return true;
  }
}

function rememberTour() {
  try {
    localStorage.setItem(SEEN_KEY, "done");
  } catch {
    /* shown again next time */
  }
}

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

const CARD_WIDTH = 352;
const GAP = 14;

export function Tour({ onClose }: { onClose: () => void }) {
  const [index, setIndex] = useState(0);
  const [box, setBox] = useState<Box | null>(null);
  const [cardHeight, setCardHeight] = useState(200);
  const card = useRef<HTMLDivElement>(null);
  const step = STEPS[index];
  const last = index === STEPS.length - 1;

  const finish = useCallback(() => {
    rememberTour();
    onClose();
  }, [onClose]);

  const measure = useCallback(() => {
    const el = step.target ? document.querySelector<HTMLElement>(step.target) : null;
    const r = el?.getBoundingClientRect();
    setBox(r && r.width > 0 && r.height > 0 ? { x: r.left - 6, y: r.top - 6, w: r.width + 12, h: r.height + 12 } : null);
  }, [step]);

  useLayoutEffect(() => {
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [measure]);

  useLayoutEffect(() => {
    if (card.current) setCardHeight(card.current.offsetHeight);
    card.current?.querySelector<HTMLElement>("[data-autofocus]")?.focus();
  }, [index, box]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") finish();
      else if (e.key === "ArrowRight") setIndex((i) => Math.min(STEPS.length - 1, i + 1));
      else if (e.key === "ArrowLeft") setIndex((i) => Math.max(0, i - 1));
      else if (e.key !== "Enter" && e.key !== " " && e.key !== "Tab") return;
      else return;
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [finish]);

  // The card goes below the part, or above it, or beside it: wherever there is room.
  let style: React.CSSProperties;
  if (!box) {
    style = {};
  } else {
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const clampX = (x: number) => Math.max(12, Math.min(vw - CARD_WIDTH - 12, x));
    const clampY = (y: number) => Math.max(12, Math.min(vh - cardHeight - 12, y));
    if (box.y + box.h + GAP + cardHeight <= vh - 12) style = { left: clampX(box.x), top: box.y + box.h + GAP };
    else if (box.y - GAP - cardHeight >= 12) style = { left: clampX(box.x), top: box.y - GAP - cardHeight };
    else if (box.x + box.w + GAP + CARD_WIDTH <= vw - 12) style = { left: box.x + box.w + GAP, top: clampY(box.y) };
    else style = { left: Math.max(12, box.x - GAP - CARD_WIDTH), top: clampY(box.y) };
  }

  return createPortal(
    <div className="tour" role="dialog" aria-modal="true" aria-label={t("Tour")}>
      {box ? <div className="tour-spot" style={{ left: box.x, top: box.y, width: box.w, height: box.h }} /> : <div className="tour-dim" />}
      <div ref={card} className={box ? "tour-card" : "tour-card is-centred"} style={{ ...style, width: CARD_WIDTH }}>
        <p className="eyebrow">{t("Step {0} of {1}", index + 1, STEPS.length)}</p>
        <h2 className="tour-title">{step.title}</h2>
        <p className="tour-body">{step.body}</p>
        {step.target && !box && <p className="tour-note">{t("This part appears once a document is open.")}</p>}
        <div className="tour-foot">
          <div className="tour-dots" aria-hidden="true">
            {STEPS.map((_, i) => (
              <span key={i} className={i === index ? "tour-dot is-current" : "tour-dot"} />
            ))}
          </div>
          {!last && (
            <Button appearance="subtle" spacing="compact" onClick={finish}>
              {t("Skip")}
            </Button>
          )}
          {index > 0 && (
            <Button spacing="compact" onClick={() => setIndex(index - 1)}>
              {t("Back")}
            </Button>
          )}
          <Button appearance="primary" spacing="compact" data-autofocus onClick={() => (last ? finish() : setIndex(index + 1))}>
            {last ? t("Done") : t("Next")}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
