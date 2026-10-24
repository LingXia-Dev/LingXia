# GPU View animation implementation contract

This is an in-progress renderer work item, not implemented animation support. Read
[ui-renderer.md](ui-renderer.md) and [ui-styling.md](ui-styling.md) first.

`lingxia_ui::animation` now supplies the presentation-bound timing core:
bounded duration/delay/iterations, candidate identity reconciliation, page-owned
frame tickets, explicit successful-presentation commit, pause/resume and reset.
Definition equality includes the animation name and validated typed effect tracks
rather than a collision-prone hash. Renaming identical tracks starts a new lifetime.
Per-node and aggregate track budgets apply before
replacing a candidate.
`animation::interpolate` supplies bounded numeric tracks, linear/step-end/cubic
Bezier easing, constant-span deadline hints and ordered scale/translate matrix
composition around an explicit origin. Track sampling is forward progress0..1;
reverse playback, easing extrapolation and other CSS animation modes are not
implemented. Property-specific clamping (such as opacity) belongs after numeric
interpolation. `Effect` clamps sampled opacity and validates sampled transforms.
Singular transforms must be handled before painting/hit testing.
Prepared frames expose active visual values; delayed/finished effects use base
style (no fill). After presentation, request the timeline's wakeup using only
visible identities. Comparing current values to actually presented values forces
a catch-up frame when a late presentation misses a step or leaves a finished
effect painted. Offscreen identities retain time without scheduling frames.
`prepare_visible` receives all live definitions plus the visible identity set.
Only visible entries are sampled and start on successful presentation. Retaining
an unseen definition must not consume its entrance; evicting a previously shown
item retains its start time. Omit a definition only for semantic removal. Invalid
visible sets leave the preceding pending candidate intact.
ViewSession owns the timeline and a generation/surface-bound candidate through
`prepare_animated`. It applies sampled visuals to retained composition before
capturing hit geometry and building pixels; preparation does not start effects.
`animation_presented` commits
only the matching current frame. Rejected surface/generation/time acknowledgements
preserve a newer valid candidate. Pixel-build failure preserves the previous
timeline and candidate too. `set_animation_surface(None, now)` parks active time;
replacement requires a fresh host epoch and invalidates pending presentation.
Reset clears animation lifetimes. With `rebuild=false` and no independent input
or viewport invalidation, preparation reuses committed layout/shaping and does
not execute the View.

Android CPU and GPU preparation now call the collected session seam. This path
has not yet passed physical animation acceptance; production authoring stays gated.
Production Tailwind animation support and showcase rendering are still missing.
Do not enable authored
animation classes until their visuals and actual-presentation acknowledgement
reach both native GPU and CPU paths.

Paint commands now carry absolute logical-space scale/translation state. CPU,
sparse GPU and Vello scene encoding apply it to primitives and glyph runs.
Composition spans inherit their preceding transform; clipping resolves that same
transform into device coordinates once. A uniform backdrop transform scales its
blur too. Nonuniform backdrop scaling, rotation, reflection and singular matrices
are rejected. Retained composition applies center-origin transforms through the
subtree without changing its layout footprint. Culling maps static ancestor clips
into local coordinates; snapshots preserve the original world clips to avoid
round-trip drift. Bounds/content clips/control bounds follow the resulting matrix.
Plot hit tests inverse-map the point into original chart geometry, including pie
and bar padding. Animated opacity replaces base opacity and remains a subtree group.
Transformed editors, scrolling containers and sliders are rejected until their
editing/gesture geometry is supported; an untransformed scrolling ancestor is valid.
Do not expose these primitives as authored animation support before compiler and
platform presentation/scheduling integration.

Math follows [CSS easing](https://www.w3.org/TR/css-easing-1/#cubic-bezier-easing-functions)
and [transform-list composition](https://www.w3.org/TR/css-transforms-1/#transform-rendering).
Retain declaration order; `scale(.96) translateY(8px)` scales the translation too.
The compiler must resolve duplicate/implicit keyframes before constructing a
track; runtime track offsets are strictly increasing with explicit endpoints.

`packages/lingxia-ui/animation.mjs` lowers a resolved animation shorthand and
PostCSS keyframes into the bounded payload consumed by `decode_definition`.
Both sides share fixtures generated from official Tailwind output using the
React showcase token file. The payload carries a bounded name, durationMs/delayMs, required
iterations (null means infinite), easing (linear/step-end or four Bezier values),
optional opacity keys and ordered scale/translate x/y tracks. Keys contain offset
and value. Native decoding caps each payload at32KiB and revalidates every track.

The lowerer merges duplicate offsets/declarations in source order and supports
identity `transform:none` endpoints. It currently requires explicit endpoints
for each animated property; implicit base-style endpoints, per-keyframe easing,
negative delays, alternate/reverse/fill modes and multiple animations reject.
`compileUtilities` connects the helper to rule lowering behind the internal
`animationLowering` option. This option is for staged integration/tests, not a
page-builder capability. The existing style cascade carries `animation` payloads;
null from `animate-none` clears them. Theme/media variants follow Tailwind ordering.
Copy nested tracks on configuration and rule resolution so callers cannot mutate
cached definitions across nodes or environment changes. Focus animation rejects.
Production page builds leave the option disabled and reject literal inline animation
styles. The View encoder transports internal class payloads in append-only field101
as JSON; `animate-none` omits it. JS serialization rejects non-JSON/nonfinite values;
JS and native decoding independently enforce32KiB per definition and256KiB per
page, measured in UTF-8 bytes rather than JS code units.
The raw renderer and ordinary ViewSession preparation reject animation records.
`prepare_animated` requires every record definition to match its prepared timeline,
then strips metadata from geometry records before layout/cache matching. Its
transformed display and hit map use the sampled effect. `prepare_view_animated`
collects definitions automatically from the final measured record window. Its
candidate inventory retains evicted virtual-row effects using field102 inventories,
including nested lists hidden by an outer eviction. Realized rows replace their
old definitions; missing keys/lists cancel retained effects. Animated virtual rows
require `itemKeys` on every owning list. Failed layout, pixel building or JS commit
leaves the old inventory, timeline and presentation candidate intact.
Android presentation/deadline scheduling is wired but still requires physical
validation. Keep production authoring disabled until that path is verified.

## Reference behavior

`examples/lingxia-showcase/lxapp/theme/tokens.css` defines two effects used by
the Stream page:

- `blink`: 900ms, step-end, infinite; opacity1 at0/49%, opacity0 at50/100%.
- `chart-in`: 350ms, cubic-bezier(0.16,1,0.3,1); opacity0 to1 and the authored
  `scale(0.96) translateY(8px)` to `scale(1) translateY(0)` transform list.

The streaming cursor exists only while a response is active. A chart enters
once per inserted chart identity, not once per token, selection or scroll frame.
Preserve transform order and the default center origin. An opacity-only fade
does not complete chart parity. General CSS transitions remain a separate gap.

## Ownership and commit boundaries

Compile animation declarations/keyframes with the existing official Tailwind
pipeline. Transport bounded typed keyframes, duration, iteration and easing;
reject unsupported properties and combinations before installing the View.
Do not hardcode the two class names in the runtime or evaluate CSS per frame.

The shared Rust session owns identity reconciliation, elapsed active time,
interpolation and the next required wakeup. Platform adapters supply a monotonic
time and visibility/presentation events. Do not add View-isolate intervals or
per-element platform animators. Apple/Harmony adapters must reuse this state.

Prepare candidate animation state alongside candidate records/display lists.
Failed preparation and rejected presentation must not consume an insertion,
restart, completion or deadline. Identify the first successful presentation
explicitly; the existing View-record commit alone is not presentation proof.
Generation and surface tokens reject stale completion callbacks.

Unrelated View updates preserve elapsed time. Removal cancels the timeline;
replacement or an explicit animation-definition change starts a new instance.
Virtual-list eviction is not semantic removal: retain the effect's lifetime
identity while its item still exists, without scheduling frames for offscreen
paint. Scrolling a chart back into view must not trigger another entrance.
The View row wrapper now carries its normalized semantic key into native node
identity, independently of the positional measurement index. Session tests cover
insertion, eviction, return after completion and replacement with an explicitly
supplied lifetime set and with automatic record collection.
Lists can now transport their complete ordered `itemKeys` inventory in field102
without materializing offscreen rows. Use it to distinguish eviction from removal
when collecting definitions; a row's normalized native key must match its slot.
Do not infer semantic removal from the current window or retain unknown keys
forever. Automatic collection runs after window refinement, never on an interim
measurement window. Unseen overscan rows retain definitions without consuming an
entrance or scheduling work. Visibility includes a node intersecting the viewport
before or after its sampled transform: inward motion can become visible, and a
temporarily outward-moving effect can continue back into view. Opacity does not
exclude a node from timing. `view_animation_wakeup` uses the last collected visible
set; like explicit wakeups, consume it only after matching successful presentation.
Hide/park/background suspends active time and cancels native wakeups. Resume
continues retained instances; disposal/reload destroys them. Never advance a
hidden page by wall-clock catch-up or keep a hidden surface rendering.

## Paint, input and scheduling

Animation-only frames reuse committed layout and shaping. Apply opacity as a
subtree group, and transforms to its complete paint/input/clip geometry; the
layout footprint stays unchanged. Never scale text by resizing the GPU surface.
Hit tests, editor geometry and accessibility bounds must describe the presented
transform, or explicitly reject unsupported animated interactive subtrees.

Keep one outstanding native wakeup per visible page. Continuous effects request
the next display opportunity only while active; step effects request only their
next value-changing deadline. Combine deadlines with caret/scroll work instead
of starting another permanent frame loop. Cancel on stop/removal/teardown and
finish one-shot animations with no further wakeups. Slow frames sample elapsed
time directly rather than replaying missed frames.

Android passes the current surface generation and `SystemClock.uptimeMillis()`
to both native preparation paths. Capture timestamps when enqueueing on the UI
thread, not when the worker starts: worker execution order must agree with pause
and presentation event order. Native preparation binds the epoch and calls
`prepare_view_animated`; successful `surface.present` queues
`nativeAnimationPresented` on the same worker. Its return is -1 for no wakeup,0
for the next frame, or a positive delay in rounded-up milliseconds. Stale epoch
or frame acknowledgements return no wakeup. Subtract acknowledgement round-trip
time before posting the callback so step deadlines do not drift.

`GpuAnimationScheduler` owns one cancellable callback, coalesced through the same
`scheduleFrame` entrypoint as input/scroll/caret work. It never marks `viewDirty`.
Callback identity rejects cancelled callbacks already dequeued by the host.
The UI checks current epoch/frame, visibility and in-flight work before accepting
an acknowledgement's delay. Pause/ancestor hiding/window hiding/surface loss park
native time on the worker and cancel callbacks; reload/disposal cancel callbacks
and release/reset session state. CPU canvas posting and successful GPU presentation
use the same acknowledgement path. These JNI signature changes require shipping
matching SDK classes and native library together.

## Acceptance before enabling showcase classes

- Deterministic clock tests: delay/end boundaries, step edges, easing, finite
  iteration completion, pause/resume, identity replacement, and candidate rollback.
- Shared display tests: group opacity, ordered center-origin transforms, nested
  clips, scrolling and presented hit geometry at multiple scales; CPU/GPU parity.
- Compiler tests: token imports, keyframe/property validation, cascade/reset and
  bounded values; unsupported effects fail rather than disappear.
- Physical Stream: visible cursor during token delivery, final cursor removal,
  one chart entrance, unchanged chart selection, cancel/clear/navigation teardown,
  hidden-page silence and no frames after a completed response settles.
- Record transition frames in both themes. Settled screenshots and a passing
  functional stream probe alone cannot establish animation fidelity or smoothness.

Land the shared timeline/commit seam first, then display transforms, compiler
lowering and platform scheduling. Enable the TSX classes only once the complete
path is ready; do not advertise the timing seam as finished animation support.
