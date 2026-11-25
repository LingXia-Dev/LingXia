<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Drawer

A panel that slides in from a side edge. Status: preview.

```ts
import { Drawer } from '@liulidesign/core/Drawer';
```

Drawer is a modal panel the height of the screen on its end or start edge, with the same title, body and footer as a Dialog. It suits a wide screen, where it leaves the page it came from in view beside it: filters for a list, the details of a row, an app's places. On a phone it takes most of the width; a BottomSheet is often the better fit there.

## Examples

### Filters, from the end edge

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { CheckboxList } from '@liulidesign/core/CheckboxList';
import { RadioGroup } from '@liulidesign/core/RadioGroup';
import { Drawer } from '@liulidesign/core/Drawer';

/** Filters for the list behind it: the list stays in view while they are chosen. */
export function Filters() {
  const open = useLocal('open', false);
  const sort = useLocal<string | null>('sort', 'edited');
  const kinds = useLocal<readonly string[]>('kinds', ['notes', 'lists']);
  return (
    <view className="items-start gap-2">
      <Button label="Filters" startIcon="filter" onClick={() => open.set(true)} data-testid="filters-open" />
      <Drawer
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Filters"
        data-testid="filters"
        actions={
          <>
            <Button label="Clear" onClick={() => { kinds.set([]); sort.set('edited'); }} />
            <Button label="Show results" variant="primary" onClick={() => open.set(false)} data-testid="filters-apply" />
          </>
        }
      >
        <RadioGroup bind={sort} label="Sort by" items={[{ value: 'edited', label: 'Last edited' }, { value: 'created', label: 'Created' }, { value: 'title', label: 'Title' }]} />
        <CheckboxList
          label="Show"
          value={kinds.value}
          onChange={next => kinds.set(next)}
          items={[{ value: 'notes', label: 'Notes' }, { value: 'lists', label: 'Lists' }, { value: 'photos', label: 'Photos' }, { value: 'archived', label: 'Archived' }]}
        />
      </Drawer>
    </view>
  );
}
```

### Navigation, from the start edge

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Item, ItemList } from '@liulidesign/core/Item';
import { Drawer } from '@liulidesign/core/Drawer';

/** From the start edge, for the places of an app. */
export function Navigation() {
  const open = useLocal('nav', false);
  return (
    <view className="items-start">
      <Button label="Menu" startIcon="menu" onClick={() => open.set(true)} data-testid="nav-open" />
      <Drawer isOpen={open.value} onOpenChange={next => open.set(next)} label="Notes" description="ana@example.com" side="start" size="sm" data-testid="nav">
        <ItemList label="Places">
          <Item label="All notes" startIcon="file" onClick={() => open.set(false)} data-testid="nav-all" />
          <Item label="Shared with me" startIcon="users" onClick={() => open.set(false)} />
          <Item label="Archive" startIcon="folder" onClick={() => open.set(false)} />
          <Item label="Settings" startIcon="settings" onClick={() => open.set(false)} />
        </ItemList>
      </Drawer>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `isOpen` (required) | `boolean` |  | Whether it is shown. A closed drawer renders nothing, so local state inside starts over. |
| `onOpenChange` (required) | `(isOpen: boolean) => void` |  | Called with `false` on a close request: Escape, back, the backdrop per `dismissOn`, the close button. |
| `label` (required) | `string` |  | The title, and the drawer's accessible name. |
| `description` | `string` |  | A sentence under the title, read as the drawer's description. |
| `children` | `Child` |  | The body; it scrolls when it is taller than the drawer. |
| `actions` | `Child` |  | The footer, typically Buttons, the main action last. |
| `headerAction` | `Child` |  | Beside the title, before the close button: one small control that acts on the whole panel, such as Refresh. |
| `hasCloseButton` | `boolean` | `true` | Shows a close button. |
| `closeLabel` | `string` |  | Names the close button. Defaults to "Close" in the page's language. |
| `side` | `DrawerSide` | `'end'` | `end` for details and filters of what is on the page; `start` for navigation. |
| `size` | `DrawerSize` | `'md'` | Maximum width: 320, 420 or 560; never more than most of the screen. |
| `dismissOn` | `DialogDismissOn` | `'any'` |  |
| `className` | `string` |  | Classes for the panel. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<dialog>` | yes | The dialog primitive on a side edge, rounded on its inner side. |
| title | `<text>` | yes | `label` as a level-2 heading. |
| close | `<button>` | no | A close button, unless `hasCloseButton` is false; `closeLabel` names it. |
| description | `<text>` | no | Text under the title. |
| body | `<scroll>` | no | `children`, in a scroll that takes the height left. A hairline shows at an edge while content is scrolled under it. |
| headerAction | `<view>` | no | `headerAction`: one small control beside the title that acts on the whole panel, such as Refresh. |
| footer | `<view>` | no | `actions`: on a phone they share the width equally, the main one last; wider, they keep their own widths at the end, wrapping when narrow. |

## What an app may override

- `className`: Classes for the panel.
- The recipe, `drawerRecipe` from `@liulidesign/core/Drawer/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`, `--text-title`, `--shadow-modal`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `focus`, `line`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: dialog.
- Name: label.
- Description: description.
- Escape: Closes it, unless `dismissOn` is `none`.
- Tab: Through the drawer only, while it is open.
- Press target: md 32 px.
- A dialog named by its title: assistive tech reads only the drawer while it is open, and focus returns to what opened it.
- It mirrors with the reading direction: `end` is the right edge in left-to-right text.
- Under reduced motion it fades and does not slide.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| description | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use `end` for what belongs to the page: filters, details, a short form. Use `start` for navigation.
- Do: Give it a close button; a backdrop tap is easy to miss as a way out.
- Don't: Use a drawer to confirm something; that is a Dialog.
- Don't: Open a drawer from a drawer.

## Limits

- It does not follow the finger; it opens and closes as a whole.
- It is always modal: a panel that stays open beside the page is the aside of a Split.

Related: Dialog, BottomSheet, Split.
