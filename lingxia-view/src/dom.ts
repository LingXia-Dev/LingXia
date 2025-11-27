export type MeasuredElement = {
  rect: { x: number; y: number; width: number; height: number };
  cornerRadius?: number;
};

export function measureElement(el: HTMLElement): MeasuredElement {
  const rect = el.getBoundingClientRect();
  const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
  const round = (v: number) => Math.round(v * dpr) / dpr;

  let cornerRadius: number | undefined;
  const radiusStr = getComputedStyle(el).borderRadius;
  const match = radiusStr && radiusStr.match(/^([0-9.]+)px/);
  if (match) {
    const parsed = parseFloat(match[1]);
    if (!Number.isNaN(parsed)) cornerRadius = parsed;
  }

  return {
    rect: {
      x: round(rect.left + window.scrollX),
      y: round(rect.top + window.scrollY),
      width: round(rect.width),
      height: round(rect.height)
    },
    cornerRadius
  };
}
