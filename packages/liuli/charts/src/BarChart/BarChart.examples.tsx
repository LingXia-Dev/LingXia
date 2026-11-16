import { useLocal } from '@lingxia/view';
import { summarize } from '../summary/en';
import { BarChart } from './BarChart';

const week = [
  { day: 'Mon', orders: 132 }, { day: 'Tue', orders: 148 }, { day: 'Wed', orders: 121 }, { day: 'Thu', orders: 163 },
  { day: 'Fri', orders: 187 }, { day: 'Sat', orders: 96 }, { day: 'Sun', orders: 74 },
];

/** Tap a bar to choose it, or drag across the bars; tap the chosen bar again to let go. */
export function Orders() {
  const day = useLocal<string | null>('day', null);
  return (
    <BarChart
      data={week}
      x="day"
      y="orders"
      label="Orders by weekday"
      title="Orders"
      description="This week"
      summarize={summarize}
      selectedKey={day.value}
      onSelect={day.set}
      data-testid="orders"
    />
  );
}

const channels = [
  { quarter: 'Q1', online: 42, store: 31 }, { quarter: 'Q2', online: 48, store: 29 },
  { quarter: 'Q3', online: 55, store: 33 }, { quarter: 'Q4', online: 67, store: 38 },
];
const thousands = (value: number) => `${value < 0 ? '-' : ''}$${Math.abs(value)}k`;

/** Two series side by side; the legend shows and hides each. */
export function Grouped() {
  const hidden = useLocal<readonly string[]>('hidden', []);
  return (
    <BarChart
      data={channels}
      x="quarter"
      y={[{ field: 'online', label: 'Online' }, { field: 'store', label: 'In store' }]}
      label="Sales by quarter and channel"
      title="Sales by channel"
      formatValue={thousands}
      summarize={summarize}
      hiddenSeries={hidden.value}
      onHiddenSeriesChange={hidden.set}
      data-testid="channels"
    />
  );
}

const cash = [
  { month: 'Jan', net: 12 }, { month: 'Feb', net: -5 }, { month: 'Mar', net: 8 }, { month: 'Apr', net: -11 },
  { month: 'May', net: 3 }, { month: 'Jun', net: 15 },
];

/** Values on both sides of zero stand on one line. */
export function Signed() {
  return <BarChart data={cash} x="month" y="net" label="Net cash flow by month" title="Net cash flow" formatValue={thousands} summarize={summarize} data-testid="cash" />;
}

const days = Array.from({ length: 30 }, (_, index) => ({ day: `${index + 1} Jun`, steps: 4200 + ((index * 37) % 11) * 610 + (index % 7 === 5 ? 3800 : 0) }));

/** Thirty bars at phone width: the bars narrow, and only the first and last are labelled. */
export function Month() {
  const day = useLocal<string | null>('day', '14 Jun');
  return <BarChart data={days} x="day" y="steps" label="Steps by day in June" title="Steps" description="June" selectedKey={day.value} onSelect={day.set} data-testid="steps" />;
}

export function Sizes() {
  return (
    <view className="gap-6">
      <BarChart data={week} x="day" y="orders" label="Orders by weekday, small" size="sm" xLabels="none" />
      <BarChart data={week} x="day" y="orders" label="Orders by weekday, large" size="lg" />
    </view>
  );
}

export function StateSelected() {
  return <BarChart data={week} x="day" y="orders" label="Orders by weekday" size="sm" selectedKey="Fri" data-testid="selected" />;
}

export function StateLoading() {
  return <BarChart data={[]} x="day" y="orders" label="Orders by weekday" title="Orders" size="sm" isLoading data-testid="loading" />;
}

export function StateEmpty() {
  return <BarChart data={[]} x="day" y="orders" label="Orders by weekday" title="Orders" size="sm" emptyLabel="No orders this week" emptyHint="New orders show here as they come in." data-testid="empty" />;
}

export function StateError() {
  return <BarChart data={week} x="day" y="orders" label="Orders by weekday" title="Orders" size="sm" error="Orders could not be loaded." onRetry={() => {}} retryLabel="Try again" data-testid="error" />;
}

/** The home tile. */
export function Specimen() {
  return <BarChart data={week} x="day" y="orders" label="Orders" size="xs" xLabels="none" className="w-28 self-center" />;
}
