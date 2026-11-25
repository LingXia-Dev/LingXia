<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Card

A surface that groups related content, or one pressable choice. Status: preview.

```ts
import { Card, ClickableCard, ClickableCardGroup } from '@liulidesign/core/Card';
```

Card groups a heading, text and actions that belong together. ClickableCard, from the same import, makes the whole card one button, for opening an item or choosing one of a few options; it is named by a short `label` and must not contain other controls. ClickableCardGroup makes cards with a `value` one radio group.

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
        <text className="text-footnote text-muted">Rests one step above the page.</text>
      </Card>
      <Card variant="sunken">
        <text className="text-callout font-semibold text-text">Sunken</text>
        <text className="text-footnote text-muted">Groups content inside another surface.</text>
      </Card>
    </view>
  );
}
```

### Floating over a map

```tsx
import { Card } from '@liulidesign/core/Card';

/** Floating: over a map or a picture, where an outline alone would be lost. */
export function Floating() {
  return (
    <view className="h-52 justify-end rounded-container bg-accent-soft p-3">
      <Card variant="floating" padding="sm" data-testid="floating">
        <text className="text-body font-medium text-text">Pier 4, east gate</text>
        <text className="text-footnote text-muted">6 minutes on foot · Opens at 08:00</text>
      </Card>
    </view>
  );
}
```

### A card with actions

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { Card, ClickableCard } from '@liulidesign/core/Card';

/** Actions go in a plain Card, never in a ClickableCard. */
export function WithActions() {
  return (
    <Said>
      {say => (
        <Card>
          <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">Back up your notes</text>
          <text className="text-footnote text-muted">Keep a copy on another device in case this one is lost.</text>
          <view className="flex-row justify-end gap-2">
            <Button label="Not now" variant="ghost" size="sm" onClick={() => say('Put off the backup')} />
            <Button label="Turn on" variant="primary" size="sm" onClick={() => say('Turned on backups')} data-testid="turn-on" />
          </view>
        </Card>
      )}
    </Said>
  );
}
```

### Choosing one card of several

```tsx
import { useLocal } from '@lingxia/view';
import { ClickableCard, ClickableCardGroup } from '@liulidesign/core/Card';

const plans = [
  { id: 'monthly', name: 'Monthly', price: '€4 a month' },
  { id: 'yearly', name: 'Yearly', price: '€40 a year' },
] as const;

/** Choosing one of several: the cards are a radio group, and the chosen one is checked. */
export function Choice() {
  const plan = useLocal<string | null>('plan', 'monthly');
  return (
    <ClickableCardGroup label="Plan" bind={plan} className="flex-row" data-testid="plans">
      {plans.map(item => (
        <ClickableCard
          key={item.id}
          value={item.id}
          label={item.name}
          description={item.price}
          className="grow basis-0"
          data-testid={`plan-${item.id}`}
        >
          <text className="text-callout font-semibold text-text">{item.name}</text>
          <text className="text-footnote text-muted">{item.price}</text>
        </ClickableCard>
      ))}
    </ClickableCardGroup>
  );
}
```

### A card that goes to a page

```tsx
import { useLocal } from '@lingxia/view';
import { ToggleButton } from '@liulidesign/core/ToggleButton';
import { ClickableCard } from '@liulidesign/core/Card';

/** A card that goes to a page and holds its own Like button: the button takes its own press, the card's goes to the page. */
export function ToPage() {
  const liked = useLocal('liked', false);
  return (
    <ClickableCard label="Spring launch, 12 tasks" to={{ page: 'item' }} data-testid="to-page">
      <view className="flex-row items-start gap-3">
        <view className="flex-1 gap-1">
          <text className="text-callout font-semibold text-text">Spring launch</text>
          <text className="text-footnote text-muted">12 tasks, 3 due this week</text>
        </view>
        <ToggleButton label="Like" value={liked.value} onChange={next => liked.set(next)} size="sm" data-testid="to-page-like" />
      </view>
    </ClickableCard>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  |  |
| `variant` | `CardVariant` | `'outlined'` | `outlined` sits on the page, `raised` rests one step above it, `floating` hovers over what is behind it (a map, a picture), `sunken` groups content inside another surface. |
| `padding` | `CardPadding` | `'md'` | The inset and the gap between children. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## ClickableCard props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  |  |
| `variant` | `CardVariant` | `'outlined'` | `outlined` sits on the page, `raised` rests one step above it, `floating` hovers over what is behind it (a map, a picture), `sunken` groups content inside another surface. |
| `padding` | `CardPadding` | `'md'` | The inset and the gap between children. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |
| `label` (required) | `string` |  | The accessible name: a short summary of the card, since its content is not read separately. |
| `description` | `string` |  | Read after the name, not drawn: the detail the card shows that the name leaves out, such as a price or a count. |
| `isDisabled` | `boolean` | `false` |  |
| `onClick` | `(() => void) \| (() => void)` |  | Runs after the group's choice. |
| `isSelected` | `boolean` | `false` | Marks the card as the chosen one among several; reported as selected. |
| `to` | `LinkTarget` |  | Makes it a link that goes there on press: `{ page, params }`, `{ tab }`, `{ back: 1 }`. Read as a link; Enter opens it and Space does not. |
| `value` | `string` |  | Inside a `ClickableCardGroup`: what the group's value carries when this card is chosen. |

## ClickableCardGroup props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<string \| null>` |  | A View-local cell the group reads and writes; use it or `value` with `onChange`. |
| `value` | `string \| null` |  | The chosen card's value; `null` while none is chosen. |
| `onChange` | `((value: string) => void)` |  | Runs with the card chosen by a tap or by arrowing onto it. |
| `label` (required) | `string` |  | Names the group for assistive input. |
| `children` (required) | `Child` |  | ClickableCards, each with a `value`. |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  | Lays the cards out: a row or a column, and the gap. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | A `view` for Card, a `button` for ClickableCard; fill, border, radius and inset. |
| group | `<view>` | no | ClickableCardGroup's radio group: one Tab stop, named by its label. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- `className` on ClickableCard: classes merged after the recipe's, so they win.
- `className` on ClickableCardGroup: Lays the cards out: a row or a column, and the gap.
- The recipe, `cardRecipe` from `@liulidesign/core/Card/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--shadow-contact`, `--shadow-raised`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `focus`, `line`, `surface`, `surface-raised`, `surface-sunken`, `text`.

## Accessibility

- Pattern: button.
- Name: label.
- Description: description (ClickableCard).
- States: selected (from isSelected (ClickableCard)); checked (from the group's value or bind (ClickableCard in a ClickableCardGroup)); disabled (from isDisabled (ClickableCard)).
- Up / Left, Down / Right: In a ClickableCardGroup: the previous or next card, chosen on arrival..
- Card is not a control; its content is read in order. The button pattern applies to ClickableCard.
- ClickableCard is read as its `label` only, so the label summarises what the card shows.
- Selection is a thicker accent border as well as a colour, so it does not rest on colour alone.
- In a ClickableCardGroup each card is a radio: checked, not selected, and the group takes one Tab stop.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| selected border | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use a plain Card with Buttons inside when a card has actions.
- Do: Give a ClickableCard a label that a listener can choose by: "Yearly, 40 a year".
- Do: Put cards that choose one value in a ClickableCardGroup, not side by side with isSelected.
- Do: Keep the card variant the same for cards that sit side by side.
- Don't: Put a Button, a menu or an input inside a ClickableCard; the View refuses a control inside a button.
- Don't: Nest a raised card inside another raised card.
- Don't: Rely on the outline alone to show where a card ends; it is decorative, below 3:1.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A focus ring that stays distinct on a selected card. | ring offset: a gap between the box and its ring | The selected border and the focus ring sit in the same place. |

Related: Item, Button.
