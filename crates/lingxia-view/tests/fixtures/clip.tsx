// Rounded overflow clips its children, nested clips intersect, and an unclipped sibling overflows.
export function render() {
  return (
    <page className="bg-canvas p-4 gap-4">
      <view className="h-32 rounded-3xl overflow-hidden bg-surface-sunken">
        <view className="-ms-6 -mt-6 w-40 h-40 rounded-full bg-accent" />
      </view>
      <view className="h-32 rounded-2xl overflow-hidden border border-line">
        <view className="m-4 h-40 rounded-xl overflow-hidden bg-chart-2">
          <text className="p-3 text-sm text-on-accent">{'Item 1 '.repeat(40)}</text>
        </view>
      </view>
      <view className="h-16 rounded-xl bg-surface">
        <view className="ms-8 w-24 h-28 rounded-lg bg-chart-4/70" />
      </view>
    </page>
  );
}
