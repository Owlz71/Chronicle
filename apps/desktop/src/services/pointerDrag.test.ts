import { describe, expect, it } from "vitest";
import { DRAG_THRESHOLD_PX, autoScrollDelta, exceedsDragThreshold } from "./pointerDrag";

describe("exceedsDragThreshold", () => {
  it("ignores pointer jitter below the threshold", () => {
    expect(exceedsDragThreshold({ x: 100, y: 100 }, { x: 102, y: 101 })).toBe(false);
  });

  it("starts a drag once the pointer travels far enough", () => {
    expect(exceedsDragThreshold({ x: 100, y: 100 }, { x: 100 + DRAG_THRESHOLD_PX, y: 100 })).toBe(true);
    expect(exceedsDragThreshold({ x: 100, y: 100 }, { x: 140, y: 170 })).toBe(true);
  });

  it("measures diagonal movement", () => {
    expect(exceedsDragThreshold({ x: 0, y: 0 }, { x: 4, y: 4 }, 5)).toBe(true);
  });
});

describe("autoScrollDelta", () => {
  const bounds = { top: 100, bottom: 400 };

  it("scrolls up near the top edge and down near the bottom edge", () => {
    expect(autoScrollDelta(bounds, 105)).toBeLessThan(0);
    expect(autoScrollDelta(bounds, 395)).toBeGreaterThan(0);
  });

  it("does not scroll in the middle", () => {
    expect(autoScrollDelta(bounds, 250)).toBe(0);
  });
});
