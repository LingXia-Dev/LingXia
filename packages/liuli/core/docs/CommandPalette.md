<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# CommandPalette

Type to find a command or a place, and run it. Status: preview.

```ts
import { CommandPalette } from '@liulidesign/core/CommandPalette';
```

CommandPalette is a dialog near the top of the screen with a search line over one list of results in named groups. Typing narrows the list, best match first; the arrow keys move a highlight that stays in view, and Return runs the row. On a phone it spans the width from the top edge, so the results scroll above the keyboard. The app opens it, from a button or its own shortcut, and can take over the filtering to fetch results.

## Examples

### Commands and places

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { CommandPalette, type CommandGroup } from '@liulidesign/core/CommandPalette';

const groups: CommandGroup[] = [
  { label: 'Go to', items: [
    { id: 'all', label: 'All notes', icon: 'file', endText: 'G then A' },
    { id: 'shared', label: 'Shared with me', icon: 'users', endText: 'G then S' },
    { id: 'archive', label: 'Archive', icon: 'folder' },
    { id: 'settings', label: 'Settings', icon: 'settings', description: 'Account, appearance, sync' },
  ] },
  { label: 'Actions', items: [
    { id: 'new', label: 'New note', icon: 'add', endText: 'Ctrl N' },
    { id: 'share', label: 'Share this note', icon: 'share' },
    { id: 'export', label: 'Export as PDF', icon: 'upload', isDisabled: true },
  ] },
];

/** Opened from a button here; an app also opens it from its own shortcut. Type to narrow, arrows to move, Return to run. */
export function Commands() {
  const open = useLocal('open', false);
  const ran = useLocal('ran', 'Nothing yet');
  return (
    <view className="items-start gap-2">
      <Button label="Search commands" startIcon="search" onClick={() => open.set(true)} data-testid="palette-open" />
      <text role="status" className="text-footnote text-muted">{`Last run: ${ran.value}`}</text>
      <CommandPalette
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Search commands"
        placeholder="Type a command or a place"
        groups={groups}
        onSelect={id => ran.set(id)}
        emptyText="No commands match"
        data-testid="palette"
      />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `isOpen` (required) | `boolean` |  | Whether it is shown. A closed palette renders nothing, so it opens empty each time. |
| `onOpenChange` (required) | `(isOpen: boolean) => void` |  | Called with `false` on a close request and after a choice. |
| `label` (required) | `string` |  | The palette\'s accessible name, and the search line\'s: "Search commands". |
| `placeholder` | `string` |  |  |
| `groups` (required) | `readonly CommandGroup[]` |  | Everything that can be chosen, in groups. Liuli filters them by what is typed, unless the app does. |
| `onSelect` (required) | `(id: string) => void` |  | Runs with the chosen item\'s id; the palette then closes. |
| `onQueryChange` | `((query: string) => void)` |  | Hands filtering to the app: called with the text as it is typed, and `groups` is shown as given. For results fetched from Logic. |
| `isLoading` | `boolean` | `false` | Shows a spinner in the search line while the app fetches. |
| `emptyText` (required) | `string` |  | Shown when nothing matches: "No commands match". |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<dialog>` | yes | The dialog primitive, anchored near the top. |
| search | `<view>` | yes | The search line: an icon, the editor, a spinner while loading. |
| list | `<scroll>` | no | The results: a listbox that scrolls. |
| group | `<view>` | no | A named group with its quiet heading. |
| option | `<button>` | no | One result: icon, label, a second line, and quiet text at the end. |
| note | `<text>` | no | `emptyText`, when nothing matches. |

## What an app may override

- No class props: it takes its look from its props, the tokens and the roles below.
- The recipe, `commandPaletteRecipe` from `@liulidesign/core/CommandPalette/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-lg`, `--text-caption`, `--text-footnote`, `--text-body`, `--shadow-modal`, `--radius-control`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: dialog.
- Name: label.
- Description: none.
- States: undefined (from undefined); undefined (from undefined).
- Up and Down: Move the highlight; rows that cannot be chosen are passed over.
- Home and End: To the first and the last row.
- Return: Runs the highlighted row and closes.
- Escape: Closes.
- Press target: lg 48 px.
- Focus stays in the search line, a combobox that names the highlighted row as it moves.
- The results are a listbox in named groups; a row with `endText` is read as "label, end text".
- When nothing matches, the note is a status and is read.
- A long list is cut by the bottom edge of the panel on purpose: a row cut in half says there is more to scroll.
- Closing returns focus to what had it.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| row | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.10:1 |
| second line and end text | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Offer every command somewhere else too; the palette is a shortcut.
- Do: Start each label with the word people will type: "New note", not "Create a new note".
- Don't: Use it to choose a value in a form; that is a Combobox.
- Don't: Put more than a few dozen rows in it unfiltered; hand filtering to the app and fetch.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| undefined | undefined | undefined |
| undefined | undefined | undefined |

Related: Combobox, SearchField, DropdownMenu, Kbd.
