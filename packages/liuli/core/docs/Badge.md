<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Badge

A short status or category label in a tinted pill. Status: preview.

```ts
import { Badge } from '@liulidesign/core/Badge';
```

Badge marks the state or kind of the thing next to it: Draft, Paid, Overdue, New. It is text, not a control, and is read as it is shown. Five tones come soft; the four status tones also come solid for the one label that must stand out.

## Examples

### Soft tones

```tsx
import { Badge } from '@liulidesign/core/Badge';

export function Tones() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Draft" data-testid="neutral" />
      <Badge label="New" tone="accent" data-testid="accent" />
      <Badge label="Paid" tone="success" />
      <Badge label="Due soon" tone="warning" />
      <Badge label="Overdue" tone="danger" data-testid="danger" />
    </view>
  );
}
```

### Solid, to stand out

```tsx
import { Badge } from '@liulidesign/core/Badge';

/** Solid fills stand out; keep one per view. */
export function Solid() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Beta" tone="accent" emphasis="solid" data-testid="solid" />
      <Badge label="Live" tone="success" emphasis="solid" />
      <Badge label="Paused" tone="warning" emphasis="solid" />
      <Badge label="Failed" tone="danger" emphasis="solid" />
    </view>
  );
}
```

### Sizes and an icon

```tsx
import { Badge } from '@liulidesign/core/Badge';

const dot = [{ d: 'M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8z', fill: 'currentColor' }] as const;

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Badge label="Small" size="sm" data-testid="small" />
      <Badge label="Online" tone="success" startIcon={<vector paths={dot} viewBox={[0, 0, 24, 24]} className="size-full" />} />
    </view>
  );
}
```

### At the end of a row

```tsx
import { Item } from '@liulidesign/core/Item';
import { Badge } from '@liulidesign/core/Badge';

/** In a pressable row the end content is not read, so the status is repeated in the description. */
export function InARow() {
  return (
    <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
      <Item
        label="Invoice 2041"
        description="Overdue, due 3 May"
        endContent={<Badge label="Overdue" tone="danger" size="sm" />}
        hasChevron
        onClick={() => {}}
        data-testid="invoice"
      />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `tone` | `"neutral" \| BadgeStatusTone` | `'neutral'` | What the badge marks; `neutral` comes soft only. |
| `emphasis` | `"soft" \| "solid"` | `'soft'` | `solid` for a badge that must stand out; at most one per view. |
| `label` (required) | `string` |  | The text, read as it is shown. Say the status in words: "Overdue", not a colour. |
| `size` | `BadgeSize` | `'md'` |  |
| `startIcon` | `Child` |  | Drawn before the label and hidden from assistive tech. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The pill: tint or fill by tone and emphasis. |
| icon | `<view>` | no | `startIcon`, hidden from assistive tech. |
| label | `<text>` | yes | The text; keep it to a word or two. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `badgeRecipe` from `@liulidesign/core/Badge/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `danger`, `danger-soft`, `on-accent`, `on-accent-soft`, `on-danger`, `on-success`, `on-warning`, `success`, `success-soft`, `text`, `warning`, `warning-soft`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Not a control and not announced on change. A badge whose change matters sits next to a status region the app keeps.
- The tone is colour only; the label carries the meaning in words.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| neutral, on a surface | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| neutral, on a selected row | `text` on `text/8` over `accent-soft` | 4.5:1 | 12.09:1 | 9.07:1 |
| soft accent | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.44:1 | 9.49:1 |
| soft success | `text` on `success-soft` | 4.5:1 | 14.34:1 | 11.76:1 |
| soft warning | `text` on `warning-soft` | 4.5:1 | 14.59:1 | 11.36:1 |
| soft danger | `text` on `danger-soft` | 4.5:1 | 14.24:1 | 12.84:1 |
| solid accent | `on-accent` on `accent` | 4.5:1 | 5.62:1 | 7.12:1 |
| solid success | `on-success` on `success` | 4.5:1 | 5.31:1 | 8.54:1 |
| solid warning | `on-warning` on `warning` | 4.5:1 | 5.40:1 | 8.84:1 |
| solid danger | `on-danger` on `danger` | 4.5:1 | 5.21:1 | 6.29:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say the status in the label; the colour only repeats it.
- Do: Keep one solid badge per view at most.
- Do: In a pressable Item, repeat the status in the row description; the end content is not read there.
- Don't: Make a badge pressable; use a Button or a row.
- Don't: Use a badge for a count on an icon; that needs its own placement and name.
- Don't: Pick tones for decoration; each tone means the status it is named for.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| An info tone and category hues (tags, labels, people). | info and categorical hue roles with soft and on- pairs | Five tones; use neutral for categories. |

Related: Item, Card.
