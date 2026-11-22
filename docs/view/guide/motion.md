# Motion

Transitions and keyframe animations run in the renderer. The View declares them with utilities;
no JavaScript runs per frame.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const shown = useLocal('shown', true);
  return (
    <page className="p-4 gap-4">
      <button onClick={() => shown.set(!shown.value)}
        className="self-start rounded-xl bg-accent-solid p-3 text-on-accent-solid active:scale-95 transition-transform duration-150">
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

`ease-spring*` times a transition by a spring instead of a curve ([Springs](#springs)).

The theme pack names the timings a design system should use instead of milliseconds: `duration-fast`,
`duration-base` and `duration-slow` (150, 200 and 300 ms in the default pack), `ease-standard` for a
change in place, `ease-enter` for what arrives and `ease-exit` for what leaves. They come from the pack's
`motion` block through `lingxia gen tokens`, so a pack changes them for every component at once. The
pack's `spring` sets the bounce of `ease-spring`.

Some changes land at once instead of animating:

- an element seen for the first time, unless it has `starting:` values (see below);
- a gradient background;
- a shadow on an element with unequal corner radii;
- a transform to `scale-0`, which paints nothing until it lands.

An editor, a `<scroll>` or a `<slider>` cannot be scaled or rotated, nor can a box that holds one,
at rest, in a state or by a keyframe animation; the build names the class and the element it holds.
Translate it, or transform a sibling:

```tsx error UI_TRANSFORM
export function render() {
  return <page><view className="hover:scale-105"><input aria-label="Name" /></view></page>;
}
```

Entering or leaving (`starting:`, `exit:`), such a box moves only by its translation; the build
warns.

A `<webview>` follows the same rule. It moves live with a translation; while its box fades, is
masked or blurred, or scales or rotates as it enters or leaves, it shows a still picture of the
page and takes no input, then the live page returns when the effect ends. The same holds under any
scale or rotation at run time (a computed `style`, a `hover:` or `active:` variant, a keyframe): the
build refuses only the ones it can see in class names.

Layout does not animate per frame. A transition of padding, margin, gap, insets or font size is
refused, naming the alternatives:

```tsx error UI_CLASS_UNSUPPORTED
export function render() {
  return <page><view className="h-10 bg-accent transition-[padding]" /></page>;
}
```

A box's size and position do animate, as paint over the finished layout: `transition-size` and
`transition-layout` (below).

## Springs

`ease-spring` (no bounce), `ease-spring-bouncy` (bounce 0.25) and `ease-[spring(0.4)]` (bounce 0–0.9)
time a transition by a spring:

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const on = useLocal('on', false);
  return (
    <page className="p-4">
      <button aria-label="Expand" onClick={() => on.set(!on.value)} className="w-40 rounded-full bg-surface-sunken p-1">
        <view className={`w-8 h-8 rounded-full bg-accent transition-transform duration-500 ease-spring-bouncy ${on.value ? 'translate-x-30' : 'translate-x-0'}`} />
      </button>
    </page>
  );
}
```

- `duration-*` is how long the spring feels, not a hard stop: it settles when it settles.
- A spring retargeted mid-flight keeps its speed as well as its position, so a second tap turns it
  around smoothly instead of stopping it dead.
- Transforms may overshoot their target; opacity and colours stop at their ends.
- Keyframe animations take no spring.
- A drag element springs to its snap point with the same timing
  ([Dragging an element](interaction.md#dragging-an-element)).

## Enter and exit

`starting:` sets the values an element enters from the first time it renders; `exit:` sets the values it
leaves toward when a re-render removes it. Both take paint properties only and animate with the element's
own transition:

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const open = useLocal('open', false);
  return (
    <page className="p-4 gap-4">
      <button onClick={() => open.set(!open.value)} className="self-start rounded-xl bg-accent-solid text-on-accent-solid px-4 py-2">Toggle</button>
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

## Size

`transition-size` (or `transition-[height]`, `transition-[width]`) grows or shrinks a box from the
size it had to the size layout gives it now, with the element's `duration-*`, `delay-*` and `ease-*`;
like `transition-layout` it adds to the other `transition-*` properties.

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const open = useLocal('open', false);
  return (
    <page className="p-4 gap-2">
      <button onClick={() => open.set(!open.value)} className="h-11 rounded-lg bg-accent-solid text-on-accent-solid">Details</button>
      <view className={`overflow-hidden rounded-lg bg-surface-raised transition-size duration-200 ${open.value ? 'h-24' : 'h-0'}`}>
        <text className="p-3">Shipped on Tuesday.</text>
      </view>
      <text className="transition-layout duration-200">Next section</text>
    </page>
  );
}
```

Layout runs once, at the new size. While the box changes it paints its background, border and
shadow at the size reached and clips its children there; the children are not laid out again each
frame, so text does not re-wrap on the way. Keep the content in the tree, change the box's size
(`h-0` to `h-24`) and give it `overflow-hidden` so the closed box hides it: content removed in the
same render is gone before the box closes. What sits
after the box jumps to its new place unless it takes `transition-layout` with the same duration
and ease. Taps and assistive tech see the final size from the first frame.

## Paths

`transition-[d]` on a `<vector>` moves its paths when `paths` changes: a chart line moving to new
data, an icon changing shape, a line drawing in. Each path's `strokeWidth`, `strokeDasharray`,
`strokeDashoffset` and solid `fill` and `stroke` colours move with it. Each new path pairs with an old one by `key` when every path on both
sides has one, otherwise by position. A pair morphs only when both paths have the same commands in
the same order (`M`, `L`, `C`, …, after arcs become curves); otherwise the new source shows at once.
A dash list of another length, a gradient, caps, joins and the viewBox are the new source's throughout.

```tsx
import { useLocal } from '@lingxia/view';

const weeks = [[12, 18, 15, 26, 22], [24, 14, 20, 12, 28]];
const line = (values: number[]) => values.map((v, i) => `${i ? 'L' : 'M'}${i * 25} ${40 - v}`).join(' ');

export function render() {
  const week = useLocal('week', 0);
  return (
    <page className="p-4 gap-3">
      <vector viewBox={[0, 0, 100, 40]} className="w-full h-24 text-accent transition-[d] duration-300"
        paths={[{ key: 'line', d: line(weeks[week.value]), fill: 'none', stroke: 'currentColor', strokeWidth: 2 }]} />
      <button onClick={() => week.set(1 - week.value)} className="h-11 rounded-lg bg-accent-solid text-on-accent-solid">Switch week</button>
    </page>
  );
}
```

To draw a line in without knowing its length, give it `pathLength: 1` so the whole path counts as 1,
then one dash of that length: `strokeDasharray: [1]` with `strokeDashoffset` going from `1` (hidden)
to `0` (drawn). Butt caps keep the hidden line from showing a dot at its ends.

## Layout motion

`transition-layout` slides an element from where layout last put it to its new position, with the
element's `duration-*`, `delay-*` and `ease-*`; it adds to the properties `transition-*` sets
(`transition-opacity transition-layout`). Use it on keyed rows so a reorder, an insertion or a removal
moves the others instead of jumping:

```tsx
import { useLocal } from '@lingxia/view';

export function render() {
  const rows = useLocal('rows', ['a', 'b', 'c']);
  return (
    <page className="p-4 gap-2">
      <button onClick={() => rows.set([...rows.value].reverse())} className="self-start rounded-xl bg-accent-solid text-on-accent-solid px-4 py-2">Reverse</button>
      {rows.value.map(row => <view key={row} className="h-10 rounded-lg bg-surface-raised transition-layout duration-200" />)}
    </page>
  );
}
```

Only position moves: a size change lands at once. Input follows the box where it is drawn. More than
256 boxes moving at a time land at once.

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
