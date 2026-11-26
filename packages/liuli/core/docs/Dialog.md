<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Dialog

Modal panel over the page for a decision or a short task. Status: preview.

```ts
import { Dialog, AlertDialog } from '@liulidesign/core/Dialog';
```

Dialog interrupts the page to ask for a decision or a short piece of input, and keeps focus and assistive tech inside until it closes. It is controlled: `isOpen` shows it, `onOpenChange(false)` reports every close request. AlertDialog, from the same import, is the two-answer confirmation: it runs its action, loads until it settles and keeps a failure in the dialog.

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
      {deleted.value && <text role="status" data-testid="outcome" className="text-footnote text-muted">Deleted</text>}
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Delete this note?"
        description="It leaves every device you are signed in on."
        size="sm"
        isAlert
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

### AlertDialog: an action that loads, and a failure that stays

```tsx
import { useLocal, useView, type View } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Switch } from '@liulidesign/core/Switch';
import { AlertDialog } from '@liulidesign/core/Dialog';

type Actions = { publish(): Promise<void> };

/** AlertDialog runs the action itself: the button loads until it settles, and a failure stays in the dialog to try again. */
export function Alert() {
  const { actions } = useView() as View<Actions>;
  const open = useLocal('open', false);
  const offline = useLocal('offline', true);
  const deleted = useLocal('deleted', false);
  return (
    <view className="gap-3">
      <Switch label="Fail the request" bind={offline} data-testid="alert-offline" />
      <Button label="Delete note" variant="destructive" width="content" onClick={() => open.set(true)} data-testid="alert-open" />
      {deleted.value && <text role="status" className="text-footnote text-muted">Deleted</text>}
      <AlertDialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Delete this note?"
        description="It leaves every device you are signed in on."
        confirmLabel="Delete"
        isDestructive
        confirmAction={async () => {
          await actions.publish();
          if (offline.value) throw Error('offline');
          deleted.set(true);
        }}
        data-testid="alert"
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

### Full screen

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { TextArea } from '@liulidesign/core/TextArea';
import { TextInput } from '@liulidesign/core/TextInput';
import { Dialog } from '@liulidesign/core/Dialog';

/** Full screen: a task with its own content to scroll, closed from its header. */
export function Full() {
  const open = useLocal('open', false);
  const title = useLocal('title', 'Trip to the coast');
  const body = useLocal('body', '');
  return (
    <view className="gap-2">
      <Button label="New note" variant="primary" width="content" onClick={() => open.set(true)} data-testid="full-open" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="New note"
        size="full"
        dismissOn="closeRequest"
        data-testid="full-dialog"
        actions={
          <>
            <Button label="Discard" onClick={() => open.set(false)} data-testid="full-discard" />
            <Button label="Save note" variant="primary" onClick={() => open.set(false)} data-testid="full-save" />
          </>
        }
      >
        <TextInput bind={title} label="Title" />
        <TextArea bind={body} label="Note" rows={8} maxRows={16} />
      </Dialog>
    </view>
  );
}
```

### With a picture above the title

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Dialog } from '@liulidesign/core/Dialog';

/** A dialog that introduces something: a picture reaches the panel's top edges, with the close button over it. */
export function Hero() {
  const open = useLocal('open', false);
  return (
    <view className="flex-row">
      <Button label="What is new" width="content" onClick={() => open.set(true)} data-testid="hero-open" />
      <Dialog
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Shared notebooks"
        description="Invite people to a notebook and write in it together. Changes show as they are made."
        size="sm"
        media={<image src="public/photos/photo-2.png" alt="" className="h-40 w-full object-cover" />}
        data-testid="hero-dialog"
        actions={
          <>
            <Button label="Not now" onClick={() => open.set(false)} />
            <Button label="Try it" variant="primary" onClick={() => open.set(false)} data-testid="hero-try" />
          </>
        }
      />
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
| `isAlert` | `boolean` | `false` | Interrupts with something to confirm or acknowledge; screen readers announce it as an alert dialog. A backdrop tap does not close it; Escape and back still do, as its cancel. |
| `media` | `Child` |  | A picture or an illustration above the title, edge to edge under the panel's top corners: for a dialog that introduces something. Give it its own height or aspect ratio. The close button, if any, sits over it. |
| `children` | `Child` |  | The body; it scrolls when the panel reaches its maximum height. |
| `actions` | `Child` |  | The footer, typically Buttons, the main action last. |
| `headerAction` | `Child` |  | Beside the title, before the close button: one small control that acts on the whole panel, such as Refresh. |
| `hasCloseButton` | `boolean` |  | Shows a close button in the header. An alert has none unless you ask for one; any other dialog has one unless you turn it off. |
| `closeLabel` | `string` |  | Names the close button. Defaults to "Close" in the page's language. |
| `dismissOn` | `DialogDismissOn` |  | Which requests close it: `any` adds a backdrop tap to `closeRequest` (Escape, back); `none` leaves closing to your code, for a step that must finish. An alert takes `closeRequest` unless you say otherwise; any other dialog takes `any`. |
| `size` | `DialogSize` | `'md'` | Maximum width: 360, 480 or 640. `full` covers the screen, for a task with its own scrolling content: an editor, a long form on a phone. |
| `className` | `string` |  | Classes for the panel. |
| `data-testid` | `string` |  |  |

## AlertDialog props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `isOpen` (required) | `boolean` |  | Whether it is shown. |
| `onOpenChange` (required) | `(isOpen: boolean) => void` |  | Called with `false` on cancel, Escape or back, and after `confirmAction` succeeds. |
| `label` (required) | `string` |  | The question, as the title and the dialog's accessible name ("Delete this note?"). |
| `description` | `string` |  | What happens if the person goes on. |
| `confirmLabel` (required) | `string` |  | Names the action by its outcome ("Delete"). |
| `confirmAction` (required) | `() => Promise<unknown>` |  | Runs the action. The dialog stays open with the button loading until it settles; it closes on success and shows `failedMessage` on failure. |
| `isDestructive` | `boolean` | `false` | Draws the action in the danger colour, for something that cannot be undone. |
| `cancelLabel` | `string` |  | Names the cancel button. Defaults to "Cancel" in the page's language. |
| `failedMessage` | `string` |  | Shown in the dialog when `confirmAction` fails; the person can try again or cancel. Defaults to "That didn't work. Try again." in the page's language. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<dialog>` | yes | The dialog primitive: a centred panel on the overlay surface, in the top layer above a scrim; at `full` it covers the screen on the page's canvas. |
| media | `<view>` | no | `media`: a picture above the title, reaching the panel's top edges; hidden from assistive tech. |
| mediaClose | `<button>` | no | With `media`, the close button sits over the picture on its own dock. |
| title | `<text>` | yes | `label` as a level-2 heading, unless `isLabelHidden`. |
| close | `<button>` | no | A close button: an alert has none unless you ask for one; any other dialog has one unless you turn it off. `closeLabel` names it. |
| description | `<text>` | no | Text under the title. |
| body | `<scroll>` | no | `children`, in a scroll that takes the remaining height. A hairline shows at an edge while content is scrolled under it. |
| headerAction | `<view>` | no | `headerAction`: one small control beside the title that acts on the whole panel, such as Refresh. |
| failed | `<view>` | no | AlertDialog: the failure row, in the body, when `confirmAction` fails. |
| failedIcon | `<vector>` | no | AlertDialog: the danger glyph that marks the failure. |
| failedText | `<text>` | no | AlertDialog: the failure message, in the body colour. |
| footer | `<view>` | no | `actions`: on a phone they share the width equally, the main one last; wider, they keep their own widths at the end, wrapping when narrow. |

## What an app may override

- `className`: Classes for the panel.
- The recipe, `dialogRecipe` from `@liulidesign/core/Dialog/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`, `--text-title`, `--shadow-modal`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `canvas`, `danger`, `focus`, `line`, `media`, `on-media`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: dialog.
- Name: label.
- Description: description.
- States: modal (from isOpen; announced as an alert dialog with isAlert).
- Escape: onOpenChange(false), unless dismissOn is none; an alert takes it as cancel, and an AlertDialog ignores it while its action runs.
- Opened from a hardware keyboard, focus moves to the first control and returns to the opener on close; opened by touch, focus stays put.
- Android back and the screen reader dismiss gesture are close requests, like Escape.
- An alert never closes on a backdrop tap. AlertDialog puts Cancel first, so a keyboard lands on the safe answer, and announces a failure as an alert.
- The close button is 32 px; a finger reaches it in a 44 pt band around it. Offer an action button too, so closing is never the only way on.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| close button over a picture | `on-media` on `media` | 3:1 | 7.94:1 | 7.94:1 |
| description | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |
| AlertDialog failure mark | `danger` on `surface-overlay` | 3:1 | 5.21:1 | 5.43:1 |
| title, full screen | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| description, full screen | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |

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
