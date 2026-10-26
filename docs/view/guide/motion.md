# Motion (gated)

> **Not available for production pages.** Transitions and animations are implemented in the compiler
> and the renderer but have not passed device acceptance. A page can only use them by opting into the
> gate below, and their behavior may change before the gate is removed. Without the opt-in every motion
> utility fails the build with `UI_ANIMATION`.

```tsx
// @ui-experimental motion
import { useLocal } from '@lingxia/ui';

export function render() {
  const shown = useLocal('shown', true);
  return (
    <page className="p-4 gap-4">
      <button onClick={() => shown.set(!shown.value)}
        className="self-start rounded-xl bg-blue-600 text-white active:scale-95 transition-transform duration-150">
        {shown.value ? 'Fade out' : 'Fade in'}
      </button>
      <view className={`w-12 h-12 rounded-lg bg-blue-600 transition-opacity duration-200 ease-out ${shown.value ? 'opacity-100' : 'opacity-0'}`} />
      <view className="w-10 h-10 rounded-lg bg-rose-600 motion-safe:animate-pulse motion-reduce:opacity-60" />
    </page>
  );
}
```

## The gate

A page opts in with this exact line in its `index.tsx`:

```ts
// @ui-experimental motion
```

The opt-in is per page. Without it, `transition-*`, `duration-*`, `delay-*`, `ease-*`, `animate-*`,
`@keyframes` in the stylesheet and an inline `style.animation` are refused:

```tsx error UI_ANIMATION
export function render() {
  return <page><view className="h-10 bg-blue-600 transition-opacity" /></page>;
}
```

## Transitions (as designed)

`transition`, `transition-colors`, `transition-opacity`, `transition-transform`, `transition-shadow` and
`transition-all` animate changes of the painted properties: opacity, transform, background, text color,
border color and shadow. `duration-*`, `delay-*` and `ease-*` (or `ease-[cubic-bezier(…)]`) set the
timing, up to 10 s each; the defaults are 150 ms and `cubic-bezier(0.4, 0, 0.2, 1)`. A transition runs when a re-render or
a state variant (`active:`, `hover:`, `focus:`) changes one of those properties.

Layout does not animate. A transition of width, height, padding, margin, gap, insets or font size is
refused, naming `transition-transform` or `transition-opacity` as the alternative:

```tsx error UI_CLASS_UNSUPPORTED
// @ui-experimental motion
export function render() {
  return <page><view className="h-10 bg-blue-600 transition-[width]" /></page>;
}
```

Animate a scale or translate instead of a size.

## Animations (as designed)

`animate-spin`, `animate-ping`, `animate-pulse`, `animate-bounce` and your own `--animate-*` tokens with
`@keyframes` in the stylesheet run keyframe animations of opacity and transforms (translate, including
`%`, rotate, scale). Keyframes may omit their endpoints, and each keyframe may set its own easing.
`animate-none` removes an animation. An element runs one animation; several comma-separated
animations, `alternate`/`reverse` directions and fill modes are refused. An animation starts when its element first appears on screen; an
element scrolled out of a list and back does not restart it.

## Reduced motion

`motion-reduce:` and `motion-safe:` follow `env.reducedMotion`, the system's reduce-motion setting.
They work on any utility today, outside the gate (`motion-reduce:opacity-60`). Pair every animation
with a `motion-safe:` prefix or a `motion-reduce:animate-none` override.

## Not planned

Layout transitions, 3D transforms, `transition-behavior`, scroll-driven animations, View-side timers
(`setTimeout`, `requestAnimationFrame`) and per-frame JavaScript animation. Motion runs in the renderer;
the View only declares it.
