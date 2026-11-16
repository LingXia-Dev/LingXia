<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ToggleButton

A button that turns one option on or off and stays down while on. Status: preview.

```ts
import { ToggleButton } from '@liulidesign/core/ToggleButton';
```

ToggleButton turns an option on or off where it applies, such as a filter over a list or a view option beside what it changes. It keeps its label in both states and reports pressed; while on it takes the soft accent fill. For a setting in a form use Switch; to choose one of a few values use SegmentedControl.

## Examples

### Filters over a list

```tsx
import { useLocal } from '@lingxia/view';
import { ToggleButton } from '@liulidesign/core/ToggleButton';

const notes = [
  { title: 'Q3 plan', unread: true, starred: true },
  { title: 'Reading list', unread: false, starred: true },
  { title: 'Interview notes', unread: true, starred: false },
  { title: 'Groceries', unread: false, starred: false },
];

/** Filters over a list: each one narrows it on its own. */
export function Filters() {
  const unread = useLocal('unread', false);
  const starred = useLocal('starred', false);
  const shown = notes.filter(note => (!unread.value || note.unread) && (!starred.value || note.starred));
  return (
    <view className="gap-3">
      <view role="group" aria-label="Filters" className="flex-row flex-wrap gap-2">
        <ToggleButton label="Unread" size="sm" isPressed={unread.value} onPressedChange={next => unread.set(next)} data-testid="unread" />
        <ToggleButton label="Starred" size="sm" isPressed={starred.value} onPressedChange={next => starred.set(next)} data-testid="starred" />
      </view>
      <text className="text-footnote text-muted" data-testid="count">{shown.length === 1 ? '1 note' : `${shown.length} notes`}</text>
    </view>
  );
}
```

### A view option

```tsx
import { useLocal } from '@lingxia/view';
import { ToggleButton } from '@liulidesign/core/ToggleButton';

/** One view option, beside the thing it changes. */
export function WordWrap() {
  const wrap = useLocal('wrap', true);
  return (
    <view className="gap-3">
      <ToggleButton label="Wrap lines" isPressed={wrap.value} onPressedChange={next => wrap.set(next)} className="self-start" data-testid="wrap" />
      <text className={wrap.value ? 'text-body text-text' : 'text-body text-text truncate'} data-testid="sample">
        Every line of this note is long enough to reach past the edge of a phone held upright.
      </text>
    </view>
  );
}
```

### Sizes

```tsx
import { Held } from '../example';
import { ToggleButton } from '@liulidesign/core/ToggleButton';

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Small" size="sm" isPressed={isPressed} onPressedChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Medium" isPressed={isPressed} onPressedChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Large" size="lg" isPressed={isPressed} onPressedChange={set} />}</Held>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The text and the accessible name; it stays the same on and off ("Unread", not "Show unread"/"Hide unread"). |
| `isPressed` (required) | `boolean` |  | Whether it is on. |
| `onPressedChange` (required) | `(isPressed: boolean) => void` |  | Runs with the new state on a press. |
| `size` | `ToggleButtonSize` | `'md'` |  |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<button>` | yes | The pressable box: a secondary button while off, the soft accent fill and an accent hairline while on. |
| label | `<text>` | yes | The text and the name; the same on and off. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `toggleButtonRecipe` from `@liulidesign/core/ToggleButton/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--text-callout`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `focus`, `surface`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: toggleButton.
- Name: label.
- Description: none.
- States: pressed (from isPressed); disabled (from isDisabled).
- Enter, Space: Turns it on or off..
- Press target: sm 32 px, md 40 px, lg 48 px.
- The state is read as pressed and drawn as the fill and the edge, never as the label's colour alone.
- Disabled keeps the soft fill when on, so a locked option still shows its state.
- Sizes sm and md are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn. Leave room between neighbours, since two bands that meet share the gap.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| label, off | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| label, on | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| boundary, off | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| disabled label, off, on a card | `text-muted` on `text/6` over `surface` | 4.5:1 | 5.24:1 | 5.72:1 |
| disabled label, on | `text-muted` on `accent-soft` | 4.5:1 | 5.16:1 | 5.39:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Label the option, not the action: "Unread", not "Show unread".
- Do: Put it next to what it changes, and apply the change at once.
- Don't: Change the label with the state; the pressed state already says which.
- Don't: Use a row of toggle buttons where only one may be on; that is a SegmentedControl.
- Don't: Use it for a setting saved with a form; that is a Switch or a Checkbox.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Icon-only toggle buttons, such as bold or italic in an editor toolbar. | an icon set for Liuli | Toggle buttons carry a text label. |

Related: Button, SegmentedControl, Switch.
