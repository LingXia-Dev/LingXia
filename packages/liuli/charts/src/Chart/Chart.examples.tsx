import { cx, useLocal } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout } from './Chart';

const budget = [
  { key: 'spent', label: 'Spent', amount: 6200, color: 1 as const },
  { key: 'committed', label: 'Committed', amount: 2300, color: 2 as const },
  { key: 'left', label: 'Left', amount: 1500, color: 3 as const },
];
const fills = { 1: 'bg-chart-1', 2: 'bg-chart-2', 3: 'bg-chart-3' } as const;
const dollars = (value: number) => `$${String(value).replace(/\B(?=(\d{3})+(?!\d))/g, ',')}`;

/** The frame around a plot of your own: here one bar split into parts, built from plain views. */
export function Budget() {
  return (
    <Chart
      label="Budget for the quarter"
      title="Budget"
      description="Third quarter, $10,000"
      summary="62% spent, 23% committed, 15% left."
      size="fit"
      legend={<ChartLegend label="Parts" variant="list" items={budget.map(part => ({ key: part.key, label: part.label, color: part.color, value: dollars(part.amount) }))} />}
      data-testid="budget"
    >
      <view aria-hidden className="flex-row gap-0.5">
        {budget.map(part => <view key={part.key} className={cx('h-6 basis-0 rounded-[3px]', fills[part.color])} style={{ flexGrow: part.amount }} />)}
      </view>
    </Chart>
  );
}

const plans = [
  { key: 'free', label: 'Free', color: 1 as const, dash: [] },
  { key: 'team', label: 'Team', color: 2 as const, dash: [6, 4] },
  { key: 'pro', label: 'Pro', color: 3 as const, dash: [2, 3] },
];

/** Squares name bars and slices; strokes name lines and carry each line's dash. */
export function Legends() {
  const hidden = useLocal<readonly string[]>('hidden', ['pro']);
  const toggle = (key: string) => hidden.set(hidden.value.includes(key) ? hidden.value.filter(each => each !== key) : [...hidden.value, key]);
  return (
    <view className="gap-5">
      <ChartLegend label="Plans" items={plans} data-testid="squares" />
      <ChartLegend label="Plans, as lines" mark="line" items={plans.map(plan => ({ ...plan, isOff: hidden.value.includes(plan.key) }))} pressedState="shown" onItemPress={toggle} data-testid="lines" />
    </view>
  );
}

export function Readout() {
  return (
    <view className="gap-3">
      <ChartReadout label="Wednesday" values={[{ text: '121' }]} data-testid="one" />
      <ChartReadout label="Q3" values={[{ label: 'Online', text: '$55k', color: 1 }, { label: 'In store', text: '$33k', color: 2 }]} data-testid="two" />
    </view>
  );
}

export function StateLoading() {
  return <Chart label="Budget for the quarter" title="Budget" size="sm" isLoading data-testid="loading" />;
}

export function StateEmpty() {
  return <Chart label="Budget for the quarter" title="Budget" size="sm" isEmpty emptyLabel="No budget set" data-testid="empty" />;
}

export function StateError() {
  return <Chart label="Budget for the quarter" title="Budget" size="sm" error="The budget could not be loaded" data-testid="error" />;
}

/** The home tile: a heading, a plot and a legend, as small as the frame goes. */
export function Specimen() {
  return (
    <Chart
      label="Budget"
      title="Budget"
      size="fit"
      legend={<ChartLegend label="Parts" items={[budget[0], budget[2]].map(part => ({ key: part.key, label: part.label, color: part.color }))} />}
      className="w-32 self-center"
    >
      <view aria-hidden className="flex-row gap-0.5">
        {budget.map(part => <view key={part.key} className={cx('h-2.5 basis-0 rounded-[2px]', fills[part.color])} style={{ flexGrow: part.amount }} />)}
      </view>
    </Chart>
  );
}
