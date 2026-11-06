// Flex edge cases: equal basis-0 segments that refuse to clip their labels, a shrinking truncated
// text beside them, wrapping, auto margins and min-content overflow.
const segments = (selected: string) => (
  <row className="gap-1 p-1 rounded-xl bg-surface-sunken">
    {['All', 'Active', 'Completed'].map(label => (
      <text key={label} className={`grow basis-0 min-h-8 px-2 py-1 rounded-lg text-xs text-center ${label === selected ? 'bg-surface shadow-sm' : 'text-muted'}`}>{label}</text>
    ))}
  </row>
);

export function render() {
  return (
    <page className="bg-canvas p-4 gap-3 items-start">
      <row className="w-[300px] items-center justify-between gap-3 p-3 border border-line rounded-lg">
        <text className="text-sm text-muted">12 items</text>{segments('All')}
      </row>
      <row className="w-[200px] p-2 border border-line rounded-lg">{segments('Completed')}</row>
      <row className="w-[300px] items-center gap-2 p-2 border border-line rounded-lg">
        <text className="min-w-0 shrink truncate text-sm">A long title that cannot fit beside it</text>{segments('Active')}
      </row>
      <row className="w-full flex-wrap gap-2">
        {Array.from({ length: 9 }, (_, i) => <text key={String(i)} className="px-3 py-1 rounded-full bg-surface text-sm">{`Item ${i + 1}`}</text>)}
      </row>
      <row className="w-full gap-2"><text className="text-sm">Start</text><text className="ms-auto text-sm">End</text></row>
    </page>
  );
}
