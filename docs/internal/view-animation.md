# View animation

How keyframe animations and transitions run on a View page. Authoring rules for the motion
utilities are in [view-styling.md](view-styling.md#motion); the renderer they feed is in
[view-renderer.md](view-renderer.md).

Invariants:

- The compiler lowers Tailwind/PostCSS once into bounded payloads: field 101 `animation`
  (keyframes) and field 171 `transition`. The runtime never evaluates CSS per frame and never
  hardcodes class names.
- The `animation` module of `lingxia-view` owns identity, time, interpolation and the next
  wakeup. Platform adapters supply a monotonic clock, vsync and visibility/presentation events only: no
  View-isolate timers, no per-element platform animators, no interpolation in JS or in the
  presenter.
- Animation state commits only when the frame that carries it is presented.
- A late frame samples current time; missed frames are never replayed.

## Keyframes (field 101)

The `lingxia-view` decoder revalidates every value; the payload is at most
32 KiB per definition and `animationBytes` (256 KiB) per page, in UTF-8 bytes on both sides.

| Part | Rule |
| --- | --- |
| Timing | `name`, `durationMs`, `delayMs` (0 to one day; no negative delay), `iterations` (`null` = infinite), `fill` (`none`/`forwards`/`backwards`/`both`) |
| Reduced motion | `whenReduced`: `base` or `keep` (see [Motion preference](#motion-preference)) |
| Easing | `linear`, `step-end` or four Bezier values; a key's easing eases the segment that starts there, and the final key cannot carry one |
| Tracks | optional opacity, optional background position, and up to 8 ordered transform tracks: `scale` (positive; reflections rejected), `translate` (pixels, or fractions of the node's own box with `relative: true`), `rotate` (degrees) |
| Keys | `{offset, value, easing?}`; offsets strictly increase from 0 to 1; the compiler fills missing endpoints with the identity |

Transform-list order is kept (`scale(.96) translateY(8px)` scales the translation) and composes
about the box center. Reverse and alternate directions and multiple animations on one element
fail in the compiler.

## Transitions (field 171)

`{properties, durationMs, delayMs, easing}`, with `durationMs` and `delayMs` up to
`transitionMs` (10 s). Properties:

| Wire name | Animates |
| --- | --- |
| `opacity`, `transform` | the node's opacity and transform |
| `bg`, `fg`, `border` | background, text colour (and descendants that inherit it), border colour |
| `shadow` | box shadows, and rings, which are shadows |
| `layout` | position: a FLIP offset from where layout last put the node, animating to zero |
| `size` | a resized box: from the size on screen to the new one |
| `d` | a `<vector>`'s paths, strokes and solid colours, between sources of the same shape |

Easing is a curve as for keyframes or a spring (`{ spring: bounce }`, bounce 0 to 0.9, from
`ease-spring*`); a spring's duration is perceptual and an interrupted spring keeps its velocity.

At each prepared frame, every node with a transition compares its natively resolved value
(after `active:`/`hover:`/theme/direction variants, read from the built scene) with the value
last committed for its identity. A difference starts an implicit instance from that value, or
from the in-flight sample if one runs, to the new one. Resolving natively is what lets a state
variant or a theme change animate without the View re-running.

- A node seen for the first time enters from its `starting:` values, or appears at its value
  without them.
- A removed node with `exit:` values leaves toward them as a ghost: only the outermost removed
  node, at most `ghosts` at once. Without them removal drops its state.
- At most `layoutMotions` nodes move or resize at once; past it further moves land at once.
- Colours interpolate in premultiplied OKLab (the black-to-white midpoint is rgb 99). A
  background fades in on a node that had none (the fill takes the node's radii); gradients jump.
- Shadows interpolate per layer, the shorter list padded with transparent zero layers; a node
  whose shadows rasterize (unequal corner radii) jumps.
- Transforms: translate, rotate and scale interpolate independently about the node's origin.
  The override replaces the laid-out translation and the static paint affine, so hit geometry
  follows the presented transform. A subtree with an editor, scroll or slider jumps rather than
  fail the frame. A target of `scale-0` paints nothing until it lands.
- Keyframe values win over a transition on the same property.

## Time and commits

One timeline per page; times are durations from a host epoch. Preparation forks it and builds
a candidate with keyframe instances, transitions and sampled visuals; only the presentation
acknowledgement for the matching generation and surface commits it. Entrances and new
transitions start at that presentation, so a frame that never reaches the screen cannot use up
part of a motion. Failed preparation, declined frames and stale acknowledgements consume
nothing. Unrelated View updates keep elapsed time; a changed definition or name starts a new
lifetime; reset clears all.

Virtual-list eviction is not removal: the list's `itemKeys` (field 102) keep evicted rows'
lifetimes without scheduling them; missing keys or lists cancel. Visibility includes nodes
intersecting the viewport before or after their sampled transform.

Animation-only frames (no rebuild, input or viewport change) reuse committed layout and shaping
and touch only paint, transforms and hit geometry. Opacity is a subtree group. Text is never
scaled by resizing the surface.

The display list names each outermost animated subtree's command range. A span
opens from and closes back to its parent's layer stack and is followed by the parent's absolute
transform, so commands outside the spans are identical across frames of one layout: renderers
cache those and re-encode only the spans. Presented frames carry a layout key; between frames
of one layout, interaction keeps a held press and focus by index, and bounds-only accessibility
changes go out at most every fourth frame and once on settle.

## Scheduling

After a successful presentation the host asks the animation module for the next wakeup over the
visible identities: the next frame for continuous change, a delay for a step, delay or
iteration boundary, none when settled. One timer per page is revoked by
any other frame, so animation, caret and scroll work coalesce into one frame per vsync.
Removal, completion and teardown leave no timer.

- A page whose encode overruns a vsync drops frames, never timing. Values that differ from what
  was presented force a catch-up frame.
- While the last wakeup was the next frame, presenting queues the next prepare right behind the
  acknowledgement on the worker, and a prepared frame lets the next one be prepared ahead while
  it waits for its vsync: at most one ahead, visible pages only, never while input waits.
- Every frame samples at its expected vsync, so spacing stays even. The interval is the
  platform's when it reports one (Android's Choreographer passes it with each vsync), else a
  moving average of vsync deltas.
- A frame prepared ahead builds on the unpresented one as if it had presented at its sample
  time; acknowledgements arrive in order and a refused frame drops the one built on it.
- Commits of one layout stay valid for input, so a tap on the presented frame still dispatches
  while newer frames of that layout are queued; queued same-layout frames yield to a waiting
  activation.
- An adapter whose present only queues a frame (CPU raster) reports the presentation as deferred
  and tells the host once it is on screen; nothing prepares meanwhile, so each
  shown frame carries its own sample.

Adapters request vsync through their frame scheduler and report it back to the host;
none keeps its own animation clock.

## Hidden pages

Hiding the page, surface loss and pause park the timeline and revoke the timer. A hidden
page's catch-up frame (environment or target change) prepares against its surface as not
visible: it samples the parked time, a change made
meanwhile lands on its end value, and its acknowledgement commits state without starting time
or scheduling. On reveal the kept frame already shows the parked state and time resumes from
it. The frame-currency gate compares environments only; animation frames neither change nor
invalidate it.

## Motion preference

`env.reducedMotion` (default false) and `env.animatorDurationScale` (0 to 10, default 1; 0 reads
as reduced motion) are read at each prepared frame. Reduced motion completes transitions at once
and shows the base style of keyframe animations whose payload says `"whenReduced":"base"`;
`"keep"` (authored under `motion-reduce:`/`motion-safe:`) keeps playing. The scale multiplies
keyframe and transition durations, delays and wakeups. Either change is an environment change,
so a hidden page renders one catch-up frame.

## Metrics

The frame metrics' `animationFrames` counts visible presentations in the last second that showed or
cleared animated values; parked frames never count. Tests assert silence with it and with
presentation counts, not with log greps. Each executed click also goes to the host's debug log.
A declined candidate keeps the View's pending updates; any other failure reverts them.

## Verification

- Fake-clock session tests verify that 200 ms at 60 Hz settles in 12–14 frames with at least 10
  intermediate samples; interruption continues within one frame step; mid-flight hit bounds follow
  the transform; CPU and composited routes agree at fixed times; reduced motion lands in one frame;
  hidden pages park.
- Host tests verify that a spinner presents once per vsync only while shown, nothing while hidden,
  removed or after a one-shot completes, with no timers left.
