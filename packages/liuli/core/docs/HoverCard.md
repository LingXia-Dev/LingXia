<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# HoverCard

A preview of what a word names. Status: preview.

```ts
import { HoverCard } from '@liulidesign/core/HoverCard';
```

HoverCard shows a small card about the thing its trigger names, a person, a term, a reference, while a pointer rests on the trigger or keyboard focus is on it. Where there is no pointer, a tap shows it. It leaves by itself, so it holds nothing people must reach: for controls or a short form, use a Popover.

## Examples

### A person, from their name

```tsx
import { Avatar } from '@liulidesign/core/Avatar';
import { StatusDot } from '@liulidesign/core/StatusDot';
import { HoverCard } from '@liulidesign/core/HoverCard';

/** A name in a line of activity: resting on it shows who that is. */
export function Person() {
  return (
    <view className="flex-row flex-wrap items-baseline gap-1">
      <HoverCard label="Ana Lima" cardLabel="About Ana Lima" className="text-body font-medium" data-testid="ana">
        <view className="flex-row items-center gap-3">
          <Avatar name="Ana Lima" isDecorative />
          <view className="gap-0.5">
            <text className="text-body font-medium text-text">Ana Lima</text>
            <text className="text-footnote text-muted">Design lead · Lisbon</text>
          </view>
        </view>
        <StatusDot label="Online" tone="success" size="sm" />
      </HoverCard>
      <text className="text-body text-text">commented on Trip to the coast</text>
    </view>
  );
}
```

### A term, opening beside it

```tsx
import { HoverCard } from '@liulidesign/core/HoverCard';

/** A term explained where it is used, opening beside it. */
export function Term() {
  return (
    <view className="flex-row flex-wrap items-baseline gap-1">
      <text className="text-footnote text-muted">Billed per</text>
      <HoverCard label="seat" placement="end" className="text-footnote" data-testid="seat">
        <text className="text-footnote text-text">A seat is one person who can edit. People who only read are free.</text>
      </HoverCard>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The trigger\'s words and accessible name: a person, a term, a reference. |
| `children` (required) | `Child` |  | The card: a few lines about what the trigger names. Nothing that must be reached to finish a task. |
| `cardLabel` | `string` |  | Names the card for assistive tech when the trigger\'s words do not: "About Ana Lima". |
| `placement` | `HoverCardPlacement` | `'bottom'` | Which side of the trigger the card opens on. Beside the trigger it flips for room. |
| `className` | `string` |  | Classes for the trigger\'s text, to match the text around it. |
| `data-testid` | `string` |  | On the trigger; the card gets `<testid>-card`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| trigger | `<button>` | yes | The words, under a dotted line that says there is more and is not a link. |
| card | `<view>` | yes | A hint popover anchored to the trigger, in the top layer. |

## What an app may override

- `className`: Classes for the trigger\'s text, to match the text around it.
- The recipe, `hoverCardRecipe` from `@liulidesign/core/HoverCard/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--shadow-raised`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `focus`, `line-strong`, `surface-overlay`, `text`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: Moves to the trigger and shows the card.
- Escape: Hides the card.
- Press target: .
- The trigger is a button named by its words; the card is named by `cardLabel`, or the same words.
- The card shows on keyboard focus as well as on hover, and on a tap on touch screens.
- Focus does not move into the card, so put nothing in it that needs focus.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| trigger | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| card text | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.10:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep the card to a few lines that add to the trigger: who, what, a status.
- Do: Make sure everything in the card can also be reached another way, such as the page the trigger stands for.
- Don't: Put buttons, links or fields in the card; it goes away when the pointer leaves.
- Don't: Use it for help about a control; that is a Tooltip.

## Limits

- The trigger is its own row or wrapped word: it cannot sit inside a sentence as a run of text.

Related: Popover, Tooltip, Link.
