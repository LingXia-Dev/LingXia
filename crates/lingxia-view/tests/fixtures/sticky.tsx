// Sticky headers inside a scroll: each pins at the top until the next section pushes it off.
export function render() {
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        {['A', 'B', 'C'].map(section => (
          <column key={section}>
            <text className="sticky top-0 z-10 px-4 py-2 text-sm font-semibold bg-surface/90 border-b border-line">{`Section ${section}`}</text>
            {Array.from({ length: 6 }, (_, i) => <text key={String(i)} className="px-4 py-3 text-sm border-b border-line">{`Item ${i + 1}`}</text>)}
          </column>
        ))}
        <text className="sticky bottom-0 px-4 py-3 text-sm bg-surface border-t border-line">Footer</text>
      </scroll>
    </page>
  );
}
