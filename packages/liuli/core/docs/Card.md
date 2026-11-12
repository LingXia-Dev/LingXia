<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Card

A surface that groups related content, or one pressable choice. Status: preview.

```ts
import { Card, ClickableCard } from '@liulidesign/core/Card';
```

Card groups a heading, text and actions that belong together. ClickableCard, from the same import, makes the whole card one button, for opening an item or choosing one of a few options; it is named by a short `label` and must not contain other controls.

## Examples

### Outlined, raised and sunken

```tsx
import { Card } from '@liulidesign/core/Card';

export function Variants() {
  return (
    <view className="gap-3">
      <Card data-testid="outlined">
        <text className="text-callout font-semibold text-text">Outlined</text>
        <text className="text-footnote text-muted">Sits on the page.</text>
      </Card>
      <Card variant="raised">
        <text className="text-callout font-semibold text-text">Raised</text>
        <text className="text-footnote text-muted">Floats above the page.</text>
      </Card>
      <Card variant="sunken">
        <text className="text-callout font-semibold text-text">Sunken</text>
        <text className="text-footnote text-muted">Groups content inside another surface.</text>
      </Card>
    </view>
  );
}
```

### A card with actions

```tsx
import { Button } from '@liulidesign/core/Button';
import { Card, ClickableCard } from '@liulidesign/core/Card';

/** Actions go in a plain Card, never in a ClickableCard. */
export function WithActions() {
  return (
    <Card>
      <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">Back up your notes</text>
      <text className="text-footnote text-muted">Keep a copy on another device in case this one is lost.</text>
      <view className="flex-row justify-end gap-2">
        <Button label="Not now" variant="ghost" size="sm" />
        <Button label="Turn on" variant="primary" size="sm" data-testid="turn-on" />
      </view>
    </Card>
  );
}
```

### Choosing one card of several

```tsx
import { useLocal } from '@lingxia/view';
import { ClickableCard } from '@liulidesign/core/Card';

const plans = [
  { id: 'monthly', name: 'Monthly', price: '€4 a month' },
  { id: 'yearly', name: 'Yearly', price: '€40 a year' },
] as const;

/** Choosing one of several: the chosen card is reported as selected. */
export function Choice() {
  const plan = useLocal('plan', 'monthly');
  return (
    <view className="flex-row gap-3">
      {plans.map(item => (
        <ClickableCard
          key={item.id}
          label={`${item.name}, ${item.price}`}
          isSelected={plan.value === item.id}
          onClick={() => plan.set(item.id)}
          className="grow basis-0"
          data-testid={`plan-${item.id}`}
        >
          <text className="text-callout font-semibold text-text">{item.name}</text>
          <text className="text-footnote text-muted">{item.price}</text>
        </ClickableCard>
      ))}
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  |  |
| `variant` | `CardVariant` | `'outlined'` | `outlined` sits on the page, `raised` floats above it, `sunken` groups content inside another surface. |
| `padding` | `CardPadding` | `'md'` | The inset and the gap between children. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## ClickableCard props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The accessible name: a short summary of the card, since its content is not read separately. |
| `onClick` (required) | `() => void` |  |  |
| `isSelected` | `boolean` | `false` | Marks the card as the chosen one among several; reported as selected. |
| `isDisabled` | `boolean` | `false` |  |
| `children` (required) | `Child` |  |  |
| `variant` | `CardVariant` | `'outlined'` | `outlined` sits on the page, `raised` floats above it, `sunken` groups content inside another surface. |
| `padding` | `CardPadding` | `'md'` | The inset and the gap between children. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | A `view` for Card, a `button` for ClickableCard; fill, border, radius and inset. |

## Accessibility

- Pattern: button.
- Name: label.
- Description: none.
- States: selected (from isSelected (ClickableCard)); disabled (from isDisabled (ClickableCard)).
- Card is not a control; its content is read in order. The button pattern applies to ClickableCard.
- ClickableCard is read as its `label` only, so the label summarises what the card shows.
- Selection is a thicker accent border as well as a colour, so it does not rest on colour alone.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| selected border | `accent` on `surface` | 3:1 | needs the resolved palette | needs the resolved palette |

Ratios are computed from crates/lingxia-theme/packs/default.json and rounded down.

## Guidance

- Do: Use a plain Card with Buttons inside when a card has actions.
- Do: Give a ClickableCard a label that a listener can choose by: "Yearly, 40 a year".
- Do: Keep the card variant the same for cards that sit side by side.
- Don't: Put a Button, a menu or an input inside a ClickableCard; the View refuses a control inside a button.
- Don't: Nest a raised card inside another raised card.
- Don't: Rely on the outline alone to show where a card ends; it is decorative, below 3:1.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Choice cards reported as a radio group with a position. | radio controls that take children, as checkbox does | Each card is a button reported as selected. |

Related: Item, Button.
