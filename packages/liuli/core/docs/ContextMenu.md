<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ContextMenu

A menu for one thing, asked for on the thing. Status: preview.

```ts
import { ContextMenu } from '@liulidesign/core/ContextMenu';
```

ContextMenu wraps a row, a card or a picture and opens a menu of actions for it when the user asks: a finger held on it, a secondary click, the Menu key. From a finger the menu docks to the bottom edge with the thing's name over it; from a mouse or the keyboard it opens at the point. Its items are DropdownMenu's. Nothing shows that the menu is there, so every action in it must also be reachable another way.

## Examples

### A menu on each row

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenuDivider, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Item, ItemList } from '@liulidesign/core/Item';
import { ContextMenu } from '@liulidesign/core/ContextMenu';

const notes = [
  ['coast', 'Trip to the coast', 'Edited yesterday'],
  ['reading', 'Reading list', 'Edited on Monday'],
  ['garden', 'Ideas for the garden', 'Edited last week'],
] as const;

/** Rows with a menu each: hold a row, or click it with the secondary button. The row itself still opens. */
export function Rows() {
  const last = useLocal('last', 'Nothing yet');
  return (
    <view className="gap-2">
      <ItemList label="Notes">
        {notes.map(([id, title, edited]) => (
          <ContextMenu
            key={id}
            label={title}
            data-testid={`note-${id}`}
            menu={
              <>
                <DropdownMenuItem label="Pin to top" onClick={() => last.set(`Pinned ${title}`)} data-testid={`pin-${id}`} />
                <DropdownMenuItem label="Share" onClick={() => last.set(`Shared ${title}`)} />
                <DropdownMenuDivider />
                <DropdownMenuItem label="Delete" variant="destructive" onClick={() => last.set(`Deleted ${title}`)} />
              </>
            }
          >
            <Item label={title} description={edited} hasChevron onClick={() => last.set(`Opened ${title}`)} />
          </ContextMenu>
        ))}
      </ItemList>
      <text role="status" className="text-footnote text-muted">{last.value}</text>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the menu for assistive tech, and heads it when it docks: "Trip to the coast". |
| `children` (required) | `Child` |  | What the menu is for: a row, a card, a picture. A long press or a secondary click on it asks for the menu. |
| `menu` (required) | `Child` |  | `DropdownMenuItem`s, dividers and groups, in order. The same actions must be reachable another way. |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  | Classes for the wrapper around `children`. |
| `data-testid` | `string` |  | On the wrapper; the menu gets `<testid>-menu`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| target | `<view>` | yes | The wrapper around what the menu is for; it listens for the ask. |
| menu | `<view>` | yes | A popover with the menu role: at the point, or docked to the bottom edge from a finger. |
| title | `<text>` | no | Docked: `label` over the items. |

## What an app may override

- `className`: Classes for the wrapper around `children`.
- The recipe, `contextMenuRecipe` from `@liulidesign/core/ContextMenu/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--shadow-raised`, `--shadow-modal`, `--radius-container`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Menu key or Shift+F10: Opens the menu for the focused thing.
- Up and Down: Move between items.
- Enter or Space: Runs the item and closes the menu.
- Escape: Closes the menu.
- Press target: lg 48 px.
- The menu is named by `label`; its items are menu items, and choosing one closes it.
- A screen reader user asks for the menu with the platform's own gesture for it.
- After a long press the same finger does not also press the thing.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| item | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.10:1 |
| docked title | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Offer the same actions in view too: a More button on the row, or on the page the row opens.
- Do: Keep the menu to the actions for this one thing, the destructive one last.
- Don't: Put an action only in a context menu; many people never find it.
- Don't: Wrap something that already uses a long press, such as selectable text.

## Limits

- It holds no submenu: a menu item that opens another menu closes the first.
- At a point the menu is placed from the size of the page; a very long menu can still reach past its edge.

Related: DropdownMenu, Item, BottomSheet.
