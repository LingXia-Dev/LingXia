import { useLocal } from '@lingxia/view';
import { Held } from '@liulidesign/core/internal/example';
import type { ChartSeries } from '../Chart/Chart';
import { summarize } from '../summary/en';
import { LineChart } from './LineChart';

const weight = [
  { week: '6 Oct', kg: 72.4 }, { week: '13 Oct', kg: 72.1 }, { week: '20 Oct', kg: 72.3 }, { week: '27 Oct', kg: 71.6 },
  { week: '3 Nov', kg: 71.2 }, { week: '10 Nov', kg: 71.4 }, { week: '17 Nov', kg: 70.8 }, { week: '24 Nov', kg: 70.5 },
];
const kilos = (value: number) => `${value.toFixed(1)} kg`;

/** A line fits its data: two kilos of change fill the plot. */
export function Weight() {
  return (
    <LineChart
      rows={weight}
      x="week"
      y="kg"
      label="Weight by week"
      title="Weight"
      description="Last 8 weeks"
      formatValue={kilos}
      summarize={summarize}
      hasEndLabel
      data-testid="weight"
    />
  );
}

/** A goal across the plot: the range widens to hold it, and its label names it with its value. */
export function Goal() {
  return (
    <LineChart
      rows={weight}
      x="week"
      y="kg"
      label="Weight by week"
      title="Weight"
      description="Last 8 weeks"
      formatValue={kilos}
      references={[{ value: 70, label: 'Goal' }]}
      data-testid="goal"
    />
  );
}

const visits = Array.from({ length: 30 }, (_, index) => ({ day: `${index + 1} Sep`, visits: Math.round(1800 + index * 46 + ((index * 53) % 17) * 38 - (index % 7 > 4 ? 520 : 0)) }));

/** An area is an amount, so it is measured from zero. */
export function Area() {
  return <LineChart rows={visits} x="day" y="visits" label="Visits by day in September" title="Visits" description="September" hasArea summarize={summarize} data-testid="visits" />;
}

const day = [
  { hour: '00:00', c: 11.2 }, { hour: '02:00', c: 10.4 }, { hour: '04:00', c: 9.8 }, { hour: '06:00', c: 10.1 }, { hour: '08:00', c: 12.6 },
  { hour: '10:00', c: 15.9 }, { hour: '12:00', c: 18.7 }, { hour: '14:00', c: 20.3 }, { hour: '16:00', c: 19.6 }, { hour: '18:00', c: 17.0 },
  { hour: '20:00', c: 14.4 }, { hour: '22:00', c: 12.5 },
];
const degrees = (value: number) => `${value.toFixed(1)}°`;

const seats = [
  { month: 'Jan', seats: 12 }, { month: 'Feb', seats: 12 }, { month: 'Mar', seats: 18 }, { month: 'Apr', seats: 18 },
  { month: 'May', seats: 25 }, { month: 'Jun', seats: 22 }, { month: 'Jul', seats: 30 },
];

const glucose = [
  { time: '07:00', mmol: 5.1 }, { time: '08:00', mmol: 8.6 }, { time: '09:00', mmol: 10.8 }, { time: '10:00', mmol: 7.9 },
  { time: '11:00', mmol: 6.2 }, { time: '12:00', mmol: 5.4 }, { time: '13:00', mmol: 9.4 }, { time: '14:00', mmol: 7.1 },
  { time: '15:00', mmol: 5.8 }, { time: '16:00', mmol: 3.6 }, { time: '17:00', mmol: 4.9 },
];

/** A reference with `to` is a zone: the range the readings should stay in, shaded under the line and named with both ends. */
export function Zone() {
  return (
    <LineChart
      rows={glucose}
      x="time"
      y="mmol"
      label="Blood glucose through the day, mmol/L"
      title="Glucose"
      description="Today, mmol/L"
      curve="smooth"
      formatValue={value => value.toFixed(1)}
      references={[{ value: 3.9, to: 10, label: 'Target' }]}
      summarize={summarize}
      data-testid="glucose"
    />
  );
}

const forecast = [
  { day: 'Mon', low: 6, mean: 10.5, high: 14 }, { day: 'Tue', low: 7, mean: 11.4, high: 16 }, { day: 'Wed', low: 9, mean: 13.8, high: 19 },
  { day: 'Thu', low: 8, mean: 12.1, high: 17 }, { day: 'Fri', low: 5, mean: 8.6, high: 12 }, { day: 'Sat', low: 4, mean: 8.9, high: 13 },
  { day: 'Sun', low: 6, mean: 11.2, high: 15 },
];

/** A band behind the line shades each day's range, low to high; a chosen day reads both. */
export function Band() {
  const day = useLocal<string | null>('day', null);
  return (
    <LineChart
      rows={forecast}
      x="day"
      y={[{ field: 'mean', label: 'Mean' }]}
      band={{ low: 'low', high: 'high', label: 'Range' }}
      curve="smooth"
      label="Temperature this week, mean and daily range"
      title="Temperature"
      description="This week"
      formatValue={degrees}
      bind={day}
      data-testid="forecast"
    />
  );
}

const checkups = [
  { date: '3 Jan', day: 2, kg: 74.8 }, { date: '17 Jan', day: 16, kg: 74.1 }, { date: '2 Mar', day: 60, kg: 72.9 },
  { date: '9 Mar', day: 67, kg: 72.6 }, { date: '20 May', day: 139, kg: 71.4 }, { date: '1 Sep', day: 243, kg: 70.2 },
];

/** Readings taken on uneven days: `xValue` places each by its day, so a long gap between visits shows as one. */
export function Spaced() {
  const visit = useLocal<string | null>('visit', null);
  return (
    <LineChart
      rows={checkups}
      x="date"
      xValue="day"
      y="kg"
      label="Weight at each check-up this year"
      title="Weight"
      description="At each check-up"
      formatValue={kilos}
      bind={visit}
      data-testid="spaced"
    />
  );
}

/** Steps suit a count that changes at set moments and holds between them: seats on a plan, a price, a limit. */
export function Steps() {
  return <LineChart rows={seats} x="month" y="seats" label="Seats on the plan by month" title="Seats" description="This year" curve="step" hasArea summarize={summarize} data-testid="seats" />;
}

/** Smoothing suits a quantity that changes gradually; the curve passes through every reading and never beyond one. */
export function Smooth() {
  return <LineChart rows={day} x="hour" y="c" label="Temperature through the day" title="Temperature" description="Today" curve="smooth" formatValue={degrees} summarize={summarize} data-testid="day" />;
}

const plans = [
  { month: 'Jan', free: 410, team: 120, pro: 64 }, { month: 'Feb', free: 455, team: 131, pro: 71 }, { month: 'Mar', free: 440, team: 150, pro: 83 },
  { month: 'Apr', free: 492, team: 171, pro: 88 }, { month: 'May', free: 530, team: 204, pro: 97 }, { month: 'Jun', free: 561, team: 236, pro: 121 },
];
const series: readonly ChartSeries<(typeof plans)[number]>[] = [{ field: 'free', label: 'Free' }, { field: 'team', label: 'Team' }, { field: 'pro', label: 'Pro' }];

/** Three lines told apart by dash as well as colour; the legend shows and hides each. */
export function Series() {
  const hidden = useLocal<readonly string[]>('hidden', []);
  return (
    <LineChart
      rows={plans}
      x="month"
      y={series}
      label="Accounts by plan and month"
      title="Accounts by plan"
      summarize={summarize}
      hiddenSeries={hidden.value}
      onHiddenSeriesChange={hidden.set}
      data-testid="plans"
    />
  );
}

/** The same plans stacked: each band sits on the ones below, so the top line is all accounts; choosing a month reads each plan's own count. */
export function Stacked() {
  const month = useLocal<string | null>('month', null);
  return (
    <LineChart
      rows={plans}
      x="month"
      y={series}
      layout="stacked"
      curve="smooth"
      label="Accounts by plan and month, stacked"
      title="All accounts"
      summarize={summarize}
      bind={month}
      data-testid="stacked"
    />
  );
}

/** Dragging across the plot, a mouse over it or the arrow keys choose the month the readout shows. */
export function Selected() {
  const month = useLocal<string | null>('month', 'Apr');
  return <LineChart rows={plans} x="month" y={series} label="Accounts by plan and month" bind={month} legendPosition="none" size="sm" data-testid="picked" />;
}

const readings = [
  { hour: '06:00', c: 14.2 }, { hour: '07:00', c: 15.1 }, { hour: '08:00', c: null }, { hour: '09:00', c: null }, { hour: '10:00', c: 19.4 },
  { hour: '11:00', c: 20.8 }, { hour: '12:00', c: 22.1 }, { hour: '13:00', c: null }, { hour: '14:00', c: 23.0 }, { hour: '15:00', c: 22.4 },
];

/** Hours without a reading are gaps, never zeros. */
export function Gaps() {
  return <LineChart rows={readings} x="hour" y="c" label="Temperature by hour" title="Temperature" formatValue={degrees} summarize={summarize} data-testid="temperature" />;
}

export function Sizes() {
  return (
    <view className="gap-6">
      <LineChart rows={weight} x="week" y="kg" label="Weight by week, small" size="sm" xLabels="none" />
      <LineChart rows={weight} x="week" y="kg" label="Weight by week, large" size="lg" />
    </view>
  );
}

export function StateLoading() {
  return <LineChart rows={[]} x="week" y="kg" label="Weight by week" title="Weight" size="sm" isLoading data-testid="loading" />;
}

export function StateEmpty() {
  return <LineChart rows={[]} x="week" y="kg" label="Weight by week" title="Weight" size="sm" emptyLabel="No weigh-ins yet" emptyHint="Add one to start the line." data-testid="empty" />;
}

export function StateError() {
  return <Held value={true}>{(hasFailed, set) => <LineChart rows={weight} x="week" y="kg" label="Weight by week" title="Weight" size="sm" error={hasFailed ? "Weigh-ins could not be loaded." : undefined} onRetry={() => set(false)} retryLabel="Try again" data-testid="error" />}</Held>;
}

export function StateSingle() {
  return <LineChart rows={weight.slice(0, 1)} x="week" y="kg" label="Weight by week" title="Weight" size="sm" data-testid="single" />;
}

/** The home tile. */
export function Specimen() {
  return <LineChart rows={weight} x="week" y="kg" label="Weight" size="xs" curve="smooth" xLabels="none" className="w-28 self-center" />;
}
