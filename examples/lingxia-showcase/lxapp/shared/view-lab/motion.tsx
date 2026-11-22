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
      <button data-testid="motion-press-button" onClick={() => {}} className="px-4 py-3 rounded-xl bg-accent-solid text-on-accent-solid text-sm active:scale-95 transition-transform duration-150">
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
  const t = useSay();
  return (
    <view className="flex-row">
      <Button testId="motion-push-open" size="sm" to={{ page: 'motion-target' }}>{t(labels.openPage)}</Button>
    </view>
  );
}

const weeks = [[12, 18, 15, 26, 22, 30, 28], [24, 14, 20, 12, 28, 18, 34]];
// One command per point, so every data set morphs into every other.
const chartLine = (values: readonly number[]) =>
  values.map((value, i) => `${i ? 'L' : 'M'}${i * 20} ${40 - value}`).join(' ');

export function MorphDemo() {
  const week = useLocal('morph-week', 0);
  const t = useSay();
  return (
    <>
      <vector data-testid="motion-morph-chart" aria-hidden viewBox={[0, 0, 120, 40]} className="w-full h-24 text-accent transition-[d] duration-500 ease-in-out"
        paths={[{ key: 'line', d: chartLine(weeks[week.value]), fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' }]} />
      <view className="flex-row items-center gap-3">
        <Button testId="motion-morph-next" size="sm" onClick={() => week.set(1 - week.value)}>{t(week.value ? labels.thisWeek : labels.lastWeek)}</Button>
        <Readout testId="motion-morph-state" state={`week=${week.value}`}>{t(readouts.week(week.value))}</Readout>
      </view>
    </>
  );
}

// The whole line counts as 1, so one dash of 1 offset from 1 to 0 reveals it end to end.
export function DrawInDemo() {
  const drawn = useLocal('draw-in-drawn', false);
  const t = useSay();
  const values = weeks[0];
  return (
    <>
      <vector data-testid="motion-draw-chart" aria-hidden viewBox={[0, 0, 120, 40]} onLayout={() => { if (!drawn.value) drawn.set(true); }} className="w-full h-24 text-accent transition-[d] duration-700 ease-out"
        paths={[{ key: 'line', d: chartLine(values), pathLength: 1, strokeDasharray: [1], strokeDashoffset: drawn.value ? 0 : 1, fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' }]} />
      <view className="flex-row">
        <Button testId="motion-draw-again" size="sm" onClick={() => drawn.set(!drawn.value)}>{t(drawn.value ? labels.erase : labels.draw)}</Button>
      </view>
    </>
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
  morph: MorphDemo,
  drawIn: DrawInDemo,
};
