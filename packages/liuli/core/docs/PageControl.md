<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# PageControl

Dots for which page of a few is in view. Status: preview.

```ts
import { PageControl } from '@liulidesign/core/PageControl';
```

PageControl draws one dot per page and stretches the current one. Pressing a dot goes to its page. Bound to the same cell as a paging scroll, the dots and the pages follow each other; Carousel does that for you.

## Examples

### Under what it pages

```tsx
import { useLocal } from '@lingxia/view';
import { PageControl } from '@liulidesign/core/PageControl';

const steps = [
  ['Write anywhere', 'Notes open in a tap and save as you type.'],
  ['Find it again', 'Search reads every note, list and photo.'],
  ['Share what you choose', 'Each note is private until you invite someone.'],
] as const;

/** Under what it pages: the dots follow the page, and pressing one goes there. */
export function Welcome() {
  const page = useLocal('page', 0);
  const [title, body] = steps[page.value];
  return (
    <view className="gap-3">
      <view className="gap-1 rounded-container bg-surface-sunken p-5">
        <text className="text-callout font-semibold text-text">{title}</text>
        <text className="text-footnote text-muted">{body}</text>
      </view>
      <PageControl label="Welcome steps" count={steps.length} bind={page} pageLabel={at => `Step ${at + 1} of ${steps.length}`} data-testid="welcome" />
    </view>
  );
}
```

### Over a picture

```tsx
import { useLocal } from '@lingxia/view';
import { PageControl } from '@liulidesign/core/PageControl';

/** Over a picture, in the colour that reads on media. */
export function OnMedia() {
  const page = useLocal('photo', 1);
  return (
    <view className="h-40 justify-end rounded-container bg-media pb-1">
      <PageControl label="Photos" count={5} bind={page} tone="on-media" pageLabel={at => `Photo ${at + 1} of 5`} data-testid="photos" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<number>` |  | A View-local cell holding the page, from 0: the same cell a paging `<scroll>` binds, so the two follow each other. |
| `value` | `number` |  | The current page, from 0. |
| `onChange` | `((page: number) => void)` |  | Runs with the page of the dot that was pressed. |
| `label` (required) | `string` |  | Names the dots for assistive tech: "Photos". |
| `count` (required) | `number` |  | How many pages there are. Past ten or so, show a count in words. |
| `pageLabel` | `((page: number) => string)` |  | Each dot's accessible name, written by the app: `page => \`Photo ${page + 1}\``. Defaults to "Page 2" in the page's language. |
| `tone` | `PageControlTone` | `'accent'` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the row; each dot gets `<testid>-<page>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row of dots, a tab list named by `label`. |
| dot | `<button>` | yes | One page: a button taller and wider than its mark. |
| mark | `<view>` | yes | The 6 px dot; the current page's is a pill in the tone. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `pageControlRecipe` from `@liulidesign/core/PageControl/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- Colour roles it paints with, set by the theme pack: `accent`, `focus`, `on-media`, `text`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: selected (from page, on the dot of the page in view).
- Tab: Moves to a dot.
- Enter or Space: Goes to its page.
- Press target: .
- Each dot is a tab named by `pageLabel` and reports whether it is the selected one.
- The current dot is told apart by its width, not only its colour; the other dots are faint on purpose.
- A dot is 14 by 26 px to the finger; a press that misses goes to the nearest dot within reach.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| current dot | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |
| current dot over media | `on-media` on `media` | 3:1 | 7.94:1 | 7.94:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep it to a handful of pages; past ten, say "3 of 24" in words.
- Do: Write `pageLabel` so it says where the dot leads: "Photo 2 of 5".
- Don't: Use dots to move through pages of results; that is Pagination.
- Don't: Use it as the only way to page; the content itself swipes.

## Limits

- Many pages are not folded into a window of dots; the row grows with `count`.

Related: Carousel, Pagination, Tabs.
