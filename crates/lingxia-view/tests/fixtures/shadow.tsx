// Outer shadows at each step, a coloured one, and shadows on a rounded and a clipped box.
export function render() {
  return (
    <page className="bg-canvas p-6 gap-6">
      <row className="gap-6">
        {['shadow-sm', 'shadow-md', 'shadow-lg', 'shadow-xl'].map(shadow => (
          <view key={shadow} className={`grow h-16 rounded-xl bg-surface ${shadow}`} />
        ))}
      </row>
      <view className="h-20 rounded-2xl bg-surface shadow-lg shadow-accent/40" />
      <row className="gap-6">
        <view className="w-20 h-20 rounded-full bg-surface shadow-xl" />
        <view className="grow h-20 rounded-xl overflow-hidden bg-surface shadow-md">
          <view className="h-10 bg-accent" />
        </view>
      </row>
    </page>
  );
}
