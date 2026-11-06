// Linear, radial and conic backgrounds with stops, and gradient strokes on a vector.
export function render() {
  return (
    <page className="bg-canvas p-4 gap-3">
      <view className="h-16 rounded-xl bg-linear-to-r from-accent to-chart-2" />
      <view className="h-16 rounded-xl bg-linear-45 from-chart-3 via-surface to-chart-4" />
      <row className="gap-3">
        <view className="grow h-28 rounded-xl bg-radial from-accent to-canvas" />
        <view className="grow h-28 rounded-full bg-conic from-chart-1 via-chart-3 to-chart-1" />
      </row>
      <vector aria-hidden className="h-24 w-full" viewBox={[0, 0, 200, 60]}
        paths={[{ d: 'M10 50 C60 0 140 60 190 10', fill: 'none', stroke: 'url(#line)', strokeWidth: 8, strokeLinecap: 'round' }]}
        gradients={{ line: { type: 'linear', x1: 0, y1: 0, x2: 200, y2: 0, units: 'userSpaceOnUse', stops: [{ offset: 0, color: 'chart-1' }, { offset: 1, color: 'chart-4' }] } }} />
    </page>
  );
}
