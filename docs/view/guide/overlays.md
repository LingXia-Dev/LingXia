# Overlays

`<dialog>` and the `popover` attribute work as in HTML. An open one draws in the top layer: above the
whole page whatever its place in the tree, unclipped by its ancestors, under the system bars and above
the keyboard.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const open = useLocal('open', false);
  return (
    <page className="p-4 gap-3">
      <button popoverTarget="more" aria-label="More" className="self-end rounded-full p-2 bg-surface">⋯</button>
      <view id="more" popover role="menu" aria-label="Actions" className="rounded-xl bg-surface-overlay p-1 shadow-lg">
        <button role="menuitem" onClick={() => open.set(true)} className="px-3 py-2 rounded-lg active:bg-surface-sunken">Delete…</button>
      </view>
      <dialog open={open.value} onClose={() => open.set(false)} closedby="any" aria-label="Delete note?"
        className="mt-auto mb-0 w-full rounded-t-2xl bg-surface-overlay p-4 gap-3 backdrop:bg-scrim transition-transform duration-300 starting:translate-y-full exit:translate-y-full">
        <text className="text-lg font-semibold">Delete note?</text>
        <button onClick={() => open.set(false)} className="rounded-xl bg-danger p-3 text-on-danger">Delete</button>
      </dialog>
    </page>
  );
}
```

## Above or below the bars

- **A dialog covers the host's navigation and tab bars** on phones while it is open or playing its exit.
  The bars take no input and assistive tech skips them meanwhile. Pad the dialog's content by
  `env.safeArea` (the system bars alone), not `env.insets`, or it leaves a gap where the bars were.
- **A popover stays under the bars.** One that opens near the tab bar is drawn beneath it: open it
  upward with `style={{ positionArea: 'top' }}` (or `top span-inline-start`, …) or use a sheet (a dialog) there.
- **Beside the anchor.** `positionArea: 'inline-end'` (or `inline-start`) puts an anchored popover at the anchor's
  side, centred on it; add `span-block-end` or `span-block-start` to line up its top or bottom with the
  anchor's (a submenu). With no room it flips to the other side, and with room on neither it opens below
  or above the anchor rather than over it.
- Opening either never shows or hides the tab bar.

## Opening and closing

- **A dialog is controlled by `open`.** A user's request to close it (Escape, Android back, a backdrop
  tap, a sheet dragged off screen) calls `onClose`; it stays open until you set `open` to false there.
  `closedby` picks the requests it takes: `closerequest` (default) Escape and back, `any` also a
  backdrop tap, `none` none, so only your code closes it.
- **A popover** needs an `id`, and `<button popoverTarget="id">` toggles it (a `<link>` inside text takes it
  too). `popover` (auto) closes on a tap outside (that tap reaches nothing else), on Escape or back, and when another auto popover opens;
  `popover="manual"` closes only from its button or code. Pass `open` and `onToggle(open)` to drive it
  from state instead.
- `anchor="<element id>"` places a popover against that element, as against the button that opens one, even when it opens from `open`. With `popover="manual"` it has no backdrop, so a field under it keeps its taps and its focus: a list of suggestions under an editor. It closes if its anchor unmounts.
- A popover closes, with `onToggle(false)`, when the button that opened it unmounts; one opened from
  state stays. A date, time or select picker closes with its field the same way.
- Escape and back close only the topmost dialog or popover.
- `onCloseRequest` on a popover is called when Escape, back or the screen reader's dismiss asks it
  to close, before it does. Return `false` to keep it open, for a draft the reader has not saved. A
  tap outside still closes it, and `popover="manual"` takes no close request, so it cannot have one.
- **A closed one renders nothing**, so `useLocal` state inside it starts over at the next open. Keep what
  must survive (a draft, a scroll position) in state declared outside it.
- `starting:` and `exit:` styles animate the open and close, as in [Motion](motion.md#enter-and-exit).
  A dialog's backdrop is styled with `backdrop:` (`bg-scrim` by default). It fades in and out when
  the dialog has `starting:`/`exit:` styles, over the dialog's own `duration-*` and `ease-*` (200 ms
  otherwise), so it stays until the panel's exit ends; without them both come and go at once. `backdrop:starting:`, `backdrop:exit:` and `backdrop:duration-*`/`ease-*`
  change how it enters and leaves.
- At most 16 are open at once ([limits](../reference/limits.md)); another stays closed with a
  `UI_BUDGET` warning and calls no `onClose` or `onToggle`.

## Floats and windows

Use `<dialog>` and popovers for anything that belongs to the page. `lx.surface.openPage(page, { as:
'float' })` opens another page as a surface of its own, outside the page that opened it: keep it for
content that must stay on screen across pages, like a mini player. A float or window always sits
above the page and its top layer, so a page's dialog never covers it.

## Sheets

A dialog is centred by auto margins; `mt-auto mb-0 w-full` sits it on the bottom edge as a sheet. Add
`drag="y"`, `dragSnapPoints` and `bind` for detents ([Dragging an element](interaction.md#dragging-an-element)): a sheet that
comes to rest wholly off screen calls `onClose` (unless `closedby="none"`), and its backdrop fades with
how much of it shows. Its `exit:` styles have nothing left to show by then.

## Focus and assistive tech

- A dialog is modal: taps outside reach only its backdrop, Tab stays inside, and assistive tech reads
  only the dialog. It needs `aria-label`.
- With a hardware keyboard, opening a dialog moves focus to its first control and closing it returns
  focus to what had it before. Opened by touch, it leaves focus and the focus ring alone.
- The `popoverTarget` button or link reports `aria-expanded`.
- `role="menu"` names a popover of actions; a `<button role="menuitem">` closes its popover after its
  `onClick`. Inside a menu the arrow keys, Home, End and a typed letter move between items
  ([Keyboard](interaction.md#keyboard)).
- `keepOpen` on a menu item runs its `onClick` and leaves the menu open: a zoom step, or a setting
  the reader may change twice.
- A menu item with `popoverTarget` opens a submenu and keeps its menu open. A choice in the submenu
  closes every menu above it, Escape closes the innermost, and Right opens and Left closes the
  submenu (swapped in a right-to-left page). A submenu has no backdrop of its own: a press on
  another item of its menu reaches that item.
- Specs find a dialog with `getByRole('dialog', { name })` and close it as a user would with
  `locator.performAction('dismiss')`. A screen reader's dismiss gesture is offered on every dialog and
  popover that Escape can close, whatever its role, and asks `onCloseRequest` first.
