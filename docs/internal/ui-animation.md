# GPU View animation implementation contract

Read [ui-renderer.md](ui-renderer.md) and [ui-styling.md](ui-styling.md) first. Motion
utilities compile on every page; native accepts field 101 (keyframes) and consumes field
171 (transitions).

## Ownership

The compiler lowers Tailwind/PostCSS once into bounded payloads; the runtime never
evaluates CSS per frame and never hardcodes class names. `lingxia_ui::animation` owns
identity, time, interpolation and the next wakeup. Platform adapters supply a monotonic
clock and visibility/presentation events only: no View-isolate timers, no per-element
platform animators, no interpolation in JS or in the presenter. Apple and Harmony
adapters must reuse this state.

## Keyframe payload (field 101)

`decode_definition` (≤32 KiB per definition, ≤256 KiB per page, measured in UTF-8 bytes on
both sides) revalidates every value: name, `durationMs`/`delayMs`, `iterations` (null =
infinite), `whenReduced` (`base`|`keep`), easing (`linear`, `step-end`, four Bezier
values), optional opacity keys and ≤8 ordered transform tracks: `scale` (positive; reflections rejected), `translate`
(pixels, or fractions of the node's own box with `relative: true`), `rotate` (degrees).
Keys are `{offset, value, easing?}`; a key's easing eases the segment that starts there
and the final key cannot carry one. Offsets strictly increase from 0 to 1; the compiler
fills missing endpoints with the identity. Transform-list order is kept
(`scale(.96) translateY(8px)` scales the translation) and composes about the box center.
Reverse/alternate/fill modes, negative delays and multiple animations reject.

## Transitions (field 171)

`{properties ⊂ [opacity, transform, bg, fg, border, shadow], durationMs (≤10 s), delayMs,
easing}`. At each prepared frame, every node with a transition compares its *natively
resolved* value (after `active:`/`hover:`/theme/RTL variants, read from the built scene)
with the value last committed for its identity. A difference starts an implicit instance
from that value, or from the in-flight sample at prepare time if one runs, to the new one.
A node seen for the first time never animates; removal drops its state. No exit or
presence animations yet.

- Colours: premultiplied OKLab (black→white midpoint is rgb 99). A background fades in on
  a node that had none (the fill is injected with the node's radii); gradients jump.
- fg overrides the node's text and descendants that inherit its colour.
- Shadows: per layer, the shorter list padded with transparent zero layers; a node whose
  shadows rasterize (unequal corner radii) jumps.
- Transforms: translate/rotate/scale interpolate independently about the node's origin.
  The override replaces the laid-out translation and the static paint affine, so hit
  geometry follows the presented transform. A subtree with an editor, scroll or slider
  jumps rather than fail the frame. A target of `scale-0` paints nothing until it lands.
- Keyframe values win over a transition on the same property.

## Time and commits

One `Timeline` per page; times are durations from a host epoch. Preparation forks it
and builds a candidate (`Prepared`) with keyframe instances, transitions and sampled
`Visual`s; only `animation_presented` for the matching generation and surface commits
it. Entrances and new transitions start at that presentation. Failed preparation,
declined frames and stale acknowledgements consume nothing. Unrelated View updates keep
elapsed time; a changed definition or name starts a new lifetime; reset clears all.

Virtual-list eviction is not removal: field 102 `itemKeys` inventories retain evicted
rows' lifetimes without scheduling them; missing keys or lists cancel. Visibility
includes nodes intersecting the viewport before or after their sampled transform.

Animation-only frames (`rebuild=false`, no input or viewport change) reuse committed
layout and shaping and touch only paint, transforms and hit geometry. Opacity is a
subtree group. Text is never scaled by resizing the surface.
`DisplayList::animated_spans()` names each outermost animated subtree's command range,
holding the frame's sampled values. A span opens from and closes back to its parent's
layer stack and is followed by the parent's absolute transform, so commands outside the
spans are identical across frames of one layout: renderers cache those and re-encode only
the spans. Presented frames carry a layout key; between frames of one layout, interaction
keeps a held press and focus by index, and bounds-only accessibility changes go out at
most every fourth frame and once on settle.

## Scheduling

After a successful presentation the host asks `view_animation_wakeup` over the visible
identities: `NextFrame` for continuous change, `After(d)` for a step, delay or iteration
boundary, `None` when settled. One timer per page (`animation_timer`) is revoked by any
other frame, so animation, caret and scroll work coalesce into one frame per vsync. A
late frame samples current time and never replays missed frames; a page whose encode
overruns a vsync drops frames, never timing. Values that differ from what was presented
force a catch-up frame. Removal, completion and teardown leave no timer. While the
last wakeup was `NextFrame`, presenting queues the next prepare right behind the
acknowledgement on the worker, and once a frame is prepared the next one is prepared
ahead while it waits for its vsync (at most one ahead, visible pages only, never while
input waits). Every frame samples at its expected vsync (EMA of vsync deltas), so spacing
stays even. A frame prepared ahead builds on the unpresented one as if it had presented
at its sample time; acknowledgements arrive in order and a refused frame drops the one
built on it. Commits of one layout stay valid for input, so a tap on the presented frame
still dispatches while newer frames of that layout are queued; queued same-layout frames
yield to a waiting activation. An adapter whose `present` only queues a frame (CPU raster)
returns `Presenter::deferred` and calls `ViewHost::deferred_presented` once it is on
screen; nothing prepares meanwhile, so each shown frame carries its own sample.

## Hidden pages

`set_visible(false)`, surface loss and pause park the timeline and revoke the timer. A
hidden page's catch-up frame (environment or target change) prepares with
`set_animation_target(surface, visible: false)`: it samples the parked time, a change
made meanwhile lands on its end value, and its acknowledgement commits state without
starting time or scheduling. On reveal the kept frame already shows the parked state and
time resumes from it. The frame-currency gate compares environments only; animation
frames neither change nor invalidate it.

## Motion preference

`env.reducedMotion` (default false) and `env.animatorDurationScale` (0..=10, default 1;
0 reads as reduced motion) are read at each prepared frame. Reduced motion completes
transitions at once and shows the base style of keyframe animations whose payload says
`"whenReduced":"base"`; `"keep"` (authored under `motion-reduce:`/`motion-safe:`) keeps
playing. The scale multiplies keyframe and transition durations, delays and wakeups.
Either change is an environment change, so a hidden page renders one catch-up frame.

## Metrics

`FrameMetrics.animationFrames` counts visible presentations in the last second that
showed or cleared animated values; parked frames never count. Tests assert silence with
it and with presentation counts, not with log greps.

## Android path

Android passes the surface generation and `SystemClock.uptimeMillis()` captured when
enqueueing on the UI thread. Successful `surface.present` (GPU) or canvas post (CPU)
queues `nativeAnimationPresented` on the same worker; it returns -1 (no wakeup), 0
(next frame) or a rounded-up delay, and the UI subtracts the round trip before posting.
`GpuAnimationScheduler` owns one cancellable callback through `scheduleFrame`, never marks
`viewDirty`, and rejects callbacks for a stale epoch, frame or visibility.

## Verification

- Fake-clock session tests (`session/tests/transition.rs`, `animation.rs`): 200 ms at
  60 Hz settles in 12–14 frames with ≥10 intermediate samples; interruption continues
  within one frame step; mid-flight hit bounds follow the transform; CPU and composited
  routes agree at fixed times; reduced motion lands in one frame; hidden pages park.
- Host tests (`session/tests/host.rs`): a spinner presents once per vsync only while
  shown, nothing hidden, removed or after a one-shot completes, no timers left.
- Device acceptance (`examples/lingxia-showcase-gpu/pages/motion-lab`, Pixel): fade,
  interruption, press scale, spin at display rate, reduced motion, hidden pages and
  CPU parity.
