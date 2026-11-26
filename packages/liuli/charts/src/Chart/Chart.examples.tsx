import { cx, useLocal } from '@lingxia/view';
import { Chart, ChartLegend, ChartReadout } from './Chart';

const budget = [
  { key: 'spent', label: 'Spent', amount: 6200, color: 1 as const },
  { key: 'committed', label: 'Committed', amount: 2300, color: 2 as const },
  { key: 'left', label: 'Left', amount: 1500, color: 3 as const },
];
const fills = { 1: 'bg-chart-1', 2: 'bg-chart-2', 3: 'bg-chart-3' } as const;
// A layout takes small numbers: a part's share of the whole, never the amount itself.
const percent = (amount: number, parts: readonly { amount: number }[]) => Math.round((amount / parts.reduce((sum, part) => sum + part.amount, 0)) * 100);
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
        {budget.map(part => <view key={part.key} className={cx('h-6 basis-0 rounded-[calc(var(--radius-sm)*0.5)]', fills[part.color])} style={{ flexGrow: percent(part.amount, budget) }} />)}
      </view>
    </Chart>
  );
}

const spending = [
  { key: 'rent', label: 'Rent', amount: 1400, color: 1 as const },
  { key: 'food', label: 'Food', amount: 620, color: 2 as const },
  { key: 'transport', label: 'Transport', amount: 310, color: 3 as const },
  { key: 'utilities', label: 'Utilities', amount: 240, color: 4 as const },
  { key: 'health', label: 'Health', amount: 180, color: 5 as const },
  { key: 'leisure', label: 'Leisure', amount: 160, color: 6 as const },
  { key: 'clothes', label: 'Clothes', amount: 120, color: 7 as const },
  { key: 'gifts', label: 'Gifts', amount: 70, color: 8 as const },
];
const eight = {
  1: 'bg-chart-1', 2: 'bg-chart-2', 3: 'bg-chart-3', 4: 'bg-chart-4', 5: 'bg-chart-5', 6: 'bg-chart-6', 7: 'bg-chart-7', 8: 'bg-chart-8',
} as const;

/** Eight series colours, the theme's `chart-1` to `chart-8`. Past eight, merge the rest. */
export function Spending() {
  return (
    <Chart
      label="Spending in May"
      title="Spending"
      description="May, $3,100"
      summary="Rent is 45% of spending; food 20%; the other six kinds share the rest."
      size="fit"
      legend={<ChartLegend label="Kinds" variant="list" items={spending.map(part => ({ key: part.key, label: part.label, color: part.color, value: dollars(part.amount) }))} data-testid="spending-legend" />}
      data-testid="spending"
    >
      <view aria-hidden className="flex-row gap-0.5">
        {spending.map(part => <view key={part.key} className={cx('h-6 basis-0 rounded-[calc(var(--radius-sm)*0.5)]', eight[part.color])} style={{ flexGrow: percent(part.amount, spending) }} data-testid={`part-${part.key}`} />)}
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
  return <Chart label="Budget for the quarter" title="Budget" size="sm" isLoading plotShape="bars" data-testid="loading" />;
}

export function StateEmpty() {
  return <Chart label="Budget for the quarter" title="Budget" size="sm" isEmpty plotShape="bars" emptyLabel="No budget set yet" emptyHint="Set one in Settings to track spending here." data-testid="empty" />;
}

export function StateError() {
  const tries = useLocal('tries', 0);
  return <Chart label="Budget for the quarter" title="Budget" size="sm" error={tries.value === 0 ? 'The budget could not be loaded.' : `Still not loaded after ${tries.value + 1} tries.`} onRetry={() => tries.set(tries.value + 1)} retryLabel="Try again" data-testid="error" />;
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
        {budget.map(part => <view key={part.key} className={cx('h-2.5 basis-0 rounded-[calc(var(--radius-sm)*0.33333)]', fills[part.color])} style={{ flexGrow: percent(part.amount, budget) }} />)}
      </view>
    </Chart>
  );
}
