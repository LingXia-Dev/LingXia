// Layout demos of the View catalogue: each is the body of one demo.
import { useBox, useLocal } from '@lingxia/view';
import { Button } from '../ui';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

/** The room the Pinned badge needs at the page's end: its bottom-4 offset and its height. */
export const pinnedRoom = 16 + 24;

export function ShowDemo() {
  const shown = useLocal('shown', true);
  const t = useSay();
  return (
    <>
      <view className="flex-row gap-2 items-center">
        <Button testId="lab-display-toggle" size="sm" onClick={() => shown.set(!shown.value)}>{t(shown.value ? labels.hide : labels.show)}</Button>
        <text data-testid="lab-display-hidden" className={shown.value ? 'text-sm' : 'hidden'}>{t(labels.hiddenLabel)}</text>
      </view>
      <view className="block gap-1">
        <text className="text-xs text-muted">{t(labels.blockNote)}</text>
        <view className="h-2 rounded bg-accent" />
      </view>
      <view className="flex-row h-12 items-center gap-2 bg-surface-sunken rounded-lg px-2">
        <view data-testid="lab-display-inline" className="inline-flex px-2 py-1 rounded bg-surface"><text className="text-xs">{t(labels.inlineNote)}</text></view>
      </view>
      <view data-testid="lab-display-fixed" className="fixed bottom-4 right-4 px-3 py-1 rounded-full bg-accent"><text className="text-xs text-on-accent">{t(labels.pinned)}</text></view>
    </>
  );
}

export function GridDemo() {
  const t = useSay();
  return (
    <>
      <view data-testid="lab-grid-auto-fit" className="grid grid-cols-[repeat(auto-fit,minmax(6rem,1fr))] gap-2">
        {t(labels.tiles).split('|').map(tile => <view key={tile} className="h-14 rounded-lg bg-surface-sunken place-items-center grid"><text className="text-xs">{tile}</text></view>)}
      </view>
      <view data-testid="lab-grid-spans" className="grid grid-cols-3 grid-rows-[auto_auto] gap-2">
        <view className="col-span-2 h-12 rounded-lg bg-accent" />
        <view className="row-span-2 rounded-lg bg-danger" />
        <view className="h-12 rounded-lg bg-surface-sunken" />
        <view className="h-12 rounded-lg bg-surface-sunken order-first" />
      </view>
    </>
  );
}

export function SizesDemo() {
  const t = useSay();
  return (
    <>
      <view className="flex-row gap-3 items-start">
        <view data-testid="lab-aspect-video" className="grow basis-0 aspect-video rounded-lg bg-accent" />
        <view data-testid="lab-aspect-square" className="w-16 aspect-square rounded-lg text-danger bg-current" />
      </view>
      <text data-testid="lab-max-ch" className="max-w-[24ch] text-sm select-none">{t(labels.wraps)}</text>
    </>
  );
}

// The box is read in render: the page runs again when its size changes, with the new numbers.
export function MeasureDemo() {
  const box = useBox('lab-box-size');
  const wide = useLocal('lab-box-wide', true);
  const t = useSay();
  const size = box.value ? `${Math.round(box.value.width)} × ${Math.round(box.value.height)}` : null;
  return (
    <>
      <view box={box} data-testid="lab-box-target" className={`h-16 rounded-lg bg-accent-soft border border-accent ${wide.value ? 'w-full' : 'w-1/2'}`} />
      <view className="flex-row items-center gap-3">
        <Button testId="lab-box-toggle" size="sm" variant="secondary" onClick={() => wide.set(!wide.value)}>{t(wide.value ? labels.narrower : labels.wider)}</Button>
        <Readout testId="lab-box-state" state={`box=${box.value ? `${Math.round(box.value.width)}x${Math.round(box.value.height)}` : 'none'}`}>{t(readouts.box(size))}</Readout>
      </view>
    </>
  );
}

export function PositionDemo() {
  const t = useSay();
  return (
    <>
      <view data-testid="lab-structure-rows" className="rounded-lg border border-line">
        {t(labels.plansRows).split('|').map(row => (
          <text key={row} className="px-3 py-2 text-sm odd:bg-surface-sunken first:rounded-t-lg last:rounded-b-lg last:font-semibold">{row}</text>
        ))}
      </view>
      <view className="flex-row gap-2">
        <view data-testid="lab-structure-empty" className="h-8 grow basis-0 rounded-lg bg-danger empty:bg-surface-sunken" />
        <view className="h-8 grow basis-0 rounded-lg bg-surface-sunken"><text className="only:text-xs only:p-2">{t(labels.onlyChild)}</text></view>
      </view>
    </>
  );
}

// HTML's details and summary: the summary says whether it is open; the body exists only while it is.
export function SectionDemo() {
  const t = useSay();
  const open = useLocal('licenses-open', false);
  return (
    <view data-testid="lab-disclosure-licenses">
      <button data-testid="lab-disclosure-licenses-summary" aria-expanded={open.value} onClick={() => open.set(!open.value)} className="py-2 bg-transparent">
        <text className="text-sm font-semibold">{t(labels.licenses)}</text>
      </button>
      {open.value && <text data-testid="lab-disclosure-body" className="text-sm text-muted">MIT, Apache-2.0, BSD-3-Clause</text>}
    </view>
  );
}

// The two adaptive parts, each on its own page so it can fill the window.
export function AdaptiveDemo() {
  const t = useSay();
  return (
    <view className="flex-row flex-wrap gap-2">
      <Button testId="lab-open-list-detail" size="sm" variant="secondary" to={{ page: 'list-and-detail' }}>{t(labels.openListDetail)}</Button>
      <Button testId="lab-open-main-companion" size="sm" variant="secondary" to={{ page: 'main-and-companion' }}>{t(labels.openMainCompanion)}</Button>
    </view>
  );
}

/** The Layout group: every demo the text table names, drawn by its body. */
export const layoutDemos = {
  show: ShowDemo,
  grid: GridDemo,
  sizes: SizesDemo,
  measure: MeasureDemo,
  position: PositionDemo,
  section: SectionDemo,
  adaptive: AdaptiveDemo,
};
