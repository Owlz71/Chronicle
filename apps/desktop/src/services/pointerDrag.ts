/** Distance in pixels a pointer must travel before it counts as a drag. */
export const DRAG_THRESHOLD_PX = 5;

export type PointerPoint = { x: number; y: number };

/** Keeps a plain click from being mistaken for a drag (and the reverse). */
export function exceedsDragThreshold(start: PointerPoint, current: PointerPoint, threshold = DRAG_THRESHOLD_PX): boolean {
  return Math.hypot(current.x - start.x, current.y - start.y) >= threshold;
}

/**
 * Returns the value of `attribute` on the nearest ancestor of the element under
 * the pointer, or `undefined` when the pointer is not over a drop target.
 */
export function dropTargetAt(x: number, y: number, attribute: string): string | undefined {
  if (typeof document === "undefined") return undefined;
  const holder = document.elementFromPoint(x, y)?.closest(`[${attribute}]`);
  return holder?.getAttribute(attribute) ?? undefined;
}

/**
 * Scroll delta for edge auto-scrolling, which HTML5 drag and drop provided for
 * free and must now be driven manually.
 */
export function autoScrollDelta(bounds: { top: number; bottom: number }, y: number, margin = 28, speed = 14): number {
  if (y < bounds.top + margin) return -speed;
  if (y > bounds.bottom - margin) return speed;
  return 0;
}
