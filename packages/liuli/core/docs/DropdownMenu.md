<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# DropdownMenu

Button that opens a short menu of actions. Status: preview.

```ts
import { DropdownMenu, DropdownMenuItem, DropdownMenuDivider } from '@liulidesign/core/DropdownMenu';
```

DropdownMenu groups a few related actions behind one button: sort options, a "more" menu on a row, account actions. Its children are DropdownMenuItems and DropdownMenuDividers; choosing an item runs its onClick and closes the menu.

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

const dots: VectorPath[] = [{ d: 'M5 12h.01M12 12h.01M19 12h.01', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' }];
const trash: VectorPath[] = [{ d: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinejoin: 'round' }];

/** An icon-only "more" menu at the end of a row, aligned to the row's end edge. */
export function More() {
  const removed = useLocal('removed', false);
  return (
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
        <DropdownMenuItem label="Rename" onClick={() => {}} />
        <DropdownMenuItem label="Share" onClick={() => {}} isDisabled />
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
  );
}
```

### A menu on every row

```tsx
import { type VectorPath } from '@lingxia/view';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';

const dots: VectorPath[] = [{ d: 'M5 12h.01M12 12h.01M19 12h.01', fill: 'none', stroke: 'currentColor', strokeWidth: 3, strokeLinecap: 'round' }];

/** Rows that each carry a menu: every menu gets its own id, so no row needs one. */
export function InList() {
  const rows = ['Inbox', 'Archive'];
  return (
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
            <DropdownMenuItem label="Open" onClick={() => {}} />
          </DropdownMenu>
        </view>
      ))}
    </view>
  );
}
```

### A menu that sets a value

```tsx
import { useLocal } from '@lingxia/view';
import { DropdownMenu, DropdownMenuItem } from '@liulidesign/core/DropdownMenu';

/** A menu that sets a value: the trigger shows the choice, and the label stays its name. */
export function Value() {
  const appearance = useLocal('appearance', 'System');
  return (
    <view className="flex-row items-center gap-3">
      <text className="grow basis-0 text-body text-text">Appearance</text>
      <DropdownMenu label="Appearance" value={appearance.value} variant="ghost" size="sm" alignment="end" data-testid="appearance">
        {['System', 'Light', 'Dark'].map(option => (
          <DropdownMenuItem key={option} label={option} onClick={() => appearance.set(option)} data-testid={`appearance-${option}`} />
        ))}
      </DropdownMenu>
    </view>
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
| `variant` | `DropdownMenuItemVariant` | `'default'` | `destructive` tints the icon with the danger role; the text stays readable. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| chevron | `<vector>` | no | The trigger is a Button-styled popover invoker; the chevron marks it as opening a menu. |
| menu | `<view>` | yes | A popover with the menu role, anchored to the trigger, in the top layer. |
| item | `<button>` | yes | DropdownMenuItem: a menu item button with icon, label and description. |
| divider | `<view>` | no | DropdownMenuDivider: a hairline between groups; hidden from assistive tech. |

## Accessibility

- Pattern: menuButton.
- Name: label.
- Description: none.
- States: expanded (from the trigger, while the menu is open (set by the View)); disabled (from isDisabled; an item's isDisabled).
- Enter, Space on the trigger: opens the menu on its first item.
- Up, Down: moves between items, wrapping.
- Home, End: first and last item.
- A letter: the next item whose name starts with it.
- Escape: closes and returns focus to the trigger.
- Press target: sm 32 px, md 40 px, lg 48 px.
- An item is named by its label and then its description, both inside the item.
- Sizes sm (32) and md (40) are below the 44 pt touch minimum; on touch layouts prefer lg until hit slop lets the press area extend past the paint.
- A tap outside, Escape and back close the menu; that tap reaches nothing else.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| item text | `text` on `surface-raised` | 4.5:1 | 16.26:1 | 11.83:1 |
| destructive icon | `danger` on `surface-raised` | 3:1 | 5.21:1 | 4.62:1 |

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
| Submenus and side placement. | inline-start and inline-end position areas | One level; top or bottom only. |
| Opening by long press or secondary click. | a long-press and secondary-click invoker | Tap on the trigger only. |
| Anchoring when opened from code with isOpen. | a popover anchor that does not depend on which control opened it | Opened from code it is placed by its classes; the trigger is the normal way in. |
| A destructive item in danger-coloured text. | a text-grade danger role (4.5:1 on surface-raised) | The icon carries the danger colour; the text stays in the text role. |

Related: Button.
