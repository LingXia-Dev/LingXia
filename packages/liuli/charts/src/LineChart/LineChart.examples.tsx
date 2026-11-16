import { useLocal } from '@lingxia/view';
import { Held } from '@liulidesign/core/example';
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
      data={weight}
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

const visits = Array.from({ length: 30 }, (_, index) => ({ day: `${index + 1} Sep`, visits: Math.round(1800 + index * 46 + ((index * 53) % 17) * 38 - (index % 7 > 4 ? 520 : 0)) }));

/** An area is an amount, so it is measured from zero. */
export function Area() {
  return <LineChart data={visits} x="day" y="visits" label="Visits by day in September" title="Visits" description="September" hasArea summarize={summarize} data-testid="visits" />;
}

const day = [
  { hour: '00:00', c: 11.2 }, { hour: '02:00', c: 10.4 }, { hour: '04:00', c: 9.8 }, { hour: '06:00', c: 10.1 }, { hour: '08:00', c: 12.6 },
  { hour: '10:00', c: 15.9 }, { hour: '12:00', c: 18.7 }, { hour: '14:00', c: 20.3 }, { hour: '16:00', c: 19.6 }, { hour: '18:00', c: 17.0 },
  { hour: '20:00', c: 14.4 }, { hour: '22:00', c: 12.5 },
];
const degrees = (value: number) => `${value.toFixed(1)}°`;

/** Smoothing suits a quantity that changes gradually; the curve passes through every reading and never beyond one. */
export function Smooth() {
  return <LineChart data={day} x="hour" y="c" label="Temperature through the day" title="Temperature" description="Today" isSmooth formatValue={degrees} summarize={summarize} data-testid="day" />;
}

const plans = [
  { month: 'Jan', free: 410, team: 120, pro: 64 }, { month: 'Feb', free: 455, team: 131, pro: 71 }, { month: 'Mar', free: 440, team: 150, pro: 83 },
  { month: 'Apr', free: 492, team: 171, pro: 88 }, { month: 'May', free: 530, team: 204, pro: 97 }, { month: 'Jun', free: 561, team: 236, pro: 121 },
];
const series = [{ field: 'free', label: 'Free' }, { field: 'team', label: 'Team' }, { field: 'pro', label: 'Pro' }];

/** Three lines told apart by dash as well as colour; the legend shows and hides each. */
export function Series() {
  const hidden = useLocal<readonly string[]>('hidden', []);
  return (
    <LineChart
      data={plans}
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

/** Dragging across the plot, a mouse over it or the arrow keys choose the month the readout shows. */
export function Selected() {
  const month = useLocal<string | null>('month', 'Apr');
  return <LineChart data={plans} x="month" y={series} label="Accounts by plan and month" selectedKey={month.value} onSelect={month.set} legend="none" size="sm" data-testid="picked" />;
}

const readings = [
  { hour: '06:00', c: 14.2 }, { hour: '07:00', c: 15.1 }, { hour: '08:00', c: null }, { hour: '09:00', c: null }, { hour: '10:00', c: 19.4 },
  { hour: '11:00', c: 20.8 }, { hour: '12:00', c: 22.1 }, { hour: '13:00', c: null }, { hour: '14:00', c: 23.0 }, { hour: '15:00', c: 22.4 },
];

/** Hours without a reading are gaps, never zeros. */
export function Gaps() {
  return <LineChart data={readings} x="hour" y="c" label="Temperature by hour" title="Temperature" formatValue={degrees} summarize={summarize} data-testid="temperature" />;
}

export function Sizes() {
  return (
    <view className="gap-6">
      <LineChart data={weight} x="week" y="kg" label="Weight by week, small" size="sm" xLabels="none" />
      <LineChart data={weight} x="week" y="kg" label="Weight by week, large" size="lg" />
    </view>
  );
}

export function StateLoading() {
  return <LineChart data={[]} x="week" y="kg" label="Weight by week" title="Weight" size="sm" isLoading data-testid="loading" />;
}

export function StateEmpty() {
  return <LineChart data={[]} x="week" y="kg" label="Weight by week" title="Weight" size="sm" emptyLabel="No weigh-ins yet" emptyHint="Add one to start the line." data-testid="empty" />;
}

export function StateError() {
  return <Held value={true}>{(hasFailed, set) => <LineChart data={weight} x="week" y="kg" label="Weight by week" title="Weight" size="sm" error={hasFailed ? "Weigh-ins could not be loaded." : undefined} onRetry={() => set(false)} retryLabel="Try again" data-testid="error" />}</Held>;
}

export function StateSingle() {
  return <LineChart data={weight.slice(0, 1)} x="week" y="kg" label="Weight by week" title="Weight" size="sm" data-testid="single" />;
}

/** The home tile. */
export function Specimen() {
  return <LineChart data={weight} x="week" y="kg" label="Weight" size="xs" isSmooth xLabels="none" className="w-28 self-center" />;
}
