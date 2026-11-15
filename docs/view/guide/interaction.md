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
        <view className="flex-row items-center gap-3">
          <image src="images/album.png" alt="" className="w-12 h-12 rounded-lg" />
          <view className="grow basis-0 gap-0.5">
            <text className="text-sm font-semibold">Morning Mix</text>
            <text className="text-xs text-muted">32 songs</text>
          </view>
          <vector aria-hidden paths={chevron} className="w-5 h-5 text-faint rtl:-scale-x-100" />
        </view>
      </button>
      <button onClick={() => liked.set(!liked.value)} aria-label={liked.value ? 'Unlike' : 'Like'}
        className={liked.value ? 'self-start rounded-full bg-danger p-3 text-on-danger' : 'self-start rounded-full bg-line p-3 text-text'}>
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
  return <page><view onClick={() => {}}><text>Not a control</text></view></page>;
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
        <view className="flex-row items-center gap-3">
          <text className="grow">Wi-Fi</text>
          <switch bind={on} aria-label="Wi-Fi" />
        </view>
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

## Where the pointer is

`onPointer={event => …}` on a `view`, `button` or `vector` reports a pointer over the element, for
scrubbing a chart or placing a tooltip. `event.x` and `event.y` are points from the top-left of the
element's content box (inside its padding and border), not clamped to it and not mirrored in a
right-to-left layout; `event.width` and `event.height` are that box, so `x / width` is a fraction.

| `phase` | When |
| --- | --- |
| `down` | a finger or mouse button goes down on the element, at once |
| `move` | it moves; a page slower than the pointer hears the latest position, never a backlog |
| `up` | it lifts |
| `cancel` | a scroll or a drag element took the gesture, or the page lost it |
| `hover`, `leave` | a mouse moves over the element with no button down, then leaves it |

The handler observes: a button inside still gets its click, and the element still hears that press.
`pointerAxis="x"` keeps horizontal movement and leaves vertical movement to the page's scroll, as
`touch-action: pan-y` does; `"y"` is the reverse and `"both"` (the default) keeps the whole gesture.
On one axis, `down` comes at once and moves wait until the direction is clear: then come the moves,
or one `cancel`, never moves and then `cancel`.
Give the element `aria-valuenow` (with `aria-valuemin`, `aria-valuemax` and an `aria-valuetext` that
reads the point, such as "March: 42") and it becomes a keyboard stop a screen reader can adjust:
Left and Right (Up and Down with `pointerAxis="y"`) and the reader's increment and decrement call
the handler with `phase` `increment` or `decrement`, `pointer: "keyboard"` and no position. Move
your selection there, as a pointer's `x` does.

In a Node test, `screen.pointer(node, { phase, x, y, width, height })` delivers an event; a device
spec drives one with `t.app.view.scrub(locator, { from, to })`, whose points are from the
element's bounding box (the same point when the element has no padding or border).

## Gestures

The host recognizes the gestures; the View receives their outcome:

| Gesture | Outcome |
| --- | --- |
| tap | the control's handler (`onClick`, `onChange`, focus for editors) |
| drag, fling | moves a `drag` element on its axis; else scrolls the innermost `<scroll>`/`<List>` on that axis; drags a `<slider>` |
| long press | selects text where `select-text` is set; editors show their text menu |
| tap on a `<plot>` | `onSelect(index)`, or `-1` when it clears the selection |

There are no pointer, touch, swipe, pinch or long-press handlers on elements, and no drag-and-drop.
Build carousels from a horizontal `<scroll>`. A pager is one with `snap-x snap-always` and `snap-start`
pages: a flick stops on the next page however fast, and `bind={page}` (a `useLocal` number) is the
page it rests on, two-way: setting it springs there. Assistive scrolling steps one page.
`<Pager aria-label="…">` from `@lingxia/view` builds that for you: its children are the pages, a screen
reader reads them as a named list ("2 of 3"), and dots under it follow the page and jump on tap.

## Dragging an element

`drag="x"` or `drag="y"` lets the finger move a `<view>` or `<button>` along one axis, its whole subtree with it, between its `dragSnapPoints`: up to 8 offsets from
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
- The release springs with the element's `transition-transform` timing; bare `duration-*`/`ease-*`
  time the release alone and, as in Tailwind, animate no class change. Without either: 300 ms, no bounce.
- Swipe rows: a `<button drag="x" dragSnapPoints={[0, -144, '-100%']} aria-actions="archive-1 delete-1">`
  over a row of `<button id="archive-1">`… behind it. While the row rests closed, those controls leave
  screen-reader navigation and the row offers them as its own actions, named by their text; specs reach
  them with `locator.performAction('Delete')` (also `increment`, `decrement`, and `dismiss` on a dialog).
- With a scroll inside on the same axis, a drag from inside it moves toward the element's lowest
  offset (opening a sheet) before the scroll scrolls, and back toward the scroll's start before the
  element closes, all in one gesture; the release flicks whichever moved last. `overscroll-contain`
  on the inner scroll keeps the gesture to the scroll. Distance past the element's points passes to a
  scroll around it on the same axis; a finger across the axis scrolls as if the element were not there.
- It needs `aria-label` (a `<button>` is named by its content). Screen readers announce it as an
  adjustable whose value is its point, 1 to the number of points; increment and decrement step it.
  Focused, the arrow keys on its axis, Home and End do the same. Automation reads the point as the
  locator's value (`toHaveValue('2')`) and `drag: { axis, index, count }` in the accessibility tree.

## Dialogs, popovers and menus

See [Overlays](overlays.md): where they draw against the host bars, closing, sheets, focus.

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
is one stop: its selected option, or its first enabled one. Inside a `role="menu"`, the up and down
arrows move between its items and wrap, Home and End go to the first and last, and a typed letter
moves to the next item whose name starts with it. Specs check focus with `toBeFocused()`.

## Feedback from Logic

On desktop, `lx.showToast`, `lx.hideToast`, `lx.showModal` and `lx.showActionSheet` draw above the page
in the View itself. The toast is a polite live region and a dark HUD in every theme, as on the phones;
the modal and the action sheet take the theme's roles and are dialogs that hold keyboard focus, and each answers Logic as a WebView page would (the modal
with `confirm`/`cancel`, the sheet with `tapIndex`, `-1` for cancel). Button colours follow the roles
(`accent`, `text-muted`), not `confirmColor`/`cancelColor`. Any other call Logic makes into the View is
refused at once with `E_UNSUPPORTED`. On phones these APIs stay native.
