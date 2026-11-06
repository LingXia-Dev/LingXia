// Overlay stacking: z-ordered absolute boxes, a fixed scrim, an open popover and a modal sheet above it all.
export function render() {
  return (
    <page className="bg-canvas p-4 gap-3">
      <view className="h-32">
        <view className="absolute top-0 start-0 w-32 h-24 rounded-xl bg-accent z-20" />
        <view className="absolute top-6 start-12 w-32 h-24 rounded-xl bg-chart-2 z-10" />
        <view className="absolute top-12 start-24 w-32 h-24 rounded-xl bg-chart-4" />
      </view>
      <button popoverTarget="menu" className="self-end px-3 py-1 rounded-full bg-surface">⋯</button>
      <view id="menu" popover open role="menu" aria-label="Actions" className="mt-48 me-4 ms-auto mb-auto w-40 p-1 rounded-xl bg-surface-raised shadow-lg">
        {['Item 1', 'Item 2', 'Item 3'].map(item => <button key={item} role="menuitem" className="px-3 py-2 text-start">{item}</button>)}
      </view>
      <dialog open aria-label="Sheet" className="mt-auto mb-0 w-full p-4 pb-8 gap-3 rounded-t-3xl bg-surface-raised backdrop:bg-scrim">
        <spacer className="self-center w-10 h-1 rounded-full bg-line-strong" />
        <text className="text-lg font-semibold">Sheet</text>
        <text className="text-sm text-muted">Lorem ipsum dolor sit amet.</text>
      </dialog>
    </page>
  );
}
