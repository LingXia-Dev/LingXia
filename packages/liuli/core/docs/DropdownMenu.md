<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# DropdownMenu

Button that opens a short menu of actions. Status: preview.

```ts
import { DropdownMenu, DropdownMenuItem, DropdownMenuCheckboxItem, DropdownMenuGroup, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuSubmenu, DropdownMenuDivider } from '@liulidesign/core/DropdownMenu';
```

DropdownMenu groups a few related actions behind one button: sort options, a "more" menu on a row, account actions. Its children are DropdownMenuItems and DropdownMenuDividers; choosing an item runs its onClick and closes the menu. DropdownMenuCheckboxItem turns an option on or off, and DropdownMenuRadioItems in a DropdownMenuRadioGroup check one choice; both keep the menu open.

## Examples

### Labelled trigger

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';

/** A labelled trigger; the menu lines up with its start edge. */
export function Basic() {
  const order = useLocal('order', 'Name');
  return (
    <view className="flex-row items-center gap-3">
      <DropdownMenu label="Sort by" data-testid="sort">
        <DropdownMenuItem label="Name" onClick={() => order.set('Name')} data-testid="sort-name" />
        <DropdownMenuItem label="Last updated" description="Newest first" onClick={() => order.set('Last updated')} data-testid="sort-updated" />
        <DropdownMenuItem label="Size" onClick={() => order.set('Size')} />
      </DropdownMenu>
      <text data-testid="order" className="text-footnote text-muted">{order.value}</text>
    </view>
  );
}
```

### Icon-only "more" menu on a row

```tsx
import { useLocal, type VectorPath } from '@lingxia/view';
import { DropdownMenu, DropdownMenuDivider, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Said, unused } from '../example';

const dots: VectorPath[] = [{ d: 'M5 12h.01M12 12h.01M19 12h.01', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' }];
const trash: VectorPath[] = [{ d: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round' }];

/** An icon-only "more" menu at the end of a row, aligned to the row's end edge. */
export function More() {
  const removed = useLocal('removed', false);
  return (
    <Said>
      {say => (
        <view className="flex-row items-center gap-3">
          <text className="grow basis-0 text-body font-medium text-text">{removed.value ? 'Removed' : 'Groceries'}</text>
          <DropdownMenu
            label="More actions"
            isLabelHidden
            icon={<vector aria-hidden paths={dots} viewBox={[0, 0, 24, 24]} className="size-5" />}
            variant="ghost"
            alignment="end"
            data-testid="more"
          >
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
            <DropdownMenuItem label="Share" onClick={unused} isDisabled />
            <DropdownMenuDivider />
            <DropdownMenuItem
              label="Delete"
              variant="destructive"
              icon={<vector aria-hidden paths={trash} viewBox={[0, 0, 24, 24]} className="size-4" />}
              onClick={() => removed.set(true)}
              data-testid="delete"
            />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}
```

### A menu on every row

```tsx
import { type VectorPath } from '@lingxia/view';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Said } from '../example';

const dots: VectorPath[] = [{ d: 'M5 12h.01M12 12h.01M19 12h.01', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' }];

/** Rows that each carry a menu: every menu gets its own id, so no row needs one. */
export function InList() {
  const rows = ['Inbox', 'Archive'];
  return (
    <Said>
      {say => (
        <view className="gap-2">
          {rows.map(row => (
            <view key={row} className="flex-row items-center gap-3">
              <text className="grow basis-0 text-body text-text">{row}</text>
              <DropdownMenu
                label={`${row} actions`}
                isLabelHidden
                icon={<vector aria-hidden paths={dots} viewBox={[0, 0, 24, 24]} className="size-5" />}
                variant="ghost"
                size="sm"
                alignment="end"
                data-testid={`menu-${row}`}
              >
                <DropdownMenuItem label="Open" onClick={() => say(`Opened ${row}`)} />
              </DropdownMenu>
            </view>
          ))}
        </view>
      )}
    </Said>
  );
}
```

### A menu that sets a value

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';

/** A menu that sets a value: the trigger shows the choice, and the menu checks it. */
export function Value() {
  const appearance = useLocal('appearance', 'System');
  return (
    <view className="flex-row items-center gap-3">
      <text className="grow basis-0 text-body text-text">Appearance</text>
      <DropdownMenu label="Appearance" value={appearance.value} variant="ghost" size="sm" alignment="end" data-testid="appearance">
        <DropdownMenuRadioGroup value={appearance.value} onChange={next => appearance.set(next)}>
          {['System', 'Light', 'Dark'].map(option => (
            <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`appearance-${option}`} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenu>
    </view>
  );
}
```

### Options and a choice, checked

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuDivider, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';

/** Options turned on and off, and one choice among several, in one menu that stays open while they change. */
export function Checkable() {
  const completed = useLocal('completed', true);
  const archived = useLocal('archived', false);
  const grouping = useLocal('grouping', 'Day');
  return (
    <DropdownMenu label="View" data-testid="view-menu">
      <DropdownMenuCheckboxItem label="Show completed" value={completed.value} onChange={next => completed.set(next)} data-testid="show-completed" />
      <DropdownMenuCheckboxItem label="Show archived" value={archived.value} onChange={next => archived.set(next)} data-testid="show-archived" />
      <DropdownMenuDivider />
      <DropdownMenuRadioGroup label="Group by" value={grouping.value} onChange={next => grouping.set(next)}>
        {['Day', 'Week', 'Month'].map(option => (
          <DropdownMenuRadioItem key={option} value={option} label={`Group by ${option.toLowerCase()}`} data-testid={`group-${option}`} />
        ))}
      </DropdownMenuRadioGroup>
    </DropdownMenu>
  );
}
```

### Groups under headings

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuDivider, DropdownMenuGroup, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';

/** Groups under their headings, when a menu holds more than one kind of thing. */
export function Grouped() {
  const sort = useLocal('sort', 'Edited');
  const completed = useLocal('done', true);
  return (
    <DropdownMenu label="View options" data-testid="options">
      <DropdownMenuGroup label="Sort by">
        <DropdownMenuRadioGroup value={sort.value} onChange={next => sort.set(next)}>
          {['Edited', 'Created', 'Title'].map(option => (
            <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`sort-${option}`} />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuGroup>
      <DropdownMenuDivider />
      <DropdownMenuGroup label="Show">
        <DropdownMenuCheckboxItem label="Completed" value={completed.value} onChange={next => completed.set(next)} data-testid="show-done" />
      </DropdownMenuGroup>
    </DropdownMenu>
  );
}
```

### A submenu

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuDivider, DropdownMenuItem, DropdownMenuSubmenu } from '@liulidesign/core/DropdownMenu';
import { Said } from '../example';

/** A submenu for a few choices under one name; it opens at the item's side. */
export function Submenu() {
  const moved = useLocal('moved', 'Notes');
  return (
    <Said>
      {say => (
        <view className="items-start gap-2">
          <DropdownMenu label="Note" data-testid="note-menu">
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
            <DropdownMenuSubmenu label="Move to" data-testid="move">
              {['Notes', 'Travel', 'Recipes', 'Archive'].map(folder => (
                <DropdownMenuItem key={folder} label={folder} onClick={() => moved.set(folder)} data-testid={`move-${folder.toLowerCase()}`} />
              ))}
            </DropdownMenuSubmenu>
            <DropdownMenuDivider />
            <DropdownMenuItem label="Delete" variant="destructive" onClick={() => say('Chose Delete')} />
          </DropdownMenu>
          <text role="status" className="text-footnote text-muted">{`In ${moved.value}`}</text>
        </view>
      )}
    </Said>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The trigger's text, the trigger's accessible name and the menu's accessible name. |
| `value` | `string` |  | For a menu that sets a value: the current choice, drawn on the trigger in place of `label` and read after it as the trigger's description (label "Appearance", value "System"). |
| `isLabelHidden` | `boolean` | `false` | Draws only `icon` on a square trigger; `label` stays the name. |
| `icon` | `Child` |  | An icon on the trigger, before the label. |
| `hasChevron` | `boolean` |  | A down chevron after the label. Defaults to shown unless the label is hidden. |
| `variant` | `ButtonVariant` | `'secondary'` | The trigger's look, from Button. |
| `size` | `ButtonSize` | `'md'` | The trigger's and the items' height. |
| `children` (required) | `Child` |  | `DropdownMenuItem`s and `DropdownMenuDivider`s, in order. |
| `isOpen` | `boolean` |  | Controls whether the menu is open; leave it out to let the trigger own it. Opened from code, the menu is placed by its classes, not under the trigger. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `placement` | `DropdownMenuPlacement` | `'bottom'` | Which side of the trigger the menu opens on. |
| `alignment` | `DropdownMenuAlignment` | `'start'` | Which edge of the trigger the menu lines up with. |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  | Classes for the trigger. |
| `menuClassName` | `string` |  | Classes for the menu panel. |
| `data-testid` | `string` |  | On the trigger. |

## DropdownMenuItem props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The item's text and its accessible name. |
| `description` | `string` |  | A second line, read as the item's description. |
| `icon` | `Child` |  | Drawn before the label; hidden from assistive tech. |
| `onClick` (required) | `() => void` |  | Runs on choice; the menu then closes. |
| `isDisabled` | `boolean` | `false` |  |
| `variant` | `DropdownMenuItemVariant` | `'default'` | `destructive` draws the label and the icon in the danger role. |
| `data-testid` | `string` |  |  |

## DropdownMenuCheckboxItem props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The item's text and its accessible name. |
| `description` | `string` |  | A second line, read as the item's description. |
| `value` (required) | `boolean` |  |  |
| `onChange` (required) | `(isChecked: boolean) => void` |  | Runs with the new state; the menu stays open. |
| `isDisabled` | `boolean` | `false` |  |
| `data-testid` | `string` |  |  |

## DropdownMenuRadioGroup props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` (required) | `string \| null` |  | The checked item's value; `null` while none is. |
| `onChange` (required) | `(value: string) => void` |  | Runs with the value of the item chosen; the menu stays open. |
| `children` (required) | `Child` |  | `DropdownMenuRadioItem`s. |
| `label` | `string` |  | Names the choices for assistive input when the menu's label does not, as in a menu with other items. |

## DropdownMenuRadioItem props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` (required) | `string` |  | What the group's `onChange` carries. |
| `label` (required) | `string` |  | The item's text and its accessible name. |
| `description` | `string` |  | A second line, read as the item's description. |
| `isDisabled` | `boolean` | `false` |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| chevron | `<vector>` | no | The trigger is a Button-styled popover invoker; the chevron marks it as opening a menu. |
| menu | `<view>` | yes | A popover with the menu role, anchored to the trigger, in the top layer. |
| item | `<button>` | yes | DropdownMenuItem: a menu item button with icon, label and description. |
| itemCheck | `<vector>` | no | A check at the end of a checked checkbox or radio item; hidden from assistive tech. |
| divider | `<view>` | no | DropdownMenuDivider: a hairline between groups; hidden from assistive tech. |
| submenuChevron | `<vector>` | no | DropdownMenuSubmenu: an item with a chevron that opens a second menu at its side; choosing there closes both. |
| back | `<button>` | no | On a narrow screen a submenu takes the panel instead: this first row names it and goes back to the menu. |

## What an app may override

- `className`: Classes for the trigger.
- `menuClassName`: Classes for the menu panel.
- The recipe, `dropdownMenuRecipe` from `@liulidesign/core/DropdownMenu/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-caption`, `--text-footnote`, `--text-body`, `--shadow-raised`, `--radius-control`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: menuButton.
- Name: label.
- Description: none.
- States: expanded (from the trigger, while the menu is open (set by the View)); checked (from a checkbox item's value; a radio item matching its group's value); disabled (from isDisabled; an item's isDisabled).
- Enter, Space on the trigger: opens the menu on its first item.
- Up, Down: moves between items, wrapping.
- Home, End: first and last item.
- A letter: the next item whose name starts with it.
- Escape: closes and returns focus to the trigger; inside a submenu that took the panel, goes back to the menu first.
- Press target: sm 32 px, md 40 px, lg 48 px.
- An item is named by its label and then its description, both inside the item.
- Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.
- A tap outside, Escape and back close the menu; that tap reaches nothing else.
- On a narrow screen the row that opens a submenu and the row that goes back are menu items that keep the menu open, so the arrow keys reach them; Escape and back go up one level before they close the menu.
- Checkbox and radio items are read as checked or not, and keep the menu open so the change can be seen.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| item text | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| destructive icon | `danger` on `surface-overlay` | 3:1 | 5.21:1 | 3.95:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep it to a handful of actions; put destructive ones last, after a divider.
- Do: Use the end alignment for a trigger at the end of a row.
- Don't: Use it to choose a value that should stay visible.
- Don't: Put it inside a Button; a popover cannot sit in a pressable control.
- Don't: Nest menus.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A submenu inside a submenu, and the menu itself opening at the trigger's side. | nothing from the View | One level of submenu; the menu opens under or over its trigger. |
| Anchoring when opened from code with isOpen. | a popover anchor that does not depend on which control opened it | Opened from code it is placed by its classes; the trigger is the normal way in. |
| A destructive item in danger-coloured text. | a text-grade danger role (4.5:1 on surface-overlay) | The icon carries the danger colour; the text stays in the text role. |

Related: Button, ContextMenu.
