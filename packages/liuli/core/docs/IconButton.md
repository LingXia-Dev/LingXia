<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# IconButton

A button that shows only an icon, with a name that is always read. Status: preview.

```ts
import { IconButton } from '@liulidesign/core/IconButton';
```

IconButton is a Button drawn as a square with its icon alone, for actions people know by sight: like, share, close, more. It has no visible text, so `label` is required and is what a screen reader says. It takes Button's variants, sizes, loading and disabled states, and is quiet by default.

## Examples

### A row of actions

```tsx
import { useLocal } from '@lingxia/view';
import { IconButton } from '@liulidesign/core/IconButton';

/** The label is never drawn and always read: name what the press does. */
export function Toolbar() {
  const liked = useLocal('liked', 0);
  return (
    <view className="flex-row items-center gap-1">
      <IconButton label="Like" icon="heart" onClick={() => liked.set(liked.value + 1)} data-testid="like" />
      <IconButton label="Share" icon="share" data-testid="share" />
      <IconButton label="Bookmark" icon="bookmark" />
      <IconButton label="More actions" icon="more" />
      <text role="status" data-testid="likes" className="ps-2 text-footnote text-muted tabular-nums">{`${liked.value} likes`}</text>
    </view>
  );
}
```

### Variants

```tsx
import { IconButton } from '@liulidesign/core/IconButton';

export function Variants() {
  return (
    <view className="flex-row items-center gap-3">
      <IconButton label="Add" icon="add" variant="primary" data-testid="add" />
      <IconButton label="Edit" icon="edit" variant="secondary" />
      <IconButton label="Settings" icon="settings" data-testid="quiet" />
      <IconButton label="Delete" icon="delete" variant="destructive" />
    </view>
  );
}
```

### Sizes

```tsx
import { IconButton } from '@liulidesign/core/IconButton';

export function Sizes() {
  return (
    <view className="flex-row items-center gap-3">
      <IconButton label="Search, small" icon="search" size="sm" variant="secondary" data-testid="small" />
      <IconButton label="Search, medium" icon="search" variant="secondary" data-testid="medium" />
      <IconButton label="Search, large" icon="search" size="lg" variant="secondary" data-testid="large" />
    </view>
  );
}
```

### At the end of a row

```tsx
import { useLocal } from '@lingxia/view';
import { Item } from '@liulidesign/core/Item';
import { IconButton } from '@liulidesign/core/IconButton';

/** At the end of a row, an icon button acts on that row alone. */
export function InRows() {
  const removed = useLocal<readonly string[]>('removed', []);
  const files = ['Budget.xlsx', 'Notes.txt', 'Plan.pdf'].filter(name => !removed.value.includes(name));
  return (
    <view role="list" aria-label="Attachments" className="divide-y divide-text/8">
      {files.map(name => (
        <view key={name} role="listitem">
          <Item
            label={name}
            endContent={<IconButton label={`Remove ${name}`} icon="close" size="sm" onClick={() => removed.set([...removed.value, name])} data-testid={`remove-${name}`} />}
          />
        </view>
      ))}
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The accessible name: what pressing it does ("Delete note"). Not drawn, so it is never optional. |
| `icon` (required) | `"add" \| "add-circle" \| "arrow-down" \| "arrow-end" \| "arrow-start" \| "arrow-up" \| "attach" \| "blocked" \| "bookmark" \| "calendar" \| "camera" \| "cancel" \| "cart" \| "chart" \| "check" \| "checklist" \| "chevron-down" \| "chevron-end" \| "chevron-start" \| "chevron-up" \| "click" \| "clock" \| "close" \| "cloud" \| "collapse" \| "compass" \| "copy" \| "dark" \| "delete" \| "document" \| "download" \| "edit" \| "error" \| "expand" \| "external-link" \| "file" \| "filter" \| "flag" \| "folder" \| "globe" \| "grid" \| "heart" \| "help" \| "hidden" \| "home" \| "image" \| "info" \| "layers" \| "light" \| "like" \| "link" \| "list" \| "location" \| "lock" \| "mail" \| "menu" \| "message" \| "microphone" \| "more" \| "more-vertical" \| "mute" \| "notifications" \| "notifications-off" \| "palette" \| "pause" \| "payment" \| "phone" \| "play" \| "print" \| "redo" \| "refresh" \| "remove" \| "rows" \| "save" \| "search" \| "send" \| "settings" \| "share" \| "sign-in" \| "sign-out" \| "sort" \| "star" \| "success" \| "tag" \| "template" \| "text-input" \| "undo" \| "unlock" \| "upload" \| "user" \| "users" \| "video" \| "visible" \| "volume" \| "warning" \| "wifi"` |  | The icon, by name. |
| `variant` | `ButtonVariant` | `'ghost'` | In a ButtonGroup the group's variant, unless set here. Quiet by default: an icon button is rarely the main action. |
| `size` | `ButtonSize` | `'md'` | 32, 40 or 48 points square. In a ButtonGroup the group's size, unless set here. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Why it is disabled: read as its description while disabled; not drawn. |
| `isLoading` | `boolean` | `false` | Shows a spinner in place of the icon and ignores presses. |
| `onClick` | `(() => void)` |  |  |
| `clickAction` | `(() => Promise<unknown>)` |  | An async action; the button shows loading until it settles. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<button>` | yes | The button: a square of 32, 40 or 48 points with the icon centred, in Button's variant; the spinner takes the icon's place while loading. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `iconButtonRecipe` from `@liulidesign/core/IconButton/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- It paints with no colour role of its own.

## Accessibility

- Pattern: button.
- Name: label.
- Description: disabledMessage, while disabled.
- States: disabled (from isDisabled); busy (from isLoading, or a pending clickAction).
- Press target: sm 32 px, md 40 px, lg 48 px.
- Role button, named by `label`; the icon is hidden. The name stays the same while loading.
- On touch, a finger reaches a 32 or 40 point button within 44 points around it, never over a neighbour; the drawn square stays its size.
- The quiet variant has no edge at rest, so it relies on its place (a toolbar, the end of a row) to read as a control.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| quiet icon on a card | `text` on `surface` | 3:1 | 16.26:1 | 13.87:1 |
| quiet icon under the pointer | `text` on `text/5` over `surface` | 3:1 | 14.76:1 | 12.27:1 |
| primary icon | `on-accent` on `accent` | 3:1 | 5.62:1 | 7.12:1 |
| destructive icon | `on-danger` on `danger` | 3:1 | 5.21:1 | 6.27:1 |
| disabled icon | `text-muted` on `text/6` over `surface` | 3:1 | 5.24:1 | 5.72:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name the action in `label`, with its object when rows repeat: "Remove Budget.xlsx".
- Do: Use it for actions with an icon people already know; otherwise use a Button with text.
- Do: Keep it quiet in a toolbar or a row; use `secondary` or `primary` when it stands alone.
- Do: Group related icon buttons in a ButtonGroup, so arrow keys move among them.
- Don't: Use an icon button as the only way to a destructive action without a confirmation.
- Don't: Put two icon buttons with the same icon and different meanings on one screen.
- Don't: Shrink it with a class; use `size`.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| The label shown as a tooltip on hover and keyboard focus. | a hint that opens from any control | The label is read by assistive tech only. |

Related: Button, ButtonGroup, Icon.
