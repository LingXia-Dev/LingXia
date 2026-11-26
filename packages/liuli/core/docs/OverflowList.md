<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# OverflowList

A line of items that keeps to one row, folding what does not fit into one element. Status: preview.

```ts
import { OverflowList } from '@liulidesign/core/OverflowList';
```

OverflowList lays out items on one line and measures them: when they do not all fit, the ones that do stay and the rest are handed to `renderOverflow`, which draws one element for them, a "+3" token or a More menu. It folds from the end, or from the start to keep the end of a path. Widths are remembered by key, so a line that widens brings items back without a second look.

## Examples

### Tags, then a count

```tsx
import { Token } from '@liulidesign/core/Token';
import { OverflowList } from '@liulidesign/core/OverflowList';

const tags = ['Design', 'Research', 'Mobile', 'Accessibility', 'Performance', 'Charts', 'Localisation', 'Testing'];

/** Tags on one line: those that do not fit become a count. */
export function Tags() {
  return (
    <OverflowList
      label="Tags"
      items={tags}
      keyOf={tag => tag}
      renderItem={tag => <Token label={tag} size="sm" />}
      renderOverflow={hidden => <Token label={`+${hidden.length}`} size="sm" data-testid="tags-more" />}
      data-testid="tags"
    />
  );
}
```

### Actions, then a More menu

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { DropdownMenu } from '@liulidesign/core/DropdownMenu';
import { IconButton } from '@liulidesign/core/IconButton';
import { OverflowList } from '@liulidesign/core/OverflowList';

const actions = [
  { id: 'reply', label: 'Reply', icon: 'reply' },
  { id: 'forward', label: 'Forward', icon: 'forward' },
  { id: 'archive', label: 'Archive', icon: 'archive' },
  { id: 'pin', label: 'Pin', icon: 'pin' },
  { id: 'delete', label: 'Delete', icon: 'delete' },
] as const;

/** A toolbar narrower than its actions: the last ones move into a More menu, in the same order. */
export function Actions() {
  return (
    <Said>
      {say => (
        <OverflowList
          label="Message actions"
          items={actions}
          keyOf={action => action.id}
          renderItem={action => <Button label={action.label} startIcon={action.icon} size="sm" variant="secondary" onClick={() => say(action.label)} data-testid={`action-${action.id}`} />}
          renderOverflow={hidden => (
            <DropdownMenu label="More actions" size="sm" items={hidden.map(action => ({ id: action.id, label: action.label, icon: action.icon, onClick: () => say(action.label) }))}>
              <IconButton label="More actions" icon="more" size="sm" data-testid="actions-more" />
            </DropdownMenu>
          )}
          data-testid="actions"
        />
      )}
    </Said>
  );
}
```

### Folding from the start of a path

```tsx
import { OverflowList } from '@liulidesign/core/OverflowList';

const path = ['Home', 'Projects', 'LingXia', 'Design', 'Components', 'OverflowList'];

/** Folding from the start keeps the end of a path, where you are. */
export function FromStart() {
  return (
    <OverflowList
      items={path}
      keyOf={step => step}
      collapseFrom="start"
      minVisible={1}
      renderItem={(step, index) => (
        <view className="flex-row items-center gap-2">
          {index > 0 && <text aria-hidden className="text-footnote text-faint">/</text>}
          <text className={index === path.length - 1 ? 'text-footnote font-medium text-text' : 'text-footnote text-muted'}>{step}</text>
        </view>
      )}
      renderOverflow={hidden => <text aria-label={`${hidden.length} more`} className="text-footnote text-muted">…</text>}
      data-testid="path"
    />
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `items` (required) | `readonly Item[]` |  | Everything the line could show, in reading order. |
| `renderItem` (required) | `(item: Item, index: number) => Child` |  | One item as it stands on the line: a Token, a Button, a link. |
| `renderOverflow` (required) | `(hidden: readonly Item[]) => Child` |  | What stands for the items that do not fit, given them in order: a "+3" token, a More menu. Not drawn when all fit. |
| `keyOf` | `((item: Item, index: number) => string)` |  | An item's stable key, so its measured width outlives a change of the list. By default its place in `items`. |
| `collapseFrom` | `OverflowListCollapse` | `'end'` | The end the items fold away from: `start` keeps the last ones, as a path keeps where you are. |
| `minVisible` | `number` | `0` | The fewest items shown, even when they do not fit. |
| `label` | `string` |  | The line's name for assistive tech, when it is a group of its own. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The line: one row that never wraps. With `label` it is a group of that name. |
| item | `<view>` | yes | Each item as `renderItem` draws it, at its own width. |
| overflow | `<view>` | no | What `renderOverflow` draws for the items that do not fit; left out when all fit. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `overflowListRecipe` from `@liulidesign/core/OverflowList/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: Reaches the items on the line, then the overflow element.
- Items that do not fit are not on the line at all, so a screen reader reaches them only through the overflow element: make it a menu or say how many it holds.
- The first frame lays out every item, clipped, to measure them; the overflow element stands in unseen and unread until the fit is known.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Give `keyOf` a stable key, so measured widths survive a change of the list.
- Do: Make the overflow a menu of the hidden actions, in the same order, or a count of the hidden tags.
- Do: Fold from the start for a path, so where you are stays on the line.
- Don't: Hide the only way to an action in a count; a "+3" token is for things to read, a menu for things to do.
- Don't: Put items that change width on their own (a live counter) on the line; their width is measured once.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Re-measuring an item whose width changes while it is folded away. | nothing from the View | Its width is the one last measured; a new key measures it again. |

Related: ButtonGroup, Token, DropdownMenu, Breadcrumb.
