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
        <ToggleButton label="Unread" size="sm" bind={unread} data-testid="unread" />
        <ToggleButton label="Starred" size="sm" bind={starred} data-testid="starred" />
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
      <ToggleButton label="Wrap lines" bind={wrap} className="self-start" data-testid="wrap" />
      <text className={wrap.value ? 'text-body text-text' : 'text-body text-text truncate'} data-testid="sample">
        Every line of this note is long enough to reach past the edge of a phone held upright.
      </text>
    </view>
  );
}
```

### With icons, and icon-only

```tsx
import { useLocal } from '@lingxia/view';
import { ToggleButton } from '@liulidesign/core/ToggleButton';

/** An icon beside the label; an icon that changes with the state; icon-only toggles in a toolbar, named by their labels. */
export function Icons() {
  const starred = useLocal('starred', true);
  const muted = useLocal('muted', false);
  const hidden = useLocal('hidden', false);
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <ToggleButton label="Starred" icon="star" bind={starred} data-testid="icons-starred" />
      <ToggleButton label="Mute" icon="volume" pressedIcon="mute" isLabelHidden bind={muted} data-testid="icons-mute" />
      <ToggleButton label="Show hidden files" icon="visible" pressedIcon="hidden" isLabelHidden bind={hidden} data-testid="icons-hidden" />
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
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Small" size="sm" value={isPressed} onChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Medium" value={isPressed} onChange={set} />}</Held>
      <Held value={true}>{(isPressed, set) => <ToggleButton label="Large" size="lg" value={isPressed} onChange={set} />}</Held>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<boolean>` |  | A View-local cell the control reads and writes; use it or `value` with `onChange`. |
| `value` | `boolean` |  | Whether it is on. |
| `onChange` | `((isPressed: boolean) => void)` |  | Runs with the new state on a press. |
| `icon` | `IconRef` |  | An icon by name from the set, or a definition, before the label. |
| `pressedIcon` | `IconRef` |  | The icon while on, where the icon itself shows the state: `volume` and `mute`. The fill and edge still show it too. |
| `isLabelHidden` | `boolean` | `false` | Shows the icon alone, in a square, for a toolbar; the label still names it. |
| `label` (required) | `string` |  | The text and the accessible name; it stays the same on and off ("Unread", not "Show unread"/"Hide unread"). |
| `size` | `ToggleButtonSize` | `'md'` |  |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

- `bind` | `value` + `onChange`: props from one side only.

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<button>` | yes | The pressable box: a secondary button while off, the soft accent fill and an accent hairline while on. |
| glyph | `<vector>` | no | The icon before the label, or alone in a square; `pressedIcon` while on. |
| label | `<text>` | no | The text and the name; the same on and off. Not drawn with `isLabelHidden`, but still the name. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `toggleButtonRecipe` from `@liulidesign/core/ToggleButton/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--text-callout`, `--shadow-control`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `focus`, `line-strong`, `surface`, `surface-overlay`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: toggleButton.
- Name: label.
- Description: none.
- States: pressed (from value or bind); disabled (from isDisabled).
- Enter, Space: Turns it on or off..
- Press target: sm 32 px, md 40 px, lg 48 px.
- The state is read as pressed and drawn as the fill and the edge, never as the label's colour alone.
- Disabled keeps the soft fill when on, so a locked option still shows its state.
- An icon-only toggle keeps its label as its name; `pressedIcon` changes the picture, never the name.
- Sizes sm and md are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn. Leave room between neighbours, since two bands that meet share the gap.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| label, off | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| label, on | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| boundary, off | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| boundary, off, on a card | `line-strong` on `surface-raised` | 3:1 | 3.47:1 | 3.01:1 |
| boundary, off, in an overlay | `line-strong` on `surface-overlay` | 3:1 | 3.47:1 | 2.57:1, below |
| disabled label, off, on a card | `text-muted` on `text/6` over `surface` | 4.5:1 | 5.24:1 | 5.72:1 |
| disabled label, on | `text-muted` on `accent-soft` | 4.5:1 | 5.16:1 | 5.39:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Label the option, not the action: "Unread", not "Show unread".
- Do: Put it next to what it changes, and apply the change at once.
- Don't: Change the label with the state; the pressed state already says which.
- Don't: Line up loose toggle buttons where only one may be on; that is a ToggleButtonGroup with `selectionMode="single"`, or a SegmentedControl for a setting.
- Don't: Use it for a setting saved with a form; that is a Switch or a Checkbox.

Related: ToggleButtonGroup, Button, SegmentedControl, Switch.
