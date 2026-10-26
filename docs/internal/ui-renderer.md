# GPU UI renderer

Invariants of the native page renderer. Authoring: [ui-authoring.md](ui-authoring.md),
[ui-styling.md](ui-styling.md); timing: [ui-animation.md](ui-animation.md).

## Ownership

`crates/lingxia-ui` owns the wire contract, View isolates, candidate
transactions, layout, shaping, scene building, interaction policy, semantics
and CPU/GPU painting. It has no lxapp, route, page-stack or WebView dependency.
`crates/lingxia-lxapp` owns page configuration, resource authority and Logic
bridging; `crates/lingxia/src/gpu_page` binds lxapp pages to platform page
shells (creation, live registry, dispose-once leases, connection-generation
gate, worker key) and `crates/lingxia/src/ffi/<platform>` only marshals. A page renders
natively exactly when its page config declares `"renderer": "gpu"`.

| Module | Owns |
| --- | --- |
| `protocol` | Wire decode/validation, generated ids (`protocol/generated.rs`) |
| `engine` | JS engine adapters and their capability list |
| `session` | `ViewSession`, `ViewHost`, lifecycle tickets, candidate prepare/commit, images, metrics |
| `layout` | Taffy styles, page root, virtual lists |
| `text` | Computed text style, Parley shaping, editing geometry, text scaling |
| `scene` | Node snapshots, display list, paint order, plots, vectors, shadows |
| `interaction` | Hit testing, scroll, reach, sliders, keyboard, pointer state |
| `semantics` | Composed control groups, accessibility projection, stable ids |
| `renderer` | CPU raster and wgpu/Vello presentation |
| `platform` | Adapter traits; platform adapters live in their own crates (`lingxia-ui-android`) |

## Wire contract

`packages/lingxia-ui/contracts/schema.json` is the single source for tags,
field ids, option enums, the CSS-named author style table (with the CSS
longhand each key accepts), element admission, wire admission (per-tag
children, content eligibility, required parent and fields, per-field tags) and
page limits (nodes, depth, animation and item-key bytes, image bytes, source
length). `contracts/generate.mjs` writes the Rust tables, lxapp's
`page/ui_limits.rs`, `wire.js`, `contract.js`, the authoring `.d.ts` and the
native-test runtime bundle; no consumer keeps its own copy. `--check` fails on
drift; a Rust test compares every compiled table with the schema. Field and
tag ids are append-only and dense; reserved fields are rejected by the decoder.

JS lowers author style to wire fields; native validation is independent and
re-checks every record. Never trust that a field reached native through the
runtime: decode bounds strings, numbers, JSON payloads and tag applicability.

Compiled modules start with `// LingXia UI module; {uiAbi, minField, requiredFeatures, compiler}`.
The banner refuses hosts whose ABI, `maxField` or features do not satisfy it,
before author code runs. Release hardening preserves that first line.

## Engines

| Engine | Platform | Candidate transactions | Bounded jobs (preemption) |
| --- | --- | --- | --- |
| QuickJS | Android, Windows, Linux | yes | yes |
| JavaScriptCore | Apple | yes | no |
| ArkJS | HarmonyOS | yes | no |

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

The microtask budget is 100 ms per turn. JS errors keep their message, code
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
- Fallback chains come from the platform font config (Android `fonts.xml`, via the
  adapter); `displayLanguage` is the shaping locale, so Han picks SC/TC/JP/KR faces.

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
- Linear gradients interpolate in sRGB or Oklab with premultiplied alpha.
- Backdrop blur/saturation is bounded (blur ≤ 64, saturation ≤ 16).
- Vectors are bounded path data (no SVG DOM): ≤32 paths, 16,384 elements per
  vector, 32,768 per frame, coordinates checked before and after the viewBox
  transform. `non-scaling-stroke` expands strokes after that transform.
- Plots (bar, line, pie) are leaves: TS owns axes, legends and tooltips.
  Geometry is bounded by a shared plot budget on both sides of the wire.

GPU presenters on one device share a `gpu::Context`; caches stay per presenter.
A driver crash during pipeline compilation cannot be caught: `renderer::sentinel`
records the adapter (backend, ids, driver, renderer release) before its first
compile and clears it after the first GPU frame presents. An adapter a dead
process left on record, or a known-crashing driver, is never used again; Android
falls back to GLES, then to the CPU renderer. Platforms supply only the store.
`env.renderer.fallback` is `"driver"` while a blocked adapter is in effect.
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
- Buttons and checkboxes may compose noninteractive content. They form one
  semantic node; its name is the explicit label or else the descendant text
  and image alternatives in order. Nested interactive controls are rejected.
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
Adapters convert OS events into host calls and execute typed effects; the host never
calls the adapter while holding its locks. Its tests use a fake adapter.

Android runs on `ViewHost`: `crates/lingxia-ui-android` implements the traits over
the Java `GpuPlatform` (vsync, posts, presentation, IME, accessibility, feedback
chrome) and exposes host calls as `GpuView` natives. Java converts events and
draws chrome; chrome geometry is `interaction::chrome` over the snapshot node.
Pages of one `appId:session` share a worker by key, so nothing slow runs on it:
decode and CPU raster use process pools (a newer CPU frame replaces one not started),
GPU attach its own thread, and a page whose attach is pending declines frames.
A hidden GPU page is frozen (no frames, input or semantics). `lxapp::surface_retention`
decides whether it keeps its surface: every tab page of the shown app plus the three
most recently shown others (~32 MB each at 1080×2504, so at most (tabs + 3) × 32 MB),
none under memory pressure (backgrounding alone is not). Android moves a kept layer off
screen (SurfaceView ignores alpha) and reports its insets as it will be shown. A kept page
shows at once only when `ViewHost::frame_is_current` holds; a hidden page whose target or
environment changed renders one catch-up frame as background work. Otherwise the outgoing
page stays on screen, frozen, until the incoming one presents: at most 300 ms when its
surface still holds a stale frame; a first visit or a recreated surface waits for its first
frame (a 2 s watchdog, paused while the window is hidden, logs one that never does).

New platform: adapter traits, text scaling and insets in the environment, `renderer` frames.

## Capability matrix

| Capability | Android | iOS | macOS | HarmonyOS | Windows |
| --- | --- | --- | --- | --- | --- |
| GPU page adapter | yes | no | no | no | no |
| Bounded JS jobs | yes | no (JSC) | no (JSC) | no (ArkJS) | yes |
| CPU raster (tests, previews) | yes | yes | yes | yes | yes |
| System text scaling | yes | — | — | — | — |
| Accessibility projection | yes | — | — | — | — |
| Hover state styles | no (reports a coarse pointer) | — | — | — | — |
| GPU driver crash sentinel | yes (Maleoon Vulkan → GLES) | — | — | — | — |

## Verification

- `cargo test -p lingxia-ui` covers layout through session behavior with real JS.
- `npm test --workspace @lingxia/ui` covers authoring through resources; `generate.mjs --check`, drift.
- Android unit tests run the JNI adapter against the host-built `lingxia-ui-android` cdylib.
- Device probes and renderer fixtures live in `lingxia-sdk/android/tests/gpu`.
  CPU and unit evidence is not device evidence; say which one a change has.
