// Scrolling demos of the View catalogue: each is the body of one demo.
import { List, reveal, useLocal } from '@lingxia/view';
import { Pager } from './kit';
import { Button } from '../ui';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

export function JumpDemo() {
  return (
    <>
      <view className="flex-row gap-2">
        <Button testId="lab-reveal-far" size="sm" variant="secondary" onClick={() => reveal('lab-reveal-row-25')}>Row 25</Button>
        <Button testId="lab-reveal-first" size="sm" variant="secondary" onClick={() => reveal('lab-reveal-row-0')}>Row 0</Button>
      </view>
      <scroll data-testid="lab-reveal-scroll" className="h-32 rounded-xl bg-surface-sunken">
        {Array.from({ length: 30 }, (_, i) => (
          <text key={`row-${i}`} id={`lab-reveal-row-${i}`} data-testid={`lab-reveal-row-${i}`} className="h-8 px-3 text-sm text-text">{`Row ${i}`}</text>
        ))}
      </scroll>
    </>
  );
}

export function StickyDemo() {
  const t = useSay();
  const days = t(labels.stickyDays).split('|');
  const rows = t(labels.stickyRows).split('|');
  const groups: [string, string[]][] = [[days[0], rows.slice(0, 2)], [days[1], rows.slice(2, 5)], [days[2], rows.slice(5)]];
  return (
    <scroll data-testid="lab-sticky-scroll" className="h-48 rounded-lg border border-line">
      {groups.map(([day, items]) => (
        <view key={day}>
          <text className="sticky top-0 px-3 py-1.5 text-xs font-semibold bg-surface-sunken">{day}</text>
          {items.map(item => <text key={item} className="px-3 py-3 text-sm border-b border-line">{item}</text>)}
        </view>
      ))}
    </scroll>
  );
}

// A sideways scroll that stops on a page; `bind` reads the page it rests on and moves it.
export function SnapDemo() {
  const page = useLocal('pager', 0);
  const t = useSay();
  return (
    <>
      <scroll data-testid="motion-pager-scroll" bind={page} className="w-72 overflow-x-auto snap-x snap-always rounded-xl bg-surface-sunken">
        <view className="flex-row">
          {t(labels.dayParts).split('|').map(name => (
            <view key={name} className="snap-start shrink-0 w-72 h-24 items-center justify-center">
              <text className="text-lg font-semibold">{name}</text>
            </view>
          ))}
        </view>
      </scroll>
      <view className="flex-row gap-2 items-center">
        <view className="grow"><Readout testId="motion-pager-state" state={`page=${page.value}`}>{t(readouts.pages(page.value))}</Readout></view>
        <Button testId="motion-pager-next" size="sm" onClick={() => page.set((page.value + 1) % 3)}>{t(labels.next)}</Button>
      </view>
    </>
  );
}

// A pager with dots under the pages, a tap on one jumps there; the pages read "n of m" as a list.
export function DotsDemo() {
  const page = useLocal('kit-pager-page', 0);
  const t = useSay();
  return (
    <>
      <Pager bind={page} label="Checkout steps" testId="lab-kit-pager-view" className="w-72">
        {['Plan', 'Pay', 'Done'].map(name => (
          <view key={name} className="h-28 items-center justify-center rounded-xl bg-surface-sunken">
            <text className="text-lg font-semibold">{name}</text>
          </view>
        ))}
      </Pager>
      <Readout testId="lab-kit-pager-state" state={`page=${page.value}`}>{t(readouts.pages(page.value))}</Readout>
    </>
  );
}

export function ContainedDemo() {
  const t = useSay();
  return (
    <scroll data-testid="lab-overscroll" className="h-24 rounded-lg border border-line overscroll-contain">
      <view className="p-3 gap-2">
        {Array.from({ length: 8 }, (_, i) => <text key={String(i)} className="text-xs">{`${t(labels.containedRow)} ${i + 1}`}</text>)}
      </view>
    </scroll>
  );
}

// Frozen rows in a frozen array: a List reuses the rows it already rendered.
const rows = Object.freeze(Array.from({ length: 40 }, (_, index) => Object.freeze({ id: `row-${index + 1}`, label: `Row ${index + 1}` })));

export function FilterDemo() {
  const query = useLocal('controls-query', '');
  const picked = useLocal('controls-picked', '');
  const t = useSay();
  const shown = Object.freeze(rows.filter(row => row.label.toLowerCase().includes(query.value.toLowerCase())));
  return (
    <>
      <input type="search" data-testid="lab-controls-search" bind={query} aria-label="Filter rows" placeholder={t(labels.filterRows)}
        className="px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm" />
      <List data-testid="lab-controls-list" className="h-48 rounded-lg border border-line" items={shown} getKey={row => row.id}
        renderItem={row => (
          <button data-testid={`lab-controls-${row.id}`} onClick={() => picked.set(row.label)} className="px-3 py-2 border-b border-line text-sm text-start">{row.label}</button>
        )} />
      <Readout testId="lab-filter-values" state={`query=${query.value} picked=${picked.value}`}>{t(readouts.filter(shown.length, picked.value))}</Readout>
    </>
  );
}

/** The Scrolling group: every demo the text table names, drawn by its body. */
export const scrollingDemos = {
  jump: JumpDemo,
  sticky: StickyDemo,
  snap: SnapDemo,
  dots: DotsDemo,
  contained: ContainedDemo,
  filter: FilterDemo,
};
