# Motion

Transitions and keyframe animations run in the renderer. The View declares them with utilities;
no JavaScript runs per frame.

```tsx
import { useLocal } from '@lingxia/ui';

export function render() {
  const shown = useLocal('shown', true);
  return (
    <page className="p-4 gap-4">
      <button onClick={() => shown.set(!shown.value)}
        className="self-start rounded-xl bg-accent text-on-accent active:scale-95 transition-transform duration-150">
        {shown.value ? 'Fade out' : 'Fade in'}
      </button>
      <view className={`w-12 h-12 rounded-lg bg-accent transition-opacity duration-200 ease-out ${shown.value ? 'opacity-100' : 'opacity-0'}`} />
      <view className="w-10 h-10 rounded-lg bg-danger motion-safe:animate-pulse motion-reduce:opacity-60" />
    </page>
  );
}
```

## Transitions

`transition`, `transition-colors`, `transition-opacity`, `transition-transform`, `transition-shadow` and
`transition-all` animate changes of the painted properties: opacity, transform, background, text color,
border color and shadow. `duration-*`, `delay-*` and `ease-*` (or `ease-[cubic-bezier(…)]`) set the
timing, up to 10 s each; the defaults are 150 ms and `cubic-bezier(0.4, 0, 0.2, 1)`. A transition runs when a re-render or
a state variant (`active:`, `hover:`, `focus:`) changes one of those properties. A change mid-flight
continues from the value on screen.

Some changes land at once instead of animating:

- an element seen for the first time, unless it has `starting:` values (see below);
- a transform of a subtree that holds an editor, a `<scroll>` or a `<slider>`;
- a gradient background;
- a shadow on an element with unequal corner radii;
- a transform to `scale-0`, which paints nothing until it lands.

Layout does not animate. A transition of width, height, padding, margin, gap, insets or font size is
refused, naming `transition-transform` or `transition-opacity` as the alternative:

```tsx error UI_CLASS_UNSUPPORTED
export function render() {
  return <page><view className="h-10 bg-accent transition-[width]" /></page>;
}
```

Animate a scale or translate instead of a size, or let `transition-layout` move the box (below).

## Enter and exit

`starting:` sets the values an element enters from the first time it renders; `exit:` sets the values it
leaves toward when a re-render removes it. Both take paint properties only and animate with the element's
own transition:

```tsx
import { useLocal } from '@lingxia/ui';

export function render() {
  const open = useLocal('open', false);
  return (
    <page className="p-4 gap-4">
      <button onClick={() => open.set(!open.value)} className="self-start rounded-xl bg-accent text-on-accent px-4 py-2">Toggle</button>
      {open.value && (
        <view className="h-16 rounded-xl bg-surface-raised transition duration-200 starting:opacity-0 starting:translate-y-2 exit:opacity-0 exit:-translate-y-2" />
      )}
    </page>
  );
}
```

A removed element keeps drawing where it was until its exit ends, clipped and scrolled with its old parent,
but it takes no input and is hidden from accessibility; the layout closes the gap at once. Only the
outermost removed element plays its exit, and at most 64 play at once per page; past that, removed elements
vanish. An element that comes back mid-exit enters from where its exit had got to. A hidden page, and
reduced motion, add and remove elements at once.

## Layout motion

`transition-layout` slides an element from where layout last put it to its new position, with the
element's `duration-*`, `delay-*` and `ease-*`; it adds to the properties `transition-*` sets
(`transition-opacity transition-layout`). Use it on keyed rows so a reorder, an insertion or a removal
moves the others instead of jumping:

```tsx
import { useLocal } from '@lingxia/ui';

export function render() {
  const rows = useLocal('rows', ['a', 'b', 'c']);
  return (
    <page className="p-4 gap-2">
      <button onClick={() => rows.set([...rows.value].reverse())} className="self-start rounded-xl bg-accent text-on-accent px-4 py-2">Reverse</button>
      {rows.value.map(row => <view key={row} className="h-10 rounded-lg bg-surface-raised transition-layout duration-200" />)}
    </page>
  );
}
```

Only position moves: a size change lands at once. Input follows the box where it is drawn. A box whose
subtree holds an editor, a `<scroll>` or a `<slider>` moves at once, as do more than 256 at a time.

## Animations

`animate-spin`, `animate-ping`, `animate-pulse`, `animate-bounce` and your own `--animate-*` tokens with
`@keyframes` in the stylesheet run keyframe animations of opacity, transforms (translate, including
`%`, rotate, scale) and `background-position` (percentages, with `from` and `to`), which sweeps a sized
gradient across its box: `background: linear-gradient(…) 0 0 / 250% 100%` with `bg-clip-text` makes a shimmer. Keyframes may omit their endpoints, and each keyframe may set its own easing.
`animate-none` removes an animation. Fill modes work as in CSS: `forwards` keeps the last keyframe after
the run, `backwards` shows the first during the delay, `both` does both. An element runs one animation; several comma-separated
animations and `alternate`/`reverse` directions are refused, as are keyframes of any other
property (`UI_CLASS_UNSUPPORTED`). An animation starts when its element first appears on screen; an
element scrolled out of a list and back does not restart it. A keyframe value wins over a transition of
the same property.

Hidden pages pause: nothing renders while a page is not on screen, and time resumes where it stopped.

## Reduced motion

`motion-reduce:` and `motion-safe:` follow `env.reducedMotion`, the system's reduce-motion setting.
With reduced motion on, transitions land at once and an animation shows its element's base style,
unless it was authored under `motion-safe:` or `motion-reduce:`, which keeps it playing when its
variant applies. The system animation speed scales every duration and delay; 0 reads as reduced motion.

## Not supported

Size transitions, shared-element transitions between pages, 3D transforms, `transition-behavior`, scroll-driven
animations, View-side timers (`setTimeout`, `requestAnimationFrame`) and per-frame JavaScript
animation. An inline `style.animation` fails with `UI_ANIMATION`; use `animate-*`.
