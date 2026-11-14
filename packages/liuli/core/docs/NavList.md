<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# NavList

A sidebar's list of places, in sections that open and close, with the current one marked. Status: preview.

```ts
import { NavList } from '@liulidesign/core/NavList';
```

NavList is the column of places beside a workspace: mailboxes, projects, the pages of a document. Rows sit in sections; a section with a title opens and closes from its heading and remembers it. The current row sits on an accent wash and is read as selected.

## Examples

### A sidebar

```tsx
import { useLocal } from '@lingxia/view';
import { NavList } from '@liulidesign/core/NavList';

const workspace = [
  { id: 'mail', items: [
    { value: 'inbox', label: 'Inbox', endText: '12' },
    { value: 'drafts', label: 'Drafts', endText: '3' },
    { value: 'sent', label: 'Sent' },
  ] },
  { id: 'projects', title: 'Projects', items: [
    { value: 'launch', label: 'Spring launch' },
    { value: 'research', label: 'User research' },
    { value: 'archive', label: 'Archive', isDisabled: true },
  ] },
] as const;

/** A sidebar: places first, then a section that closes; the current row is selected. */
export function Sidebar() {
  const current = useLocal<string>('current', 'inbox');
  return (
    <view className="max-w-[264px] gap-4">
      <NavList label="Workspace" sections={workspace} value={current.value} onChange={next => current.set(next)} data-testid="workspace" />
      <text className="text-footnote text-muted" data-testid="workspace-current">{`Showing ${current.value}`}</text>
    </view>
  );
}
```

### A section that starts closed

```tsx
import { useLocal } from '@lingxia/view';
import { NavList } from '@liulidesign/core/NavList';

const library = [
  { id: 'inputs', title: 'Inputs', items: [{ value: 'text-input', label: 'TextInput' }, { value: 'date-input', label: 'DateInput' }] },
  { id: 'feedback', title: 'Feedback', items: [{ value: 'badge', label: 'Badge' }, { value: 'toast', label: 'Toast' }] },
] as const;

/** Sections that start closed; each remembers being opened. */
export function Closed() {
  const current = useLocal<string>('current', 'text-input');
  return (
    <view className="max-w-[264px]">
      <NavList label="Components" sections={library} value={current.value} onChange={next => current.set(next)} defaultClosed={['feedback']} data-testid="library" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the list for assistive tech: "Mailboxes", "Components". |
| `sections` (required) | `readonly NavListSection[]` |  | The rows, in sections. |
| `value` (required) | `string \| null` |  | The current row's value, or `null` when none is current. |
| `onChange` (required) | `(value: string) => void` |  | Runs with the value of a row chosen by a tap, a click or Enter. |
| `defaultClosed` | `readonly string[]` | `[]` | Sections closed at first, by id; after that each remembers its own state. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the list; each row gets `<testid>-<value>`, each section heading `<testid>-section-<id>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The tree: one Tab stop, named by `label`. |
| group | `<button>` | no | A section heading: a treeitem that opens and closes its rows. |
| groupLabel | `<text>` | no | The section title, small and muted. |
| groupChevron | `<vector>` | no | Points down while open, along the line while closed; mirrors for right-to-left text. |
| item | `<button>` | yes | One row per place: a treeitem, on the accent wash when current. |
| label | `<text>` | yes | The row's text; one line, cut with an ellipsis. |
| end | `<view>` | no | `endText`: a count or short value, read as part of the row's name. |

## Accessibility

- Pattern: tree.
- Name: label.
- Description: none.
- States: selected (from value); expanded (from sections, closed by a press on the heading or by defaultClosed); disabled (from items[].isDisabled).
- Tab: Into the list, on the current row; again, out of it..
- Up, Down: The previous or next row or heading..
- Home, End: The first or last row..
- Enter, Space: Opens the row, or opens or closes a section..
- The current row is read as selected, the others as not selected; a page that is current is not yet announced as "current page".
- Rows are 36 px tall; on touch screens the View grows each press target to the platform minimum.
- A row with `endText` is named "Inbox, 12", so the count is read once.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| row text | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| section titles and counts | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| current row text | `text` on `accent-soft` | 4.5:1 | 14.09:1 | 11.26:1 |
| current row count | `text-muted` on `accent-soft` | 4.5:1 | 5.10:1 | 5.39:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep it beside the content it switches, and show the chosen place in that content at once.
- Do: Use sections with titles for long lists, and close the rarely used ones with `defaultClosed`.
- Do: Keep labels to a word or two; the column is narrow.
- Don't: Use it for actions; it chooses where to look, a Button or a menu does things.
- Don't: Use it for a few peers on one page; that is Tabs.
- Don't: Nest sections inside sections; a NavList is one level of headings.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| The current row read as "current page" | `aria-current` in the View | Read as selected |
| Typing a name to jump to its row | Type-ahead in a View `tree`, as `menu` has | Arrows, Home and End |

Related: Tabs, Item.
