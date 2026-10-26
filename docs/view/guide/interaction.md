# Interaction

```tsx
import { useLocal, type VectorPath } from '@lingxia/ui';

const chevron: VectorPath[] = [{ d: 'M9 6l6 6-6 6', fill: 'none', stroke: 'currentColor', strokeWidth: 2 }];

export function render() {
  const liked = useLocal('liked', false);
  const opened = useLocal('opened', 0);
  return (
    <page className="p-4 gap-3">
      <button onClick={() => opened.set(opened.value + 1)} className="p-3 rounded-xl bg-white text-neutral-900 active:bg-neutral-100">
        <row className="items-center gap-3">
          <image src="images/album.png" alt="" className="w-12 h-12 rounded-lg" />
          <column className="grow basis-0 gap-0.5">
            <text className="text-sm font-semibold">Morning Mix</text>
            <text className="text-xs text-neutral-500">32 songs</text>
          </column>
          <vector accessibilityHidden paths={chevron} className="w-5 h-5 text-neutral-400 rtl:-scale-x-100" />
        </row>
      </button>
      <button onClick={() => liked.set(!liked.value)} accessibilityLabel={liked.value ? 'Unlike' : 'Like'}
        className={liked.value ? 'self-start rounded-full bg-rose-600 text-white' : 'self-start rounded-full bg-neutral-200 text-neutral-900'}>
        {liked.value ? '♥' : '♡'}
      </button>
      <text className="text-xs text-neutral-500">Opened {opened.value} times</text>
    </page>
  );
}
```

## What responds to touch

Only controls receive input: `<button onClick>`, `<link onPress>`, `<checkbox>`, `<switch>`, `<radio>`,
the editors, `<slider>`, and `<plot onSelect>`. Containers, text and images have no `onClick`
(`UI_PROP`). To make a row, card or image tappable, make it the content of a `<button>`.

```tsx error UI_PROP
export function render() {
  return <page><row onClick={() => {}}><text>Not a control</text></row></page>;
}
```

## Composed controls

`<button>` and `<checkbox>` may contain layout, text, images and vectors. A button stacks its content
in a column and cannot change direction; wrap side-by-side content in a `<row>`, as above. The whole control is one target and one accessibility node, named by its
`accessibilityLabel` or else by its descendants' text and image `alt`, in order.

Controls cannot nest. A control, editor, `<scroll>`, `<list>` or `<link>` inside a composed control
fails (`UI_BUTTON`):

```tsx error UI_BUTTON
export function render() {
  return (
    <page>
      <button onClick={() => {}}>
        <text>Item</text>
        <button onClick={() => {}}>Delete</button>
      </button>
    </page>
  );
}
```

Put sibling controls side by side in a `<row>` instead.

## Press states

While a control is pressed, its `active:` styles apply (paint only). A control without `active:` styles
gets the platform's pressed overlay over its shape instead. `disabled` removes the control's handler; its
`disabled:` styles replace its whole class cascade. A click plays the platform's click feedback.

`hover:` applies only when `env.pointer` is `fine` (a mouse or trackpad). Touch devices never hover.

## Gestures

The host recognizes the gestures; the View receives their outcome:

| Gesture | Outcome |
| --- | --- |
| tap | the control's handler (`onClick`, `onPress`, `onChange`, focus for editors) |
| drag, fling | scrolls the innermost `<scroll>`/`<list>` on that axis; drags a `<slider>` |
| long press | selects text where `select-text` is set; editors show their text menu |
| tap on a `<plot>` | `onSelect(index)`, or `-1` when it clears the selection |

There are no pointer, touch, swipe, pinch or long-press handlers on elements, and no drag-and-drop.
Build carousels from a horizontal `<scroll>`, and sheets and dialogs from layout and state.

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
