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
import { Button } from '@liulidesign/core/Button';

export function Variants() {
  return (
    <view className="flex-row flex-wrap gap-2">
      <Button label="Save changes" variant="primary" data-testid="primary" />
      <Button label="Preview" />
      <Button label="Skip" variant="ghost" />
      <Button label="Delete draft" variant="destructive" />
    </view>
  );
}
```

### Sizes

```tsx
import { Button } from '@liulidesign/core/Button';

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-center gap-2">
      <Button label="Small" size="sm" />
      <Button label="Medium" />
      <Button label="Large" size="lg" />
    </view>
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
import { Button } from '@liulidesign/core/Button';

/** The reason is read after the label while disabled. Pass translated copy; Liuli ships none. */
export function DisabledWithReason() {
  const agreed = useLocal('agreed', false);
  return (
    <view className="gap-2">
      <Button
        label="Continue"
        variant="primary"
        width="fill"
        isDisabled={!agreed.value}
        disabledMessage="Accept the terms first"
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
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The visible text and the accessible name. Name the outcome: "Save changes", not "OK". |
| `variant` | `ButtonVariant` | `'secondary'` | `primary` for the one main action in view, `destructive` for an action that loses data, `link` for a quiet way out beside a form's main action ("Forgot password?"). |
| `size` | `ButtonSize` | `'md'` |  |
| `startIcon` | `Child` |  | Drawn before the label; hidden from assistive tech, since the label names the button. |
| `endContent` | `Child` |  | Drawn after the label: a count, a shortcut hint. Hidden from assistive tech. |
| `width` | `ButtonWidth` |  | `content` keeps its own width in a column; `fill` stretches across. Unset follows the parent. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Why it is disabled: read as the button's description while disabled; not drawn. |
| `isLoading` | `boolean` | `false` | Shows a spinner and ignores presses; the button stays focusable and reports busy. |
| `onClick` | `(() => void)` |  | Runs on press, before `clickAction`. |
| `clickAction` | `(() => Promise<unknown>)` |  | An async action, such as a Logic action. The button shows loading until it settles. |
| `className` | `string` |  | Merged last, so its classes win over the recipe's. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<button>` | yes | The pressable box: fill, border, press and hover paint. |
| icon | `<view>` | no | `startIcon`, replaced by the spinner while loading; a trailing slot holds `endContent`. |
| spinner | `<vector>` | no | Spins while `isLoading` or a `clickAction` is pending. |
| label | `<text>` | yes | The text; never truncated, so keep it short. |

## Accessibility

- Pattern: button.
- Name: label.
- Description: disabledMessage, while disabled.
- States: disabled (from isDisabled); busy (from isLoading, or a pending clickAction).
- Press target: sm 32 px, md 40 px, lg 48 px.
- A loading button stays enabled and focusable, reports busy, and ignores presses.
- Disabled, every variant turns to a faint ink fill with muted text, so the label stays readable in both themes.
- Sizes sm (32) and md (40) are below the 44 pt touch minimum; on touch layouts prefer lg until hit slop lets the press area extend past the paint.

| Contrast | Pair | Minimum | Proven by the theme pack |
| --- | --- | --- | --- |
| primary label | `on-accent` on `accent` | 4.5:1 | yes |
| destructive label | `on-danger` on `danger` | 4.5:1 | yes |
| secondary label | `text` on `surface` | 4.5:1 | yes |
| secondary boundary | `line-strong` on `surface` | 3:1 | yes |
| link label | `accent` on `canvas` | 4.5:1 | no |
| disabled label on its 6% ink fill | `text-muted` on `canvas` | 4.5:1 | yes |

## Guidance

- Do: Use one primary button per view, for the action most people take.
- Do: Write the label as a verb phrase that names the outcome.
- Do: Pass a Logic action to clickAction so the button shows progress by itself.
- Do: Confirm destructive actions that cannot be undone with a Dialog.
- Don't: Put two primary buttons side by side.
- Don't: Build a button from a view and classes; only a button takes presses, focus and a role.
- Don't: Hard-code copy inside the button; labels come from the app.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A 44 px press target around the sm and md sizes. | hit slop: a press area larger than the painted box | The press area equals the box; the docs steer touch layouts to lg. |
| The disabled reason shown to sighted users. | a focusable disabled state and a hint popover opened by focus or long press | The reason is read as the description; nothing is drawn. |
| Announcing that loading started. | an announcement without a mounted live region | Busy state only; a page that needs speech keeps its own status line. |

Related: DropdownMenu, Dialog.
