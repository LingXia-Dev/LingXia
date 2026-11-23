<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# TopBar

A page's own header row: a way back, its title, a control at the middle, its actions at the end. Status: preview.

```ts
import { TopBar } from '@liulidesign/core/TopBar';
```

TopBar is the header row a page draws itself, for a page that shows no host navigation bar (`navigationStyle: "custom"`). It holds a control at the start (a way back, a menu), the page's title as a heading, a control at the middle (a SearchField, a segmented control) and the page's actions at the end. The start and the end share the room left over, so the middle stays at the bar's middle while everything fits; past that the bar wraps. The page already keeps its content clear of system bars and host chrome, so the bar adds only its own padding.

## Examples

### A way back, the title and actions

```tsx
import { Said } from '../example';
import { IconButton } from '@liulidesign/core/IconButton';
import { TopBar } from '@liulidesign/core/TopBar';

/** A page's header: a way back, its title, and its actions at the end. */
export function Standard() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12">
          <TopBar
            startContent={<IconButton icon="chevron-start" label="Back" variant="ghost" onClick={() => say('Went back')} data-testid="orders-back" />}
            heading="Orders"
            endContent={[
              <IconButton key="filter" icon="filter" label="Filter" variant="ghost" onClick={() => say('Opened Filter')} data-testid="orders-filter" />,
              <IconButton key="add" icon="add" label="New order" variant="ghost" onClick={() => say('Created a new order')} data-testid="orders-add" />,
            ]}
            data-testid="orders-topbar"
          />
        </view>
      )}
    </Said>
  );
}
```

### A search field at the middle

```tsx
import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { IconButton } from '@liulidesign/core/IconButton';
import { SearchField } from '@liulidesign/core/SearchField';
import { TopBar } from '@liulidesign/core/TopBar';

/** A search field at the middle of the bar. */
export function WithSearch() {
  const query = useLocal('query', '');
  return (
    <view className="overflow-hidden rounded-container border border-text/12">
      <TopBar
        heading="Mail"
        centerContent={<SearchField label="Search mail" bind={query} data-testid="mail-search" />}
        endContent={<IconButton icon="add" label="New message" variant="ghost" onClick={unused} />}
        data-testid="mail-topbar"
      />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `heading` | `Child` |  | The page's title: a string is drawn as its heading; a node is drawn as given. It follows `startContent`. |
| `startContent` | `Child` |  | At the start, before the heading: a way back, a menu. |
| `centerContent` | `Child` |  | At the bar's middle while everything fits: a SearchField, a segmented control. |
| `endContent` | `Child` |  | At the end: the page's actions, a row of IconButtons. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The row, at least 44 pt tall, with a hairline under it. |
| start | `<view>` | yes | `startContent`, then the heading; it shares the room left over with `end`. |
| heading | `<text>` | no | A string `heading`: the page's title, a heading, cut short when it does not fit. |
| center | `<view>` | no | `centerContent`, at the bar's middle while everything fits. |
| end | `<view>` | yes | `endContent`, held to the end. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `topBarRecipe` from `@liulidesign/core/TopBar/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`.
- Colour roles it paints with, set by the theme pack: `surface`, `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- A string `heading` is the page's heading at level 1, so a screen reader can jump to it.
- Start and end follow the page's direction: in right-to-left the way back sits on the right.
- Each action is its own button, reached in order after the heading.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Draw either the host's navigation bar or a TopBar on a page, never both: set `navigationStyle: "custom"` on a page that draws its own.
- Do: Name every icon-only action with its `label`.
- Don't: Put app-close or window controls in it; those belong to the host.
- Don't: Stack a second bar under the host's navigation bar for the page's actions; give the page a TopBar instead.

Related: IconButton, SearchField, ListDetail.
