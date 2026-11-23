<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# TopBar

Custom page header row with leading, centered and trailing slots. Status: preview.

```ts
import { TopBar } from '@liulidesign/core/TopBar';
```

TopBar renders a custom in-page header row (for example, a search bar or segmented filter) with centered title/content and optional leading and trailing slots. The page already keeps its content clear of system bars and host chrome, so the bar adds only its own padding and spans whatever pane it sits in; page code never measures chrome rects.

## Examples

### Page header with trailing action

```tsx
import { Said } from '../example';
import { IconButton } from '@liulidesign/core/IconButton';
import { TopBar } from '@liulidesign/core/TopBar';

/** A page header with a title and a trailing action. */
export function StandardTopBar() {
  return (
    <Said>
      {say => (
        <view className="overflow-hidden rounded-container border border-text/12">
          <TopBar
            title="Orders"
            trailing={<IconButton icon="add" label="New order" onClick={() => say('Created new order')} />}
            data-testid="orders-topbar"
          />
        </view>
      )}
    </Said>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `leading` | `Child` |  | Optional leading control (e.g. back button or filter). |
| `title` | `string` |  | Centered title when no custom center children are provided. |
| `children` | `Child` |  | Custom center content (e.g. a SearchField). |
| `trailing` | `Child` |  | Optional trailing page actions. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | 44 dp minimum height header row with bottom hairline. |
| leading | `<view>` | no | Optional inline-start control slot. |
| center | `<view>` | yes | Centered slot holding custom `children` or `title`. |
| title | `<text>` | no | Truncated title text when `children` is omitted. |
| trailing | `<view>` | no | Optional inline-end actions slot. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `topBarRecipe` from `@liulidesign/core/TopBar/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`.
- Colour roles it paints with, set by the theme pack: `surface`, `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Leading and trailing slots are logical, so they swap sides in RTL.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Prefer declarative `<page title headerActions>` for standard titles; use `<TopBar>` when the center of the bar holds a custom control such as `<SearchField>`.
- Don't: Place app-close or split-screen host controls inside `<TopBar>`; window and app lifecycle controls belong to the host shell.

Related: SearchField, IconButton.
