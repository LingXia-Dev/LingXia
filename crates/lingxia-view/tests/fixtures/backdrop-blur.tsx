// A translucent bar blurs what lies behind it; a second one only tints.
export function render() {
  return (
    <page className="bg-canvas">
      <column className="p-4 gap-2">
        {['bg-accent', 'bg-chart-2', 'bg-chart-3', 'bg-chart-4', 'bg-chart-5'].map(color => (
          <row key={color} className="gap-2">
            <view className={`grow h-16 rounded-xl ${color}`} />
            <text className="w-24 text-sm">Item 1 lorem</text>
          </row>
        ))}
      </column>
      <view className="absolute inset-x-0 top-10 h-24 bg-surface/60 backdrop-blur-xl border-y border-line" />
      <view className="absolute inset-x-0 top-48 h-16 bg-surface/60" />
    </page>
  );
}
