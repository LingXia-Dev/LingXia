<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Dialog

Modal panel over the page for a decision or a short task. Status: preview.

```ts
import { Dialog } from '@liulidesign/core/Dialog';
```

Dialog interrupts the page to ask for a decision or a short piece of input, and keeps focus and assistive tech inside until it closes. It is controlled: `isOpen` shows it, `onOpenChange(false)` reports every close request.

## Examples

### Confirm a destructive action, as an alert dialog

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Dialog } from '@liulidesign/core/Dialog';

/** A confirmation that interrupts: Escape and back cancel, a backdrop tap does not; the destructive action is last. */
export function Confirm() {
  const open = useLocal('open', false);
  const deleted = useLocal('deleted', false);
  return (
    <view className="gap-2">
      <Button label="Delete note" variant="destructive" width="content" onClick={() => open.set(true)} data-testid="confirm-open" />
      <text role="status" data-testid="outcome" className="text-footnote text-muted">{deleted.value ? 'Deleted' : 'Not deleted'}</text>
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Delete this note?"
        description="It leaves every device you are signed in on."
        size="sm"
        isAlert
        dismissOn="closeRequest"
        data-testid="confirm-dialog"
        actions={
          <>
            <Button label="Cancel" onClick={() => open.set(false)} data-testid="cancel" />
            <Button label="Delete" variant="destructive" onClick={() => { deleted.set(true); open.set(false); }} data-testid="confirm" />
          </>
        }
      />
    </view>
  );
}
```

### A short form

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { TextInput } from '@liulidesign/core/TextInput';
import { Dialog } from '@liulidesign/core/Dialog';

/**
 * A short form. The draft lives outside the dialog, since closing discards the dialog's own state.
 * Only Escape and back cancel, so a stray backdrop tap does not lose the typing.
 */
export function Form() {
  const open = useLocal('open', false);
  const name = useLocal('name', '');
  return (
    <view className="gap-2">
      <Button label="Rename" width="content" onClick={() => open.set(true)} data-testid="rename-open" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Rename notebook"
        closeLabel="Close"
        dismissOn="closeRequest"
        data-testid="rename-dialog"
        actions={<Button label="Save" variant="primary" isDisabled={name.value.trim() === ''} onClick={() => open.set(false)} data-testid="save" />}
      >
        <TextInput bind={name} label="Name" hasAutoFocus enterKeyHint="done" onSubmit={() => open.set(false)} data-testid="name" />
      </Dialog>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `isOpen` (required) | `boolean` |  | Whether it is shown. A closed dialog renders nothing, so local state inside starts over. |
| `onOpenChange` (required) | `(isOpen: boolean) => void` |  | Called with `false` on a close request: Escape, back, the backdrop per `dismissOn`, the close button. |
| `label` (required) | `string` |  | The title, and the dialog's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Keeps `label` as the name without drawing it as a title. |
| `description` | `string` |  | One or two sentences under the title, read as the dialog's description. |
| `isAlert` | `boolean` | `false` | Interrupts with something to confirm or acknowledge; screen readers announce it as an alert dialog. Pair it with `dismissOn="closeRequest"` or `"none"`. |
| `children` | `Child` |  | The body; it scrolls when the panel reaches its maximum height. |
| `actions` | `Child` |  | The footer, typically Buttons, the main action last. |
| `closeLabel` | `string` |  | Shows a close button in the header, named by this text. |
| `dismissOn` | `DialogDismissOn` | `'any'` | Which requests close it: `any` adds a backdrop tap to `closeRequest` (Escape, back); `none` leaves closing to your code, for a step that must finish. |
| `size` | `DialogSize` | `'md'` | Maximum width: 360, 480 or 640. |
| `className` | `string` |  | Classes for the panel. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<dialog>` | yes | The dialog primitive: a centred panel in the top layer above a scrim. |
| title | `<text>` | yes | `label` as a level-2 heading, unless `isLabelHidden`. |
| close | `<button>` | no | A close button named by `closeLabel`. |
| description | `<text>` | no | Text under the title. |
| body | `<scroll>` | no | `children`, in a scroll that takes the remaining height. |
| footer | `<view>` | no | `actions`, end-aligned, wrapping when narrow. |

## Accessibility

- Pattern: dialog.
- Name: label.
- Description: description.
- States: modal (from isOpen; announced as an alert dialog with isAlert).
- Escape: onOpenChange(false), unless dismissOn is none.
- Opened from a hardware keyboard, focus moves to the first control and returns to the opener on close; opened by touch, focus stays put.
- Android back and the screen reader dismiss gesture are close requests, like Escape.
- The close button is 32 px; it is below the 44 pt touch minimum until hit slop exists, so always offer an action button too.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.10:1 |
| description | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Title it with the question or the task ("Delete this note?").
- Do: Name the actions by their outcome; put the main one last.
- Do: Keep a draft in state declared outside the dialog.
- Don't: Open a dialog from a dialog; finish or close the first.
- Don't: Put a long scrolling list in a dialog; use a page.
- Don't: Place it inside a Button; overlays cannot sit in a pressable control.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A theme-wide motion token for the enter and exit timing. | duration and easing utilities that read the theme pack's motion tokens | duration-200 ease-out in the recipe. |

Related: Button, DropdownMenu.
