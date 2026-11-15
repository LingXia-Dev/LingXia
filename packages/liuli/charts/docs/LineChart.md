<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# LineChart

A number over time, as a line that fits its data or an area from zero, for up to five series. Status: preview.

```ts
import { LineChart } from '@liulidesign/charts/LineChart';
```

LineChart shows how a number moves: a weight over weeks, visits over a month, accounts by plan. A line fits the range of its data, so small changes show; an area is measured from zero, because its fill is an amount. Rows without a number are gaps, long series are reduced to the points that hold their shape, and several lines differ by dash as well as colour.

## Examples

### A line that fits its data

```tsx
import { summarize } from '@liulidesign/charts/summary';
import { LineChart } from '@liulidesign/charts/LineChart';

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
```

### An area, from zero

```tsx
import { summarize } from '@liulidesign/charts/summary';
import { LineChart } from '@liulidesign/charts/LineChart';

const visits = Array.from({ length: 30 }, (_, index) => ({ day: `${index + 1} Sep`, visits: Math.round(1800 + index * 46 + ((index * 53) % 17) * 38 - (index % 7 > 4 ? 520 : 0)) }));
const day = [
  { hour: '00:00', c: 11.2 }, { hour: '02:00', c: 10.4 }, { hour: '04:00', c: 9.8 }, { hour: '06:00', c: 10.1 }, { hour: '08:00', c: 12.6 },
  { hour: '10:00', c: 15.9 }, { hour: '12:00', c: 18.7 }, { hour: '14:00', c: 20.3 }, { hour: '16:00', c: 19.6 }, { hour: '18:00', c: 17.0 },
  { hour: '20:00', c: 14.4 }, { hour: '22:00', c: 12.5 },
];

/** An area is an amount, so it is measured from zero. */
export function Area() {
  return <LineChart data={visits} x="day" y="visits" label="Visits by day in September" title="Visits" description="September" hasArea summarize={summarize} data-testid="visits" />;
}
```

### A smoothed line

```tsx
import { summarize } from '@liulidesign/charts/summary';
import { LineChart } from '@liulidesign/charts/LineChart';

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
```

### Three series, with a legend that hides them

```tsx
import { useLocal } from '@lingxia/view';
import { summarize } from '@liulidesign/charts/summary';
import { LineChart } from '@liulidesign/charts/LineChart';

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
```

### Scrubbing chooses a row

```tsx
import { useLocal } from '@lingxia/view';
import { LineChart } from '@liulidesign/charts/LineChart';

const plans = [
  { month: 'Jan', free: 410, team: 120, pro: 64 }, { month: 'Feb', free: 455, team: 131, pro: 71 }, { month: 'Mar', free: 440, team: 150, pro: 83 },
  { month: 'Apr', free: 492, team: 171, pro: 88 }, { month: 'May', free: 530, team: 204, pro: 97 }, { month: 'Jun', free: 561, team: 236, pro: 121 },
];
const series = [{ field: 'free', label: 'Free' }, { field: 'team', label: 'Team' }, { field: 'pro', label: 'Pro' }];

/** Dragging across the plot, a mouse over it or the arrow keys choose the month the readout shows. */
export function Selected() {
  const month = useLocal<string | null>('month', 'Apr');
  return <LineChart data={plans} x="month" y={series} label="Accounts by plan and month" selectedKey={month.value} onSelect={month.set} legend="none" size="sm" data-testid="picked" />;
}
```

### Missing readings

```tsx
import { summarize } from '@liulidesign/charts/summary';
import { LineChart } from '@liulidesign/charts/LineChart';

const degrees = (value: number) => `${value.toFixed(1)}°`;
const readings = [
  { hour: '06:00', c: 14.2 }, { hour: '07:00', c: 15.1 }, { hour: '08:00', c: null }, { hour: '09:00', c: null }, { hour: '10:00', c: 19.4 },
  { hour: '11:00', c: 20.8 }, { hour: '12:00', c: 22.1 }, { hour: '13:00', c: null }, { hour: '14:00', c: 23.0 }, { hour: '15:00', c: 22.4 },
];

/** Hours without a reading are gaps, never zeros. */
export function Gaps() {
  return <LineChart data={readings} x="hour" y="c" label="Temperature by hour" title="Temperature" formatValue={degrees} summarize={summarize} data-testid="temperature" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `data` (required) | `readonly Readonly<Record<string, ChartDatum>>[]` |  | The rows, one per point in time, oldest first. |
| `x` (required) | `string` |  | The field that names each point: "month". |
| `y` (required) | `string \| readonly ChartSeries[]` |  | The field that holds the number, or up to five series drawn as separate lines. A row without a number is a gap. |
| `keyField` | `string` |  | The field that identifies a row across updates and in `selectedKey`. By default `x`. |
| `label` (required) | `string` |  | The chart's accessible name: "Weight by week". |
| `title` | `string` |  |  |
| `description` | `string` |  |  |
| `formatValue` | `((value: number) => string)` |  | Turns a number into the text shown and read. By default the bare number. |
| `summarize` | `ChartSummarize` |  | Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. |
| `isSmooth` | `boolean` | `false` | Rounds the line between points without passing above or below them. |
| `hasArea` | `boolean` | `false` | Fills under the line and measures it from zero. One series only. |
| `hasEndDot` | `boolean` | `true` | Marks each line's latest value with a dot. |
| `hasScale` | `boolean` |  | Shows the scale: round numbers at the start edge, each with a hairline across the plot. The range reaches just past the data, so the plot is filled. By default on, except at the `xs` size. |
| `hasEndLabel` | `boolean` | `false` | Writes the latest value at the end of the line. One series only. |
| `xLabels` | `LineChartLabels` |  | Category labels under the plot: `fit` writes as many as fit side by side, each under its point; `ends` the first and last. By default `fit`, except at the `xs` size. |
| `legend` | `ChartLegendPosition` | `'bottom'` | Shown when there is more than one series. |
| `legendLabel` | `string` |  | The legend's accessible name; by default the chart's `label`. |
| `selectedKey` | `string \| null` |  | The row to mark with a rule and a dot on each line, and to read out above the plot. |
| `onSelect` | `((key: string \| null) => void)` |  | Makes the plot a scrubber: a finger dragged across it, a tap, a mouse moving over it and the arrow keys choose the nearest row and run this with its key; a mouse leaving runs it with `null`. |
| `hiddenSeries` | `readonly string[]` | `[]` | Series fields not drawn at present. |
| `onHiddenSeriesChange` | `((fields: readonly string[]) => void)` |  | Makes each legend entry a button that shows or hides its series. |
| `domainMin` | `number` |  | Widens the value range downward. A line fits its data; an area starts from zero. |
| `domainMax` | `number` |  | Widens the value range upward, so charts that are compared share a scale. |
| `size` | `ChartSize` | `'md'` |  |
| `isLoading` | `boolean` | `false` |  |
| `error` | `string` |  |  |
| `emptyLabel` | `string` |  | What to say when no row has a number. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the chart. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| scale | `<view>` | no | Round numbers at the start edge, a few to a plot, fewer in a short one; hidden from assistive tech, which reads the values themselves. |
| plot | `<view>` | yes | The box the lines fill. |
| rule | `<view>` | yes | A hairline across the plot on each number of the scale, firmer under the values. The range ends just past the data, so the plot is filled. |
| end | `<view>` | no | The latest value, written at the end of the line, level with it. |
| drawing | `<vector>` | no | The lines, the fill, the end dots and the mark on the chosen row, as one drawing; hidden from assistive tech. |
| reading | `<view>` | no | Every row as "category: values", for assistive tech only; present up to 60 rows. |
| labels | `<view>` | no | Category names under the plot: as many as fit side by side, each under its point. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: busy (from isLoading).
- Left and Right: With `onSelect`, on the plot: chooses the previous or next row.
- With `summarize`, a sentence per series with its range, lowest and highest values and trend is read first.
- Then every row is read as a list item: "3 Nov: 71.2 kg", or "Apr: Free 492, Team 171, Pro 88". Past 60 rows only the summary is read.
- The second to fifth lines are dashed, each differently, and the legend draws the same dash, so series survive without colour.
- The scale and the end figure are hidden from assistive tech: the same numbers are in the summary and the list.
- The fill under an area, the hairlines and the rule on the chosen row are decoration with no contrast minimum.
- The lines are one drawing whatever the number of points, rebuilt only for a new `data` array or encoding; the chosen row's rule and dots are a second, small drawing, so a scrub never redraws the lines.
- With `onSelect` the plot is one keyboard stop named by `label`: Left and Right, and a screen reader's adjust, step through the rows, each read as "Apr: Free 492, Team 171, Pro 88". With nothing chosen it rests on the latest row.
- A finger chooses once it moves sideways or lifts, never as it lands, so a vertical drag that starts on the chart scrolls the page. A mouse chooses as it moves over the plot and clears as it leaves.
- The chosen row is shown in the readout line above the plot, which is announced; the line keeps its height when nothing is chosen.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| first mark colour | `chart-1` on `surface` | 3:1 | 5.62:1 | 6.46:1 |
| second mark colour | `chart-2` on `surface` | 3:1 | 4.29:1 | 7.22:1 |
| third mark colour | `chart-3` on `surface` | 3:1 | 4.36:1 | 7.61:1 |
| fourth mark colour | `chart-4` on `surface` | 3:1 | 5.81:1 | 7.12:1 |
| fifth mark colour | `chart-5` on `surface` | 3:1 | 5.58:1 | 8.32:1 |
| second mark colour, straight on the page | `chart-2` on `canvas` | 3:1 | 4.01:1 | 7.96:1 |
| third mark colour, straight on the page | `chart-3` on `canvas` | 3:1 | 4.08:1 | 8.40:1 |
| title and figures | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| description, labels and legend | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use a line for a level (a weight, a price, a rate) and `hasArea` for an amount that adds up (visits, sales).
- Do: Pass `null` or leave the field out for a missing reading.
- Do: Give charts that are compared the same `domainMin` and `domainMax`.
- Do: Pass `summarize`; past 60 rows it is all a screen reader gets.
- Don't: Draw more than three or four lines on a phone; five is the limit.
- Don't: Use a line for categories without an order; use BarChart.
- Don't: Smooth noisy data such as daily counts: a monotone curve flattens at every turn, which reads as wobble. Smooth what changes gradually.
- Don't: Fill missing readings with zero.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A tooltip beside the chosen point. | nothing from the View | A rule and dots on the chosen row, and its values in the readout line above the plot, where a finger does not cover them. |
| Fitted labels and scrubbing on a host built before `useBox` and `onPointer`. | a newer host | The first and last category; `selectedKey` set by the app. |
| A window to zoom and pan a long series. | pointer coordinates on an element (`onPointer`) | Pass the rows of the period to show. |
| Lines that draw in and move between data. | animation of vector geometry | New data is drawn at once. |
| Marker shapes per series. | a laid-out box's size, readable in render (`useBox`) | Dashes; a shape drawn in a stretched drawing would be distorted. |

Related: Chart, BarChart, Sparkline.
