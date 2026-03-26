export type MeasuredElement = {
  rect: { x: number; y: number; width: number; height: number };
  cornerRadius?: number;
};

export function measureElement(el: HTMLElement): MeasuredElement {
  const rect = el.getBoundingClientRect();

  const parseRadius = (radiusStr: string): number | undefined => {
    const match = radiusStr && radiusStr.match(/^([0-9.]+)px/);
    if (!match) return undefined;
    const parsed = parseFloat(match[1]);
    return Number.isNaN(parsed) ? undefined : parsed;
  };

  const isOverflowClipping = (style: CSSStyleDeclaration): boolean =>
    style.overflow === "hidden" ||
    style.overflow === "clip" ||
    style.overflowX === "hidden" ||
    style.overflowX === "clip" ||
    style.overflowY === "hidden" ||
    style.overflowY === "clip";

  let cornerRadius = parseRadius(getComputedStyle(el).borderRadius);
  if (cornerRadius === undefined) {
    const rectMatches = (a: DOMRect, b: DOMRect, epsilon = 0.5) =>
      Math.abs(a.left - b.left) <= epsilon &&
      Math.abs(a.top - b.top) <= epsilon &&
      Math.abs(a.width - b.width) <= epsilon &&
      Math.abs(a.height - b.height) <= epsilon;

    let parent = el.parentElement;
    while (parent) {
      const style = getComputedStyle(parent);
      if (isOverflowClipping(style) && rectMatches(parent.getBoundingClientRect(), rect)) {
        const parentRadius = parseRadius(style.borderRadius);
        if (parentRadius !== undefined) {
          cornerRadius = parentRadius;
          break;
        }
      }
      parent = parent.parentElement;
    }
  }

  if (cornerRadius === undefined) {
    // Common card layout: native video is clipped by an overflow-hidden rounded ancestor
    // that doesn't have exactly the same rect (e.g. header + media block).
    const near = (a: number, b: number, epsilon = 0.75) => Math.abs(a - b) <= epsilon;
    let parent = el.parentElement;
    while (parent) {
      const style = getComputedStyle(parent);
      const parentRadius = parseRadius(style.borderRadius);
      if (isOverflowClipping(style) && parentRadius !== undefined) {
        const p = parent.getBoundingClientRect();
        const contains =
          rect.left >= p.left - 0.75 &&
          rect.right <= p.right + 0.75 &&
          rect.top >= p.top - 0.75 &&
          rect.bottom <= p.bottom + 0.75;
        const alignedLeftRight = near(rect.left, p.left) && near(rect.right, p.right);
        const touchesTopOrBottom = near(rect.top, p.top) || near(rect.bottom, p.bottom);
        if (contains && alignedLeftRight && touchesTopOrBottom) {
          cornerRadius = parentRadius;
          break;
        }
      }
      parent = parent.parentElement;
    }
  }

  // Return document coordinates (CSS pixels)
  // Native layer converts to physical pixels and handles scroll offset
  return {
    rect: {
      x: rect.left + window.scrollX,
      y: rect.top + window.scrollY,
      width: rect.width,
      height: rect.height
    },
    cornerRadius
  };
}
