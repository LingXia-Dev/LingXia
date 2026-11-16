<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Divider

A hairline between two things that belong apart. Status: preview.

```ts
import { Divider } from '@liulidesign/core/Divider';
```

Divider draws the system's subtle hairline between blocks, across a card or upright between items in a row. It can carry a word in its middle, "or". Space parts most things better than a line does; use a divider where two blocks would otherwise read as one.

## Examples

### Between two blocks

```tsx
import { Divider } from '@liulidesign/core/Divider';

/** Between two blocks of a card that belong apart. */
export function Between() {
  return (
    <view className="gap-4">
      <view className="gap-1">
        <text className="text-body font-medium text-text">Delivery</text>
        <text className="text-footnote text-muted">Tuesday 17 November, between 9 and 12</text>
      </view>
      <Divider data-testid="line" />
      <view className="gap-1">
        <text className="text-body font-medium text-text">Payment</text>
        <text className="text-footnote text-muted">Card ending 4421</text>
      </view>
    </view>
  );
}
```

### With a word on the line

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { Divider } from '@liulidesign/core/Divider';

/** A word on the line, between two ways to do the same thing. */
export function Labelled() {
  return (
    <Said>
      {say => (
        <view className="gap-4">
          <Button label="Continue with email" variant="primary" width="fill" onClick={() => say('Continued with email')} />
          <Divider label="or" data-testid="or" />
          <Button label="Use a passkey" width="fill" onClick={() => say('Used a passkey')} />
        </view>
      )}
    </Said>
  );
}
```

### Upright, in a row

```tsx
import { Divider } from '@liulidesign/core/Divider';

/** Upright, between items in a row. */
export function Vertical() {
  return (
    <view className="flex-row items-center gap-3 h-6">
      <text className="text-footnote text-muted">12 notes</text>
      <Divider orientation="vertical" data-testid="upright" />
      <text className="text-footnote text-muted">3 shared</text>
      <Divider orientation="vertical" />
      <text className="text-footnote text-muted">Edited today</text>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `orientation` | `DividerOrientation` | `'horizontal'` | `vertical` parts items in a row and takes their height. |
| `inset` | `DividerInset` | `'none'` | Sets the line in from the edge: `start` to line up with text after an icon, `both` inside a padded card. Horizontal only. |
| `label` | `string` |  | A word in the middle of the line: "or". Horizontal only. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| line | `<view>` | yes | The hairline, across or upright. |
| label | `<text>` | no | A word in the middle of a horizontal line. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `dividerRecipe` from `@liulidesign/core/Divider/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`.
- Colour roles it paints with, set by the theme pack: `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Press target: .
- The line is decoration and is not read: headings and groups carry the structure. Its contrast is below 3:1 on purpose.
- A label on the line is read as text where it stands.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| label | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Try more space first; add the line where the blocks still read as one.
- Do: Use `inset="start"` in a list whose rows begin with an icon, so the line starts where the text does.
- Don't: Rule every row of a list by hand; ItemList and Table divide their own rows.
- Don't: Use it as a border around something; that is a Card.

Related: Section, Card, Item.
