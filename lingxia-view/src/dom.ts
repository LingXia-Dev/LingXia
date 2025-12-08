export type MeasuredElement = {
  rect: { x: number; y: number; width: number; height: number };
  cornerRadius?: number;
};

export function measureElement(el: HTMLElement): MeasuredElement {
  const rect = el.getBoundingClientRect();

  let cornerRadius: number | undefined;
  const radiusStr = getComputedStyle(el).borderRadius;
  const match = radiusStr && radiusStr.match(/^([0-9.]+)px/);
  if (match) {
    const parsed = parseFloat(match[1]);
    if (!Number.isNaN(parsed)) cornerRadius = parsed;
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
