<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# PieChart

Parts of one whole, as a ring or a solid pie, listed with their amounts in a legend that selects. Status: preview.

```ts
import { PieChart } from '@liulidesign/charts/PieChart';
```

PieChart shows how one total divides into a few parts: spending by category, storage by kind. Its `nameField`, `valueField` and `keyField` are fields of the rows' own type (`Row`), so a misspelt one fails to compile. By default it is a ring with the total in its hole. The parts are named in a legend under it, with their amounts and shares, and a legend row chooses its part: the slice grows, the others step back, and the hole shows the chosen part.

## Examples

### A ring with the total, selected through its legend

```tsx
import { useLocal } from '@lingxia/view';
import { summarize } from '@liulidesign/charts/summary';
import { PieChart } from '@liulidesign/charts/PieChart';

const spending = [
  { category: 'Rent', amount: 1450 }, { category: 'Food', amount: 620 }, { category: 'Transport', amount: 210 },
  { category: 'Leisure', amount: 180 }, { category: 'Savings', amount: 400 },
];
const dollars = (value: number) => `$${grouped(value)}`;
const percent = (share: number) => `${Math.round(share * 100)}%`;

/** A row in the legend chooses its part; the hole then shows that part in place of the total. */
export function Spending() {
  const part = useLocal<string | null>('part', null);
  return (
    <PieChart
      rows={spending}
      nameField="category"
      valueField="amount"
      label="Spending by category"
      title="Spending"
      description="October"
      formatValue={dollars}
      formatShare={percent}
      totalLabel="Total"
      summarize={summarize}
      value={part.value}
      onChange={part.set}
      data-testid="spending"
    />
  );
}
```

### A solid pie

```tsx
import { summarize } from '@liulidesign/charts/summary';
import { PieChart } from '@liulidesign/charts/PieChart';

const percent = (share: number) => `${Math.round(share * 100)}%`;
const storage = [{ kind: 'Photos', gb: 38 }, { kind: 'Apps', gb: 21 }, { kind: 'Messages', gb: 9 }, { kind: 'Free', gb: 60 }];
const gigabytes = (value: number) => `${value} GB`;

export function Solid() {
  return <PieChart rows={storage} nameField="kind" valueField="gb" label="Storage by kind" title="Storage" variant="pie" formatValue={gigabytes} formatShare={percent} summarize={summarize} data-testid="storage" />;
}
```

### Many parts, the small ones gathered

```tsx
import { summarize } from '@liulidesign/charts/summary';
import { PieChart } from '@liulidesign/charts/PieChart';

const grouped = (value: number) => String(value).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
const percent = (share: number) => `${Math.round(share * 100)}%`;
const languages = [
  { language: 'English', readers: 4210 }, { language: 'Spanish', readers: 1870 }, { language: 'Chinese', readers: 1540 }, { language: 'Hindi', readers: 960 },
  { language: 'French', readers: 610 }, { language: 'German', readers: 480 }, { language: 'Japanese', readers: 350 }, { language: 'Korean', readers: 190 },
];

/** Eight parts are too many to tell apart: the four largest keep a slice, the rest share one. */
export function Merged() {
  return (
    <PieChart
      rows={languages}
      nameField="language"
      valueField="readers"
      label="Readers by language"
      title="Readers by language"
      formatValue={grouped}
      formatShare={percent}
      otherLabel="Other"
      maxParts={4}
      totalLabel="Readers"
      summarize={summarize}
      data-testid="languages"
    />
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `rows` (required) | `readonly Row[]` |  | The rows, one per part, in the order drawn clockwise from the top. Parts without a positive number are left out. |
| `nameField` (required) | `ChartField<Row>` |  | The field that names each part: "category". |
| `valueField` (required) | `ChartField<Row>` |  | The field that holds each part's amount. |
| `keyField` | `ChartField<Row>` |  | The field that identifies a row across updates and in `value`. By default `nameField`. |
| `label` (required) | `string` |  | The chart's accessible name: "Spending by category". |
| `title` | `string` |  |  |
| `description` | `string` |  |  |
| `formatValue` | `((value: number) => string)` |  | Turns an amount into the text shown and read. By default the bare number. |
| `formatShare` | `((share: number) => string)` |  | Turns a part of the whole, 0 to 1, into text for the legend: "32%". Without it the legend shows amounts only. |
| `summarize` | `ChartSummarize` |  | Writes the summary a screen reader hears from the chart's facts; see `@liulidesign/charts/summary/en`. |
| `variant` | `PieChartVariant` | `'donut'` | `donut` has a hole for the total; `pie` is solid. |
| `totalLabel` | `string` |  | Names the total shown in a donut's hole: "Total". Without it the hole is empty until a part is chosen. |
| `otherLabel` | `string` |  | Names one part that gathers every part after the first `maxParts`: "Other". Without it every part is drawn up to 24, and any past those are one part named "Other" in the page's language. |
| `maxParts` | `number` | `5` | How many parts keep their own slice when `otherLabel` is set. |
| `legendPosition` | `ChartLegendPosition` | `'bottom'` | The legend lists each part with its amount and share. |
| `legendLabel` | `string` |  | The legend's accessible name; by default the chart's `label`. |
| `value` | `string \| null` |  | The chosen part's key; its slice grows a little and keeps its colour, the rest step back. |
| `onChange` | `((key: string \| null) => void)` |  | Makes each legend row a button; runs with its part's key, or `null` when the chosen one is pressed again. |
| `defaultValue` | `string \| null` |  | Where the choice starts when the chart keeps it itself: give this instead of `value`; `onChange` still hears each choice. |
| `size` | `ChartSize` | `'md'` |  |
| `isLoading` | `boolean` | `false` |  |
| `error` | `string` |  |  |
| `emptyLabel` | `string` |  | What to say when no part has an amount. Defaults to "No data" in the page's language. |
| `emptyHint` | `string` |  | A second, quieter line under `emptyLabel`: what to do next. |
| `onRetry` | `(() => void)` |  | With `error`: tries again. |
| `retryLabel` | `string` |  | The words on the retry button. Defaults to "Try again" in the page's language. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the chart; a legend row's button is `<id>-legend-<key>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| plot | `<view>` | yes | The box that centres the ring. |
| drawing | `<vector>` | no | The slices, clockwise from the top with a constant gap between them; hidden from assistive tech. |
| centre | `<view>` | no | A donut's hole: the total under `totalLabel`, or the chosen part; announced when it changes. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `pieChartRecipe` from `@liulidesign/charts/PieChart/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-callout`.
- Colour roles it paints with, set by the theme pack: `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: pressed (from value); busy (from isLoading).
- Tab: Moves through the legend rows when they can be pressed.
- Enter or Space: Chooses the part, or lets it go.
- Press target: md 36 px.
- The legend is the reading: a list where each part is its name, amount and share. With `onChange` each row is a button that reports pressed.
- With `summarize`, a sentence with the number of parts, the total, and the largest and smallest part is read first.
- The hole is a status region: choosing a part announces its name and amount.
- Slices are told apart by colour and by their order, which the legend repeats from the top clockwise; the gap between slices keeps neighbours apart without relying on hue.
- A legend row is 36 px tall and as wide as the chart.
- The ring is one drawing, rebuilt only for a new `rows` array, encoding or chosen part; the legend is five elements per part.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| first mark colour | `chart-1` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| second mark colour | `chart-2` on `surface` | 3:1 | 4.30:1 | 7.21:1 |
| third mark colour | `chart-3` on `surface` | 3:1 | 4.36:1 | 7.60:1 |
| fourth mark colour | `chart-4` on `surface` | 3:1 | 5.81:1 | 7.11:1 |
| fifth mark colour | `chart-5` on `surface` | 3:1 | 5.65:1 | 8.32:1 |
| second mark colour, straight on the page | `chart-2` on `canvas` | 3:1 | 4.01:1 | 7.96:1 |
| third mark colour, straight on the page | `chart-3` on `canvas` | 3:1 | 4.07:1 | 8.39:1 |
| title and figures | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| description, labels and legend | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| a merged remainder slice | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| a merged remainder slice, straight on the page | `line-strong` on `canvas` | 3:1 | 3.24:1 | 3.90:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it for two to five parts that add up to a meaningful whole.
- Do: Set `otherLabel` when there may be more parts; the rest are gathered into one neutral slice.
- Do: Order the rows by amount, largest first, unless the parts have an order of their own.
- Do: Pass `formatShare` so the legend shows each part's share.
- Do: At large text, a total too wide for the hole is written under the ring; keep `formatValue` short so it stays inside.
- Don't: Compare two pies; use a grouped BarChart.
- Don't: Use it for values that do not add up to a whole, or that can be negative.
- Don't: Hide the legend unless the parts are named elsewhere; slices carry no labels.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Choosing a slice by pressing it. | pointer coordinates on an element (`onPointer`) | The legend rows are the buttons. |
| Slices that grow and turn when the data changes. | animation of vector geometry | New data is drawn at once. |
| Labels on or beside slices. | a laid-out box's size, readable in render (`useBox`) | The legend names every part. |

Related: Chart, BarChart.
