import { List, type View } from '@lingxia/view';

interface Row { id: number; title: string; note: string; count: number; done: boolean }

// A Logic-owned list: the bench changes one record per tick.
export function render({ data }: View<{}, { tick: number; rows: Row[] }>) {
  return (
    <view>
      <text>{`tick ${data.tick}`}</text>
      <List className="h-[760px]" items={data.rows} getKey={row => String(row.id)}
        renderItem={row => (
          <view className="flex-row p-2 gap-3">
            <view className="grow">
              <text>{row.title}</text>
              <text>{row.note}</text>
            </view>
            <text>{String(row.count)}</text>
          </view>
        )} />
    </view>
  );
}
