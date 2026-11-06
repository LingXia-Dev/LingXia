// Gradient masks fade a scroll edge and a striped block.
export function render() {
  return (
    <page className="bg-canvas p-4 gap-4">
      <scroll className="h-40 rounded-xl bg-surface mask-b-from-60%">
        <column className="p-3 gap-2">
          {Array.from({ length: 12 }, (_, i) => <text key={String(i)} className="text-sm">{`Item ${i + 1}`}</text>)}
        </column>
      </scroll>
      <row className="h-24 mask-r-from-30% mask-r-to-90%">
        {['bg-accent', 'bg-chart-2', 'bg-chart-3', 'bg-chart-4'].map(color => <view key={color} className={`grow ${color}`} />)}
      </row>
    </page>
  );
}
