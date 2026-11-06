// Fixed, auto-fit and arbitrary templates, spans, ordering, and a definitions grid of fragments.
import { Fragment } from '@lingxia/view';

export function render() {
  const cell = 'h-12 rounded-lg bg-surface-sunken grid place-items-center';
  return (
    <page className="bg-canvas p-4 gap-4">
      <grid className="grid-cols-[repeat(auto-fit,minmax(6rem,1fr))] gap-2">
        {Array.from({ length: 7 }, (_, i) => <view key={String(i)} className={cell}><text className="text-xs">{`Item ${i + 1}`}</text></view>)}
      </grid>
      <grid className="grid-cols-3 grid-rows-[auto_auto] gap-2">
        <view className="col-span-2 h-12 rounded-lg bg-accent" />
        <view className="row-span-2 rounded-lg bg-chart-2" />
        <view className="h-12 rounded-lg bg-surface-sunken" />
        <view className="h-12 rounded-lg bg-chart-4 order-first" />
      </grid>
      <grid className="grid-cols-[auto_1fr] gap-x-4 gap-y-1">
        {['Term', 'A longer term', 'X'].map(term => (
          <Fragment key={term}><text className="text-sm font-semibold">{term}</text><text className="text-sm text-muted">Lorem ipsum dolor sit amet.</text></Fragment>
        ))}
      </grid>
    </page>
  );
}
