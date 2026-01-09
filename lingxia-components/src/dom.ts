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
      const overflowHidden =
        style.overflow === "hidden" ||
        style.overflow === "clip" ||
        style.overflowX === "hidden" ||
        style.overflowX === "clip" ||
        style.overflowY === "hidden" ||
        style.overflowY === "clip";
      if (overflowHidden && rectMatches(parent.getBoundingClientRect(), rect)) {
        const parentRadius = parseRadius(style.borderRadius);
        if (parentRadius !== undefined) {
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
