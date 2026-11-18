<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Popover

A button that opens a panel of content above or below it, closed by a tap outside. Status: preview.

```ts
import { Popover } from '@liulidesign/core/Popover';
```

Popover shows a small panel beside the button that opened it: view options, a share link, a short form. It is not modal; a tap outside, Escape or back closes it and focus goes back to the button. For a list of actions use DropdownMenu; for a note about something use Tooltip; for a decision use Dialog.

## Examples

### View options under the trigger

```tsx
import { useLocal } from '@lingxia/view';
import { Switch } from '@liulidesign/core/Switch';
import { Popover } from '@liulidesign/core/Popover';

/** View options for a list: switches that apply at once, in a panel under the trigger. */
export function ViewOptions() {
  const previews = useLocal('previews', true);
  const dates = useLocal('dates', false);
  return (
    <view className="gap-3">
      <Popover label="View options" title="Show in the list" data-testid="options">
        <Switch bind={previews} label="Previews" size="sm" data-testid="previews" />
        <Switch bind={dates} label="Dates" size="sm" data-testid="dates" />
      </Popover>
      <text className="text-footnote text-muted" data-testid="summary">
        {[previews.value && 'Previews', dates.value && 'Dates'].filter(Boolean).join(' and ') || 'Titles only'}
      </text>
    </view>
  );
}
```

### A share link above the trigger

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Popover } from '@liulidesign/core/Popover';

/** Sharing a note: the link and one action, opening above a trigger that sits low on the page. */
export function Share() {
  const copied = useLocal('copied', false);
  return (
    <view className="items-end">
      <Popover label="Share" title="Anyone with the link can view" placement="top" alignment="end" variant="primary" data-testid="share">
        <text className="text-footnote text-muted select-text">notes.example.com/s/q3-plan</text>
        <Button label={copied.value ? 'Copied' : 'Copy link'} size="sm" width="content" onClick={() => copied.set(true)} data-testid="copy" />
      </Popover>
    </view>
  );
}
```

### Beside the trigger

```tsx
import { Popover } from '@liulidesign/core/Popover';

/** Beside its trigger: a tool at the edge of a canvas opens its options at its side. */
export function Beside() {
  return (
    <view className="items-start gap-2">
      <Popover label="Line" title="Line" placement="end" data-testid="beside">
        <text className="text-footnote text-muted">Weight and style for the shapes you draw next.</text>
      </Popover>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The trigger's text and accessible name; the panel's name too, unless `title` is set. |
| `title` | `string` |  | A heading at the top of the panel, and its accessible name. |
| `children` (required) | `Child` |  | The panel's content: a few lines, a short form, a handful of controls. |
| `isLabelHidden` | `boolean` | `false` | Draws only `icon` on a square trigger; `label` stays the name. |
| `icon` | `"add" \| "add-circle" \| "arrow-down" \| "arrow-end" \| "arrow-start" \| "arrow-up" \| "attach" \| "blocked" \| "bookmark" \| "calendar" \| "camera" \| "cancel" \| "cart" \| "chart" \| "check" \| "checklist" \| "chevron-down" \| "chevron-end" \| "chevron-start" \| "chevron-up" \| "click" \| "clock" \| "close" \| "cloud" \| "collapse" \| "compass" \| "copy" \| "dark" \| "delete" \| "document" \| "download" \| "edit" \| "error" \| "expand" \| "external-link" \| "file" \| "filter" \| "flag" \| "folder" \| "globe" \| "grid" \| "heart" \| "help" \| "hidden" \| "home" \| "image" \| "info" \| "layers" \| "light" \| "like" \| "link" \| "list" \| "location" \| "lock" \| "mail" \| "menu" \| "message" \| "microphone" \| "more" \| "more-vertical" \| "mute" \| "notifications" \| "notifications-off" \| "palette" \| "pause" \| "payment" \| "phone" \| "play" \| "print" \| "redo" \| "refresh" \| "remove" \| "rows" \| "save" \| "search" \| "send" \| "settings" \| "share" \| "sign-in" \| "sign-out" \| "sort" \| "star" \| "success" \| "tag" \| "template" \| "text-input" \| "undo" \| "unlock" \| "upload" \| "user" \| "users" \| "video" \| "visible" \| "volume" \| "warning" \| "wifi"` |  | An icon from the set, on the trigger before the label. |
| `variant` | `ButtonVariant` | `'secondary'` | The trigger's look, from Button. |
| `size` | `ButtonSize` | `'md'` |  |
| `placement` | `PopoverPlacement` | `'bottom'` | Which side of the trigger the panel opens on. Beside the trigger it flips to the other side for room, and falls back to below or above. |
| `alignment` | `PopoverAlignment` | `'start'` | Which edge of the trigger the panel lines up with: for `end` and `start`, `start` is the top of the trigger and `end` its bottom. |
| `isOpen` | `boolean` |  | Controls whether it is open; leave it out to let the trigger own it. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  | Classes for the trigger. |
| `panelClassName` | `string` |  | Classes for the panel. |
| `data-testid` | `string` |  | On the trigger; the panel gets `<testid>-panel`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<view>` | yes | The raised card above or below the trigger, named by `title` or else `label`. |
| title | `<text>` | no | `title`, drawn as a heading at the top of the panel. |

## What an app may override

- `className`: Classes for the trigger.
- `panelClassName`: Classes for the panel.
- The recipe, `popoverRecipe` from `@liulidesign/core/Popover/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`, `--shadow-raised`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `surface-overlay`, `text`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: expanded (from whether the panel is open).
- Enter, Space: On the trigger, opens or closes the panel..
- Escape: Closes the panel; focus returns to the trigger..
- Press target: sm 32 px, md 40 px, lg 48 px.
- The trigger reports expanded while the panel is open.
- A tap outside closes the panel and reaches nothing else, as with any light-dismiss popover.
- It opens below the trigger or above it only; near the bottom of the screen open it above, as it stays under the tab bar.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| panel edge | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep the panel short: a title and a few controls or lines.
- Do: Apply its controls at once; it has no Save.
- Do: Open it upward when the trigger sits near the bottom of the screen.
- Don't: Put a task that must be finished in a popover; use a Dialog or a BottomSheet.
- Don't: Use it for a list of actions; that is a DropdownMenu.
- Don't: Open a popover from a popover.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A panel beside its trigger, at the start or end side. | side values for a popover's `positionArea` (`inline-end`, `inline-start`), flipping to the other side where there is no room | Above or below the trigger, lined up with its start, centre or end. |

Related: DropdownMenu, Tooltip, Dialog.
