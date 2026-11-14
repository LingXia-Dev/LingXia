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

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The trigger's text and accessible name; the panel's name too, unless `title` is set. |
| `title` | `string` |  | A heading at the top of the panel, and its accessible name. |
| `children` (required) | `Child` |  | The panel's content: a few lines, a short form, a handful of controls. |
| `isLabelHidden` | `boolean` | `false` | Draws only `icon` on a square trigger; `label` stays the name. |
| `icon` | `Child` |  | An icon on the trigger, before the label. |
| `variant` | `ButtonVariant` | `'secondary'` | The trigger's look, from Button. |
| `size` | `ButtonSize` | `'md'` |  |
| `placement` | `PopoverPlacement` | `'bottom'` | Which side of the trigger the panel opens on. |
| `alignment` | `PopoverAlignment` | `'start'` | Which edge of the trigger the panel lines up with. |
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
| title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.10:1 |
| panel edge | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep the panel short: a title and a few controls or lines.
- Do: Apply its controls at once; it has no Save.
- Do: Open it upward when the trigger sits near the bottom of the screen.
- Don't: Put a task that must be finished in a popover; use a Dialog or a BottomSheet.
- Don't: Use it for a list of actions; that is a DropdownMenu.
- Don't: Open a popover from a popover.

Related: DropdownMenu, Tooltip, Dialog.
