<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Button

Runs an action when pressed, with variants, sizes and a loading state. Status: preview.

```ts
import { Button } from '@liulidesign/core/Button';
```

Button starts an action: save, send, delete, open a dialog. Its `label` is both its text and its accessible name, and it can wait on an async action with `clickAction`. To open a menu use DropdownMenu.

## Examples

### Variants

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';

export function Variants() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap gap-2">
          <Button label="Save changes" variant="primary" onClick={() => say('Saved changes')} data-testid="primary" />
          <Button label="Preview" onClick={() => say('Opened the preview')} />
          <Button label="Turn off internet" variant="tonal" startIcon="pause" onClick={() => say('Turned off the internet')} data-testid="tonal" />
          <Button label="Skip" variant="ghost" onClick={() => say('Skipped')} />
          <Button label="Delete draft" variant="destructive" onClick={() => say('Deleted the draft')} />
        </view>
      )}
    </Said>
  );
}
```

### A quiet filled button for a card's own action

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { Card } from '@liulidesign/core/Card';

/** A card's own action, filled but quiet beside the page's one primary button; off while it cannot run. */
export function VariantTonal() {
  return (
    <Said>
      {say => (
        <view className="gap-3">
          <Card>
            <view className="gap-3">
              <text className="text-body">Emily's devices are online.</text>
              <view className="flex-row flex-wrap gap-2">
                <Button label="Turn off internet" variant="tonal" startIcon="pause" onClick={() => say('Turned off the internet')} />
                <Button label="Study time" variant="tonal" isDisabled disabledMessage="No schedule is set" />
              </view>
            </view>
          </Card>
        </view>
      )}
    </Said>
  );
}
```

### Sizes

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';

export function Sizes() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap items-center gap-2">
          <Button label="Small" size="sm" onClick={() => say('Chose Small')} />
          <Button label="Medium" onClick={() => say('Chose Medium')} />
          <Button label="Large" size="lg" onClick={() => say('Chose Large')} />
        </view>
      )}
    </Said>
  );
}
```

### With icons

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';

/** An icon before the label says what kind of action it is; one after it says where the button leads. */
export function WithIcons() {
  return (
    <Said>
      {say => (
        <view className="flex-row flex-wrap gap-3">
          <Button label="New note" variant="primary" startIcon="add" onClick={() => say('Created a note')} data-testid="new-note" />
          <Button label="Share" startIcon="share" onClick={() => say('Shared')} />
          <Button label="Continue" endIcon="arrow-end" onClick={() => say('Continued')} data-testid="continue-on" />
          <Button label="Delete" variant="destructive" startIcon="delete" onClick={() => say('Deleted')} />
        </view>
      )}
    </Said>
  );
}
```

### Waiting on a Logic action

```tsx
import { useView, type View } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';

type Actions = { publish(): Promise<void> };

/** `clickAction` takes a Logic action as it is; the button shows loading until it settles. */
export function AsyncAction() {
  const { actions } = useView() as View<Actions>;
  return <Button label="Publish" variant="primary" width="content" clickAction={() => actions.publish()} data-testid="publish" />;
}
```

### Disabled, with the reason

```tsx
import { useLocal } from '@lingxia/view';
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';

/** The reason is read after the label while disabled. Pass translated copy; Liuli ships none. */
export function DisabledWithReason() {
  const agreed = useLocal('agreed', false);
  return (
    <Said>
      {say => (
        <view className="gap-2">
          <Button
            label="Continue"
            variant="primary"
            width="fill"
            isDisabled={!agreed.value}
            disabledMessage="Accept the terms first"
            onClick={() => say('Continued')}
            data-testid="continue"
          />
          <Button
            label={agreed.value ? 'Withdraw consent' : 'Accept the terms'}
            variant="ghost"
            width="content"
            onClick={() => agreed.set(!agreed.value)}
            data-testid="toggle-terms"
          />
        </view>
      )}
    </Said>
  );
}
```

### Floating over a list

```tsx
import { Said } from '../example';
import { Button } from '@liulidesign/core/Button';
import { Item } from '@liulidesign/core/Item';

const notes = [
  ['Trip to the coast', 'Edited yesterday'],
  ['Reading list', 'Edited on Monday'],
  ['Ideas for the garden', 'Edited last week'],
  ['Recipes to try', 'Edited on 2 November'],
  ['Gift list', 'Edited on 28 October'],
  ['Books of the year', 'Edited on 14 October'],
] as const;

/** Floating: the screen's one main action, inside the frame, over a list that scrolls under it. */
export function Floating() {
  return (
    <Said>
      {say => (
        <view className="relative h-72 overflow-hidden rounded-container border border-text/12 bg-surface">
          <scroll className="h-full">
            <view className="divide-y divide-text/8">
              {notes.map(([label, edited]) => <Item key={label} label={label} description={edited} />)}
            </view>
          </scroll>
          <view className="absolute bottom-4 end-4">
            <Button label="New note" variant="primary" size="lg" startIcon="add" isFloating onClick={() => say('Created a note')} data-testid="floating" />
          </view>
        </view>
      )}
    </Said>
  );
}
```

### Going to a page

```tsx
import { Button } from '@liulidesign/core/Button';

/** A button that goes to a page: a link drawn as a button, read as a link. */
export function ToPage() {
  return <Button label="Open settings" endIcon="chevron-end" to={{ page: 'settings' }} data-testid="to-settings" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The visible text and the accessible name. Name the outcome: "Save changes", not "OK". |
| `variant` | `ButtonVariant` | `'secondary'` | In a ButtonGroup the group's variant, unless set here. `primary` for the one main action in view, `tonal` for a secondary action inside a card or row, filled without an edge, `destructive` for an action that loses data, `link` for a quiet way out beside a form's main action ("Forgot password?"). |
| `size` | `ButtonSize` | `'md'` | In a ButtonGroup the group's size, unless set here. |
| `startIcon` | `IconRef` |  | An icon before the label, by name or as a definition; hidden from assistive tech, since the label names the button. |
| `endIcon` | `IconRef` |  | An icon after the label: a chevron for a button that opens something, an arrow for one that goes on. |
| `isIconOnly` | `boolean` | `false` | Draws `startIcon` alone in a square; `label` stays the accessible name. For a named icon button use IconButton. |
| `endContent` | `Child` |  | Drawn after the label: a count, a shortcut hint. Hidden from assistive tech. |
| `isFloating` | `boolean` | `false` | Lifts the button off the page with a shadow, for the one action that floats over scrolling content. |
| `width` | `ButtonWidth` |  | `content` keeps its own width in a column; `fill` stretches across. Unset follows the parent. |
| `edge` | `ButtonEdge` |  | For a ghost or link button at the edge of a row: its padding hangs past that side, so the label or the icon's drawing lines up with the content edge, as the text above and below does. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Why it is disabled: read as the button's description while disabled; not drawn. |
| `isLoading` | `boolean` | `false` | Shows a spinner and ignores presses; the button stays focusable and reports busy. |
| `className` | `string` |  | Merged last, so its classes win over the recipe's. |
| `data-testid` | `string` |  |  |
| `onClick` | `(() => void)` |  | Runs on press, before `clickAction`. |
| `clickAction` | `(() => Promise<unknown>)` |  | An async action, such as a Logic action. The button shows loading until it settles. |
| `to` | `LinkTarget` |  | Makes it a link that goes there on press: `{ page, params }`, `{ tab }`, `{ back: 1 }`. Read as a link; Enter opens it and Space does not. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<button>` | yes | The pressable box: fill, border, press and hover paint. |
| glyph | `<vector>` | no | `startIcon`, replaced by the spinner while loading, and `endIcon`; from the icon set in use, hidden from assistive tech. |
| icon | `<view>` | no | The trailing slot that holds `endContent`. |
| spinner | `<vector>` | no | Spins while `isLoading` or a `clickAction` is pending. |
| label | `<text>` | yes | The text; never truncated, so keep it short. |

## What an app may override

- `className`: Merged last, so its classes win over the recipe's.
- The recipe, `buttonRecipe` from `@liulidesign/core/Button/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--spacing-control-lg`, `--text-footnote`, `--text-body`, `--text-callout`, `--shadow-control`, `--shadow-control-solid`, `--shadow-raised`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-solid`, `accent-solid-hover`, `accent-solid-pressed`, `danger-solid`, `danger-solid-hover`, `danger-solid-pressed`, `focus`, `on-accent-solid`, `on-danger-solid`, `surface`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: button.
- Name: label.
- Description: disabledMessage, while disabled.
- States: disabled (from isDisabled); busy (from isLoading, or a pending clickAction).
- Press target: sm 32 px, md 40 px, lg 48 px.
- A loading button stays enabled and focusable, reports busy, and ignores presses.
- Disabled, every variant turns to a faint ink fill with muted text, so the label stays readable in both themes.
- Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| primary label | `on-accent-solid` on `accent-solid` | 4.5:1 | 5.62:1 | 4.80:1 |
| destructive label | `on-danger-solid` on `danger-solid` | 4.5:1 | 5.21:1 | 4.82:1 |
| secondary label | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| secondary boundary | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |
| tonal label on its 6% ink fill, on the page | `text` on `text/6` over `canvas` | 4.5:1 | 13.54:1 | 13.46:1 |
| tonal label on its 6% ink fill, on a card | `text` on `text/6` over `surface` | 4.5:1 | 14.47:1 | 11.94:1 |
| link label | `accent` on `canvas` | 4.5:1 | 5.25:1 | 6.83:1 |
| disabled label on its 6% ink fill, on the page | `text-muted` on `text/6` over `canvas` | 4.5:1 | 4.90:1 | 6.45:1 |
| disabled label on its 6% ink fill, on a card | `text-muted` on `text/6` over `surface` | 4.5:1 | 5.24:1 | 5.72:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Give a ghost or link button at the start or end of a row `edge`, so its label or icon lines up with the text above and below instead of sitting in by its padding.
- Do: Use one primary button per view, for the action most people take.
- Do: Use `tonal` for a secondary action inside a card or a row, where an outlined button adds a second edge inside the card's.
- Do: Write the label as a verb phrase that names the outcome.
- Do: Pass a Logic action to clickAction so the button shows progress by itself.
- Do: Confirm destructive actions that cannot be undone with a Dialog.
- Do: At large text, set buttons side by side in a row that wraps (`flex-wrap`): a button keeps its label on one line and wraps it only when it alone is wider than the row.
- Don't: Put two primary buttons side by side.
- Don't: Build a button from a view and classes; only a button takes presses, focus and a role.
- Don't: Hard-code copy inside the button; labels come from the app.
- Don't: Use a button for words in a sentence that lead to another page; that is a Link.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| The disabled reason shown to sighted users. | a focusable disabled state, so a hint note can open from it | The reason is read as the description; nothing is drawn. |
| Announcing that loading started. | an announcement without a mounted live region | Busy state only; a page that needs speech keeps its own status line. |
| A keyboard focus ring that shows on a filled button. | ring offset: a gap between the box and its ring | A ring on the fill blends into it, so Button draws no focus style of its own. |

Related: IconButton, ButtonGroup, DropdownMenu, Dialog.
