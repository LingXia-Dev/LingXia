<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# DropdownMenu

A short menu of actions that opens from its trigger. Status: preview.

```ts
import { DropdownMenu, DropdownMenuItem, DropdownMenuCheckboxItem, DropdownMenuGroup, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuSubmenu, DropdownMenuDivider } from '@liulidesign/core/DropdownMenu';
```

DropdownMenu groups a few related actions behind one trigger: sort options, a "more" menu on a row, account actions. Its children are the trigger, a Button or IconButton; `menu` holds DropdownMenuItems and DropdownMenuDividers, and choosing an item runs its onClick and closes the menu. DropdownMenuCheckboxItem turns an option on or off, and DropdownMenuRadioItems in a DropdownMenuRadioGroup check one choice; both keep the menu open. `items` takes the same rows as data instead of `menu`: actions, `{ type: 'divider' }`, `{ type: 'group', label, items }`, and an action with `items` of its own as a submenu; checkable items are composed in `menu`.

## Examples

### Labelled trigger

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Button } from '@liulidesign/core/Button';

/** A labelled trigger; the menu lines up with its start edge. */
export function Basic() {
  const order = useLocal('order', 'Name');
  return (
    <view className="flex-row items-center gap-3">
      <DropdownMenu
        label="Sort by"
        menu={<>
          <DropdownMenuItem label="Name" onClick={() => order.set('Name')} data-testid="sort-name" />
          <DropdownMenuItem label="Last updated" description="Newest first" onClick={() => order.set('Last updated')} data-testid="sort-updated" />
          <DropdownMenuItem label="Size" onClick={() => order.set('Size')} />
        </>}
      >
        <Button label="Sort by" endIcon="chevron-down" data-testid="sort" />
      </DropdownMenu>
      <text data-testid="order" className="text-footnote text-muted">{order.value}</text>
    </view>
  );
}
```

### Icon-only "more" menu on a row

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuDivider, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Said, unused } from '../example';
import { IconButton } from '@liulidesign/core/IconButton';

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
            alignment="end"
            menu={<>
              <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
              <DropdownMenuItem label="Share" onClick={unused} isDisabled />
              <DropdownMenuDivider />
              <DropdownMenuItem
                label="Delete"
                variant="destructive"
                icon="delete"
                onClick={() => removed.set(true)}
                data-testid="delete"
              />
            </>}
          >
            <IconButton label="More actions" icon="more" data-testid="more" />
          </DropdownMenu>
        </view>
      )}
    </Said>
  );
}
```

### A menu on every row

```tsx
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Said } from '../example';
import { IconButton } from '@liulidesign/core/IconButton';

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
              <DropdownMenu label={`${row} actions`} size="sm" alignment="end" menu={<DropdownMenuItem label="Open" onClick={() => say(`Opened ${row}`)} />}>
                <IconButton label={`${row} actions`} icon="more" size="sm" data-testid={`menu-${row}`} />
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
import { Button } from '@liulidesign/core/Button';

/** A menu that sets a value: the trigger shows the choice, and the menu checks it. */
export function Value() {
  const appearance = useLocal('appearance', 'System');
  return (
    <view className="flex-row items-center gap-3">
      <text className="grow basis-0 text-body text-text">Appearance</text>
      <DropdownMenu
        label="Appearance"
        size="sm"
        alignment="end"
        menu={
          <DropdownMenuRadioGroup value={appearance.value} onChange={next => appearance.set(next)}>
            {['System', 'Light', 'Dark'].map(option => (
              <DropdownMenuRadioItem key={option} value={option} label={option} data-testid={`appearance-${option}`} />
            ))}
          </DropdownMenuRadioGroup>
        }
      >
        <Button label={appearance.value} variant="ghost" size="sm" endIcon="chevron-down" data-testid="appearance" />
      </DropdownMenu>
    </view>
  );
}
```

### Options and a choice, checked

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuDivider, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';
import { Button } from '@liulidesign/core/Button';

/** Options turned on and off, and one choice among several, in one menu that stays open while they change. */
export function Checkable() {
  const completed = useLocal('completed', true);
  const archived = useLocal('archived', false);
  const grouping = useLocal('grouping', 'Day');
  return (
    <DropdownMenu label="View" menu={<>
      <DropdownMenuCheckboxItem label="Show completed" value={completed.value} onChange={next => completed.set(next)} data-testid="show-completed" />
      <DropdownMenuCheckboxItem label="Show archived" value={archived.value} onChange={next => archived.set(next)} data-testid="show-archived" />
      <DropdownMenuDivider />
      <DropdownMenuRadioGroup label="Group by" value={grouping.value} onChange={next => grouping.set(next)}>
        {['Day', 'Week', 'Month'].map(option => (
          <DropdownMenuRadioItem key={option} value={option} label={`Group by ${option.toLowerCase()}`} data-testid={`group-${option}`} />
        ))}
      </DropdownMenuRadioGroup>
    </>}>
      <Button label="View" endIcon="chevron-down" data-testid="view-menu" />
    </DropdownMenu>
  );
}
```

### An item for a set, partly on

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuDivider } from '@liulidesign/core/DropdownMenu';
import { Button } from '@liulidesign/core/Button';

const labels = ['Work', 'Personal', 'Travel'] as const;

/** An item that turns a set on and off: mixed while only some of the set is on. */
export function StateMixed() {
  const shown = useLocal<readonly string[]>('labels', ['Work']);
  const isAll = shown.value.length === labels.length;
  return (
    <view className="flex-row">
      <DropdownMenu label="Labels" menu={<>
        <DropdownMenuCheckboxItem label="All labels" value={isAll} isIndeterminate={shown.value.length > 0} onChange={next => shown.set(next ? labels : [])} data-testid="labels-all" />
        <DropdownMenuDivider />
        {labels.map(label => (
          <DropdownMenuCheckboxItem key={label} label={label} value={shown.value.includes(label)}
            onChange={next => shown.set(next ? [...shown.value, label] : shown.value.filter(each => each !== label))} data-testid={`label-${label}`} />
        ))}
      </>}>
        <Button label="Labels" endIcon="chevron-down" data-testid="labels-menu" />
      </DropdownMenu>
    </view>
  );
}
```

### Groups under headings

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuDivider, DropdownMenuGroup, DropdownMenuRadioGroup, DropdownMenuRadioItem } from '@liulidesign/core/DropdownMenu';
import { Button } from '@liulidesign/core/Button';

/** Groups under their headings, when a menu holds more than one kind of thing. */
export function Grouped() {
  const sort = useLocal('sort', 'Edited');
  const completed = useLocal('done', true);
  return (
    <DropdownMenu label="View options" menu={<>
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
    </>}>
      <Button label="View options" endIcon="chevron-down" data-testid="options" />
    </DropdownMenu>
  );
}
```

### A submenu

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuDivider, DropdownMenuItem, DropdownMenuSubmenu } from '@liulidesign/core/DropdownMenu';
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';

/** A submenu for a few choices under one name; it opens at the item's side. */
export function Submenu() {
  const moved = useLocal('moved', 'Notes');
  return (
    <Said>
      {say => (
        <view className="items-start gap-2">
          <DropdownMenu label="Note" menu={<>
            <DropdownMenuItem label="Rename" onClick={() => say('Chose Rename')} />
            <DropdownMenuSubmenu label="Move to" data-testid="move">
              {['Notes', 'Travel', 'Recipes', 'Archive'].map(folder => (
                <DropdownMenuItem key={folder} label={folder} onClick={() => moved.set(folder)} data-testid={`move-${folder.toLowerCase()}`} />
              ))}
            </DropdownMenuSubmenu>
            <DropdownMenuDivider />
            <DropdownMenuItem label="Delete" variant="destructive" onClick={() => say('Chose Delete')} />
          </>}>
            <Button label="Note" endIcon="chevron-down" data-testid="note-menu" />
          </DropdownMenu>
          <text role="status" className="text-footnote text-muted">{`In ${moved.value}`}</text>
        </view>
      )}
    </Said>
  );
}
```

### The rows as data

```tsx
import { DropdownMenu } from '@liulidesign/core/DropdownMenu';
import { Said, unused } from '../example';
import { Button } from '@liulidesign/core/Button';

/** The rows as data: actions, a submenu, a group under a heading and a destructive action after a divider. */
export function Data() {
  return (
    <Said>
      {say => (
        <DropdownMenu
          label="Note actions"
          items={[
            { label: 'Rename', icon: 'edit', onClick: () => say('Chose Rename'), 'data-testid': 'data-rename' },
            { label: 'Duplicate', icon: 'copy', onClick: () => say('Chose Duplicate') },
            {
              label: 'Move to',
              icon: 'folder',
              items: [
                { label: 'Inbox', onClick: () => say('Moved to Inbox'), 'data-testid': 'data-inbox' },
                { label: 'Archive', onClick: () => say('Moved to Archive') },
              ],
              'data-testid': 'data-move',
            },
            { type: 'divider' },
            {
              type: 'group',
              label: 'Share',
              items: [
                { label: 'Copy link', icon: 'link', onClick: () => say('Copied the link') },
                { label: 'Invite someone', icon: 'share', description: 'They can edit it', onClick: unused, isDisabled: true },
              ],
            },
            { type: 'divider' },
            { label: 'Delete', icon: 'delete', variant: 'destructive', onClick: () => say('Chose Delete'), 'data-testid': 'data-delete' },
          ]}
        >
          <Button label="Actions" endIcon="chevron-down" data-testid="data" />
        </DropdownMenu>
      )}
    </Said>
  );
}
```

### On a link in a sentence

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';
import { Link } from '@liulidesign/core/Link';

/** On a link in a sentence: the choice is made where it is read. */
export function InSentence() {
  const order = useLocal('sentence-order', 'newest');
  return (
    <text className="text-body text-text" data-testid="sentence">
      Showing notes,{' '}
      <DropdownMenu
        label="Order"
        menu={<>
          <DropdownMenuItem label="Newest first" onClick={() => order.set('newest')} />
          <DropdownMenuItem label="Oldest first" onClick={() => order.set('oldest')} />
        </>}
      >
        <Link label={`${order.value} first`} data-testid="order-link" />
      </DropdownMenu>
      .
    </text>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` (required) | `Child` |  | The trigger: a Button or an IconButton, or any element, which is then wrapped in a plain button. |
| `menu` | `Child` |  | `DropdownMenuItem`s, dividers, groups and checkable items, in order. Give this or `items`, not both. |
| `items` | `readonly DropdownMenuEntry[]` |  | The rows as data: actions, dividers, groups and submenus. Checkable items are composed in `menu`. |
| `label` (required) | `string` |  | The menu's accessible name. |
| `size` | `ButtonSize` | `'md'` | The items' height; match the trigger's size. |
| `isOpen` | `boolean` |  | Controls whether the menu is open; leave it out to let the trigger own it. Opened from code, the menu is placed by its classes, not under the trigger. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `placement` | `DropdownMenuPlacement` | `'bottom'` | Which side of the trigger the menu opens on. |
| `alignment` | `DropdownMenuAlignment` | `'start'` | Which edge of the trigger the menu lines up with. |
| `menuClassName` | `string` |  | Classes for the menu panel. |
| `data-testid` | `string` |  | On the trigger: the wrapper, or a Button or Link child without a test id of its own. The menu gets `<testid>-menu`. |

## DropdownMenuItemData props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `id` | `string` | `its place in the list` | The row's own name: keys it, so it keeps its focus when the list reorders or grows; unique within its list. |
| `label` (required) | `string` |  | The row's text and its accessible name. |
| `description` | `string` |  | A second line, read as the row's description. Not on a submenu. |
| `icon` | `IconRef` |  | An icon by name from the set, or a definition, before the label; hidden from assistive tech. |
| `onClick` | `(() => void)` |  | Runs on choice, with nothing; the menu then closes. A row that keeps the menu open is a checkable row, composed in `menu`. Leave it out on a submenu. |
| `items` | `readonly DropdownMenuItemData[]` |  | Makes the row a submenu of these actions, one level deep. |
| `isDisabled` | `boolean` | `false` |  |
| `variant` | `DropdownMenuItemVariant` | `'default'` | `destructive` draws the icon in the danger role. Not on a submenu. |
| `data-testid` | `string` |  |  |

## DropdownMenuItem props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The item's text and its accessible name. |
| `description` | `string` |  | A second line, read as the item's description. |
| `icon` | `IconRef` |  | An icon by name from the set, or a definition, before the label; hidden from assistive tech. |
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
| `isIndeterminate` | `boolean` | `false` | Stands for a set that is partly on, such as "All labels": draws a dash and is read as mixed. A press asks for all. |
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
| menu | `<view>` | yes | A popover with the menu role, anchored to the trigger, in the top layer. |
| item | `<button>` | yes | DropdownMenuItem: a menu item button with icon, label and description. |
| itemCheck | `<vector>` | no | A check at the end of a checked checkbox or radio item; hidden from assistive tech. |
| divider | `<view>` | no | DropdownMenuDivider: a hairline between groups; hidden from assistive tech. |
| submenuChevron | `<vector>` | no | DropdownMenuSubmenu: an item with a chevron that opens a second menu at its side; choosing there closes both. |
| back | `<button>` | no | On a narrow screen a submenu takes the panel instead: this first row names it and goes back to the menu. |

## What an app may override

- `menuClassName`: Classes for the menu panel.
- The recipe, `dropdownMenuRecipe` from `@liulidesign/core/DropdownMenu/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-caption`, `--text-footnote`, `--text-body`, `--shadow-raised`, `--radius-control`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: menuButton.
- Name: label.
- Description: none.
- States: expanded (from the trigger, while the menu is open (set by the View)); checked (from a checkbox item's value; a radio item matching its group's value); disabled (from the trigger's isDisabled; an item's isDisabled).
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

- Do: At the end of a row, open it from a ghost Button with `edge="end"`: its padding hangs past the edge, so the chevron lines up with the values and badges in the rows around it.
- Do: Use a `Link` as the trigger for words in a sentence: put the overlay in the `<text>`, around the Link, and the panel opens from the linked words.
- Do: Keep it to a handful of actions; put destructive ones last, after a divider.
- Do: Use the end alignment for a trigger at the end of a row.
- Do: Give a labelled trigger the `chevron-down` end icon, so it reads as opening a menu.
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
