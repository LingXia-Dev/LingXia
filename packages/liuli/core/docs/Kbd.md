<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Kbd

A keyboard shortcut, drawn as key caps. Status: preview.

```ts
import { Kbd } from '@liulidesign/core/Kbd';
```

Kbd shows the keys of one shortcut, each as a small key cap, in the order they are held. It only shows them: the app binds the shortcut. It belongs where a pointer and a keyboard are present, at the end of a menu row, in a tooltip or in a list of shortcuts.

## Examples

### A list of shortcuts

```tsx
import { Kbd } from '@liulidesign/core/Kbd';

const shortcuts = [
  { key: 'search', label: 'Search notes', keys: ['⌘', 'K'], spoken: 'Command K' },
  { key: 'new', label: 'New note', keys: ['⌘', 'N'], spoken: 'Command N' },
  { key: 'archive', label: 'Move to archive', keys: ['⇧', '⌘', 'A'], spoken: 'Shift Command A' },
  { key: 'close', label: 'Close the panel', keys: ['Esc'], spoken: 'Escape' },
] as const;

/** A list of shortcuts as a Mac shows them: what each does, and its keys at the end of the row. */
export function Shortcuts() {
  return (
    <view className="gap-3">
      {shortcuts.map(shortcut => (
        <view key={shortcut.key} className="flex-row items-center justify-between gap-4">
          <text className="text-body text-text">{shortcut.label}</text>
          <Kbd keys={shortcut.keys} label={shortcut.spoken} data-testid={shortcut.key} />
        </view>
      ))}
    </view>
  );
}
```

### Beside a hint

```tsx
import { Kbd } from '@liulidesign/core/Kbd';

/** In a line of help, at the size of the text it sits with. */
export function Hint() {
  return (
    <view className="gap-2">
      <view className="flex-row items-center gap-2">
        <Kbd keys={['Enter']} size="md" data-testid="enter" />
        <text className="text-body text-muted">to send</text>
      </view>
      <view className="flex-row items-center gap-2">
        <Kbd keys={['Shift', 'Enter']} size="md" />
        <text className="text-body text-muted">for a new line</text>
      </view>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `keys` (required) | `readonly string[]` |  | The keys of one shortcut, in the order they are held: `['⌘', 'K']`. Each is drawn as a key cap. |
| `label` | `string` |  | How the shortcut is read: "Command K". Without it the keys are read as written. |
| `size` | `KbdSize` | `'sm'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row of key caps. |
| key | `<view>` | yes | One key cap: an outlined box at least as wide as it is tall. |
| label | `<text>` | yes | The key's symbol or name. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `kbdRecipe` from `@liulidesign/core/Kbd/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--radius-segment`.
- Colour roles it paints with, set by the theme pack: `line`, `surface`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Press target: .
- With `label` the shortcut is read once, as the label says: "Command K". Give it whenever a key is a symbol or short for a word.
- Without it each key is read as written, which suits keys that are words: "Enter".
- It takes no focus and no press.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| key | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write the keys the way the platform shows them: ⌘ ⌥ ⇧ on a Mac, Ctrl and Shift elsewhere.
- Do: Show a shortcut where its action is: beside the menu item, in the button's tooltip.
- Don't: Show shortcuts on a phone; there is no keyboard to press.
- Don't: Put several shortcuts in one Kbd; use one for each.

## Limits

- The keys are whatever the app passes; Liuli does not know the platform's modifier names.

Related: Tooltip, DropdownMenu.
