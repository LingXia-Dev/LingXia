<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Toast

A short message about something that just happened, over the bottom of the screen. Status: preview.

```ts
import { Toast, useToast, ToastRegion } from '@liulidesign/core/Toast';
```

A toast confirms an action that worked away from where it was pressed, or offers to undo it: a note archived, a link copied. A page holds one `ToastRegion`; `useToast()` shows a toast from anywhere inside it. One shows at a time and the rest wait in order; a toast with the same `id` replaces the one queued, so a running count updates in place. One without an action closes by itself after `duration`; one with an action stays until it is closed. A busy one, for work still under way, turns a spinner and stays until its result replaces it.

## Examples

### Archive with Undo

```tsx
import { ToastRegion } from '@liulidesign/core/Toast';

/** Archiving a note, with Undo: the toast stays until Undo, dismiss or a swipe closes it. */
export function Archive() {
  return (
    <ToastRegion>
      <Notes />
    </ToastRegion>
  );
}
```

### Tones, queued

```tsx
import { ToastRegion } from '@liulidesign/core/Toast';

/** One per tone; pressed in a row, they queue and show one at a time. */
export function Tones() {
  return (
    <ToastRegion>
      <ToneButtons />
    </ToastRegion>
  );
}
```

### Updated in place

```tsx
import { ToastRegion } from '@liulidesign/core/Toast';

/** A progress message updated in place by its id, rather than queued behind itself: busy, so it stays, until the last update says it is done. */
export function Progress() {
  return (
    <ToastRegion>
      <Uploader />
    </ToastRegion>
  );
}
```

### Each face, at phone width

```tsx
import { Said } from '../example';
import { Toast } from '@liulidesign/core/Toast';

/**
 * Each face at the width a toast takes on a phone: a message, success, busy, with Undo, and danger
 * with Retry. 32 px apart and below, the raised shadow's reach (8 px down plus a 24 px blur), so each
 * shadow ends before the next card and the stage's edge.
 */
export function Faces() {
  return (
    <Said>
      {say => (
        <view className="gap-8 pb-8">
          <Toast message="Note moved to Travel" onDismiss={() => say('Dismissed')} />
          <Toast message="Changes saved" tone="success" onDismiss={() => say('Dismissed')} />
          <Toast message="Exporting 24 notes" isBusy onDismiss={() => say('Dismissed')} />
          <Toast message={'"Q3 plan" archived'} action={{ label: 'Undo', onClick: () => say('Undid the archive') }} onDismiss={() => say('Dismissed')} />
          <Toast message="Could not save the note" tone="danger" action={{ label: 'Retry', onClick: () => say('Retried') }} onDismiss={() => say('Dismissed')} />
        </view>
      )}
    </Said>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `message` (required) | `string` |  |  |
| `tone` | `BannerTone` | `'neutral'` |  |
| `action` | `ToastAction` |  |  |
| `isBusy` | `boolean` | `false` | A spinner in the tone's glyph's place, and the toast reported busy. |
| `onDismiss` | `(() => void)` |  | Shows a dismiss button. |
| `dismissLabel` | `string` |  | The dismiss button's name, with `onDismiss`. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the toast; the action gets `<testid>-action`, the dismiss button `<testid>-dismiss`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| region | `<view>` | yes | The top-layer box the toast sits in, clear of the system bars and the keyboard. |
| toast | `<view>` | yes | The raised card; a sideways swipe closes it. |
| icon | `<vector>` | no | The tone's glyph, hidden from assistive tech; neutral toasts have none. |
| spinner | `<view>` | no | While busy, in the glyph's place, hidden from assistive tech; still under reduced motion. |
| stack | `<view>` | no | At accessibility text sizes, the message over its action, so the message keeps the card's width. |
| message | `<text>` | yes | What happened, in a few words. |
| action | `<Button>` | no | One link button that follows up, such as Undo; the toast closes after it. |
| dismiss | `<button>` | yes | A close button named by `dismissLabel`. |
| announcer | `<text>` | yes | Hidden live texts, polite and assertive pairs, mounted with the region; each new toast takes the one of its pair cleared last, so a repeated message is spoken again. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `toastRecipe` from `@liulidesign/core/Toast/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-body`, `--shadow-raised`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `danger`, `focus`, `info`, `line`, `success`, `surface-overlay`, `text`, `text-muted`, `warning`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- States: busy (from isBusy).
- Tab: Reaches the action and the dismiss button after the page..
- Escape: Nothing on the toast; it stays with what has focus..
- Press target: md 36 px.
- Each new message is announced politely; a danger toast is announced assertively.
- The announcement comes from live text that lives as long as the region, as a region inserted with its message is not announced.
- The tone is in the glyph and the words, never in colour alone.
- A busy toast is reported busy and never closes by itself; the toast that replaces it is announced like any other.
- A toast without an action closes after `duration`; the count pauses while it is pressed, under a resting pointer or holding focus. A toast with an action stays until closed.
- A toast is not modal: Escape and back go to whatever has focus, never to the toast. The dismiss button is its keyboard path.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| message | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |
| dismiss icon | `text-muted` on `surface-overlay` | 3:1 | 5.89:1 | 4.84:1 |
| action | `accent` on `surface-overlay` | 4.5:1 | 5.62:1 | 4.51:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say what happened in a few words: "Note archived", not "Success".
- Do: Offer Undo for an action that is easy to regret.
- Do: Give a running message an `id`, so each update replaces it; while the work runs make it busy, and end with the result.
- Do: Show a failure with `toast.show(e)` in its `catch`: it reads as the `errorText` words, `danger`, and a cancel shows nothing.
- Do: Where a page keeps a control or bar at the bottom end, move the toasts with `position` or keep that edge clear with `inset`; they show in the bottom end corner, up to 400 pt wide.
- Don't: Put something people must read or act on in a toast; use a Banner or a Dialog.
- Don't: Show a toast for the result people can already see where they pressed.
- Don't: Use a toast for an error that stops work; show it where the work is.

Related: Banner, Dialog.
