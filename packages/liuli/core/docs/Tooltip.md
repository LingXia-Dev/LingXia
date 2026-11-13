<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Tooltip

An info button that opens a short note about what is beside it. Status: preview.

```ts
import { Tooltip } from '@liulidesign/core/Tooltip';
```

Tooltip explains something the screen cannot say in place: why a limit applies, what a term means. Today it opens on a press (a tap, Enter or Space), not on hover or focus, so it works the same with touch, mouse and keyboard.

## Examples

### Beside the thing it explains

```tsx
import { Tooltip } from '@liulidesign/core/Tooltip';

/** Beside the thing it explains. */
export function Storage() {
  return (
    <view className="flex-row items-center gap-1">
      <text className="text-body text-text">Storage used: 9.6 of 10 GB</text>
      <Tooltip label="About storage" content="Attachments count toward storage. Notes themselves do not." data-testid="storage" />
    </view>
  );
}
```

### Opening below

```tsx
import { Tooltip } from '@liulidesign/core/Tooltip';

/** Below the button, where there is no room above. */
export function Below() {
  return (
    <view className="flex-row items-center gap-1">
      <text className="text-body text-text">Two-step sign-in</text>
      <Tooltip label="About two-step sign-in" content="A code from your phone, on top of your password." placement="bottom" data-testid="two-step" />
    </view>
  );
}
```

### Next to a field label

```tsx
import { Tooltip } from '@liulidesign/core/Tooltip';

/** Next to a field label, for what the label cannot say. */
export function InForm() {
  return (
    <view className="gap-1.5">
      <view className="flex-row items-center gap-0.5">
        <text className="text-footnote font-medium text-text">Workspace URL</text>
        <Tooltip label="About the workspace URL" content="People you invite open this address. You can change it once a month." size="sm" data-testid="url" />
      </view>
      <view className="min-h-control-md justify-center px-3 rounded-control border border-line-strong bg-surface">
        <text className="text-body text-text">notes.example.com/atlas</text>
      </view>
    </view>
  );
}
```

### Sizes

```tsx
import { Tooltip } from '@liulidesign/core/Tooltip';

export function Sizes() {
  return (
    <view className="flex-row items-center gap-4">
      <Tooltip label="Small" content="Small." size="sm" />
      <Tooltip label="Medium" content="Medium." />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The info button's name: what the note is about ("About storage"). |
| `content` (required) | `string` |  | The note: one or two short sentences. |
| `placement` | `TooltipPlacement` | `'top'` | Which side of the button the note opens on. |
| `size` | `TooltipSize` | `'md'` |  |
| `isOpen` | `boolean` |  | Controlled open state; pass `onOpenChange` with it. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the info button; the note gets `<testid>-content`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| trigger | `<button>` | yes | The info button, named by `label`; it reports whether the note is open. |
| icon | `<vector>` | yes | The info glyph. |
| bubble | `<view>` | yes | The note: a tooltip popover beside the button, in the inverse of the page. |
| text | `<text>` | yes | The note's text; the button is described by it while open. |

## Accessibility

- Pattern: button.
- Name: label.
- Description: the note, while open.
- States: expanded (from isOpen, or a press).
- Enter, Space: Opens or closes the note; focus alone does not open it..
- Escape: Closes it..
- Press target: sm 32 px, md 40 px.
- A tap anywhere else closes the note, and that tap reaches nothing else.
- Put only words in the note; anything to press belongs in a DropdownMenu or a Dialog.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| note text | `surface` on `text` | 4.5:1 | 16.26:1 | 13.87:1 |
| info glyph | `text-muted` on `surface` | 3:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Name the button for its subject: "About storage".
- Do: Keep the note to one or two short sentences.
- Don't: Hide what someone needs to finish a task in a note; write it on the page.
- Don't: Give every label a note; add one only where people get stuck.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Opening on hover with a mouse, after a short delay, and on keyboard focus. | a hint popover that a tap elsewhere closes, as a light-dismiss popover does | The note opens on a press only. |
| A live trigger shown pressed in the gallery. | a way to hold a view in a forced state (pressed, open) for display | The pressed and open pictures are still drawings. |

Related: DropdownMenu, Dialog.
