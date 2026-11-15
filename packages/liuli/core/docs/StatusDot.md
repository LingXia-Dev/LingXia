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

### The five tones

```tsx
import { StatusDot } from '@liulidesign/core/StatusDot';

/** The five tones. */
export function Tones() {
  return (
    <view className="gap-2">
      <StatusDot label="Published" tone="success" />
      <StatusDot label="Needs review" tone="warning" />
      <StatusDot label="Rejected" tone="danger" />
      <StatusDot label="In progress" tone="accent" />
      <StatusDot label="Draft" />
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
| `size` | `StatusDotSize` | `'md'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The dot and its word in a row. |
| dot | `<view>` | yes | A filled dot in the tone's mark colour; a hollow ring for neutral. |
| label | `<text>` | no | The status in words. |

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
| success dot | `success-mark` on `surface` | 3:1 | 3.61:1 | 6.91:1 |
| warning dot | `warning-mark` on `surface` | 3:1 | 3.64:1 | 8.03:1 |
| danger dot | `danger-mark` on `surface` | 3:1 | 3.91:1 | 5.19:1 |
| accent dot | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| neutral ring | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write the status as one or two words: "Running", "Needs review".
- Do: Keep one meaning per tone across the app.
- Don't: Hide the word unless the same word is already beside the dot.
- Don't: Use a dot for a count or an unread mark; that is a Badge.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |

Related: Badge, Avatar, Banner.
