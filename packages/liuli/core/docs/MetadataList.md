<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# MetadataList

What is known about one thing, as labels and their values. Status: preview.

```ts
import { MetadataList } from '@liulidesign/core/MetadataList';
```

MetadataList shows the facts about a single record: who owns it, when it was made, what state it is in. In rows the labels share a column and the values line up beside them; stacked, each label sits over its value, a few to a line. A value can be drawn as a Badge or an Avatar and is still read as its text. For many records side by side, use a Table.

## Examples

### Labels beside their values

```tsx
import { Badge } from '@liulidesign/core/Badge';
import { MetadataList } from '@liulidesign/core/MetadataList';

/** What is known about a record: the labels in one column, the values in the next. */
export function Invoice() {
  return (
    <MetadataList
      label="Invoice details"
      items={[
        { key: 'customer', label: 'Customer', value: 'Blue Harbor Ltd' },
        { key: 'status', label: 'Status', value: 'Overdue', content: <Badge label="Overdue" tone="danger" size="sm" /> },
        { key: 'issued', label: 'Issued', value: '9 October 2026' },
        { key: 'due', label: 'Due', value: '9 November 2026' },
        { key: 'address', label: 'Billing address', value: '14 Quay Street, Harbour Buildings, Unit 3, Bristol BS1 4DA' },
      ]}
      data-testid="invoice"
    />
  );
}
```

### Labels over their values

```tsx
import { MetadataList } from '@liulidesign/core/MetadataList';

/** Each label over its value, two to a line: for a few short facts at the top of a page. */
export function Stacked() {
  return (
    <MetadataList
      label="File details"
      layout="stacked"
      items={[
        { key: 'size', label: 'Size', value: '4.2 MB' },
        { key: 'kind', label: 'Kind', value: 'PDF document' },
        { key: 'created', label: 'Created', value: '12 March 2026' },
        { key: 'owner', label: 'Owner', value: 'Ana Lima' },
      ]}
      data-testid="file"
    />
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the list for assistive tech: "Details". |
| `items` (required) | `readonly MetadataItem[]` |  |  |
| `layout` | `MetadataListLayout` | `'rows'` | `rows` puts each label beside its value; `stacked` puts it above, in `columns`. |
| `columns` | `1 \| 2 \| 3` | `2` | With `stacked`: how many pairs share a line. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## MetadataItem props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `key` (required) | `string` |  | Identifies the pair: its test id is `<id>-<key>`. |
| `label` (required) | `string` |  | What the value is: "Owner", "Created". |
| `value` (required) | `string` |  | The value as text: shown, and what is read. |
| `content` | `Child` |  | Drawn in place of the value's text: a Badge, an Avatar with a name. The text is still what is read. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The pairs, a list named by `label`. |
| item | `<view>` | yes | One pair, read as "Label: value". |
| label | `<text>` | yes | What the value is, in muted footnote type. |
| value | `<text>` | yes | The value; it wraps rather than truncating. |
| content | `<view>` | no | Drawn in place of the value's text. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `metadataListRecipe` from `@liulidesign/core/MetadataList/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`.
- Colour roles it paints with, set by the theme pack: `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Press target: .
- The pairs are a list, so a screen reader says how many there are; each is read as one phrase, "Status: Overdue".
- A value drawn by `content` is read as its `value` text, so a badge's meaning must be in that text.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| value | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| label | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Order the pairs by what people look for first, not by the record's fields.
- Do: Use `rows` for a list that may have long values; `stacked` for a handful of short ones.
- Do: Format dates and amounts as the rest of the app does.
- Do: At large text, a label word wider than its third widens the label column; keep labels to short words.
- Don't: Use it for fields people edit; that is a form.
- Don't: Leave a pair with an empty value; leave the pair out, or say "None".

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A label column as wide as its longest label. | text measurement before layout | A third of the row. |
| A value that can be copied with a press. | nothing from the View | The app puts an IconButton in `content`. |

Related: Table, Item, Badge.
