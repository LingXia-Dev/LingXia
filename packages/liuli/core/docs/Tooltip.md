<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Tooltip

A short hint naming its trigger, shown on hover, focus or a long press. Status: preview.

```ts
import { Tooltip } from '@liulidesign/core/Tooltip';
```

Tooltip wraps a trigger, usually an IconButton, and shows its name in words while a pointer rests on it or keyboard focus is on it. On a phone it shows on a long press and hides 1.5 s after release, never on a tap, so the tap still presses the trigger. For a note someone opens on purpose, use InfoTip.

## Examples

### On icon buttons

```tsx
import { IconButton } from '@liulidesign/core/IconButton';
import { Tooltip } from '@liulidesign/core/Tooltip';

/** A toolbar of icon buttons: each hint says the button's name in words. */
export function Toolbar() {
  return (
    <view className="flex-row items-center gap-1">
      <Tooltip content="Undo" data-testid="undo"><IconButton label="Undo" icon="undo" data-testid="undo" /></Tooltip>
      <Tooltip content="Redo"><IconButton label="Redo" icon="redo" /></Tooltip>
      <Tooltip content="Share"><IconButton label="Share" icon="share" /></Tooltip>
    </view>
  );
}
```

### Opening below

```tsx
import { IconButton } from '@liulidesign/core/IconButton';
import { Tooltip } from '@liulidesign/core/Tooltip';

/** Below the trigger, where there is no room above. */
export function Below() {
  return (
    <view className="flex-row">
      <Tooltip content="Print" placement="bottom" data-testid="print"><IconButton label="Print" icon="print" data-testid="print" /></Tooltip>
    </view>
  );
}
```

### On words

```tsx
import { Tooltip } from '@liulidesign/core/Tooltip';

/** Words as the trigger: wrapped in one plain button, a single focus stop. */
export function OnWords() {
  return (
    <view className="flex-row">
      <Tooltip content="Last synced 2 minutes ago" data-testid="synced"><text className="text-footnote text-muted">Synced</text></Tooltip>
    </view>
  );
}
```

### On a link in a sentence

```tsx
import { Tooltip } from '@liulidesign/core/Tooltip';
import { Link } from '@liulidesign/core/Link';

/** On a link in a sentence: the hint spells out the short name the link shows. */
export function InSentence() {
  return (
    <text className="text-body text-text">
      Shipped by{' '}
      <Tooltip content="United Parcel Service">
        <Link label="UPS" data-testid="carrier-link" />
      </Tooltip>
      {' '}this morning.
    </text>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  | The trigger, usually an IconButton; any other element is wrapped in a plain button. |
| `content` (required) | `string` |  | The hint: the trigger's own name in words, a few at most. It never says what the trigger's label lacks. |
| `placement` | `TooltipPlacement` | `'top'` | Which side of the trigger the hint opens on. |
| `data-testid` | `string` |  | On the trigger: the wrapper, or a Button or Link child without a test id of its own. The hint gets `<testid>-content`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| bubble | `<view>` | yes | The hint: a hint popover beside the trigger, in the inverse of the page. |
| text | `<text>` | yes | The hint's words; the trigger is described by them while shown. |

## What an app may override

- No class props: it takes its look from its props, the tokens and the roles below.
- The recipe, `tooltipRecipe` from `@liulidesign/core/Tooltip/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--shadow-raised`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `surface`, `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: the hint, while shown.
- Tab: Focus on the trigger shows the hint; leaving it hides it..
- Escape: Hides the hint; focus stays on the trigger..
- The trigger keeps its own name; the hint repeats it in words and never says what that name lacks.
- A Button, IconButton or ToggleButton takes the hint itself; any other element is wrapped in one plain button, one focus stop.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| hint text | `surface` on `text` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use a `Link` as the trigger for words in a sentence: put the overlay in the `<text>`, around the Link, and the panel opens from the linked words.
- Do: Give every icon-only button in a toolbar a tooltip with its name.
- Do: Keep it to a few words: "Undo", "Share".
- Do: On a phone a tooltip does not show: what someone needs to read there goes in an `InfoTip`, which opens on a tap.
- Don't: Put anything in a tooltip that is not already the trigger's name; it is not read on a tap, and many never see it.
- Don't: Put a tooltip on a button whose label is already drawn.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Showing on a long press on a phone and hiding 1.5 s after release, never on a tap; where a ContextMenu shares the trigger, the long press opens the menu instead. | a hint popover that interest shows on a long press on touch | Shown by the View's interest: a pointer resting or keyboard focus. |

Related: InfoTip, IconButton, HoverCard.
