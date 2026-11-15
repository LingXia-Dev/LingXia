<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# CircularProgress

How much is done, as a ring. Status: preview.

```ts
import { CircularProgress } from '@liulidesign/core/CircularProgress';
```

CircularProgress shows an amount from 0 to `max` as an arc around a ring, with the app's own words in the middle at the larger sizes. Without `value` the arc spins: the task is running and its length is unknown. Use it where a ProgressBar has no room: a tile, the end of a row.

## Examples

### A goal on a tile

```tsx
import { CircularProgress } from '@liulidesign/core/CircularProgress';

/** A goal on a tile: the ring, and the app's words in its middle. */
export function Goal() {
  return (
    <view className="flex-row items-center gap-4">
      <CircularProgress label="Steps today" value={7200} max={10000} valueLabel="72%" size="lg" data-testid="steps" />
      <view className="gap-0.5">
        <text className="text-body font-medium text-text">7,200 steps</text>
        <text className="text-footnote text-muted">2,800 to your goal of 10,000</text>
      </view>
    </view>
  );
}
```

### At the end of a row

```tsx
import { Item, ItemList } from '@liulidesign/core/Item';
import { CircularProgress } from '@liulidesign/core/CircularProgress';

/** At the end of a row, where a bar has no room. */
export function InRows() {
  return (
    <ItemList label="Uploads">
      <Item label="Harbour at dawn.jpg" description="4.2 MB" endContent={<CircularProgress label="Uploading Harbour at dawn.jpg" value={64} valueLabel="64%" size="sm" data-testid="row" />} />
      <Item label="Market stalls.jpg" description="3.1 MB" endContent={<CircularProgress label="Uploading Market stalls.jpg" size="sm" data-testid="waiting" />} />
      <Item label="Tram 28.jpg" description="5.6 MB" endContent={<CircularProgress label="Uploaded Tram 28.jpg" value={100} valueLabel="Done" size="sm" tone="success" />} />
    </ItemList>
  );
}
```

### Sizes

```tsx
import { CircularProgress } from '@liulidesign/core/CircularProgress';

export function Sizes() {
  return (
    <view className="flex-row items-center gap-5">
      <CircularProgress label="Storage used" value={40} valueLabel="40%" size="sm" />
      <CircularProgress label="Storage used" value={40} valueLabel="40%" />
      <CircularProgress label="Storage used" value={40} valueLabel="40%" size="lg" />
    </view>
  );
}
```

### Meters in a tone

```tsx
import { CircularProgress } from '@liulidesign/core/CircularProgress';

/** A meter in a tone: the words say how it stands too. */
export function Tones() {
  return (
    <view className="flex-row items-center gap-5">
      <CircularProgress label="Monthly goal" value={100} valueLabel="Done" tone="success" size="lg" />
      <CircularProgress label="Storage" value={86} valueLabel="86%" tone="warning" size="lg" />
      <CircularProgress label="Data plan" value={100} valueLabel="Over" tone="danger" size="lg" data-testid="over" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The accessible name: what is in progress ("Uploading", "Daily goal"). Not drawn. |
| `value` | `number` |  | The amount done, from 0 to `max`. Leave it out while the amount is unknown. |
| `max` | `number` | `100` |  |
| `valueLabel` | `string` |  | The amount as the app words it ("72%", "3 of 8"): read in place of a percentage, and drawn in the middle at `md` and `lg`. |
| `size` | `CircularProgressSize` | `'md'` | 24, 48 or 80 px. `sm` has no room for words. |
| `tone` | `ProgressBarTone` | `'accent'` | The arc's colour, for a ring that is a meter. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The square the ring fills; it carries the progressbar role. |
| track | `<vector>` | yes | The whole ring, in faint ink. |
| fill | `<vector>` | no | The arc from twelve o'clock, clockwise; a quarter arc that spins while the amount is unknown. |
| value | `<text>` | no | `valueLabel` in the middle, at `md` and `lg`. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `circularProgressRecipe` from `@liulidesign/core/CircularProgress/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-callout`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger-mark`, `success-mark`, `text`, `warning-mark`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Press target: .
- Role progressbar, named by `label`. With `value` it reports the value, 0 and `max`; `valueLabel` is read in place of a percentage.
- Without `value` it reports no value, which assistive tech reads as busy.
- The words in the middle are not read twice: the ring already carries them.
- Under reduced motion the unknown arc stands still.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| arc on a card | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| arc against the track | `accent` on `text/10` over `surface` | 3:1 | 4.62:1 | 4.75:1 |
| success arc | `success-mark` on `surface` | 3:1 | 3.61:1 | 6.91:1 |
| warning arc | `warning-mark` on `surface` | 3:1 | 3.64:1 | 8.03:1 |
| danger arc | `danger-mark` on `surface` | 3:1 | 3.91:1 | 5.19:1 |
| value | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Give `valueLabel` in the words the screen uses: "72%", "3 of 8".
- Do: Use `sm` at the end of a row or inside a button; say the amount beside it if it matters.
- Don't: Use a ring where a bar fits; a bar is easier to compare and to read.
- Don't: Rely on the tone alone; say "nearly full" or "over" in words.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |

Related: ProgressBar, Stat.
