<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Sparkline

A small chart without axes: the shape of a series as a line, an area or bars. Status: preview.

```ts
import { Sparkline } from '@liulidesign/charts/Sparkline';
```

Sparkline shows how a number has moved, beside the number itself: in a Stat, a table cell or a list row. It has no axes, labels or interaction, so it answers "which way, and how steadily", never "how much". It is one drawing computed from `values`, and its `label` says in words what a sighted reader takes from the shape.

## Examples

### A trend, the latest value marked

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';

const week = [12, 14, 13, 17, 16, 21, 24];

export function Trend() {
  return <Sparkline values={week} label="Revenue, last 7 days: up from 12k to 24k" hasEndDot data-testid="trend" />;
}
```

### Line, area and bars

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';

const month = [38, 41, 36, 44, 47, 43, 52, 55, 49, 58, 61, 57, 66, 70, 64, 73, 78, 71, 69, 80, 84, 79, 88, 91, 86, 95, 99, 93, 104, 108];
const net = [6, 9, -4, 3, -7, -2, 8, 11, -3, 5];

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
```

### A rounded line that stays within its data

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';

const week = [12, 14, 13, 17, 16, 21, 24];

export function Smooth() {
  return <Sparkline values={week} isSmooth variant="area" label="Sessions, last 7 days: up from 12k to 24k" data-testid="smooth" />;
}
```

### Tones

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';

const week = [12, 14, 13, 17, 16, 21, 24];
const falling = [31, 29, 30, 26, 27, 22, 19];

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
```

### Sizes

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';

const month = [38, 41, 36, 44, 47, 43, 52, 55, 49, 58, 61, 57, 66, 70, 64, 73, 78, 71, 69, 80, 84, 79, 88, 91, 86, 95, 99, 93, 104, 108];

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
```

### Missing readings

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';

/** Days without a reading are gaps, never zeros. */
export function Gaps() {
  return <Sparkline values={[18, 21, NaN, NaN, 24, 22, NaN, 27, NaN, 25, 29, 31]} label="Temperature by hour, with four hours missing" data-testid="gaps" />;
}
```

### In list rows

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';

const cities = [
  { name: 'Lisbon', now: '24°', values: [19, 20, 22, 24, 25, 24, 24], label: 'Lisbon, last 7 days: 19 to 24 degrees' },
  { name: 'Oslo', now: '9°', values: [12, 11, 11, 10, 8, 9, 9], label: 'Oslo, last 7 days: 12 to 9 degrees' },
  { name: 'Nairobi', now: '27°', values: [26, 27, 26, 28, 27, 27, 27], label: 'Nairobi, last 7 days: steady near 27 degrees' },
];

/** In a row, the sparkline sits between the name and the figure it explains. */
export function InRows() {
  return (
    <view role="list" aria-label="Cities" className="divide-y divide-line">
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
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `values` (required) | `readonly number[]` |  | The numbers, oldest first; a new array when they change. A value that is not finite (`NaN`) is a gap. Past 240 values it keeps the 240 that hold the shape, peaks and dips included. |
| `label` (required) | `string` |  | What the chart shows, in words; it is all a screen reader gets: "Revenue, last 7 days: up from 12k to 24k". |
| `variant` | `SparklineVariant` | `'line'` | `area` fills under the line; `bar` draws one bar per value. |
| `isSmooth` | `boolean` | `false` | Rounds the line between points without passing above or below them. Lines and areas only. |
| `hasEndDot` | `boolean` | `false` | Marks the latest value with a dot. Lines and areas only. |
| `tone` | `SparklineTone` | `'series'` | `series` is the first chart colour; `positive` and `negative` say the trend is good or bad. |
| `domainMin` | `number` |  | The lowest value drawn. A line fits its data; an area or bars start from zero. |
| `domainMax` | `number` |  | The highest value drawn. |
| `size` | `SparklineSize` | `'md'` | The height: 24, 32 or 48. In a column the width fills the container; in a row give it one with `className` (`w-20`, `grow`). |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The box, sized by `size` and the container, and the image assistive tech reads by `label`. |
| plot | `<vector>` | no | The drawing, stretched to the box; absent when no value is finite. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `sparklineRecipe` from `@liulidesign/charts/Sparkline/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- One image named by `label`; the drawing itself is hidden. A screen reader gets nothing but the label, so it carries the finding: the direction, and the first and last values.
- It takes no focus and has no actions.
- It is two elements however many values it draws, and its paths are rebuilt only for a new `values` array or encoding.
- The fill under an area is decoration and has no contrast minimum: the line above it carries the values.
- Positive and negative tones differ by colour alone, so the label, or the figure beside it, must say which it is.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| series line and bars | `chart-1` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| series line and bars, straight on the page | `chart-1` on `canvas` | 3:1 | 5.25:1 | 6.83:1 |
| neutral line | `text-muted` on `surface` | 3:1 | 5.89:1 | 6.64:1 |
| positive line | `success-mark` on `surface` | 3:1 | 3.71:1 | 7.61:1 |
| negative line | `danger-mark` on `surface` | 3:1 | 3.70:1 | 5.42:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Put it next to the figure it explains, and write the finding in `label`: "Revenue, last 7 days: up from 12k to 24k".
- Do: Use a line for a trend, an area for an amount measured from zero, bars for separate counts or values on both sides of zero.
- Do: Give sparklines that are compared with each other the same `domainMin` and `domainMax`.
- Do: In a row, give it a width: `className="w-20"` or `grow`. It has no width of its own, so beside other content it would shrink to nothing.
- Do: Pass `NaN` for a missing reading, so it shows as a gap.
- Don't: Use it where a value must be read off: it has no scale. Show the figure, or a chart with axes.
- Don't: Draw more than one series; put two sparklines side by side.
- Don't: Set a tone for looks; `positive` and `negative` are claims about the data.
- Don't: Fill missing readings with zero.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| The value under a finger, and a moving marker. | pointer coordinates on an element | The figure it explains sits beside it. |
| Drawing in when it appears, and moving between two sets of values. | animation of vector geometry | New values are drawn at once. |

Related: Stat.
