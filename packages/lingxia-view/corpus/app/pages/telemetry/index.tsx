import { useLocal, type View } from '@lingxia/view';

export function render({ data }: View<object, { hourly: number[]; share: number[]; latency: number[] }>) {
  const picked = useLocal('picked', -1);
  return (
    <page className="bg-canvas">
      <scroll className="grow">
        <column className="p-4 gap-4">
          <column className="p-4 gap-2 rounded-2xl bg-surface border border-line">
            <text className="text-sm font-semibold">Usage today</text>
            <plot kind="bar" values={data.hourly} aria-label="Hourly usage" selectedIndex={picked.value} onSelect={index => picked.set(index)} className="h-40" />
          </column>
          <row className="gap-4">
            <column className="grow basis-0 p-4 gap-2 rounded-2xl bg-surface border border-line">
              <text className="text-sm font-semibold">By device</text>
              <plot kind="pie" values={data.share} aria-label="Usage by device" className="h-36" />
            </column>
            <column className="grow basis-0 p-4 gap-2 rounded-2xl bg-surface border border-line">
              <text className="text-sm font-semibold">Latency</text>
              <plot kind="line" values={data.latency} smooth area aria-label="Latency" className="h-36" />
            </column>
          </row>
        </column>
      </scroll>
    </page>
  );
}
