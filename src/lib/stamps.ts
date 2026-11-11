import { useRef } from "react";

/**
 * For every page, the revision at which its picture last changed. A page
 * view re-renders when its stamp moves, so an edit repaints only the pages
 * the engine says it touched.
 */
export function usePageStamps(revision: number, changed: number[] | null, count: number): number[] {
  const ref = useRef<{ revision: number; stamps: number[] }>({ revision: -1, stamps: [] });
  const cur = ref.current;
  if (cur.revision !== revision || cur.stamps.length !== count) {
    const all = changed == null || cur.revision < 0 || cur.stamps.length !== count;
    const stamps = all ? Array.from({ length: count }, () => revision) : [...cur.stamps];
    if (!all) for (const p of changed!) if (p < count) stamps[p] = revision;
    ref.current = { revision, stamps };
  }
  return ref.current.stamps;
}
