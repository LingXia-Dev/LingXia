<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Chart

The frame every chart sits in: heading, plot, legend, readout and the loading, empty and error states. Status: preview.

```ts
import { Chart, ChartLegend, ChartReadout } from '@liulidesign/charts/Chart';
```

Chart is what BarChart, LineChart and PieChart share, and the frame for a plot of your own. It gives the plot a fixed height, names the chart for assistive tech, reads a summary sentence, and swaps the plot for a skeleton, a message or an error. ChartLegend and ChartReadout, from the same import, are the legend and the line that shows the chosen datum.

## Examples

### A plot of your own in the frame

```tsx
import { cx } from '@lingxia/view';
import { Chart, ChartLegend } from '@liulidesign/charts/Chart';

const budget = [
  { key: 'spent', label: 'Spent', amount: 6200, color: 1 as const },
  { key: 'committed', label: 'Committed', amount: 2300, color: 2 as const },
  { key: 'left', label: 'Left', amount: 1500, color: 3 as const },
];
const fills = { 1: 'bg-chart-1', 2: 'bg-chart-2', 3: 'bg-chart-3' } as const;
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
        {budget.map(part => <view key={part.key} className={cx('h-6 basis-0 rounded-[3px]', fills[part.color])} style={{ flexGrow: percent(part.amount, budget) }} />)}
      </view>
    </Chart>
  );
}
```

### Eight series colours

```tsx
import { cx } from '@lingxia/view';
import { Chart, ChartLegend } from '@liulidesign/charts/Chart';

const percent = (amount: number, parts: readonly { amount: number }[]) => Math.round((amount / parts.reduce((sum, part) => sum + part.amount, 0)) * 100);
const dollars = (value: number) => `$${String(value).replace(/\B(?=(\d{3})+(?!\d))/g, ',')}`;
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
  1: 'bg-chart-1', 2: 'bg-chart-2', 3: 'bg-chart-3', 4: 'bg-chart-4', 5: 'bg-chart-5', 6: 'bg-hue-3', 7: 'bg-hue-5', 8: 'bg-hue-4',
} as const;

/** Eight series colours: the five chart roles, then three of the theme's hues. Past eight, merge the rest. */
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
        {spending.map(part => <view key={part.key} className={cx('h-6 basis-0 rounded-[3px]', eight[part.color])} style={{ flexGrow: percent(part.amount, spending) }} data-testid={`part-${part.key}`} />)}
      </view>
    </Chart>
  );
}
```

### Legends for bars and for lines

```tsx
import { useLocal } from '@lingxia/view';
import { ChartLegend } from '@liulidesign/charts/Chart';

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
```

### The readout line

```tsx
import { ChartReadout } from '@liulidesign/charts/Chart';

export function Readout() {
  return (
    <view className="gap-3">
      <ChartReadout label="Wednesday" values={[{ text: '121' }]} data-testid="one" />
      <ChartReadout label="Q3" values={[{ label: 'Online', text: '$55k', color: 1 }, { label: 'In store', text: '$33k', color: 2 }]} data-testid="two" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The chart's accessible name: what it shows ("Revenue by month"). Drawn only as `title`. |
| `title` | `string` |  | The heading above the plot. |
| `description` | `string` |  | One line under the title: the period, the unit. |
| `summary` | `string` |  | The chart's finding in sentences, read after the name and not drawn. |
| `readout` | `Child` |  | The selected datum, shown above the plot; a `ChartReadout`. |
| `children` | `Child` |  | The plot: marks drawn to fill the box. |
| `axis` | `Child` |  | What sits under the plot and belongs to it: the category labels. |
| `legend` | `Child` |  | A `ChartLegend`. |
| `legendPosition` | `ChartLegendPosition` | `'bottom'` |  |
| `size` | `ChartPlotSize` | `'md'` | The plot's height: 64, 112, 176 or 240, or `fit` to take the height of the plot's own content. `xs` is for a tile or a row, with nothing but the marks. |
| `isLoading` | `boolean` | `false` | Stands a skeleton in for the plot; the chart is reported busy. |
| `plotShape` | `ChartPlotShape` | `'block'` | What the plot is: the loading skeleton takes its outline and the empty state its mark, so a chart with nothing to draw still looks like itself. |
| `error` | `string` |  | Why the data could not be shown; drawn in place of the plot with an error mark, and announced. |
| `onRetry` | `(() => void)` |  | With `error`: tries again. Pass `retryLabel` with it. |
| `retryLabel` | `string` |  | The words on the retry button: "Try again". |
| `isEmpty` | `boolean` | `false` | Shows `emptyLabel` in place of the plot. |
| `emptyLabel` | `string` |  | What to say when there is nothing to draw: "No sales yet". |
| `emptyHint` | `string` |  | A second, quieter line under `emptyLabel`: what to do next. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the chart. |

## ChartLegend props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The list's accessible name: "Series", "Parts". |
| `items` (required) | `readonly ChartLegendItem[]` |  |  |
| `variant` | `ChartLegendVariant` | `'inline'` | `inline` wraps entries on a line; `list` gives each entry a row with its figures. |
| `mark` | `"line" \| "square"` | `'square'` | `line` draws each swatch as a stroke with the entry's dash. |
| `selectedKey` | `string \| null` |  | The entry shown as chosen. Lists only. |
| `onItemPress` | `((key: string) => void)` |  | Makes each entry a button and runs with its key. |
| `pressedState` | `"selected" \| "shown"` | `'selected'` | What a pressed entry means to assistive tech: `selected` picks one, `shown` toggles a series. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## ChartReadout props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` | `string` |  | The datum's category: "Wednesday". Leave it out to keep the line's height with nothing in it. |
| `values` | `readonly ChartReadoutValue[]` | `[]` |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The group, named by `label`. |
| title | `<text>` | no | The heading; hidden from assistive tech when it repeats `label`. |
| description | `<text>` | no | One line under the title. |
| summary | `<text>` | no | The finding in sentences; read, not drawn. |
| readout | `<view>` | no | The chosen datum, on a line that keeps its height when nothing is chosen, and announced when it changes. |
| plot | `<view>` | yes | The box the marks fill, at the height `size` sets; it holds the skeleton or the message in the other states. |
| axis | `<view>` | no | Category labels under the plot; hidden from assistive tech, since each mark is read with its category. |
| legend | `<view>` | no | The list of series or parts. |
| legendItem | `<button>` | no | One entry: a swatch of its mark and its name; a button when the legend can be pressed. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on ChartLegend: classes merged after the recipe's, so they win.
- The recipe, `chartRecipe` from `@liulidesign/charts/Chart/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-body`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `chart-1`, `chart-2`, `chart-3`, `chart-4`, `chart-5`, `danger`, `hue-3`, `hue-4`, `hue-5`, `line-strong`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: busy (from isLoading); pressed (from ChartLegend selectedKey, or an entry that is shown).
- Tab, Enter or Space: Reach a legend entry that is a button, and press it.
- Press target: md 36 px.
- A group named by `label`, read in order: the title when it differs from the name, the description, the summary, the readout, the marks, the legend.
- The readout is a status region: choosing a datum announces its category and figures.
- The legend is a list. An entry that hides a series reports pressed while the series is shown; an entry that selects reports pressed while chosen.
- An error is an alert and is announced when it appears. While loading the group is busy and the skeleton is hidden.
- Legend swatches repeat the mark: a square for bars and slices, a stroke with the line's dash for lines, so a series is never named by colour alone.
- An inline legend entry is 18 px tall; on touch, prefer the list variant (36 px rows) when entries can be pressed.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| first mark colour | `chart-1` on `surface` | 3:1 | 5.62:1 | 6.46:1 |
| second mark colour | `chart-2` on `surface` | 3:1 | 4.29:1 | 7.22:1 |
| third mark colour | `chart-3` on `surface` | 3:1 | 4.36:1 | 7.61:1 |
| fourth mark colour | `chart-4` on `surface` | 3:1 | 5.81:1 | 7.12:1 |
| fifth mark colour | `chart-5` on `surface` | 3:1 | 5.58:1 | 8.32:1 |
| sixth mark colour | `hue-3` on `surface` | 3:1 | 7.64:1 | 5.28:1 |
| seventh mark colour | `hue-5` on `surface` | 3:1 | 8.24:1 | 8.12:1 |
| eighth mark colour | `hue-4` on `surface` | 3:1 | 4.87:1 | 10.50:1 |
| second mark colour, straight on the page | `chart-2` on `canvas` | 3:1 | 4.01:1 | 7.96:1 |
| third mark colour, straight on the page | `chart-3` on `canvas` | 3:1 | 4.08:1 | 8.40:1 |
| title and figures | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| description, labels and legend | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| a legend row under the pointer | `text` on `text/4` over `surface` | 4.5:1 | 15.06:1 | 12.59:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use BarChart, LineChart or PieChart when one fits; use Chart to frame a plot they do not cover.
- Do: Say what the chart shows in `label`, and its finding in `summary`.
- Do: Hide a plot of your own from assistive tech and give its values as a legend list or a summary.
- Do: Give every state its words: `emptyLabel` and `error` come from the app.
- Don't: Draw a border, a background or grid lines around the plot; the card it sits in is the frame.
- Don't: Put a legend on a chart with one series.
- Don't: Use colours outside the eight series colours for marks.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A value axis with ticks, and fitted labels for a plot of your own. | nothing from the View | BarChart and LineChart fit their category labels and write the ends of their scale; a plot of your own passes its labels as `axis`. |
| A tooltip that follows a finger or a pointer. | nothing from the View | The readout line above the plot, which BarChart and LineChart fill as they are scrubbed. |
| Marks that move between two sets of data. | animation of vector geometry | Bars change height through layout transitions; lines and slices change at once. |

Related: BarChart, LineChart, PieChart, Sparkline.
