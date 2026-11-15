<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Pagination

Moves through a long set a page at a time. Status: preview.

```ts
import { pagesShown, Pagination } from '@liulidesign/core/Pagination';
```

Pagination sits under a table or a list that is shown a page at a time. With a mouse it is a row of page numbers between two arrows: the first, the last, the current page and its neighbours. With a finger it is the two arrows around the position, "3 / 12", since a row of small numbers is hard to press. The app holds the page and loads its rows.

## Examples

### Under a list of results

```tsx
import { useLocal } from '@lingxia/view';
import { Pagination } from '@liulidesign/core/Pagination';

const words = { pageLabel: (page: number) => `Page ${page}`, previousLabel: 'Previous page', nextLabel: 'Next page' };

/** Twelve pages: a finger gets the arrows and the position, a mouse the page numbers. */
export function Results() {
  const page = useLocal('page', 3);
  return (
    <view className="gap-2">
      <Pagination label="Results pages" page={page.value} pageCount={12} onChange={page.set} positionLabel={(at, of) => `Page ${at} of ${of}`} {...words} data-testid="results" />
      <text role="status" className="text-footnote text-muted tabular-nums">{`Showing ${(page.value - 1) * 20 + 1}–${page.value * 20} of 240`}</text>
    </view>
  );
}
```

### Page numbers

```tsx
import { useLocal } from '@lingxia/view';
import { Pagination } from '@liulidesign/core/Pagination';

const words = { pageLabel: (page: number) => `Page ${page}`, previousLabel: 'Previous page', nextLabel: 'Next page' };

/** The page numbers, whatever the pointer: first, last, the current page and its neighbours. */
export function Pages() {
  const page = useLocal('page', 6);
  return <Pagination label="Pages" variant="pages" page={page.value} pageCount={12} onChange={page.set} {...words} data-testid="pages" />;
}
```

### Arrows and the position

```tsx
import { useLocal } from '@lingxia/view';
import { Pagination } from '@liulidesign/core/Pagination';

const words = { pageLabel: (page: number) => `Page ${page}`, previousLabel: 'Previous page', nextLabel: 'Next page' };

/** The arrows and the position, whatever the pointer. */
export function Compact() {
  const page = useLocal('page', 1);
  return <Pagination label="Pages" variant="compact" page={page.value} pageCount={4} onChange={page.set} {...words} data-testid="compact" />;
}
```

### Page numbers in a narrow row

```tsx
import { useLocal } from '@lingxia/view';
import { Pagination } from '@liulidesign/core/Pagination';

const words = { pageLabel: (page: number) => `Page ${page}`, previousLabel: 'Previous page', nextLabel: 'Next page' };

/** Page numbers in a row too narrow for them: the neighbours go first, then the numbers give way to the position. */
export function Narrow() {
  const page = useLocal('page', 6);
  return (
    <view className="gap-3">
      <view className="w-[280px]"><Pagination label="Pages" variant="pages" page={page.value} pageCount={12} onChange={page.set} {...words} data-testid="narrow" /></view>
      <view className="w-[232px]"><Pagination label="Pages" variant="pages" page={page.value} pageCount={12} onChange={page.set} {...words} data-testid="narrower" /></view>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the control for assistive tech: "Pages", "Results pages". |
| `page` (required) | `number` |  | The current page, from 1. |
| `pageCount` (required) | `number` |  | How many pages there are. |
| `onChange` (required) | `(page: number) => void` |  | Runs with the page chosen. |
| `pageLabel` (required) | `(page: number) => string` |  | A page button's accessible name: `page => \`Page ${page}\``. |
| `previousLabel` (required) | `string` |  |  |
| `nextLabel` (required) | `string` |  |  |
| `siblings` | `number` | `1` | How many pages to show on each side of the current one. |
| `variant` | `"auto" \| "compact" \| "pages"` | `'auto'` | `compact` shows the two arrows around "3 / 12"; `auto` uses it where the pointer is a finger. Page numbers that do not fit their row give way to it. |
| `positionLabel` | `((page: number, pageCount: number) => string)` |  | With `compact`: the text between the arrows, read as the position. By default "3 / 12". |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the control; a page is `<id>-<n>`, the arrows `<id>-previous` and `<id>-next`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The control, a group named by `label`, between its two arrows. |
| pages | `<view>` | no | The page numbers, with a mouse or `variant="pages"`. |
| page | `<button>` | no | One page: a 40 px button, pressed when it is the current page. |
| gap | `<view>` | no | An ellipsis where pages are left out; not read. |
| position | `<text>` | no | The position between the arrows, with a finger or `variant="compact"`; announced as it changes. |

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: pressed (from page, on the current page's button); disabled (from the first or last page, on its arrow).
- Tab: Through the arrows and the page numbers.
- Enter or Space: Goes to the page.
- Press target: md 40 px.
- Each page button is named by `pageLabel`, "Page 6", and the current one reports pressed.
- The arrows are named by `previousLabel` and `nextLabel` and are disabled at the ends.
- The compact position is a status: a screen reader hears it when the page changes. Give `positionLabel` words, "Page 3 of 12", where "3 / 12" would be read badly.
- The current page is marked by its fill and a heavier weight as well as by its state.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| page number | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| current page | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.44:1 | 9.49:1 |
| ellipsis | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say beside it which rows are shown: "Showing 41–60 of 240".
- Do: Leave `variant` on `auto`. With `pages`, numbers that do not fit their row drop their neighbours, then give way to the position.
- Do: Keep the page in the app's state, so coming back to the list keeps the place.
- Don't: Page a set that fits on one screen.
- Don't: Use it for steps in a task; that is a stepper.
- Don't: Use it in a feed people scroll; load more as the end comes into view.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Choosing how many rows a page holds, and jumping to a page by typing it. | nothing from the View | An app puts a Select or a NumberInput beside it. |

Related: Table, Tabs, IconButton.
