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
    <view className="md:max-w-[264px] gap-4">
      <NavList label="Workspace" sections={workspace} bind={current} data-testid="workspace" />
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

/** Only the section in use starts open; each remembers being opened. */
export function Closed() {
  const current = useLocal<string>('current', 'text-input');
  return (
    <view className="md:max-w-[264px]">
      <NavList label="Components" sections={library} bind={current} defaultOpenSections={['inputs']} data-testid="library" />
    </view>
  );
}
```

### Rows with icons

```tsx
import { useLocal } from '@lingxia/view';
import { NavList } from '@liulidesign/core/NavList';

const mailboxes = [
  { id: 'mail', items: [
    { value: 'inbox', label: 'Inbox', icon: 'mail', endText: '12' },
    { value: 'starred', label: 'Starred', icon: 'star' },
    { value: 'sent', label: 'Sent', icon: 'send' },
    { value: 'drafts', label: 'Drafts', icon: 'document', endText: '3' },
  ] },
  { id: 'labels', title: 'Labels', items: [
    { value: 'work', label: 'Work', icon: 'tag' },
    { value: 'travel', label: 'Travel', icon: 'tag' },
  ] },
] as const;

/** Rows with icons: each leads with its mark, muted until the row is current. */
export function Icons() {
  const current = useLocal<string>('current', 'inbox');
  return (
    <view className="md:max-w-[264px]">
      <NavList label="Mailboxes" sections={mailboxes} bind={current} data-testid="mailboxes" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<string \| null>` |  | A View-local cell the control reads and writes; use it or `value` with `onChange`. |
| `value` | `string \| null` |  | The current row's value, or `null` when none is current. |
| `onChange` | `((value: string) => void)` |  | Runs with the value of a row chosen by a tap, a click or Enter. |
| `label` (required) | `string` |  | Names the list for assistive tech: "Mailboxes", "Components". |
| `sections` (required) | `readonly NavListSection[]` |  | The rows, in sections. |
| `defaultOpenSections` | `readonly string[]` |  | Sections open at first, by id; leave it out to open every section. After that each remembers its own state. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the list; each row gets `<testid>-<value>`, each section heading `<testid>-section-<id>`. |

- `bind` | `value` + `onChange`: props from one side only.

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
| current | `<view>` | no | An unseen mark, one per current row: its layout is when the row is brought into view. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `navListRecipe` from `@liulidesign/core/NavList/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `focus`, `text`, `text-muted`.

## Accessibility

- Pattern: tree.
- Name: label.
- Description: none.
- States: selected (from value or bind); expanded (from sections, open as `defaultOpenSections` lists them, then by a press on the heading); disabled (from items[].isDisabled).
- Tab: Into the list, on the current row; again, out of it..
- Up, Down: The previous or next row or heading..
- Home, End: The first or last row..
- Enter, Space: Opens the row, or opens or closes a section..
- The current row is read as selected, the others as not selected; a page that is current is not yet announced as "current page".
- Rows are 36 px tall; on touch screens the View grows each press target to the platform minimum.
- A row with `endText` is named "Inbox, 12", so the count is read once.
- In a sidebar that scrolls, the current row is brought into view when the list first appears and whenever another row becomes current from outside the list; a row in a closed section stays where it is.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| row text | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| section titles and counts | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| current row text | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |
| current row count | `text-muted` on `accent-soft` | 4.5:1 | 5.16:1 | 5.39:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep it beside the content it switches, and show the chosen place in that content at once.
- Do: Use sections with titles for long lists, and open only the ones in use with `defaultOpenSections`.
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
