import { List, type View } from '@lingxia/view';

interface Row { id: number; title: string; note: string; count: number; done: boolean }

// A Logic-owned list: the bench changes one record per tick.
export function render({ data }: View<{}, { tick: number; rows: Row[] }>) {
  return (
    <column>
      <text>{`tick ${data.tick}`}</text>
      <List className="h-[760px]" items={data.rows} getKey={row => String(row.id)}
        renderItem={row => (
          <row className="p-2 gap-3">
            <column className="grow">
              <text>{row.title}</text>
              <text>{row.note}</text>
            </column>
            <text>{String(row.count)}</text>
          </row>
        )} />
    </column>
  );
}
