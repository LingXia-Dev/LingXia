// Group opacity composites overlapping children once; per-child opacity shows the overlap.
export function render() {
  const pair = (className: string) => (
    <row className={`h-24 ${className}`}>
      <view className="w-24 h-24 rounded-full bg-accent" />
      <view className="-ms-10 w-24 h-24 rounded-full bg-chart-2" />
    </row>
  );
  return (
    <page className="bg-canvas p-4 gap-4">
      {pair('opacity-50')}
      <row className="h-24">
        <view className="w-24 h-24 rounded-full bg-accent opacity-50" />
        <view className="-ms-10 w-24 h-24 rounded-full bg-chart-2 opacity-50" />
      </row>
      <view className="opacity-60 p-3 rounded-xl bg-surface border border-line">
        <text className="text-sm">Lorem ipsum dolor sit amet in a translucent group.</text>
      </view>
    </page>
  );
}
