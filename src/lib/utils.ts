export function cx(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}

const collator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

/** Natural, case-insensitive comparison: "file2" sorts before "file10". */
export function compareNames(a: string, b: string): number {
  return collator.compare(a, b);
}

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT"
  );
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export const IS_MAC =
  typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

/** Label for the primary modifier key in shortcut hints. */
export const MOD = IS_MAC ? "⌘" : "Ctrl";
