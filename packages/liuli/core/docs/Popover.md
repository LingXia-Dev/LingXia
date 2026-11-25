<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Popover

A panel of content that opens beside its trigger, closed by a tap outside. Status: preview.

```ts
import { Popover } from '@liulidesign/core/Popover';
```

Popover shows a small panel, its `content`, beside the trigger that opened it: view options, a share link, a short form. Its children are the trigger, a Button or IconButton, or any element, which is wrapped in a plain button. The panel is a non-modal dialog: Tab can leave it, and a tap outside, Escape or back closes it and focus goes back to the trigger. For a list of actions use DropdownMenu; for a note about something use InfoTip; for a decision use Dialog.

## Examples

### View options under the trigger

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Switch } from '@liulidesign/core/Switch';
import { Popover } from '@liulidesign/core/Popover';

/** View options for a list: switches that apply at once, in a panel under the trigger. */
export function ViewOptions() {
  const previews = useLocal('previews', true);
  const dates = useLocal('dates', false);
  return (
    <view className="gap-3">
      <Popover
        label="View options"
        title="Show in the list"
        content={<>
          <Switch bind={previews} label="Previews" size="sm" data-testid="previews" />
          <Switch bind={dates} label="Dates" size="sm" data-testid="dates" />
        </>}
        data-testid="options"
      >
        <Button label="View options" data-testid="options" />
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
      <Popover
        label="Share"
        title="Anyone with the link can view"
        placement="top"
        alignment="end"
        content={<>
          <text className="text-footnote text-muted select-text">notes.example.com/s/q3-plan</text>
          <Button label={copied.value ? 'Copied' : 'Copy link'} size="sm" width="content" onClick={() => copied.set(true)} data-testid="copy" />
        </>}
        data-testid="share"
      >
        <Button label="Share" variant="primary" data-testid="share" />
      </Popover>
    </view>
  );
}
```

### Beside the trigger

```tsx
import { Button } from '@liulidesign/core/Button';
import { Popover } from '@liulidesign/core/Popover';

/** Beside its trigger: a tool at the edge of a canvas opens its options at its side. */
export function Beside() {
  return (
    <view className="items-start gap-2">
      <Popover label="Line" title="Line" placement="end" content={<text className="text-footnote text-muted">Weight and style for the shapes you draw next.</text>} data-testid="beside">
        <Button label="Line" data-testid="beside" />
      </Popover>
    </view>
  );
}
```

### Against an element that is not its trigger

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Popover } from '@liulidesign/core/Popover';

/** Against something that is not its trigger: a row's count opens its detail from a button at the foot of the list. */
export function Anchored() {
  const isOpen = useLocal('open', false);
  return (
    <view className="gap-3">
      <view className="flex-row items-center justify-between gap-3">
        <text className="text-body text-text">Shared albums</text>
        <view id="album-conflicts" className="rounded-full bg-warning-soft px-2 py-0.5"><text className="text-footnote font-medium text-text">3 conflicts</text></view>
      </view>
      <Button label="Explain conflicts" width="content" onClick={() => isOpen.set(true)} data-testid="explain" />
      <Popover
        anchor="album-conflicts"
        isOpen={isOpen.value}
        onOpenChange={isOpen.set}
        label="Conflicts"
        title="3 photos changed in two places"
        content={<text className="text-footnote text-muted">Pick which copy to keep from each photo's page.</text>}
        data-testid="conflicts"
      />
    </view>
  );
}
```

### On a link in a sentence

```tsx
import { Popover } from '@liulidesign/core/Popover';
import { Link } from '@liulidesign/core/Link';

/** On a link in a sentence: the panel opens from the linked words, which stay in their line. */
export function InSentence() {
  return (
    <text className="text-body text-text">
      Your parcel arrives{' '}
      <Popover label="Delivery window" content={<text className="text-footnote text-text">Sunday between 18:00 and 20:00. Someone needs to be in to sign.</text>} data-testid="window">
        <Link label="on Sunday" data-testid="window-link" />
      </Popover>
      , at the front door.
    </text>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `children` | `Child` |  | The trigger: a Button, an IconButton or any element, which is then wrapped in a plain button. |
| `anchor` | `string` |  | The id of an element elsewhere on the page to open against, in place of a trigger: a chart mark, a row's badge. |
| `isOpen` | `boolean` |  | Controls whether it is open; leave it out to let the trigger own it. Whether it is open: with `anchor` there is no trigger, so the caller opens it. |
| `content` (required) | `Child` |  | The panel's content: a few lines, a short form, a handful of controls. |
| `label` (required) | `string` |  | The panel's accessible name, unless `title` is set. |
| `title` | `string` |  | A heading at the top of the panel, and its accessible name. |
| `placement` | `PopoverPlacement` | `'bottom'` | Which side of the trigger the panel opens on. Beside the trigger it flips to the other side for room, and falls back to below or above. |
| `alignment` | `PopoverAlignment` | `'start'` | Which edge of the trigger the panel lines up with: for `end` and `start`, `start` is the top of the trigger and `end` its bottom. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `panelClassName` | `string` |  | Classes for the panel. |
| `data-testid` | `string` |  | On the trigger (the wrapper, or a Button or Link child without a test id of its own), and on the panel as `<testid>-panel`. |

- `children` | `anchor`: props from one side only.

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<view>` | yes | The raised card above or below the trigger, named by `title` or else `label`. |
| title | `<text>` | no | `title`, drawn as a heading at the top of the panel. |

## What an app may override

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
- Tab: Moves through the panel's controls and on past it; the panel stays open..
- Escape: Closes the panel; focus returns to the trigger..
- Press target: sm 32 px, md 40 px, lg 48 px.
- The panel is a dialog named by `title`, else `label`; it is not modal, so the page stays readable around it.
- The trigger reports expanded while the panel is open.
- A tap outside closes the panel and reaches nothing else, as with any light-dismiss popover.
- It opens below the trigger or above it only; near the bottom of the screen open it above, as it stays under the tab bar.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| panel edge | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use a `Link` as the trigger for words in a sentence: put the overlay in the `<text>`, around the Link, and the panel opens from the linked words.
- Do: Keep the panel short: a title and a few controls or lines.
- Do: Apply its controls at once; it has no Save.
- Do: Open it upward when the trigger sits near the bottom of the screen.
- Don't: Put a task that must be finished in a popover; use a Dialog or a BottomSheet.
- Don't: Use it for a list of actions; that is a DropdownMenu.
- Don't: Open a popover from a popover.
- Don't: Reach for `anchor` when a trigger cannot hold it, such as a chart mark; the caller then opens and closes it.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A panel beside its trigger, at the start or end side. | side values for a popover's `positionArea` (`inline-end`, `inline-start`), flipping to the other side where there is no room | Above or below the trigger, lined up with its start, centre or end. |

Related: DropdownMenu, InfoTip, Dialog.
