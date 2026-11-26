import { useLocal } from '@lingxia/view';
import { Held } from '@liulidesign/core/internal/example';
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
      rows={week}
      x="day"
      y="orders"
      label="Orders by weekday"
      title="Orders"
      description="This week"
      summarize={summarize}
      bind={day}
      data-testid="orders"
    />
  );
}

/** A target and the week's average across the bars; each label keeps clear of the other and of the figures. */
export function Target() {
  return (
    <BarChart
      rows={week}
      x="day"
      y="orders"
      label="Orders by weekday"
      title="Orders"
      description="This week"
      references={[{ value: 160, label: 'Target' }, { value: 132, label: 'Average' }]}
      data-testid="target"
    />
  );
}

const sleep = [
  { night: 'Mon', hours: 6.4 }, { night: 'Tue', hours: 7.2 }, { night: 'Wed', hours: 7.8 }, { night: 'Thu', hours: 5.9 },
  { night: 'Fri', hours: 8.3 }, { night: 'Sat', hours: 9.1 }, { night: 'Sun', hours: 7.5 },
];

/** A zone behind the bars: the range to stay in, named with both ends. */
export function Zone() {
  return (
    <BarChart
      rows={sleep}
      x="night"
      y="hours"
      label="Hours slept by night"
      title="Sleep"
      description="This week, hours"
      formatValue={value => value.toFixed(1)}
      references={[{ value: 7, to: 9, label: 'Goal' }]}
      data-testid="sleep"
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
      rows={channels}
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

/** The same two channels as one bar per quarter: the bar is the quarter's total, each channel a piece of it. */
export function Stacked() {
  return (
    <BarChart
      rows={channels}
      x="quarter"
      y={[{ field: 'online', label: 'Online' }, { field: 'store', label: 'In store' }]}
      layout="stacked"
      label="Sales by quarter, online and in store"
      title="Sales"
      formatValue={thousands}
      summarize={summarize}
      data-testid="stacked"
    />
  );
}

const cash = [
  { month: 'Jan', net: 12 }, { month: 'Feb', net: -5 }, { month: 'Mar', net: 8 }, { month: 'Apr', net: -11 },
  { month: 'May', net: 3 }, { month: 'Jun', net: 15 },
];

/** Values on both sides of zero stand on one line. */
export function Signed() {
  return <BarChart rows={cash} x="month" y="net" label="Net cash flow by month" title="Net cash flow" formatValue={thousands} summarize={summarize} data-testid="cash" />;
}

/** Each bar takes the tone of what its value means: a gain, a loss. The figures keep their signs, so colour is not the only cue. */
export function Tones() {
  return (
    <BarChart
      rows={cash}
      x="month"
      y="net"
      toneOf={row => (row.net < 0 ? 'negative' : 'positive')}
      label="Net cash flow by month"
      title="Net cash flow"
      formatValue={thousands}
      summarize={summarize}
      data-testid="tones"
    />
  );
}

/** One bar picked out in another colour, the rest stepped back: the latest week. */
export function Highlight() {
  return (
    <BarChart
      rows={week}
      x="day"
      y="orders"
      toneOf={(_, index) => (index === week.length - 1 ? 1 : 'neutral')}
      label="Orders by weekday, today picked out"
      title="Orders"
      description="Today against the week"
      summarize={summarize}
      data-testid="highlight"
    />
  );
}

const apps = [
  { app: 'Messages', minutes: 94 }, { app: 'Maps and navigation', minutes: 61 }, { app: 'Photos', minutes: 38 },
  { app: 'Music', minutes: 27 }, { app: 'Calendar', minutes: 12 },
];
const minutes = (value: number) => `${Math.floor(value / 60) ? `${Math.floor(value / 60)} h ` : ''}${value % 60} min`;

/** A ranked list: each name over its bar, with room for long names; tap a line to choose it. */
export function Horizontal() {
  const app = useLocal<string | null>('app', null);
  return (
    <BarChart rows={apps} x="app" y="minutes" orientation="horizontal" label="Screen time by app" title="Screen time" description="Today" formatValue={minutes} summarize={summarize} bind={app} data-testid="apps" />
  );
}

/** Stacked and lying down: each quarter's total at the end of its line, the channels as pieces of its bar. */
export function HorizontalStacked() {
  return (
    <BarChart
      rows={channels}
      x="quarter"
      y={[{ field: 'online', label: 'Online' }, { field: 'store', label: 'In store' }]}
      layout="stacked"
      orientation="horizontal"
      label="Sales by quarter, online and in store"
      title="Sales"
      formatValue={thousands}
      data-testid="lying-stacked"
    />
  );
}

/** A limit across lying bars: a mark through each bar it crosses, named once under the lines. */
export function HorizontalLimit() {
  return (
    <BarChart
      rows={apps}
      x="app"
      y="minutes"
      orientation="horizontal"
      references={[{ value: 60, label: 'Daily limit' }]}
      label="Screen time by app, against a daily limit"
      title="Screen time"
      description="Today"
      formatValue={minutes}
      summarize={summarize}
      data-testid="apps-limit"
    />
  );
}

/** Grouped and lying down: with no figure on each line, the bars read against a scale under them. */
export function HorizontalGrouped() {
  return (
    <BarChart
      rows={channels}
      x="quarter"
      y={[{ field: 'online', label: 'Online' }, { field: 'store', label: 'In store' }]}
      orientation="horizontal"
      label="Sales by quarter, online and in store"
      title="Sales"
      formatValue={thousands}
      data-testid="lying-grouped"
    />
  );
}

const days = Array.from({ length: 30 }, (_, index) => ({ day: `${index + 1} Jun`, steps: 4200 + ((index * 37) % 11) * 610 + (index % 7 === 5 ? 3800 : 0) }));

/** Thirty bars at phone width: the bars narrow, and only the first and last are labelled. */
export function Month() {
  const day = useLocal<string | null>('day', '14 Jun');
  return <BarChart rows={days} x="day" y="steps" label="Steps by day in June" title="Steps" description="June" bind={day} data-testid="steps" />;
}

export function Sizes() {
  return (
    <view className="gap-6">
      <BarChart rows={week} x="day" y="orders" label="Orders by weekday, small" size="sm" xLabels="none" />
      <BarChart rows={week} x="day" y="orders" label="Orders by weekday, large" size="lg" />
    </view>
  );
}

export function StateSelected() {
  return <BarChart rows={week} x="day" y="orders" label="Orders by weekday" size="sm" value="Fri" data-testid="selected" />;
}

export function StateLoading() {
  return <BarChart rows={[]} x="day" y="orders" label="Orders by weekday" title="Orders" size="sm" isLoading data-testid="loading" />;
}

export function StateEmpty() {
  return <BarChart rows={[]} x="day" y="orders" label="Orders by weekday" title="Orders" size="sm" emptyLabel="No orders this week" emptyHint="New orders show here as they come in." data-testid="empty" />;
}

export function StateError() {
  return <Held value={true}>{(hasFailed, set) => <BarChart rows={week} x="day" y="orders" label="Orders by weekday" title="Orders" size="sm" error={hasFailed ? "Orders could not be loaded." : undefined} onRetry={() => set(false)} retryLabel="Try again" data-testid="error" />}</Held>;
}

/** The home tile. */
export function Specimen() {
  return <BarChart rows={week} x="day" y="orders" label="Orders" size="xs" xLabels="none" className="w-28 self-center" />;
}
