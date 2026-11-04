# Interaction

```tsx
import { useLocal, type VectorPath } from '@lingxia/view';

const chevron: VectorPath[] = [{ d: 'M9 6l6 6-6 6', fill: 'none', stroke: 'currentColor', strokeWidth: 2 }];

export function render() {
  const liked = useLocal('liked', false);
  const opened = useLocal('opened', 0);
  return (
    <page className="p-4 gap-3">
      <button onClick={() => opened.set(opened.value + 1)} className="p-3 rounded-xl bg-surface text-text active:bg-surface-sunken">
        <row className="items-center gap-3">
          <image src="images/album.png" alt="" className="w-12 h-12 rounded-lg" />
          <column className="grow basis-0 gap-0.5">
            <text className="text-sm font-semibold">Morning Mix</text>
            <text className="text-xs text-muted">32 songs</text>
          </column>
          <vector aria-hidden paths={chevron} className="w-5 h-5 text-faint rtl:-scale-x-100" />
        </row>
      </button>
      <button onClick={() => liked.set(!liked.value)} aria-label={liked.value ? 'Unlike' : 'Like'}
        className={liked.value ? 'self-start rounded-full bg-danger p-3 text-on-accent' : 'self-start rounded-full bg-line p-3 text-text'}>
        {liked.value ? '♥' : '♡'}
      </button>
      <text className="text-xs text-muted">Opened {opened.value} times</text>
    </page>
  );
}
```

## What responds to touch

Only controls receive input: `<button onClick>`, `<link onClick>`, `<checkbox>`, `<switch>`, `<radio>`,
the editors, `<slider>`, and `<plot onSelect>`. Containers, text and images have no `onClick`
(`UI_PROP`). To make a row, card or image tappable, make it the content of a `<button>`.

```tsx error UI_PROP
export function render() {
  return <page><row onClick={() => {}}><text>Not a control</text></row></page>;
}
```

## Composed controls

`<button>` and `<checkbox>` may contain layout, text, images and vectors. A button stacks its content
in a column unless its classes say otherwise (`flex-row`, `grid`). The control
is named by its `aria-label` or else by its descendants' text and image `alt`, in order.

A composed control may also hold controls of its own: a row that opens a page can carry a switch and a
delete button. A press goes to the innermost control under the finger, and only that control shows it
pressed; the row activates when the press lands on its other content. Assistive technology sees the row
and each inner control as separate nodes, and the row's name leaves the inner controls' text out.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const on = useLocal('on', true);
  return (
    <page>
      <button onClick={() => {}} className="rounded-xl bg-surface px-4 py-3">
        <row className="items-center gap-3">
          <text className="grow">Wi-Fi</text>
          <switch bind={on} aria-label="Wi-Fi" />
        </row>
      </button>
    </page>
  );
}
```

A `<scroll>` or `<List>` inside a composed control fails (`UI_BUTTON`): a drag in it would both scroll
and press. So does a control whose only content is another control; put the action on that control:

```tsx error UI_BUTTON
export function render() {
  return (
    <page>
      <button onClick={() => {}}>
        <button onClick={() => {}}>Delete</button>
      </button>
    </page>
  );
}
```

## Press states

While a control is pressed, its `active:` styles apply (paint only, transforms included, so
`active:scale-95` works). A control without `active:` styles
gets the platform's pressed overlay over its shape instead. `disabled` removes the control's handler; its
`disabled:` styles replace its whole class cascade. A click plays the platform's click feedback.

A press belongs to its control, not to the frame it started on. If the View re-renders under the
finger (a toggle's label flips), releasing still runs that control's current handler; only a control
that went away cancels the press.

`hover:` applies only when `env.pointer` is `fine` (a mouse or trackpad). Touch devices never hover.

## Gestures

The host recognizes the gestures; the View receives their outcome:

| Gesture | Outcome |
| --- | --- |
| tap | the control's handler (`onClick`, `onChange`, focus for editors) |
| drag, fling | moves a `drag` element on its axis; else scrolls the innermost `<scroll>`/`<List>` on that axis; drags a `<slider>` |
| long press | selects text where `select-text` is set; editors show their text menu |
| tap on a `<plot>` | `onSelect(index)`, or `-1` when it clears the selection |

There are no pointer, touch, swipe, pinch or long-press handlers on elements, and no drag-and-drop.
Build carousels from a horizontal `<scroll>`.

## Dragging an element

`drag="x"` or `drag="y"` lets the finger move a `<view>`, `<row>`, `<column>`, `<stack>`, `<grid>` or
`<button>` along one axis, its whole subtree with it, between its `dragSnapPoints`: up to 8 offsets from
where layout put it, in px or `%` of its own size (`x` offsets mirror in RTL, like `start`/`end`). The
points are also its bounds: past the first or last it follows only part of the finger. On release it
springs to the nearest point, carried on by the finger's speed; a flick moves at least one point on.
The spring takes the element's transition timing (`transition-transform duration-500 ease-spring-bouncy`);
without one it is 300 ms with no bounce.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const at = useLocal('at', 0);
  const heading = useLocal('heading', -1);
  return (
    <page className="p-4 gap-3">
      <view drag="x" dragSnapPoints={[0, 120, 240]} bind={at} aria-label="Card"
        onDragStart={() => heading.set(-1)} onDragEnd={({ index }) => heading.set(index)}
        className="w-20 h-16 rounded-lg bg-accent transition-transform duration-300 ease-spring" />
      <text className="text-sm text-muted">{`at ${at.value}, heading for ${heading.value}`}</text>
      <button onClick={() => at.set(0)}>Back to start</button>
    </page>
  );
}
```

- `bind` takes a `useLocal` number: the index of the point the element rests on. It changes when the
  element lands, not while it moves; setting it springs the element there. While a finger holds the
  element, a value set from code waits for the release, and the finger's point wins.
- `onDragStart()` runs once the finger has moved past slop on the element's axis (a press inside it is
  cancelled then); `onDragEnd({ index, velocity })` on release, with the point it heads for and px/s.
- A finger on a moving element catches it where it is.
- It needs `aria-label` (a `<button>` is named by its content). Screen readers announce it as an
  adjustable whose value is its point, 1 to the number of points; increment and decrement step it.
  Focused, the arrow keys on its axis, Home and End do the same. Automation reads the point as the
  locator's value (`toHaveValue('2')`) and `drag: { axis, index, count }` in the accessibility tree.

## Dialogs, popovers and menus

`<dialog>` and the `popover` attribute work as in HTML. Open ones draw in the top layer: above the
whole page (under the system bars, above the keyboard) whatever their place in the tree, unclipped
by their ancestors. On phones an open dialog also covers the host's navigation and tab bars, which
take no input meanwhile; pad its content by `env.safeArea` (the system bars alone), not `env.insets`.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const open = useLocal('open', false);
  return (
    <page className="p-4 gap-3">
      <button popoverTarget="more" aria-label="More" className="self-end rounded-full p-2 bg-surface">⋯</button>
      <view id="more" popover role="menu" aria-label="Actions" className="rounded-xl bg-surface-raised p-1 shadow-lg">
        <button role="menuitem" onClick={() => open.set(true)} className="px-3 py-2 rounded-lg active:bg-surface-sunken">Delete…</button>
      </view>
      <dialog open={open.value} onClose={() => open.set(false)} closedby="any" aria-label="Delete note?"
        className="mt-auto mb-0 w-full rounded-t-2xl bg-surface-raised p-4 gap-3 backdrop:bg-scrim transition-transform duration-300 starting:translate-y-full exit:translate-y-full">
        <text className="text-lg font-semibold">Delete note?</text>
        <button onClick={() => open.set(false)} className="rounded-xl bg-danger p-3 text-on-danger">Delete</button>
      </dialog>
    </page>
  );
}
```

- `<dialog open onClose closedby>` is always modal: a backdrop (`backdrop:` styles it, `bg-scrim` by
  default) takes taps, focus stays inside, and assistive tech reads only the dialog. `closedby`:
  `closerequest` (default) closes on Escape or Android back, `any` also on a backdrop tap, `none` only from code.
  It needs `aria-label`. A dialog is centred by auto margins; `mt-auto mb-0 w-full` makes a sheet.
- `popover` (`auto`) or `popover="manual"` on a `<view>`, `<row>`, `<column>`, `<stack>` or `<grid>`
  with an `id`; `<button popoverTarget="id">` toggles it and gets `aria-expanded`. An auto popover
  closes on a tap outside (the tap goes nowhere else), on Escape or Android back, and when another auto popover opens.
  Opened by its invoker it sits below it, flipping above when needed; `positionArea` (`top`,
  `bottom span-inline-start`, …) picks the side. `open` and `onToggle(open)` control it from state.
- `role="menu"` names a popover of actions; a `<button role="menuitem">` closes its popover after `onClick`.
- At most 16 are open at once; another stays closed with a `UI_BUDGET` warning. A closed one renders
  nothing, so local state inside it resets.

## Hit testing

A tap goes to the topmost control under the finger, in paint order (`z-*` and `order` included).
Non-interactive boxes do not block it: a decoration drawn over a button lets taps through to the button.
A point outside an ancestor's `overflow-hidden` clip hits nothing in that subtree. Transformed controls
hit-test their transformed shape; a `<link>` responds only on its own line pieces. `opacity-0` content is
still hit; use `hidden` to remove a control.

A tap on a control that changed between frames still reaches it when only its handler changed; if its
geometry or content changed, the tap is dropped rather than delivered to the wrong target.

## Keyboard

A hardware keyboard moves focus between controls in tree order and draws a focus ring. A radio group
is one stop: its selected option, or its first enabled one.

## Feedback from Logic

On desktop, `lx.showToast`, `lx.hideToast`, `lx.showModal` and `lx.showActionSheet` draw above the page
in the View itself. The toast is a polite live region and a dark HUD in every theme, as on the phones;
the modal and the action sheet take the theme's roles and are dialogs that hold keyboard focus, and each answers Logic as a WebView page would (the modal
with `confirm`/`cancel`, the sheet with `tapIndex`, `-1` for cancel). Button colours follow the roles
(`accent`, `text-muted`), not `confirmColor`/`cancelColor`. Any other call Logic makes into the View is
refused at once with `E_UNSUPPORTED`. On phones these APIs stay native.
