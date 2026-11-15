// Gestures & pointer demos of the View catalogue: each is the body of one demo.
import { Fragment, useBox, useLocal, useView } from '@lingxia/view';
import { Button } from '../ui';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

// A card the finger moves along its track: it springs to the nearest of three points, a flick
// carries it one on; arrows and assistive tech step it, and `bind` follows where it lands.
// A box read in render and a pointer scrubbed over it: the marker follows x, arrows and a
// screen reader step it, and a vertical drag still scrolls the page.
export function ScrubDemo() {
  const count = 12;
  const box = useBox('lab-scrub');
  const at = useLocal('lab-scrub-at', 3);
  const last = useLocal('lab-scrub-last', 'none');
  const width = box.value?.width ?? 0;
  const size = box.value ? `${box.value.width}x${box.value.height}` : null;
  const t = useSay();
  return (
    <>
      <view box={box} data-testid="lab-scrub-plot" pointerAxis="x" aria-label="Month" aria-valuenow={at.value} aria-valuemin={0} aria-valuemax={count - 1}
        aria-valuetext={`Month ${at.value + 1}`}
        onPointer={event => {
          if (event.phase === 'increment') at.set(Math.min(count - 1, at.value + 1));
          else if (event.phase === 'decrement') at.set(Math.max(0, at.value - 1));
          else if (event.phase !== 'leave' && event.phase !== 'cancel' && event.width > 0)
            at.set(Math.max(0, Math.min(count - 1, Math.floor(event.x / event.width * count))));
          last.set(`${event.phase} ${event.pointer}`);
        }}
        className="flex-row h-24 rounded-xl bg-surface-sunken">
        <view style={{ width: width ? (at.value + 0.5) * width / count : 0 }} />
        <view data-testid="lab-scrub-marker" className="w-0.5 self-stretch bg-accent" />
      </view>
      <Readout testId="lab-scrub-state" state={`box=${size ?? 'none'} at=${at.value} last=${last.value}`}>
        {t(readouts.scrub(size, at.value, last.value))}
      </Readout>
    </>
  );
}

export function DragDemo() {
  const at = useLocal('drag-at', 0);
  const last = useLocal('drag-last', 'none');
  const t = useSay();
  return (
    <>
      <view className="h-20 rounded-xl bg-surface-sunken p-2">
        <view drag="x" dragSnapPoints={[0, 112, 224]} bind={at} aria-label="Card" data-testid="lab-drag-card"
          onDragStart={() => last.set('dragging')} onDragEnd={({ index }) => last.set(`released at ${index}`)}
          className="w-20 h-16 rounded-lg bg-accent items-center justify-center duration-500 ease-spring-bouncy">
          <text className="text-sm text-on-accent">Drag</text>
        </view>
      </view>
      <Readout testId="lab-drag-state" state={`at=${at.value} last=${last.value}`}>{t(readouts.drag(at.value, last.value))}</Readout>
      <Button testId="lab-drag-reset" size="sm" variant="secondary" onClick={() => at.set(0)}>{t(labels.backToStart)}</Button>
    </>
  );
}

// A panel holding a list: pulled up it opens before the list scrolls; pulled down from the list,
// the list returns to its top before the panel closes, all in one gesture.
export function DragChainDemo() {
  const at = useLocal('chain-at', 1);
  const t = useSay();
  return (
    <>
      <view className="h-56 rounded-xl bg-surface-sunken overflow-hidden">
        <view drag="y" dragSnapPoints={[0, 96]} bind={at} aria-label="Panel" data-testid="lab-chain-panel"
          className="h-56 rounded-t-xl bg-surface-raised">
          <text data-testid="lab-chain-title" className="p-3 text-sm font-semibold">{t(labels.pullUp)}</text>
          <scroll data-testid="lab-chain-list" className="grow basis-0">
            {Array.from({ length: 20 }, (_, i) => (
              <text key={`c${i}`} data-testid={`lab-chain-row-${i + 1}`} className="px-3 py-2 text-sm border-b border-line">{`Item ${i + 1}`}</text>
            ))}
          </scroll>
        </view>
      </view>
      <Readout testId="lab-chain-state" state={`at=${at.value}`}>{t(readouts.chain(at.value))}</Readout>
    </>
  );
}

// A sheet with detents: dragged between half and full height, dragged off it closes, and its scrim
// fades with how much of it is on screen. Its list scrolls once the sheet is fully open.
export function DetentsDemo() {
  const open = useLocal('detents-open', false);
  const detent = useLocal('detents-at', 1);
  const closed = useLocal('detents-closed', 0);
  const safeBottom = useView().env.safeArea?.bottom ?? 0;
  const t = useSay();
  return (
    <>
      <view className="flex-row gap-2 items-center">
        <Button testId="lab-detents-open" onClick={() => { detent.set(1); open.set(true); }}>{t(labels.openSheet)}</Button>
        <Readout testId="lab-detents-state" state={`open=${open.value} at=${detent.value} closed=${closed.value}`}>{t(readouts.sheet(open.value, detent.value, closed.value))}</Readout>
      </view>
      <dialog open={open.value} onClose={() => { closed.set(closed.value + 1); open.set(false); }} closedby="any" aria-label="Places"
        drag="y" dragSnapPoints={[0, '45%', '100%']} bind={detent} data-testid="lab-detents-sheet"
        style={{ paddingBottom: 16 + safeBottom }}
        className="mt-auto mb-0 w-full h-[85dvh] p-4 gap-3 rounded-t-3xl bg-surface-raised transition-transform duration-300 ease-spring starting:translate-y-full exit:translate-y-full">
        <view className="self-center w-10 h-1 rounded-full bg-line-strong" />
        <text data-testid="lab-detents-title" role="heading" headingLevel={3} className="text-base font-semibold">Places</text>
        <scroll data-testid="lab-detents-list" className="grow basis-0">
          {Array.from({ length: 30 }, (_, i) => <text key={`p${i}`} data-testid={`lab-detents-row-${i + 1}`} className="py-2 text-sm">{`Place ${i + 1}`}</text>)}
        </scroll>
      </dialog>
    </>
  );
}

export function SwipeRowsDemo() {
  const items = useLocal('swipe-items', ['Groceries', 'Dentist', 'Rent']);
  const last = useLocal('swipe-last', 'none');
  const remove = (item: string) => { items.set(items.value.filter(other => other !== item)); last.set(`deleted ${item}`); };
  const t = useSay();
  return (
    <>
      <view className="overflow-hidden bg-surface">
        {items.value.map((item, index) => (
          <Fragment key={item}>
            {index > 0 && <view className="h-px bg-line" />}
            <SwipeRow item={item} onOpen={() => last.set(`opened ${item}`)} onArchive={() => last.set(`archived ${item}`)} onDelete={() => remove(item)} />
          </Fragment>
        ))}
      </view>
      <Readout testId="lab-swipe-state" state={`rows=${items.value.length} last=${last.value}`}>{t(readouts.rows(items.value.length, last.value))}</Readout>
    </>
  );
}

// A pager: a horizontal scroll that stops on every page (`snap-always`), its page bound both ways.
export function PagerDemo() {
  const page = useLocal('pager-page', 0);
  const pages = ['Morning', 'Noon', 'Evening'];
  const t = useSay();
  return (
    <>
      <scroll data-testid="lab-pager-scroll" bind={page} className="w-72 overflow-x-auto snap-x snap-always rounded-xl bg-surface-sunken">
        <view className="flex-row">
          {pages.map(name => (
            <view key={name} className="snap-start shrink-0 w-72 h-28 items-center justify-center">
              <text className="text-lg font-semibold">{name}</text>
            </view>
          ))}
        </view>
      </scroll>
      <view className="flex-row gap-2 items-center">
        <view className="grow"><Readout testId="lab-pager-state" state={`page=${page.value}`}>{t(readouts.pages(page.value))}</Readout></view>
        <Button testId="lab-pager-next" size="sm" variant="secondary" onClick={() => page.set((page.value + 1) % pages.length)}>{t(labels.next)}</Button>
      </view>
    </>
  );
}

// A pressable row holding its own controls: each press goes to the innermost control under it.
export function NestedControlsDemo() {
  const opened = useLocal('opened', 0);
  const wifi = useLocal('wifi', true);
  const removed = useLocal('removed', 0);
  const t = useSay();
  return (
    <>
      <button data-testid="lab-nested-row" onClick={() => opened.set(opened.value + 1)} className="rounded-xl bg-surface-sunken px-4 py-3">
        <view className="flex-row items-center gap-3">
          <text className="grow text-sm">Wi-Fi</text>
          <switch data-testid="lab-nested-switch" bind={wifi} aria-label="Wi-Fi" />
          <button data-testid="lab-nested-remove" onClick={() => removed.set(removed.value + 1)} className="rounded-lg px-2 py-1 text-sm text-danger">Remove</button>
        </view>
      </button>
      <Readout testId="lab-nested-state" state={`opened=${opened.value} wifi=${wifi.value} removed=${removed.value}`}>{t(readouts.nested(opened.value, wifi.value, removed.value))}</Readout>
    </>
  );
}

export function SwipeRow({ item, onOpen, onArchive, onDelete }: { item: string; onOpen: () => void; onArchive: () => void; onDelete: () => void }) {
  const open = useLocal('open', 0);
  const id = item.toLowerCase();
  const strip = 'h-full items-center-safe justify-center text-sm font-medium';
  return (
    <view className="grid *:col-start-1 *:row-start-1 h-12">
      <view className="flex-row absolute top-0 bottom-0 right-0 w-[144px]">
        <button id={`archive-${id}`} data-testid={`lab-swipe-archive-${id}`} onClick={() => { open.set(0); onArchive(); }}
          className={`${strip} w-[72px] bg-accent text-on-accent`}>Archive</button>
        <button id={`delete-${id}`} data-testid={`lab-swipe-delete-${id}`} onClick={onDelete}
          className={`${strip} w-[72px] bg-danger text-on-danger`}>Delete</button>
      </view>
      <button drag="x" dragSnapPoints={[0, -144, '-100%']} bind={open} aria-actions={`archive-${id} delete-${id}`}
        data-testid={`lab-swipe-row-${id}`} onClick={onOpen} onDragEnd={({ index }) => { if (index === 2) onDelete(); }}
        className="absolute inset-0 px-4 justify-center items-start text-start bg-surface text-sm text-text">{item}</button>
    </view>
  );
}

/** The Gestures & pointer group: every demo the text table names, drawn by its body. */
export const gestureDemos = {
  pointer: ScrubDemo,
  stops: DragDemo,
  chain: DragChainDemo,
  sheet: DetentsDemo,
  rows: SwipeRowsDemo,
  pages: PagerDemo,
  nested: NestedControlsDemo,
};
