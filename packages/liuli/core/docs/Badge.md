<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Badge

A short status or category label in a tinted pill. Status: lab.

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
    <view className="rounded-container border border-line bg-surface">
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

## Accessibility

- Pattern: none (holds a control).
- Name: label.
- Description: none.
- Not a control and not announced on change. A badge whose change matters sits next to a status region the app keeps.
- The tone is colour only; the label carries the meaning in words.

| Contrast | Pair | Minimum | Proven by the theme pack |
| --- | --- | --- | --- |
| neutral | `text` on `surface-sunken` | 4.5:1 | yes |
| soft accent | `on-accent-soft` on `accent-soft` | 4.5:1 | yes |
| soft success | `text` on `success-soft` | 4.5:1 | no |
| soft warning | `text` on `warning-soft` | 4.5:1 | no |
| soft danger | `text` on `danger-soft` | 4.5:1 | no |
| solid accent | `on-accent` on `accent` | 4.5:1 | yes |
| solid success | `on-success` on `success` | 4.5:1 | yes |
| solid warning | `on-warning` on `warning` | 4.5:1 | yes |
| solid danger | `on-danger` on `danger` | 4.5:1 | yes |

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
| Proof that text reads on the soft status tints. | text on success-soft, warning-soft and danger-soft contrast pairs in the theme pack | The pairs are listed here and unproven; neutral and accent are proven. |
| An info tone and category hues (tags, labels, people). | info and categorical hue roles with soft and on- pairs | Five tones; use neutral for categories. |

Related: Item, Card.
