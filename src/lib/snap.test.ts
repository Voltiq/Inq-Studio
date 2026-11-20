import { describe, expect, it } from "vitest";
import { snapMove, snapResize, type SnapBox } from "./snap";

const box = (x0: number, y0: number, x1: number, y1: number, base?: number): SnapBox => ({ x0, y0, x1, y1, base });

describe("snapping a dragged box", () => {
  const targets = [box(100, 100, 300, 120, 116), box(100, 200, 200, 240)];

  it("snaps left edges and baselines, and draws guides along the aligned elements", () => {
    // A line 4 px right of the left edge and 3 px below the first baseline.
    const s = snapMove(box(0, 0, 80, 20, 16), 104, 103, targets);
    expect([s.dx, s.dy]).toEqual([100, 100]);
    const gx = s.guides.find((g) => g.axis === "x")!;
    expect(gx.at).toBe(100);
    // The vertical guide runs from the dragged line down past both targets.
    expect([gx.from, gx.to]).toEqual([100, 240]);
    expect(s.guides.find((g) => g.axis === "y")!.at).toBe(116);
  });

  it("snaps centres to centres and right edges to right edges", () => {
    // Centre of a 40 px box near the centre of the second target (150).
    expect(snapMove(box(0, 0, 40, 10), 132, 300, targets).dx).toBe(130);
    // Right edge near the first target's right edge (300).
    expect(snapMove(box(0, 0, 50, 10), 247, 400, targets).dx).toBe(250);
  });

  it("snaps to the page's centre and leaves far drags alone", () => {
    const page = { width: 600, height: 800 };
    const s = snapMove(box(0, 0, 100, 20), 252, 500, [], { page });
    expect(s.dx).toBe(250);
    const free = snapMove(box(0, 0, 10, 10), 37, 61, targets, { page });
    expect([free.dx, free.dy, free.guides.length]).toEqual([37, 61, 0]);
  });

  it("only considers the nearest elements", () => {
    const far = box(1000, 1000, 1200, 1020);
    const many = [...Array.from({ length: 12 }, (_, i) => box(0, 50 + i * 30, 20, 60 + i * 30)), far];
    // Dragged near the far box's left edge, but the far box is not among the nearest two.
    expect(snapMove(box(0, 0, 10, 10), 997, 10, many, { nearest: 2 }).dx).toBe(997);
  });
});

describe("snapping a resize", () => {
  it("snaps the dragged corner to edges nearby", () => {
    const s = snapResize(box(0, 0, 100, 50), 96, 47, [box(150, 0, 202, 20), box(0, 95, 50, 120)]);
    expect([s.dx, s.dy]).toEqual([102, 45]);
  });
});
