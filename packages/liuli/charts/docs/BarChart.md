<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# BarChart

Amounts by category, as bars on a zero line, with selection and grouped series. Status: preview.

```ts
import { BarChart } from '@liulidesign/charts/BarChart';
```

BarChart compares amounts across a few categories: orders by weekday, sales by quarter. It takes rows and the names of the fields to draw, so the same rows can feed another chart or a table. Each category can be pressed to read its value, several series stand side by side, and values below zero hang from the same line.

## Examples

### One series, with selection

```tsx
import { useLocal } from '@lingxia/view';
import { summarize } from '@liulidesign/charts/summary';
import { BarChart } from '@liulidesign/charts/BarChart';

const week = [
  { day: 'Mon', orders: 132 }, { day: 'Tue', orders: 148 }, { day: 'Wed', orders: 121 }, { day: 'Thu', orders: 163 },
  { day: 'Fri', orders: 187 }, { day: 'Sat', orders: 96 }, { day: 'Sun', orders: 74 },
];

/** Tap a bar to read its value above the plot; tap it again to let go. */
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
```

### Two series, with a legend that hides them

```tsx
import { useLocal } from '@lingxia/view';
import { summarize } from '@liulidesign/charts/summary';
import { BarChart } from '@liulidesign/charts/BarChart';

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
```

### Values above and below zero

```tsx
import { summarize } from '@liulidesign/charts/summary';
import { BarChart } from '@liulidesign/charts/BarChart';

const thousands = (value: number) => `${value < 0 ? '-' : ''}$${Math.abs(value)}k`;
const cash = [
  { month: 'Jan', net: 12 }, { month: 'Feb', net: -5 }, { month: 'Mar', net: 8 }, { month: 'Apr', net: -11 },
  { month: 'May', net: 3 }, { month: 'Jun', net: 15 },
];

/** Values on both sides of zero stand on one line. */
export function Signed() {
  return <BarChart data={cash} x="month" y="net" label="Net cash flow by month" title="Net cash flow" formatValue={thousands} summarize={summarize} data-testid="cash" />;
}
```

### Thirty bars

```tsx
import { useLocal } from '@lingxia/view';
import { BarChart } from '@liulidesign/charts/BarChart';

const days = Array.from({ length: 30 }, (_, index) => ({ day: `${index + 1} Jun`, steps: 4200 + ((index * 37) % 11) * 610 + (index % 7 === 5 ? 3800 : 0) }));

/** Thirty bars at phone width: the bars narrow, and only the first and last are labelled. */
export function Month() {
  const day = useLocal<string | null>('day', '14 Jun');
  return <BarChart data={days} x="day" y="steps" label="Steps by day in June" title="Steps" description="June" selectedKey={day.value} onSelect={day.set} data-testid="steps" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `data` (required) | `readonly Readonly<Record<string, ChartDatum>>[]` |  | The rows, one per category, in the order drawn. |
| `x` (required) | `string` |  | The field that names each category: "month". |
| `y` (required) | `string \| readonly ChartSeries[]` |  | The field that holds the number, or several series to draw side by side in each category. |
| `keyField` | `string` |  | The field that identifies a row across updates and in `selectedKey`. By default `x`. |
| `label` (required) | `string` |  | The chart's accessible name: "Orders by weekday". |
| `title` | `string` |  |  |
| `description` | `string` |  |  |
| `formatValue` | `((value: number) => string)` |  | Turns a number into the text shown and read: units, separators, the locale. By default the bare number. |
| `summarize` | `ChartSummarize` |  | Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. |
| `xLabels` | `BarChartLabels` |  | Category labels under the bars: every one, or the first and last. By default every one up to seven bars. |
| `valueLabels` | `BarChartValueLabels` | `'auto'` | How the chart shows its scale. `auto` writes each value over its bar up to eight bars of one series, and otherwise the top of the scale, a round number, above the plot; `always` writes every value; `none` shows neither. |
| `legend` | `ChartLegendPosition` | `'bottom'` | Shown when there is more than one series. |
| `legendLabel` | `string` |  | The legend's accessible name; by default the chart's `label`. |
| `selectedKey` | `string \| null` |  | The chosen row's key; its bars keep their colour and the rest step back. |
| `onSelect` | `((key: string \| null) => void)` |  | Makes each category a button; runs with its key, or `null` when the chosen one is pressed again. Up to 36 categories. |
| `hiddenSeries` | `readonly string[]` | `[]` | Series fields not drawn at present. |
| `onHiddenSeriesChange` | `((fields: readonly string[]) => void)` |  | Makes each legend entry a button that shows or hides its series. |
| `domainMin` | `number` |  | Widens the value range downward; zero is always in range. |
| `domainMax` | `number` |  | Widens the value range upward, so charts that are compared share a scale. |
| `size` | `ChartSize` | `'md'` |  |
| `isLoading` | `boolean` | `false` |  |
| `error` | `string` |  |  |
| `emptyLabel` | `string` |  | What to say when no row has a number. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the chart; a bar's button is `<id>-<key>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| scale | `<text>` | no | The top of the scale, a round number, above a hairline; shown when the bars carry no figures. |
| value | `<text>` | no | A bar's value, written over it (under it, below zero). |
| plot | `<view>` | yes | The box the marks and the slots share. |
| drawing | `<vector>` | yes | The bars: every bar of a series is one outline, rounded on its free end; one drawing above zero and one below, and a fainter pair for bars that stepped back. Hidden from assistive tech. |
| baseline | `<view>` | yes | The zero line, the chart's only rule. |
| slots | `<view>` | yes | The row of categories over the drawing; a list for assistive tech when the bars cannot be pressed. |
| slot | `<button>` | yes | One category at full height: its name and values for assistive tech, its press target, and its figure when bars carry them. |
| labels | `<view>` | no | Category names under the bars: all of them, or the first and last. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: pressed (from selectedKey); busy (from isLoading).
- Tab: Moves through the categories when they can be pressed.
- Enter or Space: Chooses the category, or lets it go.
- Each category is read as its name and values: "Thu: 163", or "Q3: Online $55k, In store $33k". With `onSelect` each is a button that reports pressed; without it the plot is a list.
- With `summarize`, a sentence with the range, the lowest and highest values and the trend is read before the bars.
- Choosing a category is reported as pressed on its button. Where the bars carry no figures (grouped bars, more than eight bars) a readout line above the plot shows the chosen category and announces it; where every bar carries its value there is no readout line.
- Figures over the bars and the scale label are hidden from assistive tech: each category already reads its values.
- The chosen category keeps its colour and the others step back to 30%, which is not a contrast claim: the pressed state and the readout say which is chosen.
- A category's press target is its full-height slot, as wide as the plot divided by the number of categories: 41 px for seven bars at 360 pt, under 10 px for thirty. Past 36 categories the bars are not buttons.
- The zero line is a quiet rule with no contrast minimum: the bars' own feet show where zero is.
- A chart is one element per category and about a dozen more, whatever the number of bars; the bars themselves are two to four drawings.
- Series in a group differ by colour and by position, and the legend lists them in the order drawn.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| first series, as a bar | `chart-1/85` on `surface` | 3:1 | 4.20:1 | 5.05:1 |
| second series, as a bar | `chart-2/85` on `surface` | 3:1 | 3.51:1 | 5.54:1 |
| third series, as a bar | `chart-3/85` on `surface` | 3:1 | 3.40:1 | 5.85:1 |
| fourth series, as a bar | `chart-4/85` on `surface` | 3:1 | 4.40:1 | 5.50:1 |
| fifth series, as a bar | `chart-5/85` on `surface` | 3:1 | 4.16:1 | 6.36:1 |
| second series, straight on the page | `chart-2/85` on `canvas` | 3:1 | 3.33:1 | 6.01:1 |
| third series, straight on the page | `chart-3/85` on `canvas` | 3:1 | 3.22:1 | 6.34:1 |
| title and figures | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| description, labels and legend | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Pass rows as they are and name the fields: `x="day" y="orders"`. Pass a new array when the data changes; the bars are rebuilt only then.
- Do: Format values with `formatValue`, in the person's locale and units, and keep them short: a figure over a bar has the bar's width.
- Do: Pass `summarize` so a screen reader gets the finding first.
- Do: Keep to about twelve categories on a phone when each should be pressed.
- Do: Give charts that are compared the same `domainMax`.
- Don't: Use bars for a trend over many points in time; use LineChart.
- Don't: Draw more than three series side by side on a phone.
- Don't: Colour single-series bars differently from each other; colour says which series.
- Don't: Start a bar anywhere but zero; the chart does not let you.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Ticks between zero and the top of the scale, and labels for every category that fit. | a laid-out box's size, readable in render (`useBox`) | Each value over its bar up to eight bars, the top of the scale beyond; all category labels up to seven bars, the first and last beyond. |
| Scrubbing across bars, and a tooltip by the bar. | pointer coordinates on an element (`onPointer`) | A press on a bar; the value is over the bar, or in the readout above the plot. |
| Stacked bars and horizontal bars. | nothing from the View | Grouped vertical bars. |
| Series told apart without colour. | pattern fills | Position within the group, the legend's order, and each bar's spoken name. |

Related: Chart, LineChart, PieChart, Sparkline.
