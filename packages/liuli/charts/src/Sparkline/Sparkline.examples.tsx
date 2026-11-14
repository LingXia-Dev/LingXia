import { Sparkline } from './Sparkline';

const week = [12, 14, 13, 17, 16, 21, 24];
const month = [38, 41, 36, 44, 47, 43, 52, 55, 49, 58, 61, 57, 66, 70, 64, 73, 78, 71, 69, 80, 84, 79, 88, 91, 86, 95, 99, 93, 104, 108];
const falling = [31, 29, 30, 26, 27, 22, 19];
const net = [6, 9, -4, 3, -7, -2, 8, 11, -3, 5];

export function Trend() {
  return <Sparkline values={week} label="Revenue, last 7 days: up from 12k to 24k" hasEndDot data-testid="trend" />;
}

function Captioned({ caption, children }: { caption: string; children: JSX.Element }) {
  return (
    <view className="gap-1.5">
      {children}
      <text className="text-footnote text-muted">{caption}</text>
    </view>
  );
}

export function Variants() {
  return (
    <view className="gap-5">
      <Captioned caption="Line: the shape of a trend">
        <Sparkline values={month} label="Sign-ups, last 30 days: up from 38 to 108" data-testid="line" />
      </Captioned>
      <Captioned caption="Area: an amount, measured from zero">
        <Sparkline values={month} variant="area" label="Sign-ups, last 30 days: up from 38 to 108" data-testid="area" />
      </Captioned>
      <Captioned caption="Bar: separate counts, above and below zero">
        <Sparkline values={net} variant="bar" label="Net change by day: six days up, four down" data-testid="bar" />
      </Captioned>
    </view>
  );
}

export function Smooth() {
  return <Sparkline values={week} isSmooth variant="area" label="Sessions, last 7 days: up from 12k to 24k" data-testid="smooth" />;
}

/** The tone says whether the trend is good news; the label says it in words. */
export function Tones() {
  return (
    <view className="grid grid-cols-2 gap-x-4 gap-y-5">
      <Captioned caption="Series">
        <Sparkline values={week} label="Visits: up from 12k to 24k" />
      </Captioned>
      <Captioned caption="Neutral">
        <Sparkline values={week} tone="neutral" label="Visits: up from 12k to 24k" />
      </Captioned>
      <Captioned caption="Positive">
        <Sparkline values={week} tone="positive" label="Sales: up from 12k to 24k" data-testid="positive" />
      </Captioned>
      <Captioned caption="Negative">
        <Sparkline values={falling} tone="negative" label="Sales: down from 31k to 19k" data-testid="negative" />
      </Captioned>
    </view>
  );
}

export function Sizes() {
  return (
    <view className="gap-5">
      <Captioned caption="Small, 24">
        <Sparkline values={month} size="sm" label="Sign-ups: up from 38 to 108" data-testid="small" />
      </Captioned>
      <Captioned caption="Medium, 32">
        <Sparkline values={month} label="Sign-ups: up from 38 to 108" />
      </Captioned>
      <Captioned caption="Large, 48">
        <Sparkline values={month} size="lg" label="Sign-ups: up from 38 to 108" data-testid="large" />
      </Captioned>
    </view>
  );
}

/** Days without a reading are gaps, never zeros. */
export function Gaps() {
  return <Sparkline values={[18, 21, NaN, NaN, 24, 22, NaN, 27, NaN, 25, 29, 31]} label="Temperature by hour, with four hours missing" data-testid="gaps" />;
}

export function StateEmpty() {
  return <Sparkline values={[]} label="No readings yet" data-testid="empty" />;
}

export function StateSingle() {
  return <Sparkline values={[42]} label="One reading: 42" data-testid="single" />;
}

export function StateFlat() {
  return <Sparkline values={[8, 8, 8, 8, 8, 8]} label="Steady at 8" data-testid="flat" />;
}

export function StateMarked() {
  return <Sparkline values={falling} hasEndDot label="Down from 31 to 19" data-testid="marked" />;
}

const cities = [
  { name: 'Lisbon', now: '24°', values: [19, 20, 22, 24, 25, 24, 24], label: 'Lisbon, last 7 days: 19 to 24 degrees' },
  { name: 'Oslo', now: '9°', values: [12, 11, 11, 10, 8, 9, 9], label: 'Oslo, last 7 days: 12 to 9 degrees' },
  { name: 'Nairobi', now: '27°', values: [26, 27, 26, 28, 27, 27, 27], label: 'Nairobi, last 7 days: steady near 27 degrees' },
];

/** In a row, the sparkline sits between the name and the figure it explains. */
export function InRows() {
  return (
    <view role="list" aria-label="Cities" className="divide-y divide-text/8">
      {cities.map(city => (
        <view key={city.name} role="listitem" className="flex-row items-center gap-4 py-2.5">
          <text className="grow basis-0 text-body text-text">{city.name}</text>
          <Sparkline values={city.values} tone="neutral" size="sm" label={city.label} className="w-20 self-center" />
          <text className="w-[4ch] text-end text-body font-medium text-text tabular-nums">{city.now}</text>
        </view>
      ))}
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <Sparkline values={week} variant="area" isSmooth hasEndDot label="Trend" className="w-24 self-center" />;
}
