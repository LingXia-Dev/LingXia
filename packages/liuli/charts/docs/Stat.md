<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Stat

One figure with its label, how it changed, and an optional small chart. Status: preview.

```ts
import { Stat } from '@liulidesign/charts/Stat';
```

Stat shows a number a person checks often: revenue, open orders, a response time. Under the figure it can say how the number changed, with an arrow for the direction and a tone for whether that is good news, and it can hold a Sparkline of its history. The app formats every string; Stat lays them out and reads them as one group.

## Examples

### A figure, its change and its history

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';
import { Stat } from '@liulidesign/charts/Stat';

const revenue = [31, 34, 33, 38, 36, 41, 40, 44, 47, 45, 48];

/** The figure, how it changed in the app's words, and its last eleven weeks. */
export function Revenue() {
  return (
    <Stat
      label="Revenue"
      value="$48,250"
      delta="12.4%"
      deltaDirection="up"
      deltaLabel="Up 12.4% from last month"
      description="vs last month"
      chart={<Sparkline values={revenue} variant="area" isSmooth hasEndDot tone="positive" label="Revenue by week: up from 31k to 48k" />}
      data-testid="revenue"
    />
  );
}
```

### A figure alone

```tsx
import { Stat } from '@liulidesign/charts/Stat';

export function Plain() {
  return <Stat label="Open orders" value="128" data-testid="orders" />;
}
```

### Tone set apart from direction

```tsx
import { Sparkline } from '@liulidesign/charts/Sparkline';
import { Stat } from '@liulidesign/charts/Stat';

const latency = [4.1, 3.9, 4.0, 3.6, 3.7, 3.4, 3.3, 3.2];
const errors = [12, 11, 14, 13, 17, 19, 18, 23];

/** Down is good news for a wait, and up is bad news for errors: the tone is set apart from the direction. */
export function Tones() {
  return (
    <view className="grid grid-cols-2 gap-x-4 gap-y-5">
      <Stat
        label="Response time"
        value="3.2 s"
        delta="0.9 s"
        deltaDirection="down"
        deltaTone="positive"
        deltaLabel="Down 0.9 seconds, an improvement"
        chart={<Sparkline values={latency} tone="positive" label="Response time by day: down from 4.1 to 3.2 seconds" />}
        data-testid="latency"
      />
      <Stat
        label="Errors"
        value="23"
        delta="5"
        deltaDirection="up"
        deltaTone="negative"
        deltaLabel="Up 5 from yesterday, worse"
        chart={<Sparkline values={errors} tone="negative" label="Errors by day: up from 12 to 23" />}
        data-testid="errors"
      />
    </view>
  );
}
```

### Small and medium

```tsx
import { Stat } from '@liulidesign/charts/Stat';

export function Sizes() {
  return (
    <view className="grid grid-cols-2 items-end gap-x-4">
      <Stat label="Small" value="1,204" size="sm" delta="3.1%" deltaDirection="up" data-testid="small" />
      <Stat label="Medium" value="1,204" delta="3.1%" deltaDirection="up" data-testid="medium" />
    </view>
  );
}
```

### Four figures, two to a row

```tsx
import { Card } from '@liulidesign/core/Card';
import { Stat } from '@liulidesign/charts/Stat';

/** Four figures on a phone: two columns of cards, each figure with how it changed. */
export function Dashboard() {
  return (
    <view className="grid grid-cols-2 gap-3">
      <Card variant="sunken">
        <Stat label="Revenue" value="$48,250" size="sm" delta="12.4%" deltaDirection="up" deltaLabel="Up 12.4%" />
      </Card>
      <Card variant="sunken">
        <Stat label="Orders" value="1,204" size="sm" delta="3.1%" deltaDirection="up" deltaLabel="Up 3.1%" />
      </Card>
      <Card variant="sunken">
        <Stat label="Refunds" value="37" size="sm" delta="8" deltaDirection="up" deltaTone="negative" deltaLabel="Up 8, worse" />
      </Card>
      <Card variant="sunken">
        <Stat label="Avg. order" value="$40.07" size="sm" delta="0.0%" deltaLabel="No change" />
      </Card>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | What the figure measures: "Revenue". Also names the group for assistive tech. |
| `value` (required) | `string` |  | The figure as the app formats it: "$48,250", "3.2 s". Liuli formats no numbers. |
| `delta` | `string` |  | The change since the last period, as the app formats it: "12.4%". Drawn after an arrow. |
| `deltaDirection` | `StatDirection` | `'flat'` | Which way the figure moved; picks the arrow. |
| `deltaTone` | `StatTone` |  | Whether the change is good news. By default up is positive, down negative and flat neutral; set it when up is bad (errors, cost). |
| `deltaLabel` | `string` |  | The change in words, read in place of the arrow and `delta`: "Up 12.4% from last month". |
| `description` | `string` |  | A short note after the change: "vs last month". |
| `chart` | `Child` |  | A small chart under the figure, usually a Sparkline. |
| `size` | `StatSize` | `'md'` | The figure's size: title or display type. |
| `isLoading` | `boolean` | `false` | Stands a skeleton in for the figure and leaves out the change and the chart; the group is reported busy. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the group. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The group, named by `label`. |
| label | `<text>` | yes | What the figure measures; hidden from assistive tech, since the group carries it as its name. |
| value | `<text>` | yes | The figure, in tabular figures so it does not shift as it updates. |
| change | `<view>` | no | The line under the figure: the change, then the note. |
| delta | `<view>` | no | The arrow and `delta`, in the tone's colour. |
| deltaSpoken | `<text>` | no | `deltaLabel`, read in place of the arrow and `delta`; not drawn. |
| description | `<text>` | no | A short note after the change. |
| chart | `<view>` | no | The slot under the figure, as wide as the Stat. |
| skeleton | `<view>` | no | Stands in for the figure while loading. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: busy (from isLoading).
- A group named by `label`, read in order: the figure, the change, the note, then the chart by its own label.
- The arrow is hidden. With `deltaLabel` the drawn change is hidden too and the words are read: "Up 12.4% from last month". Without it only `delta` is read, which loses the direction.
- Direction is drawn as an arrow and tone as colour, so the change never depends on colour alone.
- While loading the group is reported busy and keeps its name; the skeleton is hidden.
- It takes no focus. To make it open a page, put it in a ClickableCard.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| figure | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| label and note | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| positive change | `success` on `surface` | 4.5:1 | 5.31:1 | 7.61:1 |
| negative change | `danger` on `surface` | 4.5:1 | 5.21:1 | 5.42:1 |
| positive change, straight on the page | `success` on `canvas` | 4.5:1 | 4.96:1 | 8.40:1 |
| negative change, straight on the page | `danger` on `canvas` | 4.5:1 | 4.87:1 | 5.98:1 |
| positive change in a sunken card | `success` on `surface-sunken` | 4.5:1 | 4.69:1 | 7.93:1 |
| negative change in a sunken card | `danger` on `surface-sunken` | 4.5:1 | 4.60:1 | 5.65:1 |
| label and note in a sunken card | `text-muted` on `surface-sunken` | 4.5:1 | 5.20:1 | 6.93:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Format `value` and `delta` in the app, in the person's locale and units.
- Do: Pass `deltaLabel` whenever there is a `delta`, saying the direction and what it is compared with.
- Do: Set `deltaTone` when up is bad news: errors, cost, waiting time.
- Do: Keep one period across the figures of a dashboard, and say it once in `description` or a heading.
- Don't: Put a unit or a sign in `label`; it names the measure.
- Don't: Show more than one figure in a Stat; use one Stat each.
- Don't: Use the `sm` figure alone on a page; it is for cards two to a row.
- Don't: Leave a figure blank while it loads; set `isLoading`.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A figure that counts to its new value. | a text content transition | The new figure replaces the old one at once. |
| A figure larger than display type. | a larger step in Liuli's type scale | `md` is display type, 28 px. |

Related: Sparkline.
