<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Breadcrumb

Where a page sits: the levels above it, each a way back up. Status: preview.

```ts
import { Breadcrumb } from '@liulidesign/core/Breadcrumb';
```

Breadcrumb shows the path to the current page in a hierarchy: a folder in a drive, a record in a section. Each level above is a button that goes back to it; the last item is the current page and is not pressable. A deep trail folds its middle levels into a menu, keeping the first level and the last two.

## Examples

### A path of three levels

```tsx
import { useLocal } from '@lingxia/view';
import { Breadcrumb } from '@liulidesign/core/Breadcrumb';

/** Where a file sits: each level above it is a way back. */
export function Path() {
  const opened = useLocal('opened', '');
  const go = (name: string) => () => opened.set(name);
  return (
    <view className="gap-2">
      <Breadcrumb
        label="Breadcrumb"
        items={[
          { key: 'home', label: 'Home', onClick: go('Home') },
          { key: 'projects', label: 'Projects', onClick: go('Projects') },
          { key: 'plan', label: 'Q3 plan' },
        ]}
        data-testid="path"
      />
      {opened.value !== '' && <text role="status" className="text-footnote text-muted">{`Opened ${opened.value}`}</text>}
    </view>
  );
}
```

### A deep trail, folded

```tsx
import { Said } from '../example';
import { Breadcrumb } from '@liulidesign/core/Breadcrumb';

/** A deep trail: the middle levels fold into a menu, and the first level and the last two stay. */
export function Deep() {
  return (
    <Said>
      {say => (
        <Breadcrumb
          label="Breadcrumb"
          moreLabel="More levels"
          items={[
            { key: 'drive', label: 'Drive', onClick: () => say('Opened Drive') },
            { key: 'clients', label: 'Clients', onClick: () => say('Opened Clients') },
            { key: 'northwind', label: 'Northwind', onClick: () => say('Opened Northwind') },
            { key: '2026', label: '2026', onClick: () => say('Opened 2026') },
            { key: 'invoices', label: 'Invoices', onClick: () => say('Opened Invoices') },
            { key: 'inv', label: 'INV-1043.pdf' },
          ]}
          data-testid="deep"
        />
      )}
    </Said>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the trail for assistive tech: "Breadcrumb", "You are in". |
| `items` (required) | `readonly BreadcrumbItem[]` |  | The levels from the top down; the last is the current page. |
| `maxItems` | `number` | `4` | Past this many levels the middle ones fold into a menu; it needs `moreLabel`. |
| `moreLabel` | `string` |  | Names the menu that holds the folded levels: "More levels". Without it nothing is folded. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## BreadcrumbItem props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `key` (required) | `string` |  | Identifies the level: its test id is `<id>-<key>`. |
| `label` (required) | `string` |  | The level's name. |
| `onClick` | `(() => void)` |  | Opens the level. The last item is where the person is, and takes none. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The trail, a list named by `label`; it wraps by whole levels. |
| item | `<view>` | yes | One level with the separator before it. |
| link | `<button>` | no | A level above the current page: a button in muted text. |
| label | `<text>` | yes | A level's name, cut short past twelve characters' width or so; the current page is heavier and in the text colour. |
| separator | `<vector>` | no | A chevron between levels; it mirrors for right-to-left text and is not read. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `breadcrumbRecipe` from `@liulidesign/core/Breadcrumb/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `focus`, `text`, `text-faint`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: Through the levels above the current page.
- Enter or Space: Opens the level.
- Press target: sm 32 px.
- The trail is a list, so a screen reader says how many levels there are and where each stands.
- The current page is the last item and plain text: it is told apart by not being a button, by its weight and by its colour.
- Folded levels are in a menu named by `moreLabel`; they stay reachable, in order.
- A level is 32 px tall, below the touch minimum: on a phone the page's own back button is the main way up, and the trail is for seeing where one is.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| levels above | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| current page | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Start at the top of the hierarchy and end on the current page.
- Do: Pass `moreLabel` wherever a trail can be deep, so it folds instead of wrapping to three lines.
- Do: Use it on wide screens and for deep hierarchies; a two-level app needs only a back button.
- Don't: Use it for the steps of a task, or for history: it shows where a page is, not how the person got there.
- Don't: Make the current page a link.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Folding by the room there is, not by a count. | nothing from the View | Folds past `maxItems`; otherwise the trail wraps. |
| The current page announced as current. | a current-page state for assistive tech | The current page is the last item of the list and not a button. |

Related: Tabs, NavList, DropdownMenu.
