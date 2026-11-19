<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# StatusDot

How something stands, as a dot and a word. Status: preview.

```ts
import { StatusDot } from '@liulidesign/core/StatusDot';
```

StatusDot is a small dot in a status mark colour with the status in words beside it: a service that is running, a record that needs review. The word carries the meaning; the dot lets the eye find it down a column. Neutral is a hollow ring. For a status that should stand out in a line of text, use a Badge.

## Examples

### Down a column

```tsx
import { Item, ItemList } from '@liulidesign/core/Item';
import { StatusDot } from '@liulidesign/core/StatusDot';

/** Services and how each stands: the word says it, the dot lets the eye find it. */
export function Services() {
  return (
    <ItemList label="Services">
      <Item label="Sync" endContent={<StatusDot label="Running" tone="success" size="sm" data-testid="sync" />} />
      <Item label="Search" endContent={<StatusDot label="Slow" tone="warning" size="sm" />} />
      <Item label="Exports" endContent={<StatusDot label="Down" tone="danger" size="sm" />} />
      <Item label="Backups" endContent={<StatusDot label="Paused" size="sm" data-testid="backups" />} />
    </ItemList>
  );
}
```

### The six tones

```tsx
import { StatusDot } from '@liulidesign/core/StatusDot';

/** The five tones. */
export function Tones() {
  return (
    <view className="gap-2">
      <StatusDot label="Published" tone="success" />
      <StatusDot label="Needs review" tone="warning" />
      <StatusDot label="Rejected" tone="danger" />
      <StatusDot label="Scheduled" tone="info" data-testid="info" />
      <StatusDot label="In progress" tone="accent" />
      <StatusDot label="Draft" />
    </view>
  );
}
```

### Categories in the eight hues

```tsx
import { StatusDot } from '@liulidesign/core/StatusDot';

/** A hue says which, not how: the calendars an event can be on. */
export function Hues() {
  return (
    <view className="flex-row flex-wrap gap-x-5 gap-y-2">
      <StatusDot label="Work" hue={1} data-testid="hue-1" />
      <StatusDot label="Family" hue={2} />
      <StatusDot label="Travel" hue={3} />
      <StatusDot label="Garden" hue={4} />
      <StatusDot label="Birthdays" hue={5} />
      <StatusDot label="Choir" hue={6} />
      <StatusDot label="Swimming" hue={7} />
      <StatusDot label="Holidays" hue={8} />
    </view>
  );
}
```

### Without its words

```tsx
import { StatusDot } from '@liulidesign/core/StatusDot';

/** Without its words, where they are already beside it. */
export function DotOnly() {
  return (
    <view className="flex-row items-center gap-2">
      <StatusDot label="Online" tone="success" isLabelHidden data-testid="bare" />
      <text className="text-body font-medium text-text">Ana Lima</text>
      <text className="text-footnote text-muted">Online</text>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The status in words: "Online", "Degraded", "Paused". Shown beside the dot, and what is read. |
| `isLabelHidden` | `boolean` | `false` | Hides the words, keeping them as the dot\'s name: only where the same words are already beside it. |
| `tone` | `StatusDotTone` | `'neutral'` |  |
| `hue` | `Hue` |  | A category's colour in place of a tone, one of the theme's eight hues: which calendar, which project. Not for a status. |
| `size` | `StatusDotSize` | `'md'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The dot and its word in a row. |
| dot | `<view>` | yes | A filled dot in the tone's mark colour; a hollow ring for neutral. |
| label | `<text>` | no | The status in words. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `statusDotRecipe` from `@liulidesign/core/StatusDot/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger-mark`, `hue-1`, `hue-2`, `hue-3`, `hue-4`, `hue-5`, `hue-6`, `hue-7`, `hue-8`, `info-mark`, `line-strong`, `success-mark`, `text`, `warning-mark`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Press target: .
- The word is read; the dot is not. With `isLabelHidden` the dot is an image named by the word.
- Colour is never the only sign: the word says the status, and neutral differs by shape.
- It does not announce a change; say that with a Toast or a status of your own.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| success dot | `success-mark` on `surface` | 3:1 | 3.71:1 | 7.61:1 |
| warning dot | `warning-mark` on `surface` | 3:1 | 3.70:1 | 8.18:1 |
| danger dot | `danger-mark` on `surface` | 3:1 | 3.70:1 | 5.42:1 |
| info dot | `info-mark` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| accent dot | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| hue 1 dot | `hue-1` on `surface` | 3:1 | 3.72:1 | 9.96:1 |
| hue 2 dot | `hue-2` on `surface` | 3:1 | 5.25:1 | 12.25:1 |
| hue 3 dot | `hue-3` on `surface` | 3:1 | 7.68:1 | 5.30:1 |
| hue 4 dot | `hue-4` on `surface` | 3:1 | 4.87:1 | 10.50:1 |
| hue 5 dot | `hue-5` on `surface` | 3:1 | 8.29:1 | 8.18:1 |
| hue 6 dot | `hue-6` on `surface` | 3:1 | 6.42:1 | 5.15:1 |
| hue 7 dot | `hue-7` on `surface` | 3:1 | 7.35:1 | 7.44:1 |
| hue 8 dot | `hue-8` on `surface` | 3:1 | 3.72:1 | 5.44:1 |
| neutral ring | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write the status as one or two words: "Running", "Needs review".
- Do: Keep one meaning per tone across the app.
- Don't: Hide the word unless the same word is already beside the dot.
- Don't: Use a dot for a count or an unread mark; that is a Badge.

## Limits

- It does not pulse or blink.

Related: Badge, Avatar, Banner.
