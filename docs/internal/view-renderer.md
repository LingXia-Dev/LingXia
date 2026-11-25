# View renderer

The native renderer behind View pages: wire decoding, View isolates, layout,
text, paint, interaction and the platform adapters. Authoring:
[view-authoring.md](view-authoring.md), [view-styling.md](view-styling.md);
motion: [view-animation.md](view-animation.md); colours and scales:
[theme.md](theme.md); bounds: [view-engine-limits.md](view-engine-limits.md).

Invariants:

- Native validation is independent of the JS runtime and re-checks every record.
- A render is a candidate; committed state changes only on commit.
- All geometry (hit testing, scroll, reach, chrome) runs in the shared core
  against the presented frame; adapters convert events and compute none.
- The host never calls adapter code while holding its own or the renderer's locks.

## Ownership

`crates/lingxia-view` owns the wire contract, View isolates, candidate
transactions, layout, shaping, scene building, interaction policy, semantics
and CPU/GPU painting. It has no lxapp, route, page-stack or WebView dependency.
`crates/lingxia-lxapp` owns page configuration, resource authority and Logic
bridging; `crates/lingxia/src/gpu_page` binds lxapp pages to platform page
shells (creation, live registry, dispose-once leases, connection-generation
gate, worker key) and `crates/lingxia/src/ffi/<platform>` only marshals. Every page of an lxapp
renders natively exactly when its pages export `render`: the CLI reads the exports and writes
`"renderer": "gpu"` into the built `lxapp.json`, which the runtime reads; source manifests and page
configs name no renderer.

| Module | Owns |
| --- | --- |
| `protocol` | Wire decode/validation, generated ids (`protocol/generated.rs`), scale references |
| `animation` | Keyframe and transition timelines, wakeups ([view-animation.md](view-animation.md)) |
| `engine` | JS engine adapters and their capability list |
| `session` | `ViewSession`, `ViewHost`, lifecycle tickets, candidate prepare/commit, images, metrics |
| `layout` | Taffy styles, page root, virtual lists |
| `text` | Computed text style, Parley shaping, editing geometry, text scaling |
| `scene` | Node snapshots, display list, paint order, plots, vectors, shadows |
| `interaction` | Hit testing, scroll, reach, sliders, keyboard, pointer state |
| `semantics` | Composed control groups, accessibility projection, stable ids |
| `renderer` | CPU raster and wgpu/Vello presentation |
| `platform` | Adapter traits, host chrome colours; adapters live in their own crates (`lingxia-view-android`, `-apple`, `-harmony`, `-windows`) |

## Wire contract

`contracts/view/schema.json` is the single source for tags,
field ids, option enums, the CSS-named author style table (with the CSS
longhand each key accepts), element admission, wire admission (per-tag
children, content eligibility, required parent and fields, per-field tags) and
page limits. `contracts/view/generate.mjs` writes every consumer's copy (see
[view-authoring.md](view-authoring.md#contract-package)); `--check` fails on
drift and a Rust test compares every compiled table with the schema. Field and
tag ids are append-only and dense, because shipped modules and hosts name them
by number; reserved fields are rejected by the decoder.

The schema `frame` section types what adapters receive (snapshot, semantics,
automation); it generates `GpuFrame.java` and `GpuFrame.swift`.
`contracts/view/vectors/frame.json` holds real host output, re-recorded with
`LINGXIA_UPDATE_VECTORS=1 cargo test -p lingxia-view --lib every_message`; Rust
holds it to the schema, the Android JVM and Swift suites decode it, and the
JVM suite fails on any key the decoder never reads.

`tests/properties.rs` lays out generated wire buffers (`tests/support/wire_gen.rs`, mostly
admitted) and requires finite geometry with visible bounds inside the viewport; replay with
`LINGXIA_PROPERTY_SEED`/`LINGXIA_PROPERTY_CASES`. `crates/lingxia-view/fuzz` holds cargo-fuzz
targets over the same generator, raw wire JSON, automation requests and the strict JSON
contracts: `scripts/ci/view-fuzz-smoke.sh [seconds]` (nightly).

Never trust that a field reached native through the runtime: decode bounds
strings, numbers, JSON payloads and tag applicability, so a bug or a
hand-written buffer in the isolate cannot corrupt layout or paint.

Module compatibility (the banner, `minField`, `maxField`) is specified in
[view-authoring.md](view-authoring.md#module-compatibility). Release hardening
must preserve the banner line.

## Engines

View isolates run on QuickJS on every platform, with candidate transactions
and bounded jobs (preemption), linked beside the platform's Logic
engine (JavaScriptCore on Apple, ArkJS on HarmonyOS) through rong's per-engine
types (`rong::quickjs::JSRuntime`). The Logic engine stays rong's primary:
`lingxia` pins `primary-jscore` on Apple, so linking QuickJS cannot flip it.
A page that needs bounded jobs (all compiled modules do) cannot load on an
engine without them. `Engine::FEATURES` is the only source of these flags.

## Lifecycle and transactions

`session::lifecycle` issues typed tickets (page, surface, connection, commit).
Stale tickets are rejected rather than ignored silently. A render is a
candidate: `prepare` resolves and encodes without touching committed state;
`commit` adopts locals, callbacks and resources; `discard` drops it. Resources
(requests, streams, channels, leave guards) start only on commit and stop on
removal, reset or bridge replacement.

Failures classify into retryable rebuild, session reload and incompatible
module. A failed candidate keeps the committed frame and exposes a host Retry
that rebuilds in the same session. Execution timeouts poison the isolate:
input and scheduling stop until an explicit reload resets locals and rebinds
Logic through the current endpoint and generation.

Draining microtasks shares the render budget (`evalBudgetMs` and its
multipliers); past it the turn fails with `UI_TIMEOUT`. JS errors keep their message, code
and JSX trace (`at <tag> (file:line:col)`, `at Component (…)`) through the
engine boundary.

## Layout

- Taffy resolves percentages against the parent's content box; geometry
  beyond 1e12 logical or device units is rejected after layout.
- Flex basis is auto, a length or a percentage; growth never implies zero basis.
- `page` is the root that owns the safe area: padding absorbs insets and the
  keyboard (bottom = max(inset, keyboard)); scroll and list children fill it.
- Margins accept `auto`; insets apply to relative and absolute positioning;
  absolute children are out of flow and paint in z-index order within their
  stacking parent. `overflow: hidden` clips descendants and hit testing.
- `scrollAxis` selects one axis per scroll; virtual lists are vertical.
  Gestures transfer leftover distance only to ancestors on the same axis.
- Virtual lists measure windows natively and request rows by index. Row
  wrappers carry the semantic key; `itemKeys` is the ordered key inventory
  (unique, ≤256 bytes each, ≤256 KiB per page) that lets state follow
  reorders without calling offscreen factories. Anchors move the window
  without rendering skipped rows.
- System text scaling is a sampled `[authorSize, logicalSize]` curve applied once
  to font size, line height and letter spacing; fixed author heights stay fixed.
- System fonts split at the adapter. Core loads and names faces from the files or
  directories it is given and holds the generic-family defaults; each adapter
  discovers them (Android `fonts.xml`, CoreText, HarmonyOS font paths) and
  registers the result through `set_system_fonts`. `language` is the
  shaping locale, so Han picks SC/TC/JP/KR faces.

## Text

Computed text style inherits family, size, weight, style, line height (a CSS
multiplier recomputes per node; pixels inherit), letter spacing, alignment,
decoration, transform, white-space, word-break and overflow-wrap. Shaping
caches key on every input that changes glyphs. `lineClamp` and
`textOverflow: ellipsis` truncate during shaping; semantics keep the authored
string. Transforms (`uppercase` etc.) are paint-only.

## Paint

- One display list feeds CPU and GPU backends; tests compare their pixels.
- Opacity composites a node and its subtree as one group.
- Borders paint as one outer-minus-inner path; unequal widths give elliptical
  inner corners shared with scroll clips and hit geometry.
- Outer shadows follow CSS spread geometry, knock out the owner and never
  change layout, hit bounds or scroll extent. Up to eight layers.
- Gradients (linear, radial, conic) paint through one brush and paint transform on all backends.
- Backdrop blur/saturation is bounded (blur ≤ 64, saturation ≤ 16).
- Vector paths may carry an SVG transform and url(#id) gradient paint. ≤32 paths,
  16,384 elements per vector, 32,768 per frame, coordinates checked before and
  after the viewBox transform; `non-scaling-stroke` expands strokes after it.
- Clips may carry an affine; rotated subtrees clip and hit-test their true shape.
- Plots (bar, line, pie) are leaves: TS owns axes, legends and tooltips.
  Geometry is bounded by a shared plot budget on both sides of the wire.

GPU presenters on one device share a `gpu::Context`; caches stay per presenter.
A presenter holds at least two in-flight targets: `present(revision)` must not be
invalidated by a later `prepare` (depth-2 pipelining prepares the next frame while
one waits for its vsync).
While a page animates over a static bottom segment, the presenter keeps that segment
rendered as one surface-sized texture (8.5 MB at 1080x1962). It counts against the
page's budget only while visible and animating, and `Presenter::release_transient`
drops it when the page hides or the system reports memory pressure.
A driver crash during pipeline compilation cannot be caught: `renderer::sentinel`
records the adapter (backend, ids, driver, renderer release) before its first
compile and clears it after the first GPU frame presents. An adapter a dead
process left on record, or a known-crashing driver, is never used again; Android
falls back to GLES, then to the CPU renderer. Platforms supply only the store.
`env.renderer.fallback` is `"driver"` while a blocked adapter is in effect.
On Android the GPU path needs WebGL2 downlevel limits and 4096 px textures; pipelines bind
at most 5 sampled textures, 1 sampler and 1 uniform per stage, in 4 bind groups, with no
storage buffers or compute. A device below that renders on the CPU.
Lock order is presenter registry → context → renderer registry → scene
renderer. Never call host code while holding these locks.

## Interaction and semantics

All hit testing, scrolling, reach, slider stepping, keyboard navigation and
plot selection run in the shared core against the presented frame. Adapters
convert OS events and report pointer state; they compute no geometry.

- Dispatch uses committed handles; a handle from a discarded or superseded
  candidate is dead. Queued clicks rebind to a new frame only when the whole
  control snapshot except its handle is unchanged.
- State styles are paint-only deltas: `focusStyle` (editors), `pressedStyle`
  (buttons, checkboxes, switches, radios) and `hoverStyle`, which the runtime
  emits only for fine pointers. `pointer` is part of the environment.
- Switches, checkboxes and radios paint their own press state on the drawn
  indicator and never get the platform overlay, which would fill their padded touch box.
- Buttons and checkboxes may compose noninteractive content. They form one
  semantic node; its name is the explicit label or else the descendant text
  and image alternatives in order. Controls nested in a composed control stay their own nodes and targets
  (`semantics/groups.rs`); a scroll or list inside one is rejected.
- Image `alt=""` marks decoration; omitted alt keeps the image exposed.
- Editor focus survives reorder by token; disable, removal and reset end it.

## Images

Loads are asynchronous and cancellable. Identity is `(src, retryKey)`; the session
owns settled outcomes and delivers `onLoad`/`onError` once per identity, after
presentation. Obsolete tokens cancel queued and running loads. Resource authority and
byte caches stay in lxapp. Decodes are sized to the painted box (half-octave steps per
side; only growth re-requests) and shared through one process LRU (`decodedImageBytes`,
keyed by content hash and size) that evicts, never errors. A page binds at most
`visibleImageBytes` of painted sources; past it a source gets `onError` and a
placeholder. Both schema limits are ~5% and ~3% of a 2 GB device. On Android the
`ImageProvider` calls the facade's `ImageLoader` (`gpu_page::load_image`, keyed by the
page's token); dropping a handle drops its future and transport. Bytes never cross JNI.

## Platform adapters

`platform/mod.rs` defines what a platform supplies: `Presenter` (prepare on
the View worker, present on the main thread), `FrameScheduler` (vsync),
`MainThread` (post, delayed post), `EnvironmentSource`, `ImageProvider`,
`AccessibilitySink`, `InputMethod`, `Feedback`, `HostEvents` and `Clock`.
`session::host::ViewHost` drives a page through them: it owns the View worker
queue (`ViewWorker`, one isolate per app session), frame sequencing, dispatch,
images, the Logic bridge, the bootstrap deadline, recovery and caret blink.
Adapters convert OS events into host calls and execute typed effects; the host
never calls the adapter while holding its locks. Its tests use a fake adapter.

Pages of one `appId:session` share a worker by key, so nothing slow runs on it:
decode and CPU raster use process pools (a newer CPU frame replaces one not
started), GPU attach its own thread, and a page whose attach is pending
declines frames.

A new platform needs the adapter traits, text scaling and insets in the
environment, and `renderer` frames.

### Android

`crates/lingxia-view-android` implements the traits over the Java
`GpuPlatform` (vsync, posts, presentation, IME, accessibility, feedback chrome)
and exposes host calls as `GpuView` natives. Java converts events and draws
chrome; chrome geometry is `interaction::chrome` over the snapshot node.

Android hides a kept layer through its SurfaceControl and also moves it off
screen (SurfaceView ignores alpha). The move alone is not enough: a node
outside the window may never report its new position, and the layer stays
under the current page, where any reveal, slide or pull-to-refresh exposes it.
It reports the kept page's insets as they will be when shown.

### Windows

Windows runs each GPU page on its own UI thread, as a WebView2 page runs, with
a parent window from the host's native-view host and a child content window
inside it (`crates/lingxia-view-windows`, `crates/lingxia/src/ffi/windows/gpu_page.rs`).
The page registers under its webtag as `WindowsNativeContent`, so
`find_webview_handler` returns it and the host presents, moves, hides and
reparents it exactly as a WebView; a call that only a WebView2 controller
supports is a no-op for it.

- The swap chain is bound to the content window: release the presenter before
  that window is destroyed, including when a host destroys the window holding
  it (`WM_DESTROY`), and wait out an attach still running on the render
  thread. A swap chain made for a window that is gone poisons the shared GPU
  context for every page.
- Touch arrives as `WM_POINTER` (and the registered `LingXiaViewTouch` message
  automation posts); text input runs through IMM32 against the page's editor
  mirror.
- Press and focus-ring feedback are drawn into the frame, because a window over
  the swap chain cannot blend; recovery chrome is a native child panel.
- UI Automation answers `WM_GETOBJECT` with a fragment tree over the semantic
  projection. UIA may call from any thread: providers hold only the window and
  a node id and reach the page with a message sent to its thread.

## Hidden pages and returns

A hidden GPU page is frozen: no frames, input or semantics.
`lxapp::surface_retention` decides whether it keeps its surface (~32 MB each
at 1080×2504):

| Page | Keeps its surface |
| --- | --- |
| Tab page of the shown app | yes, except under memory pressure: a tab bar is cycled and would defeat a smaller recency cap |
| Just under the top of the shown app's stack | always, even under memory pressure: a back gesture reveals it |
| Other hidden pages on a stack | the three most recently shown |
| Off the stack | never |

Memory pressure releases every surface but the revealing page's;
backgrounding alone is not pressure.

A kept page shows at once only when `ViewHost::frame_is_current` holds; a
hidden page whose target or environment changed renders one catch-up frame as
background work. Otherwise the outgoing page stays on screen, frozen, until
the incoming one presents: at most 300 ms when its surface still holds a stale
frame; a first visit or a recreated surface waits for its first frame (a 2 s
watchdog, paused while the window is hidden, logs one that never does).

Returning to a kept page shows its last frame at once, then the new one, never
black or the bare canvas:

- Where the surface keeps that frame while hidden (Harmony's window, Android's
  retained layer) the surface itself shows it.
- Windows clips a returning page to one pixel until it presents (up to
  300 ms) and leaves the page it replaces on screen meanwhile, since an owned
  child window is shown before its swap chain is known to hold this page's
  frame at the current geometry.
- Where the surface cannot keep the frame (an Apple page out of its window,
  Android after `TRIM_MEMORY`, any host whose surfaces go with screen-off or
  the background) the host view reads the last frame back before trimming or
  suspending and shows that snapshot over the surface until a frame presents
  at the current size.
- Apple trims a page leaving its window instead of suspending it: the surface
  stays, parked at 1×1, with scene and image caches; targets and depth go.

A host showing a cover calls `ViewHost::cover`; the core asks for a frame and
calls `HostEvents::uncover` on the first one that reaches the screen at the
current size (a held frame does not). A page released by retention drops its
cover. Input during a cover reaches the kept scene, which has the cover's
layout; a page that changed while hidden is hit at its new layout before the
cover comes off. The reveal metrics carry `covers`, `snapshotShown` and
`lastCoverMs`, and the `cover`, `present` and `uncover` debug lines are the
presentation log: on a return, the cover never comes off before a frame at the
current size is shown.

## Verification

- `cargo test -p lingxia-view` covers layout through session behavior with real JS.
- `npm test --workspace @lingxia/view` covers authoring through resources; `generate.mjs --check`, drift.
- Android unit tests run the JNI adapter against the host-built `lingxia-view-android` cdylib.
- Device probes and renderer fixtures live in `lingxia-sdk/android/tests/gpu`.
  CPU and unit evidence is not device evidence; say which one a change has.
