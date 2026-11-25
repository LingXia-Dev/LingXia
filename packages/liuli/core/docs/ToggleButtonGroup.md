<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# ToggleButtonGroup

Toggle buttons that belong together, one on at a time or any number, as one keyboard stop. Status: preview.

```ts
import { ToggleButtonGroup } from '@liulidesign/core/ToggleButtonGroup';
```

ToggleButtonGroup lays out toggles that act on the same thing, such as filters over a list or a toolbar's layout choice. With `selectionMode="single"` one is on at a time and it is a radio group; with `"multiple"` each turns on or off on its own and it is a toolbar of toggle buttons. Either way the group is one Tab stop and the arrow keys move between the toggles. For a setting in a form use SegmentedControl.

## Examples

### Any number on

```tsx
import { useLocal } from '@lingxia/view';
import { ToggleButtonGroup } from '@liulidesign/core/ToggleButtonGroup';

const notes = [
  { title: 'Q3 plan', pinned: true, starred: true, files: false },
  { title: 'Reading list', pinned: false, starred: true, files: true },
  { title: 'Interview notes', pinned: true, starred: false, files: true },
  { title: 'Groceries', pinned: false, starred: false, files: false },
];

/** Any number on: filters that each narrow the list, read as toggle buttons. */
export function Filters() {
  const on = useLocal<readonly string[]>('on', ['starred']);
  const shown = notes.filter(note => (!on.value.includes('pinned') || note.pinned) && (!on.value.includes('starred') || note.starred) && (!on.value.includes('files') || note.files));
  return (
    <view className="gap-3">
      <ToggleButtonGroup
        label="Filters"
        selectionMode="multiple"
        size="sm"
        items={[
          { value: 'pinned', label: 'Pinned', icon: 'bookmark' },
          { value: 'starred', label: 'Starred', icon: 'star' },
          { value: 'files', label: 'With files', icon: 'attach' },
        ]}
        bind={on}
        data-testid="filters"
      />
      <text className="text-footnote text-muted" data-testid="filters-count">{shown.length === 1 ? '1 note' : `${shown.length} notes`}</text>
    </view>
  );
}
```

### One on at a time, icon-only

```tsx
import { useLocal } from '@lingxia/view';
import { ToggleButtonGroup } from '@liulidesign/core/ToggleButtonGroup';

const notes = [
  { title: 'Q3 plan', pinned: true, starred: true, files: false },
  { title: 'Reading list', pinned: false, starred: true, files: true },
  { title: 'Interview notes', pinned: true, starred: false, files: true },
  { title: 'Groceries', pinned: false, starred: false, files: false },
];

/** One on at a time, icon-only, in a toolbar: read as a radio group, each toggle named by its label. */
export function Layout() {
  const layout = useLocal('layout', 'grid');
  return (
    <view className="gap-3">
      <ToggleButtonGroup
        label="Layout"
        selectionMode="single"
        items={[
          { value: 'list', label: 'List', icon: 'list', isLabelHidden: true },
          { value: 'grid', label: 'Grid', icon: 'grid', isLabelHidden: true },
          { value: 'rows', label: 'Rows', icon: 'rows', isLabelHidden: true },
        ]}
        bind={layout}
        data-testid="layout"
      />
      <text className="text-footnote text-muted" data-testid="layout-shown">{`Showing notes as a ${layout.value}`}</text>
    </view>
  );
}
```

### One on at a time, with labels

```tsx
import { useLocal } from '@lingxia/view';
import { ToggleButtonGroup } from '@liulidesign/core/ToggleButtonGroup';

/** One on at a time, with labels. */
export function Single() {
  const pen = useLocal('pen', 'medium');
  return (
    <ToggleButtonGroup
      label="Pen"
      selectionMode="single"
      items={[
        { value: 'fine', label: 'Fine' },
        { value: 'medium', label: 'Medium' },
        { value: 'broad', label: 'Broad' },
      ]}
      bind={pen}
      data-testid="pen"
    />
  );
}
```

### Sizes

```tsx
import { unused } from '../example';
import { ToggleButtonGroup } from '@liulidesign/core/ToggleButtonGroup';

export function Sizes() {
  const items = [
    { value: 'list', label: 'List', icon: 'list', isLabelHidden: true },
    { value: 'grid', label: 'Grid', icon: 'grid', isLabelHidden: true },
  ] as const;
  return (
    <view className="gap-3">
      <ToggleButtonGroup label="Small" selectionMode="single" size="sm" items={items} value="grid" onChange={unused} />
      <ToggleButtonGroup label="Medium" selectionMode="single" items={items} value="grid" onChange={unused} />
      <ToggleButtonGroup label="Large" selectionMode="single" size="lg" items={items} value="grid" onChange={unused} />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `selectionMode` (required) | `"multiple" \| "single"` |  | One on at a time, read as a radio group: text alignment, a pen. Each on or off on its own, read as toggle buttons: bold, italic, underline. |
| `bind` | `Local<readonly string[]> \| Local<string>` |  | A View-local cell the group reads and writes; use it or `value` with `onChange`. |
| `value` | `string \| readonly string[]` |  | The value that is on. The values that are on. |
| `onChange` | `((value: string) => void) \| ((value: readonly string[]) => void)` |  | Runs with the toggle pressed or arrowed onto. Runs with the values on after a press, in the items' order. |
| `label` (required) | `string` |  | Names the group for assistive tech: "Text style", "Alignment". |
| `items` (required) | `readonly ToggleButtonGroupItem[]` |  | The toggles, in order. |
| `size` | `ToggleButtonSize` | `'md'` |  |
| `isDisabled` | `boolean` | `false` | Disables every toggle. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the group; each toggle gets `<testid>-<value>`. |

- `bind` | `value` + `onChange`: props from one side only.

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row, named by `label`: a radio group with one on at a time, a toolbar otherwise. Each toggle is drawn as a ToggleButton. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `toggleButtonGroupRecipe` from `@liulidesign/core/ToggleButtonGroup/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: checked (from value or bind, with selectionMode single); pressed (from value or bind, with selectionMode multiple); disabled (from isDisabled, or an item's isDisabled).
- Tab: Enters the group on one toggle and leaves it.
- Left, Right: Move between the toggles; with one on at a time, the arrival turns it on.
- Enter or Space: Turns the focused toggle on, or on and off with any number on.
- One on at a time: role radiogroup, each toggle a radio read as checked. Any number on: role toolbar, each toggle a button read as pressed.
- An icon-only toggle keeps its label as its name. The state is drawn as the soft accent fill and the accent edge, never as the icon's colour alone.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| boundary, on | `accent` on `accent-soft` | 3:1 | 4.93:1 | 5.02:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name the group for what its toggles share: "Filters", "Layout".
- Do: Keep icon-only toggles to pictures people know, and keep the label as the name.
- Don't: Use it for a setting saved with a form; that is a SegmentedControl, a RadioGroup or a CheckboxList.
- Don't: Mix actions into it; a button that does something once belongs in a ButtonGroup.

Related: ToggleButton, ButtonGroup, SegmentedControl.
