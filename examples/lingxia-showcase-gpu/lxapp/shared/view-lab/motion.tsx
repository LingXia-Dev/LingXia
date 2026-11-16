// Motion demos of the View catalogue: each is the body of one demo.
import { useLocal, useView } from '@lingxia/view';
import { Button } from '../ui';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';

// The panel keeps its text and changes only its height; the row after it follows by layout motion.
export function PanelDemo() {
  const open = useLocal('lab-panel-open', false);
  const t = useSay();
  return (
    <>
      <button data-testid="lab-panel-toggle" aria-expanded={open.value} onClick={() => open.set(!open.value)}
        className="h-11 px-3 rounded-lg bg-surface-sunken text-sm font-semibold items-start justify-center active:opacity-70">
        {t(labels.panelHeading)}
      </button>
      <view data-testid="lab-panel-box"
        className={`overflow-hidden rounded-lg bg-accent-soft transition-size duration-300 ease-out ${open.value ? 'h-24' : 'h-0'}`}>
        <text className="p-3 text-sm">{t(labels.panelBody)}</text>
      </view>
      <view data-testid="lab-panel-after" className="h-10 px-3 rounded-lg border border-line justify-center transition-layout duration-300 ease-out">
        <text className="text-sm">{t(labels.panelAfter)}</text>
      </view>
      <Readout testId="lab-panel-state" state={`open=${open.value}`}>{t(readouts.panel(open.value))}</Readout>
    </>
  );
}

export function PressDemo() {
  const t = useSay();
  return (
    <view className="flex-row">
      <button data-testid="motion-press-button" onClick={() => {}} className="px-4 py-3 rounded-xl bg-accent text-on-accent text-sm active:scale-95 transition-transform duration-150">
        {t(labels.pressHold)}
      </button>
    </view>
  );
}

// `interrupt` asks the page's Logic to reverse the fade 100 ms in: sooner than two taps can land.
export function FadeDemo() {
  const { data, actions } = useView();
  const shown = useLocal('shown', true);
  const t = useSay();
  const visible = shown.value && !(data as { dipped?: boolean }).dipped;
  return (
    <view className="flex-row items-center gap-3">
      <Button testId="motion-fade-toggle" size="sm" onClick={() => shown.set(!shown.value)}>{t(shown.value ? labels.fadeOut : labels.fadeIn)}</Button>
      <Button testId="motion-fade-interrupt" size="sm" variant="secondary" onClick={() => actions.interrupt()}>{t(labels.interrupt)}</Button>
      <view data-testid="motion-fade-box" className={`w-12 h-12 rounded-lg bg-accent transition-opacity duration-200 ease-out ${visible ? 'opacity-100' : 'opacity-0'}`} />
    </view>
  );
}

export function PresenceDemo() {
  const present = useLocal('present', true);
  const t = useSay();
  return (
    <>
      <view className="flex-row"><Button testId="motion-presence-toggle" size="sm" onClick={() => present.set(!present.value)}>{t(present.value ? labels.remove : labels.add)}</Button></view>
      {present.value && (
        <view data-testid="motion-presence-card" className="h-12 rounded-lg bg-accent transition duration-300 starting:opacity-0 starting:translate-y-3 exit:opacity-0 exit:-translate-y-3" />
      )}
    </>
  );
}

export function ReorderDemo() {
  const order = useLocal('order', ['One', 'Two', 'Three', 'Four']);
  const t = useSay();
  const names = t(labels.orderRows).split('|');
  const name = (id: string) => names[['One', 'Two', 'Three', 'Four'].indexOf(id)];
  return (
    <>
      <view className="flex-row items-center gap-3">
        <Button testId="motion-layout-rotate" size="sm" onClick={() => order.set([...order.value.slice(1), order.value[0]])}>{t(labels.rotate)}</Button>
        <Button testId="motion-layout-reverse" size="sm" variant="secondary" onClick={() => order.set([...order.value].reverse())}>{t(labels.reverse)}</Button>
      </view>
      {order.value.map(id => (
        <view key={id} data-testid={`motion-layout-${id}`} className="flex-row h-10 px-3 items-center rounded-lg bg-surface-sunken transition-layout duration-300">
          <text className="text-sm">{name(id)}</text>
        </view>
      ))}
    </>
  );
}

export function LoopDemo() {
  return (
    <vector data-testid="motion-spin-icon" aria-hidden className="w-10 h-10 text-accent animate-spin" paths={[{ d: 'M12 3a9 9 0 1 0 9 9', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round' }]} />
  );
}

export function ReduceDemo() {
  return <view data-testid="motion-reduce-box" className="w-10 h-10 rounded-lg bg-danger motion-safe:animate-pulse motion-reduce:opacity-60" />;
}

export function PushDemo() {
  const { actions } = useView();
  const t = useSay();
  return (
    <view className="flex-row">
      <Button testId="motion-push-open" size="sm" onClick={() => actions.open({ page: 'motion-target' })}>{t(labels.openPage)}</Button>
    </view>
  );
}

/** The Motion group: every demo the text table names, drawn by its body. */
export const motionDemos = {
  press: PressDemo,
  fade: FadeDemo,
  presence: PresenceDemo,
  reorder: ReorderDemo,
  loop: LoopDemo,
  reduce: ReduceDemo,
  push: PushDemo,
  panel: PanelDemo,
};
