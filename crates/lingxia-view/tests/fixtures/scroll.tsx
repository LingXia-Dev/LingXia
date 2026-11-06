// Scroll content larger than its box on both axes, a nested scroll, and a keyed list anchored at its end.
import { List } from '@lingxia/view';

const rows = Array.from({ length: 60 }, (_, i) => ({ id: `r${i}`, text: `Item ${i + 1}` }));

export function render() {
  return (
    <page className="bg-canvas p-4 gap-3">
      <scroll className="h-32 rounded-xl border border-line">
        <column className="p-3 gap-2">
          {Array.from({ length: 10 }, (_, i) => <text key={String(i)} className="text-sm">{`Row ${i + 1}`}</text>)}
          <scroll className="h-16 rounded-lg bg-surface-sunken">
            {Array.from({ length: 8 }, (_, i) => <text key={String(i)} className="px-2 py-1 text-xs">{`Nested ${i + 1}`}</text>)}
          </scroll>
        </column>
      </scroll>
      <scroll className="h-20 rounded-xl border border-line overflow-x-auto">
        <row className="p-3 gap-2">{Array.from({ length: 12 }, (_, i) => <view key={String(i)} className="w-20 h-12 shrink-0 rounded-lg bg-surface-sunken" />)}</row>
      </scroll>
      <List items={rows} getKey={row => row.id} anchor="end" className="grow rounded-xl border border-line"
        renderItem={row => <text className="px-3 py-2 text-sm border-b border-line">{row.text}</text>} />
    </page>
  );
}
