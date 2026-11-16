<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# TreeList

Things inside things, opened in place. Status: preview.

```ts
import { TreeList } from '@liulidesign/core/TreeList';
```

TreeList shows nested nodes as rows set in by their depth: folders and files, the sections of a document. A branch carries a chevron and opens or closes in place; any row can be the selected one. The app keeps the selection, and may keep the open branches too. For one level of destinations in groups, use NavList.

## Examples

### Folders and notes

```tsx
import { useLocal } from '@lingxia/view';
import { TreeList, type TreeNode } from '@liulidesign/core/TreeList';

const folders: TreeNode[] = [
  {
    id: 'notes', label: 'Notes', icon: 'folder', endText: '48',
    children: [
      { id: 'travel', label: 'Travel', icon: 'folder', endText: '12', children: [
        { id: 'coast', label: 'Trip to the coast', icon: 'file' },
        { id: 'lisbon', label: 'Lisbon in March', icon: 'file' },
      ] },
      { id: 'recipes', label: 'Recipes', icon: 'folder', endText: '9', children: [
        { id: 'bread', label: 'Weekend bread', icon: 'file' },
      ] },
      { id: 'reading', label: 'Reading list', icon: 'file' },
    ],
  },
  { id: 'shared', label: 'Shared with me', icon: 'users', endText: '6', children: [
    { id: 'plan', label: 'Q3 plan', icon: 'file' },
  ] },
  { id: 'archive', label: 'Archive', icon: 'folder', isDisabled: true },
];

/** Folders with their notes: a branch opens in place, and the row that was pressed is the current one. */
export function Folders() {
  const current = useLocal<string | null>('current', 'coast');
  return (
    <view className="gap-2">
      <TreeList label="Folders" nodes={folders} value={current.value} onChange={id => current.set(id)} defaultExpanded={['notes', 'travel']} data-testid="folders" />
      <text role="status" className="text-footnote text-muted">{`Current: ${current.value}`}</text>
    </view>
  );
}
```

### An outline, opened from outside

```tsx
import { useLocal } from '@lingxia/view';
import { TreeList, type TreeNode } from '@liulidesign/core/TreeList';

const outline: TreeNode[] = [
  { id: 'intro', label: 'Introduction' },
  { id: 'setup', label: 'Setting up', children: [
    { id: 'install', label: 'Install' },
    { id: 'first', label: 'Your first page', children: [{ id: 'layout', label: 'Layout' }, { id: 'state', label: 'State' }] },
  ] },
  { id: 'publish', label: 'Publishing' },
];

/** An outline without icons, opened from outside. */
export function Outline() {
  const current = useLocal<string | null>('section', 'layout');
  const open = useLocal<readonly string[]>('open', ['setup', 'first']);
  return <TreeList label="Contents" nodes={outline} value={current.value} onChange={id => current.set(id)} expanded={open} data-testid="outline" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the tree for assistive tech: "Folders". |
| `nodes` (required) | `readonly TreeNode[]` |  |  |
| `value` (required) | `string \| null` |  | The selected node's id, or null. |
| `onChange` (required) | `(id: string) => void` |  | Runs with the id of the row that was pressed; a branch is selected and toggled by the same press. |
| `expanded` | `Local<readonly string[]>` |  | A View-local cell holding the ids of the open branches, to read or set them from outside. |
| `defaultExpanded` | `readonly string[]` | `[]` | The branches open at first, when `expanded` is not given. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The tree, named by `label`. |
| row | `<button>` | yes | One node: a tree item set in by its level. |
| chevron | `<vector>` | no | On a branch; it turns when the branch opens. A leaf keeps its width empty. |
| icon | `<vector>` | no | An icon before the label; not read. |
| label | `<text>` | yes | The node's name, cut short with an ellipsis. |
| end | `<text>` | no | A count or a size at the end of the row. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `treeListRecipe` from `@liulidesign/core/TreeList/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `focus`, `text`, `text-muted`.

## Accessibility

- Pattern: tree.
- Name: label.
- Description: none.
- States: expanded (from expanded, or defaultExpanded and the rows opened since); selected (from value); disabled (from isDisabled on a node).
- Up and Down: Move between the rows in view.
- Right: Opens a closed branch; on an open one, moves to its first child (Left, right to left).
- Left: Closes an open branch; on any other row, moves to its parent (Right, right to left).
- Enter or Space: Selects the row; on a branch it also opens or closes it.
- Press target: md 36 px.
- Each row is a tree item that reports its level, whether it is selected, and on a branch whether it is open.
- A row with `endText` is read as "label, end text".
- The selected row is told apart by its fill and its weight.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| selected row | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| count | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep it to a few levels; past six, rows stop stepping in.
- Do: Open the branches that lead to the current row, so it is in view.
- Don't: Use a tree for a flat list in groups; that is NavList.
- Don't: Put actions in the rows; a row selects, and a branch also opens.

## Limits

- Rows are not checked, dragged or loaded on demand.

Related: NavList, Details, Breadcrumb.
