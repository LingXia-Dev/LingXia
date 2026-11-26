<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Icon

An icon by what it means, from a set an app or theme can replace. Status: preview.

```ts
import { iconPaths, useIcon, Icon, IconSetProvider } from '@liulidesign/core/Icon';
```

Icon draws one icon in the colour of the text around it. Code asks for an icon by meaning, such as `close`, `search` or `chevron-end`, and the set in use decides its shape: the default set is 97 icons from Lucide, and `IconSetProvider` swaps in another set, or a single icon, for everything inside it. Icons that point along the reading direction turn round in right-to-left layouts.

## Examples

### Icons by name

```tsx
import { Icon } from '@liulidesign/core/Icon';

/** Icons are named by what they mean, so a theme can redraw them without touching this code. */
export function Common() {
  return (
    <view className="flex-row flex-wrap gap-5 text-text">
      <Icon name="search" data-testid="search" />
      <Icon name="add" />
      <Icon name="edit" />
      <Icon name="share" />
      <Icon name="delete" />
      <Icon name="settings" />
      <Icon name="more" />
    </view>
  );
}
```

### Sizes

```tsx
import { Icon } from '@liulidesign/core/Icon';

export function Sizes() {
  return (
    <view className="flex-row items-center gap-5 text-text">
      <Icon name="calendar" size="sm" data-testid="small" />
      <Icon name="calendar" data-testid="medium" />
      <Icon name="calendar" size="lg" data-testid="large" />
    </view>
  );
}
```

### The size of the text around it

```tsx
import { Icon } from '@liulidesign/core/Icon';

/** `inline` takes its size from the text around it: larger in a title, smaller in a footnote. */
export function Inline() {
  return (
    <view className="gap-3 text-text">
      <view className="flex-row items-center gap-2 text-title">
        <Icon name="calendar" size="inline" data-testid="in-title" />
        <text className="text-title font-semibold">Planning</text>
      </view>
      <view className="flex-row items-center gap-1.5 text-footnote text-muted">
        <Icon name="clock" size="inline" data-testid="in-footnote" />
        <text className="text-footnote text-muted">Updated an hour ago</text>
      </view>
    </view>
  );
}
```

### Colour from the text around it

```tsx
import { Icon } from '@liulidesign/core/Icon';

/** An icon takes the colour of the text around it. */
export function Colours() {
  return (
    <view className="gap-3">
      <view className="flex-row items-center gap-2 text-text">
        <Icon name="document" />
        <text className="text-body">Quarterly report</text>
      </view>
      <view className="flex-row items-center gap-2 text-muted">
        <Icon name="clock" size="sm" />
        <text className="text-footnote">Edited 2 hours ago</text>
      </view>
      <view className="flex-row items-center gap-2 text-danger">
        <Icon name="error" size="sm" data-testid="tinted" />
        <text className="text-footnote">Upload failed</text>
      </view>
      <view className="flex-row items-center gap-2 text-text/40">
        <Icon name="close" size="sm" data-testid="faded" />
        <text className="text-footnote">Cancelled</text>
      </view>
    </view>
  );
}
```

### Read, or silent

```tsx
import { Icon } from '@liulidesign/core/Icon';

/** Alone, an icon needs a label to be read; beside text that says the same, it stays silent. */
export function Labelled() {
  return (
    <view className="flex-row items-center gap-5">
      <Icon name="lock" label="Private" className="text-muted" data-testid="private" />
      <view className="flex-row items-center gap-1.5 text-success">
        <Icon name="success" size="sm" data-testid="silent" />
        <text className="text-footnote font-medium">Paid</text>
      </view>
    </view>
  );
}
```

### Right to left

```tsx
import { Icon } from '@liulidesign/core/Icon';

/** Icons that point along the reading direction turn round in a right-to-left layout; the rest do not. */
export function Mirrored() {
  return (
    <view className="gap-3 text-text">
      <view className="flex-row items-center gap-5">
        <Icon name="chevron-end" data-testid="forward" />
        <Icon name="arrow-end" />
        <Icon name="send" />
        <Icon name="search" />
      </view>
      <view dir="rtl" className="flex-row items-center gap-5">
        <Icon name="chevron-end" data-testid="forward-rtl" />
        <Icon name="arrow-end" />
        <Icon name="send" />
        <Icon name="search" />
      </view>
    </view>
  );
}
```

### Your own icon, and a swapped one

```tsx
import { Icon, IconSetProvider, type IconDefinition } from '@liulidesign/core/Icon';

const bolt: IconDefinition = { paths: ['M13 2 4.5 13.5H11L10 22l8.5-11.5H12z'] };
const cross: IconDefinition = { paths: ['M7 7l10 10M17 7 7 17', 'M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0z'] };

/** An icon of the app's own, and a set that redraws one name for everything inside it. */
export function Custom() {
  return (
    <view className="flex-row items-center gap-5 text-text">
      <Icon icon={bolt} data-testid="own" />
      <Icon name="close" data-testid="default-close" />
      <IconSetProvider icons={{ close: cross }}>
        <view className="flex-row items-center gap-5">
          <Icon name="close" data-testid="swapped-close" />
          <Icon name="search" data-testid="kept-search" />
        </view>
      </IconSetProvider>
    </view>
  );
}
```

### The whole set

```tsx
import { Icon, iconNames } from '@liulidesign/core/Icon';

const half = Math.ceil(iconNames.length / 2);

/** Every name in the set, in two columns. */
export function Everything() {
  return (
    <view className="flex-row gap-4">
      {[iconNames.slice(0, half), iconNames.slice(half)].map((column, index) => (
        <view key={String(index)} className="min-w-0 grow basis-0 gap-3">
          {column.map(name => (
            <view key={name} className="min-w-0 flex-row items-center gap-2.5">
              <Icon name={name} className="text-text" />
              <text className="min-w-0 grow basis-0 truncate text-footnote text-muted">{name}</text>
            </view>
          ))}
        </view>
      ))}
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `name` | `"add" \| "add-circle" \| "align-center" \| "align-end" \| "align-start" \| "arrow-down" \| "arrow-end" \| "arrow-start" \| "arrow-up" \| "attach" \| "blocked" \| "bold" \| "bookmark" \| "calendar" \| "camera" \| "cancel" \| "cart" \| "chart" \| "check" \| "checklist" \| "chevron-down" \| "chevron-end" \| "chevron-start" \| "chevron-up" \| "click" \| "clock" \| "close" \| "cloud" \| "code" \| "collapse" \| "compass" \| "copy" \| "dark" \| "delete" \| "document" \| "download" \| "edit" \| "error" \| "expand" \| "external-link" \| "file" \| "filter" \| "flag" \| "folder" \| "formula" \| "globe" \| "grid" \| "heading" \| "heart" \| "help" \| "hidden" \| "highlight" \| "home" \| "image" \| "info" \| "italic" \| "layers" \| "light" \| "like" \| "link" \| "list" \| "location" \| "lock" \| "mail" \| "menu" \| "message" \| "microphone" \| "more" \| "more-vertical" \| "mute" \| "notifications" \| "notifications-off" \| "palette" \| "pause" \| "payment" \| "phone" \| "play" \| "print" \| "redo" \| "refresh" \| "remove" \| "rows" \| "save" \| "search" \| "send" \| "settings" \| "share" \| "sign-in" \| "sign-out" \| "sort" \| "star" \| "stop" \| "strikethrough" \| "subscript" \| "success" \| "superscript" \| "table" \| "tag" \| "template" \| "text-input" \| "underline" \| "undo" \| "unlock" \| "upload" \| "user" \| "users" \| "video" \| "visible" \| "volume" \| "warning" \| "wifi"` |  | The icon by what it means: `close`, `search`, `chevron-end`. The set in use decides how it is drawn. |
| `icon` | `IconDefinition` |  | An icon of the app's own, or one module from a set, drawn as given whatever set is in use. |
| `size` | `IconSize` | `'md'` | 16, 20 or 24 points; `inline` follows the text around it, 20 beside body text and growing with a heading. |
| `label` | `string` |  | What the icon says, when nothing beside it does. Without it the icon is decoration and is not read. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

- `name` | `icon`: props from one side only.

## IconSetProvider props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `icons` (required) | `Partial<Readonly<Record<"info" \| "success" \| "warning" \| "add" \| "remove" \| "close" \| "check" \| "edit" \| "delete" \| "copy" \| "share" \| "download" \| "upload" \| "refresh" \| "search" \| "filter" \| "sort" \| "more" \| "more-vertical" \| "menu" \| "settings" \| "send" \| "save" \| "undo" \| "redo" \| "link" \| "external-link" \| "attach" \| "print" \| "sign-in" \| "sign-out" \| "chevron-down" \| "chevron-up" \| "chevron-start" \| "chevron-end" \| "arrow-up" \| "arrow-down" \| "arrow-start" \| "arrow-end" \| "home" \| "expand" \| "collapse" \| "error" \| "help" \| "cancel" \| "add-circle" \| "blocked" \| "lock" \| "unlock" \| "visible" \| "hidden" \| "user" \| "users" \| "calendar" \| "clock" \| "mail" \| "phone" \| "message" \| "notifications" \| "notifications-off" \| "star" \| "heart" \| "like" \| "bookmark" \| "tag" \| "flag" \| "location" \| "image" \| "camera" \| "file" \| "document" \| "folder" \| "cloud" \| "globe" \| "light" \| "dark" \| "cart" \| "payment" \| "chart" \| "list" \| "grid" \| "play" \| "pause" \| "stop" \| "volume" \| "mute" \| "microphone" \| "video" \| "wifi" \| "palette" \| "click" \| "text-input" \| "checklist" \| "compass" \| "layers" \| "rows" \| "template" \| "bold" \| "italic" \| "underline" \| "strikethrough" \| "code" \| "highlight" \| "superscript" \| "subscript" \| "heading" \| "table" \| "formula" \| "align-start" \| "align-center" \| "align-end", IconDefinition>>>` |  | The icons to draw for the names they cover; the rest stay as the default set draws them. |
| `children` (required) | `Element` |  | One element; every Liuli component inside it draws from `icons`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| glyph | `<vector>` | yes | The drawing, on a 24-unit grid, in the current text colour. |
| box | `<view>` | no | A box around the drawing that carries `label`; present only with a label. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `iconRecipe` from `@liulidesign/core/Icon/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Without `label` an icon is decoration: it is hidden from assistive tech, and the text or control beside it says what it means.
- With `label` it is an image named by the label. Use it only when the icon stands alone and carries meaning.
- An icon is never a control. A pressable icon is an IconButton, which has the press target and the name.
- An icon that carries meaning alone needs 3:1 against its background; `text-faint` does not reach it on every surface.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| an icon that stands alone, in text colour | `text` on `surface` | 3:1 | 16.26:1 | 13.87:1 |
| an icon that stands alone, muted | `text-muted` on `surface` | 3:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Ask by meaning: `name="delete"`, not the shape. A theme may draw it differently.
- Do: Set the colour on the icon or its container with a text colour class; the icon follows it.
- Do: Use `sm` beside footnote text, `md` beside body text and in controls, `lg` on its own.
- Do: Use `inline` in a heading or a line of another size; the icon grows with that text and with the reader's text size.
- Do: Draw an icon the set lacks with `icon`, on a 24-unit grid with a stroke of the same weight.
- Do: Give a component's icon prop (`startIcon`, `icon`) a name; to change how a name draws inside components, replace it with `IconSetProvider`.
- Don't: Give an icon a label when text beside it says the same; it would be read twice.
- Don't: Use an icon alone for an action people rarely take; pair it with a word.
- Don't: Flip a directional icon yourself; the set marks which ones mirror.
- Don't: Scale an icon with a class to sizes between the three; the stroke would go soft.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A stroke that keeps one weight at every size. | nothing from the View; a decision on optical sizes | The stroke scales with the icon: 1.2 px at 16, 1.5 px at 20, 1.8 px at 24. |
| Filled and two-tone variants of a name. | a second set and a variant on the name | One outline per name; an app passes its own filled icon with `icon`. |

Related: Button, Item.
