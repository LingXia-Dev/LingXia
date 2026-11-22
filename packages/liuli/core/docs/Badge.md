<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Badge

A short status or category label in a tinted pill. Status: preview.

```ts
import { Badge } from '@liulidesign/core/Badge';
```

Badge marks the state or kind of the thing next to it: Draft, Paid, Overdue, New. It is text, not a control, and is read as it is shown. Six tones come soft; the five status tones also come solid for the one label that must stand out. A category (a project, a label, a team) takes one of eight hues in place of a tone.

## Examples

### Soft tones

```tsx
import { Badge } from '@liulidesign/core/Badge';

export function Tones() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Draft" data-testid="neutral" />
      <Badge label="New" tone="accent" data-testid="accent" />
      <Badge label="Scheduled" tone="info" data-testid="info" />
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
      <Badge label="Update" tone="info" emphasis="solid" />
      <Badge label="Live" tone="success" emphasis="solid" />
      <Badge label="Paused" tone="warning" emphasis="solid" />
      <Badge label="Failed" tone="danger" emphasis="solid" />
    </view>
  );
}
```

### Categories in the eight hues

```tsx
import { Badge } from '@liulidesign/core/Badge';

/** Kinds of things, not states: each project keeps its hue wherever it is named. */
export function Hues() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Badge label="Design" hue={1} data-testid="hue-1" />
      <Badge label="Research" hue={2} />
      <Badge label="Hardware" hue={3} />
      <Badge label="Field trials" hue={4} />
      <Badge label="Marketing" hue={5} />
      <Badge label="Legal" hue={6} />
      <Badge label="Support" hue={7} />
      <Badge label="Archive" hue={8} />
    </view>
  );
}
```

### Sizes and an icon

```tsx
import { Badge } from '@liulidesign/core/Badge';

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Badge label="Small" size="sm" data-testid="small" />
      <Badge label="Online" tone="success" startIcon="success" />
    </view>
  );
}
```

### At the end of a row

```tsx
import { Said } from '../example';
import { Item } from '@liulidesign/core/Item';
import { Badge } from '@liulidesign/core/Badge';

/** In a pressable row the badge is read after the second line, as part of the row's description. */
export function InARow() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12 bg-surface">
          <Item
            label="Invoice 2041"
            description="Due 3 May"
            endContent={<Badge label="Overdue" tone="danger" size="sm" />}
            hasChevron
            onClick={() => say('Opened Invoice 2041')}
            data-testid="invoice"
          />
        </view>
      )}
    </Said>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `tone` | `"neutral" \| BadgeStatusTone` | `'neutral'` | What the badge marks; `neutral` comes soft only. |
| `emphasis` | `"soft" \| "solid"` | `'soft'` | `solid` for a badge that must stand out; at most one per view. |
| `hue` | `Hue` |  | A category's colour, one of the theme's eight hues: a dot in the hue on its soft fill. For kinds of things (a project, a label), never a status. |
| `label` (required) | `string` |  | The text, read as it is shown. Say the status in words: "Overdue", not a colour. |
| `size` | `BadgeSize` | `'md'` |  |
| `startIcon` | `IconRef` |  | An icon by name from the set, or a definition, before the label; hidden from assistive tech. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The pill: tint or fill by tone and emphasis. |
| icon | `<vector>` | no | `startIcon`, an icon from the set in use, hidden from assistive tech. |
| dot | `<view>` | no | With `hue` and no icon: a dot in the hue, hidden from assistive tech. |
| label | `<text>` | yes | The text; keep it to a word or two. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `badgeRecipe` from `@liulidesign/core/Badge/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `danger`, `danger-soft`, `hue-1`, `hue-1-soft`, `hue-2`, `hue-2-soft`, `hue-3`, `hue-3-soft`, `hue-4`, `hue-4-soft`, `hue-5`, `hue-5-soft`, `hue-6`, `hue-6-soft`, `hue-7`, `hue-7-soft`, `hue-8`, `hue-8-soft`, `info`, `info-soft`, `on-accent`, `on-accent-soft`, `on-danger`, `on-info`, `on-success`, `on-warning`, `success`, `success-soft`, `text`, `warning`, `warning-soft`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Not a control and not announced on change. A badge whose change matters sits next to a status region the app keeps.
- The tone is colour only; the label carries the meaning in words.
- A hue is colour only too: the label names the category, and the dot is not read.
- A solid badge is a label, so it takes its tone's own pair, not the solid fills of pressed controls: a row of solid badges reads as one family in both appearances.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| neutral, on a surface | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| neutral, on a selected row | `text` on `text/8` over `accent-soft` | 4.5:1 | 12.23:1 | 9.07:1 |
| soft accent | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.54:1 | 9.49:1 |
| soft info | `text` on `info-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| soft success | `text` on `success-soft` | 4.5:1 | 14.33:1 | 11.76:1 |
| soft warning | `text` on `warning-soft` | 4.5:1 | 14.59:1 | 11.36:1 |
| soft danger | `text` on `danger-soft` | 4.5:1 | 14.12:1 | 12.22:1 |
| solid accent | `on-accent` on `accent` | 4.5:1 | 5.62:1 | 7.12:1 |
| solid info | `on-info` on `info` | 4.5:1 | 5.62:1 | 7.12:1 |
| solid success | `on-success` on `success` | 4.5:1 | 5.31:1 | 8.72:1 |
| solid warning | `on-warning` on `warning` | 4.5:1 | 5.40:1 | 9.41:1 |
| solid danger | `on-danger` on `danger` | 4.5:1 | 5.21:1 | 6.27:1 |
| hue 1, text on its fill | `text` on `hue-1-soft` | 4.5:1 | 14.25:1 | 11.24:1 |
| hue 1, the dot on its fill | `hue-1` on `hue-1-soft` | 3:1 | 3.26:1 | 8.07:1 |
| hue 2, text on its fill | `text` on `hue-2-soft` | 4.5:1 | 14.42:1 | 11.00:1 |
| hue 2, the dot on its fill | `hue-2` on `hue-2-soft` | 3:1 | 4.65:1 | 9.71:1 |
| hue 3, text on its fill | `text` on `hue-3-soft` | 4.5:1 | 14.23:1 | 11.31:1 |
| hue 3, the dot on its fill | `hue-3` on `hue-3-soft` | 3:1 | 6.72:1 | 4.32:1 |
| hue 4, text on its fill | `text` on `hue-4-soft` | 4.5:1 | 14.34:1 | 11.06:1 |
| hue 4, the dot on its fill | `hue-4` on `hue-4-soft` | 3:1 | 4.29:1 | 8.37:1 |
| hue 5, text on its fill | `text` on `hue-5-soft` | 4.5:1 | 14.16:1 | 11.41:1 |
| hue 5, the dot on its fill | `hue-5` on `hue-5-soft` | 3:1 | 7.22:1 | 6.73:1 |
| hue 6, text on its fill | `text` on `hue-6-soft` | 4.5:1 | 14.17:1 | 11.31:1 |
| hue 6, the dot on its fill | `hue-6` on `hue-6-soft` | 3:1 | 5.60:1 | 4.20:1 |
| hue 7, text on its fill | `text` on `hue-7-soft` | 4.5:1 | 14.32:1 | 11.05:1 |
| hue 7, the dot on its fill | `hue-7` on `hue-7-soft` | 3:1 | 6.47:1 | 5.92:1 |
| hue 8, text on its fill | `text` on `hue-8-soft` | 4.5:1 | 14.22:1 | 11.20:1 |
| hue 8, the dot on its fill | `hue-8` on `hue-8-soft` | 3:1 | 3.25:1 | 4.40:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say the status in the label; the colour only repeats it.
- Do: Keep one solid badge per view at most.
- Do: In a pressable Item the badge is read after the row's second line, so the second line need not repeat it.
- Do: At large text, the pill grows taller with its label; keep the label to a word or two.
- Don't: Make a badge pressable; use a Button or a row.
- Don't: Use a badge for a count on an icon; that needs its own placement and name.
- Don't: Pick tones for decoration; each tone means the status it is named for.
- Don't: Use a hue for a status, or a tone for a category; hues carry no meaning beyond "the same kind".

Related: Item, Card.
