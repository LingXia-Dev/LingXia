# UI renderer implementation

Image retry identity is `(src, retryKey)` (field106); fields107/108 hold load/error
callbacks. `ViewSession` owns settled outcomes and per-node delivery; adapters
acknowledge presentation before dispatch. Preserve settled state across ordinary
rerenders, release it on semantic removal, and reject stale transport tokens
atomically with bitmap/status binding. Android cancels queued/running authorized
loads when native request tokens become obsolete, releasing queue capacity;
the transfer future is dropped without changing host grants or cache ownership.

Virtual-list reconciliation consumes resolved content insets/size from layout,
including individual padding and borders; do not reinterpret author style there.
Font weight inherits with other text style fields and participates in shaping
and cache keys. Editor accessibility names use field41, independently of values
and placeholder hints.

`crates/lingxia-ui` owns schema validation, thread-confined View contexts,
state replication, Taffy layout, Parley shaping and CPU/GPU rendering.
It has no lxapp, route authority, page-stack or WebView dependency.

Flex basis is field99: auto, a bounded logical length, or a bounded percentage
string. Validate before layout and keep length/percentage distinct. Growth never
implicitly selects zero basis; omitted basis is auto. Taffy resolves basis in the
parent's main axis before distributing free space. Both axes and retained layout
invalidation must use the same record value. This changes the former implicit
zero-basis growth behavior; callers requiring it must author an explicit zero.

Shared interaction belongs in this crate: category/pie selection is available
through `PlotSnapshot::{hit_test,next_selection,selectable}`; use
`NodeSnapshot::hit_test_plot` when resolving a rendered node so content and
ancestor clips participate. Coordinates are logical pixels. These helpers do
not dispatch callbacks or establish frame freshness. Use
`ViewSession::select_plot` for point, next-item and explicit-index events; it
resolves the committed node and callback and rejects stale generations or
pending text geometry. Android forwards these events through `nativePlot`;
its Java adapter no longer computes plot angles, radii or category selection.

Editor focus paint overrides (field98) are bounded JSON field/value pairs for
color/background/opacity/borderColor/gradient/shadow/placeholderColor only. Validate
them using ordinary style validators before accepting the View record. Apply them
to a copy after editor reconciliation and before retained-layout comparison; keep
base records intact for blur. Visibility/autofocus transitions during composition
require another layout before presentation, including the retained-scroll path.
Use candidate Inputs state so a failed frame restores the previous focus/paint.
The same rules apply on disable/removal; no platform-specific style evaluator.

Keep platform adapters responsible for surfaces, event conversion, IME and
accessibility system APIs. Move reusable hit/selection/scroll policies and
resource request lifetime management into the shared core. Resource loading
authority and persistent caching stay in lxapp, connected through the lingxia
facade. Apple/Harmony GPU page adapters are not implemented by merely selecting
their JS engine or enabling a wgpu backend.

Related GPU presenters share a cloneable `gpu::Context` via `with_context`.
Compatible surfaces reuse its device/queue; scene, glyph, image and composition
caches remain presenter-owned. A resumed presenter rejects a different device
before replacing its surface, because its caches belong to the original device.
Android derives the context from a live presenter while holding the registry
lock, so concurrent first attachments cannot create separate domains. The final
presenter drops the context; no permanent process singleton retains devices.
Lock order is presenter registry, context, renderer registry, scene renderer;
never acquire these in reverse. Surface creation/configuration and render
upload/submit/present hold the context lock; do not invoke host callbacks under
these locks. `metrics.gpuDevices` and `gpuRenderers` report the domain's compatible
device and renderer counts, not texture memory or total GPU usage.

Retained scrolling accumulates a two-axis translation per ancestor. Each
region still owns one scalar offset, applied along its snapshot `axis`; translate
paint, control/content bounds and glyphs together while keeping the owner's
clip fixed relative to its parent. Reveal clips the target through each region
and commits candidate offsets only after the complete chain and composition
succeed. `scrollAxis` (field100) selects horizontal or vertical on `scroll` only;
omission is vertical, and virtual lists remain vertical. Taffy measures overflow
along that axis. Android locks drag/fling direction after touch slop and transfers
remaining distance only to ancestors on the same axis. Vertical gestures in a
horizontal region select a vertical ancestor. Accessible forward/backward use
the region's axis length; reveal accounts for pending offsets before committing.

Virtual-list row wrappers carry the same normalized semantic key as their JS row
scope; field15 (`rowIndex`) remains the measurement position. Do not use the row
index as the native identity when the author supplies a key: insertion/reordering
must preserve descendant identity. Leading/trailing layout spacers stretch across
the list's content width. A
zero-width spacer is excluded from Taffy's scroll overflow and can make distant
window reconciliation repeatedly clamp offsets and fail to converge.

Optional `list.itemKeys` supplies the complete ordered semantic row-key inventory
without calling offscreen row factories. Its length must equal `count`; keys are
unique strings whose normalized `key:<JSON string>` fits the256-byte native key
limit. A row's explicit JSX key must agree; omission inherits its supplied item
key. Snapshot the array with the candidate. Reindex retained row-local state by
key, retaining evicted rows and releasing removed keys only on successful commit.
Anchor seed caches cannot reuse an old row under a changed key.
Field102 carries this inventory as JSON. JS and Rust independently bound all
inventories on a page to256KiB UTF-8, reject malformed/duplicate keys, and verify
realized row wrappers against their field15 position. Existing lists may omit the
inventory; window-only records alone are insufficient to infer offscreen removal
for animation lifetimes. `ViewSession::prepare_view_animated` uses this inventory
for shared animation collection; Android preparation now calls it, with production
authoring gated until physical presentation/scheduling verification.

Pie `pieLayout.emphasisScale` is an optional 0..64 logical-pixel extension of
the selected slice's outer radius; omission is zero. Keep the inner radius,
angles and unselected slices unchanged. Paint and native hit testing use the
same expanded edge and retain the plot's content/ancestor clipping. Transport
the value in `pie_layout` snapshots so queued input invalidates when it changes.

Line plots accept `markerEmphasisScale` (field 94, 1..4, default 1). Scale only
the selected empty-circle path; keep its stroke width, center, line and area
unchanged. Category hit geometry is independent of marker size. Plot clipping
also applies to enlarged markers at the chart edges.

Plot `onSelect` receives an item index or -1 to clear selection. Android emits
-1 when tapping an empty part of a selected pie's viewport, and exposes the
standard accessibility Clear Selection action for selected interactive plots.
Use the normal generation/identity checks; stale dismissal must not clear a
new chart. Android also clears selected plots when a stationary content tap
lands outside their clipped viewport; scrolling and long presses must not
be treated as dismissal taps. Target collection remains in the Android adapter.

TSX/Tailwind authoring and native style lowering follow
[ui-styling.md](ui-styling.md). Read it before extending classes, theme handling
or computed styles; Tailwind compilation stays outside the View isolate.

`vector` (tag 19) is a decorative leaf that may be composed inside a button.
Field 80 carries JSON `{viewBox, paths}`; each path has SVG `d` and optional
`fill`, `stroke`, `strokeWidth`. Default viewBox is `[0,0,24,24]`, default fill
is inherited `currentColor`, and default stroke is `none` with width 1. Colors
are hex, `currentColor` or `none`. Size defaults to 24 logical pixels; fit the
viewBox centrally with preserved aspect ratio inside border/padding and clip
to its viewport. Paths fill nonzero; strokes use butt caps and miter joins.

Paths optionally accept `strokeDasharray`: at most 16 positive lengths in
1/64..1024 viewBox units; omission or an empty array is solid. Odd arrays repeat
twice, following SVG. Before stroke expansion, reject estimated subdivisions
(`perimeter / shortest dash + path elements`) above 4096; expanded outlines
still share the normal vector/frame budgets. By default, dashes scale with the
viewBox like the rest of the path.

Vector `fit` defaults to `contain`; explicit `fill` scales axes independently.
Path `vectorEffect: 'non-scaling-stroke'` expands its stroke after that transform,
keeping width and dash lengths in logical pixels. Other paths keep normal
viewBox-unit strokes. Preserve fill/stroke/path order. Deferred expansion must
recheck subdivision, the 16,384-element vector and 32,768-element frame budgets
after layout, before painting. Reject transformed stroke spans over 32,768 and
coordinates beyond the renderer's 1e12 bound before stroke expansion.
Pixel 3 verifies chart grid dash/gap pixels across the line in light/dark;
CPU raster tests verify fixed width and spacing at two stretched viewport sizes.

Parse bounded path data through Kurbo, expand strokes into shared filled
outlines, then paint those paths in CPU, sparse GPU and Vello backends. Retained
scrolling translates both paths and their clips. This primitive has no XML,
external resources or SVG DOM. Reject unknown properties and nonfinite geometry.
Bound source JSON to 32 KiB, paths to 32, each path string to 16 KiB and 1024
parsed elements, coordinates to +/-16384, stroke width to 0..1024, and positive
viewBox dimensions to 1/64..16384. Count input and stroke-output elements against
16,384 per vector and 32,768 per frame. Apply the coordinate budget again after
the viewBox transform, before paths enter a display list.
Pixel 3 verifies vector fill, hollow strokes, inherited color updates, layered
paths, centered viewBox fit and retained scroll pixels. API reference icons and
Home's filled success icon pass navigation, theme, language and rotation checks.

Width, height and their min/max constraints accept logical lengths or percentage
strings. Keep existing numeric wire fields unchanged; append percentage fields
74–79 in width/height/minWidth/minHeight/maxWidth/maxHeight order. Their values
are percentages (72 means 72%), finite and within 0..16384. Reject records with
both forms of one dimension. Resolve percentages in Taffy against the parent's
content box, never against the viewport in JS. After layout, reject nonfinite
geometry or coordinates beyond 1e12 in either logical or device units: nested
percentages can multiply otherwise bounded authored dimensions.
Pixel 3 verifies fractions, min/max constraints, padded/bordered parent sizing,
live parent resizing and complete right-aligned IPv6 wrapping. Device result
rows use the reference page's 72% value limit; Home, API and Device navigation,
theme and orientation regressions pass on the new native build.

`paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft` are nonnegative
logical lengths (wire fields 59–62). Each overrides uniform `padding` for that
edge; omitted edges retain the primitive's existing padding default. Use the
resolved Taffy edges for text origins, clipping, caret/hit geometry and control
minimum widths. Search adds its icon reserve after resolving left padding.
Taffy leaf measurement receives border-box known dimensions and content-box
available space. Use available width for text wrapping; known width includes
padding/borders and can undercount lines before painting.
Padding changes invalidate retained layout. Tailwind shorthands lower to all
affected edges in cascade order; inline padding overrides class-derived edges.
Buttons and single-line input/search center their text in the padded content
height; textareas remain top-aligned. Keep the text offset separate from the
content viewport, and apply it to paint, caret/selection and hit-test coordinates.

`wordBreak` (field 63: `normal`, `break-all`, `keep-all`) and `overflowWrap`
(field 64: `normal`, `break-word`, `anywhere`) inherit from native parents.
Pass them into Parley before shaping; cache keys include both resolved values.
Measurement and painting use the same layout policy without modifying text.
Pixel 3 verifies a previously overflowing IPv6 address wraps inside the result
card with its accessible text unchanged, alongside Device navigation, network
listener cleanup, orientation and dark-theme regression checks.

`textAlign` (71) inherits through layout parents and accepts `start`, `end`,
`left`, `center`, `right`, and `justify`. Buttons retain their centered default;
an explicit value overrides it. Apply alignment after breaking lines, before
glyph painting and input geometry. Cached shaping remains alignment-neutral.
Single-line editors disable wrapping and align within at least their full text
width so overflowing text retains a nonnegative origin for caret scrolling.
Pixel 3 verifies multiline glyph edges across all six alignments, physical
left/right versus RTL start/end, explicit overrides, cache reuse, and actual
typing after taps in right-aligned and centered editors.

Solid borders use `borderWidth` (65), `borderColor` (66), and top/right/bottom/left
edge widths (67–70). Widths are nonnegative logical lengths; explicit edges
override the uniform width. The default color and `currentColor` resolve to the
node's inherited text color. Taffy owns border sizing. Text/editor content
origins include the resolved border edges.

Paint the border as one outer-minus-inner path so translucent corners blend
once. Unequal widths produce elliptical inner corners. Preserve their radii
in retained scroll clips and Android hit/highlight geometry. Background and
border paints stay outside the inner foreground clip; editor foreground
replacement must preserve both clip layers. Image, slider and plot content
uses the inner rectangle, transported as `control_bounds` for Android input.
Pixel 3 checks verify translucent corner pixels, asymmetric scroll clipping
and hit rejection, currentColor, slider/plot selection, image borders, and editor
foreground plus actual typing. Device light/dark cards, long IPv6 wrapping,
orientation and API accordion/navigation regressions pass. This covers solid
borders; full showcase visual parity remains pending.

`boxShadow` (73) is `none` or bounded JSON with up to eight outer shadows.
Each layer has optional `offsetX`, `offsetY`, `blur`, `spread` and `color`;
lengths default to zero and color to inherited `currentColor`. Bound offsets
and signed spread to +/-1024, blur to 0..256 and the encoded array to 8 KiB.
Reject inset and unknown properties. CSS's first shadow paints on top.

Share [CSS spread geometry and owner knockout](https://www.w3.org/TR/css-backgrounds-3/#shadow-shape)
between all backends. Gaussian sigma is half the CSS blur radius; zero blur uses a crisp
rounded path. Uniform corners use native blurred-rectangle paints. Independent corners use
a cached premultiplied mask shared by CPU, sparse GPU and Vello. Shadows paint before backgrounds and never change layout, hit bounds
or scroll extent. Cull by shadow extent, allowing a shadow to remain visible
when its owner has scrolled outside the viewport. Ancestor clips and opacity
still apply. Editor outer clips begin after shadow paints; foreground replacement
must preserve their commands and nested content clips.
Pixel 3 checks cover translucent-owner knockout, layer order, external blur,
Tailwind color override, reset/reuse and real editor typing. Existing bordered
editor, scroll clipping, slider and plot probes also pass after the clip change.

`style.opacity` is a finite number in `[0, 1]` (default `1`, wire field 58).
Composite each node and its descendants as one group; applying alpha to each
primitive separately changes overlapping colors. Nested groups multiply opacity;
adjacent siblings with equal opacity remain separate groups. CPU and both GPU
encoders consume the same layer commands. Retained scrolling preserves these
boundaries, and opacity changes invalidate paint reuse. Opacity affects painting
only: it does not remove layout, hit targets or accessibility nodes. This is the
compositing foundation; transition timing and lifecycle remain unimplemented.
The next animation implementation follows [ui-animation.md](ui-animation.md),
including presentation-bound timelines and cancellation of hidden-page wakeups.
Pixel 3 cold embedded APK checks cover overlap, distinct sibling groups, opacity
updates, scrolling, rotation retention and light/dark pixels. Chart selection
regressions pass in both themes; these are correctness checks, not FPS evidence.

`useRequest(name, method, params)` declares a scoped unary page request. It starts
after commit, repeats when the committed action identity or JSON parameters
change, and exposes `pending`, `value`, `error` and event-only `reload()`.
Discarded candidates issue no request. Removal, reset and replacement ignore
late results; ordinary unary calls already sent to Logic still finish there.
Streams and host routes are not accepted. Keep startup queries here rather than
calling actions during render or putting mutable request state at module scope.

`useStream(name, method, { onData })` owns a manual page stream. Call `start(params)`
or `cancel()` from events and read `status`, `error`, `result`. Restart cancels
the previous producer; scope removal and bridge replacement invalidate callbacks.
Only committed callback changes take effect. A throwing `onData` fails and
cancels that stream. Keep handles out of module globals and `useLocal` JSON state.

Android queued clicks and held presses may follow an unchanged control into a
new presented frame when unrelated nodes update. Rebind by identity only if the
entire control snapshot except its regenerated callback handle still matches;
movement, clipping, label/state changes, removal and page/scale changes cancel.
Editable text retains its stricter displayed-frame contract.

Buttons and checkboxes may contain noninteractive layout, text, vector and image
children. Composed controls require
an explicit nonblank `accessibilityLabel`; do not mix inline text with children
or nest interactive controls, scroll containers or plots. Both JS and the wire
decoder enforce this contract. Composed checkboxes replace the default indicator
with their children while retaining checked state and the boolean change callback.
Android exposes one accessible control and routes
the entire row to its callback. Descendants still paint and lay out normally;
their content fingerprint participates in press rebinding so changed child
content cancels a held gesture even when the outer bounds remain unchanged.
Pixel 3 verifies description-area taps, one accessible control per composed row,
accordion retention through native Back and tabs, and light/dark layout. These
checks do not establish screen-reader speech or full showcase visual parity.
Pixel 3 also verifies composed checkbox checked semantics, one accessible node,
label-area taps and Tab/Space/Enter activation, plus wrapped ToDo rows in a wide
viewport. ToDo temporarily disables controls while saving; keyboard focus must
be reacquired afterward. This does not establish uninterrupted keyboard focus.

Image `alt` (wire105) supplies the shared semantic label. An empty string marks
decoration and excludes the image from platform accessibility projections;
omission preserves legacy exposure. Descriptions are bounded to 1024 UTF-8 bytes,
cannot contain NUL or consist only of whitespace, and do not affect layout or
painting. Composed controls still own their descendants' semantics. A hidden
decorative image's `testId` does not create an accessibility node for automation.
Android projects descriptions as ImageView content descriptions; this alone
does not establish screen-reader speech or support on other platforms.

System text scaling belongs to layout, not the GPU transform. Native environment
`textScale` is a bounded, monotonic list of `[authorSize, logicalSize]` samples
from zero through 16384; omitted mapping means identity for existing hosts.
The shared layer interpolates samples, scales inherited font sizes exactly once,
and scales absolute line height and letter spacing by the font's effective ratio.
Density still converts all logical geometry to surface pixels. A mapping change
invalidates retained layout and reveals the active editor; invalid mappings do
not replace the current environment. Samples do not enter the public View env.

Android samples `TypedValue.applyDimension(SP, ...)` once per configuration,
at unit intervals through 256 and doubling intervals above that. This preserves
the platform's current piecewise conversion and bounds transport; future curves
with additional breakpoints between samples are interpolated. Do not replace it
with a `Configuration.fontScale` multiplier: Android 14 scaling is nonlinear.
Other adapters must supply their native mapping before claiming system text
scaling support. Fixed author heights remain fixed; product controls should use
minimum height and content padding when their text must grow.
Single-line input/search defaults provide a minimum height rather than fixing
the content height. Explicit author heights retain their existing constraints.
Text scaling also invalidates offscreen list measurements while retaining the
visible-row anchor; clearing only the retained display list is insufficient.
Android's native bottom tabbar reserves enough height for the platform-scaled
label and icon. Its configured dimension is the lower bound; both the item
container and host reservation use the calculated thickness, including the
opaque border. Default 100% text keeps the existing 64dp strip. Environment
insets continue to come from actual native layout, for both GPU and WebView.

Android scanner and media-picker overlays register Back callbacks against their
view lifecycle. The top picker cancels before the scanner; neither forwards Back
to the covered GPU page. Removing the overlay removes its callback.

Android GPU connection bootstrap reports `UI_BOOTSTRAP_TIMEOUT` after five
seconds without both bridge ready and an initial state. A visible waiting
message replaces silent blank content; it blocks stale surface interaction.
This is a recoverable deadline, not connection revocation: current-generation
readiness removes the message. Replacement, park, reload, error and disposal
cancel the previous timer; old-generation updates cannot clear the new timer.

Android candidate failures retain the committed surface and expose a host-owned
Retry action. Retry requests a rebuild in the same View session; it must not
call `GpuView.reload()` and bypass Logic bridge reconnection. Only a successfully
presented rebuild clears this feedback; a caret/scroll repaint is insufficient.
Reload, park, connection replacement and disposal revoke old recovery actions.
Compatibility failures request an app update instead of offering a retry loop.
Execution timeouts stop input and scheduling against the interrupted isolate.
Their explicit Reload action resets View locals through the host's current
endpoint and generation, then rebinds Logic through `PageInstance`. Old native
callbacks cannot reload a replacement connection. Late click/plot/reach errors
also carry their captured page generation. Standalone Logic-free hosts reload
their own View. Creation failures before a native endpoint exists remain the
page-creation owner's responsibility.

`plot` is a leaf with `kind` (`bar`, `line`, `pie`), finite `values`, hex `colors`
and a required `accessibilityLabel`. Its assigned width/height define the plot
area; labels, legends and selection belong to TS components. Wire tag 18 and
fields 51–53 are append-only; field 41 carries the accessible summary.
Both CPU and GPU consume the same bounded rectangle, stroke and area geometry, including
butt-capped donut slices drawn as continuous paths to avoid tessellation seams.
Validate 1–128 values, 1–16 colors and the shared 16,384-unit plot geometry
budget before painting. Pie values are nonnegative.

Pie plots optionally accept `pieLayout` (field 88, bounded JSON) with `center`
(two fractions of content width/height), `innerRadius` and `outerRadius`
(fractions of half the smaller content dimension). Require finite 0..1 values
and inner < outer; reject unknown properties and layouts on non-pies.
Omission preserves the centered legacy donut. Publish the resolved layout in
plot snapshots; Android hit testing uses it and rejects holes/outside radii.
Layout changes invalidate queued selection just like data/geometry changes.

Bar/line plots optionally accept paired `domainMin`/`domainMax` (fields 85/86).
Both must be finite within +/-1e9, strictly ordered, and contain zero and every
value. Reject partial domains and domains on pies. Omission keeps the data-derived
range. Preserve f64 bounds through decode and snapshots; use them for geometry
so TS axis labels and native samples share a scale without altering source data.
Bars use four-pixel top-left/top-right corners and square bottom corners,
including negative bars, matching the reference's `[4,4,0,0]` geometry.
Clamp the radius to half the width and the full height for short bars. Encode
each as one shared filled path and count its eight elements in both plot budgets.
Pixel 3 verifies the top arc and both square bottom corners in light/dark;
CPU pixels cover positive and negative bars, with path-bound tests for tiny bars.

Line plots accept optional `smooth` and `area` booleans (fields 54/55).
Smoothing uses eight monotone interpolation segments per interval; it preserves
samples without overshooting adjacent values. Area uses the same curve and a
closed gradient fill, fading the primary color toward zero on either side of
the baseline. Translate the gradient with the retained path when scrolling.
Count interpolated points, area closure and markers in the shared geometry
budget on both JS and native sides. Line markers use two filled circles: an
eight-pixel colored outer circle and four-pixel white center, matching the
reference's six-pixel emptyCircle with a centered two-pixel stroke. Count both
circles per marker; shrink them together when plot padding is constrained.
Pixel 3 verifies marker center/ring colors in light and dark themes; the previous
filled marker fails the same center-pixel assertion. CPU raster tests also check
that pixels outside the circular corner retain the underlying background.
These flags do not change accessible data.
The independent embedded Android showcase verifies gradient pixels, streamed
bar/line/donut results, legend toggling and selection retained across rotation.
Chart animation and exact axis styling remain pending.

Plot selection uses optional `onSelect(index)` and controlled `selectedIndex`
(fields 56/57; `-1` means none). Dispatch only committed numeric callback handles
and accept integer indices within the current series. Android category selection
uses the painted padding; donut selection excludes the hole and zero slices.
The presented plot snapshot owns hit geometry and selection state. Rebind held
or queued clicks only when geometry, clipping, data and state still match;
regenerated callback handles alone may differ. Scrolling cancels a plot press.
TS owns tooltip content. Android exposes explicit item selection actions for
accessibility and cycles items when activating a focused plot.
The cold embedded Pixel 3 APK verifies physical bar/line/donut taps, TS
selection hints, explicit accessibility item actions, donut-hole rejection and
scrolling without selection changes in light/dark. Rotation retains selection.
These checks do not establish screen-reader speech or hardware-keyboard parity.
When only `onSelect` handles change, reuse retained layout and paint while
rebinding the snapshot handle. Series, selection and style changes still
invalidate that reuse. A failed candidate must retain the last committed
callback. Tests compare reused and fresh pixels and assert zero layout passes
for unchanged bar/line/pie plots; this is not a device frame-rate claim.

`useLeaveGuard(name, enabled)` owns a committed, scoped leave confirmation.
Read `pending` to show the question; call `answer(boolean)` from an event.
Use `request("back" | "home")` for user-facing navigation controls. Ordinary
Logic navigation retains its programmatic behavior. Removing or disabling a
guard cancels its pending question; bridge replacement invalidates old answers.
The runtime registers guards only after a successful render commit. Android
delivers native leave requests through the current GPU connection generation.

Android surface containers resolve a page instance to its renderer-owned View.
Mount and visibility events belong to the page instance for both renderers;
GPU pause/resume follows container visibility. Closing a page surface lets the
native page lifecycle dispose its renderer. Only unmanaged URL WebViews are
destroyed directly by the container.
The top visible modal excludes lower root children from accessibility and
restores their original importance values when hidden or closed. Automation
must disable `FLAG_INCLUDE_NOT_IMPORTANT_VIEWS` when asserting this boundary.
Surface geometry must also follow measured overlay size changes: rotation
insets can arrive before the new width is available. Relayout the existing
page View so local state and bridge ownership survive the resize.

Read the primitive-array length and enforce the shared 100,000-value wire limit
before fetching any elements into native memory. Convert one JS value at a time
without retaining an intermediate vector of JS handles. Keep primitive type,
finite-number and full native schema validation; getter errors must propagate.
Pixel 3 injection verifies an oversized description fails before its throwing
first-element getter runs; source reload restores normal search and input behavior.

## Next implementation gates

Reference [NSH at f64ea2bd93](https://github.com/longbridge/nsh/tree/f64ea2bd93ee38feef0711e6ae2d5c386dc9618a),
especially its GPUI text input model/layout/platform split. Its Android input
uses native EditText; its custom GPUI editor is the relevant rendering reference.
Keep Vello/Parley, the View/Logic split and existing authorization boundaries.

Proceed Android-first in separately verified renderer commits:

1. Derive caret, selection and composition geometry from the displayed Parley
   layout, mapping logical UTF-16 offsets through secure and single-line display.
2. Keep the caret visible and blink only while focused, without an idle frame loop.
3. Add selection gestures, clipboard and bounded undo; verify CJK composition,
   emoji and keyboard lifecycle on the physical device.
4. Measure JS, layout, shaping, queue and presentation costs separately, including
   P95 and memory. CPU timing is not GPU timing; compare identical workloads on
   the same device before changing rendering or reactive-update architecture.

NSH's desktop, emulator and headless results do not satisfy our device gates.
The product gate remains a GPU showcase verified on Android; showcase edits may
stay local while renderer milestones are committed. Other platform gates remain
required for cross-platform completion.

### Android showcase parity gate

Use the existing React showcase as the functional inventory. Rewrite its Views
with the UI language and retain Logic/API behavior. A single GPU playground does
not satisfy this gate. Exclude the native-component gallery and its video,
picker, navigator and media-swiper demonstrations. Keep ordinary media APIs,
system dialogs, device APIs and file pickers in scope; calling a native API does
not make a page a native-component demonstration.

Use the independent `examples/lingxia-showcase-gpu` host and its `lxapp/`.
Keep `examples/lingxia-showcase` as a read-only functional reference. The GPU
Android package is `com.lingxia.example.gpu.dev`, with separate app storage;
results from the earlier host do not count as verification of this package.
Keep Android automation probes and their drivers in
`lingxia-sdk/android/tests/gpu/`, outside examples and bundled resources.
Keep shared interaction semantics tests in `crates/lingxia-ui`; the showcase
retains page fixtures and its parity checklist.

Track these workflows in the local GPU showcase:

- Home: greeting, appearance, display language, version and network information.
- ToDo: add, toggle one/all, all/active/completed filters, delete, clear completed,
  storage restoration and empty states.
- API navigation and UI feedback: navigation/back, toast, modal, action sheet,
  page chrome, pull-to-refresh, leave guard and feedback.
- Device/system: device information, Wi-Fi, location, system settings, cache,
  notifications and capability-dependent controls.
- Data/integration: file, clipboard, sharing, cloud, stream, channel and bridge
  regression pages. Stream/channel transport alone does not establish page parity.
- Media APIs outside embedded native components; desktop surface behavior stays
  in the cross-platform inventory but is not an Android device acceptance claim.

For each migrated workflow, verify real actions and results on physical Android,
navigation and recreation, persistence where applicable, keyboard/back behavior,
portrait/landscape, light/dark and failure/cancel states. Keep a per-workflow
record of pending checks. Preserve the matched-workload performance and GPU
automation gates; functional parity does not replace them. Commit renderer work
by verified milestone, leaving showcase source and probes uncommitted.

The local five-page bundle now separates the product Home, ToDo, Clipboard and
Share Views from the renderer playground. Pixel 3 checks pass after repacking the
APK, disconnecting dev transport and cold-starting: greeting/flush, product
language/appearance, page retention, task actions and cold-process storage,
keyboard submission, clipboard text/image round trips, sharing-sheet opening and
file-picker cancellation. These verify the corresponding GPU paths, not full
showcase parity; remaining workflows and failure cases stay in the local checklist.

The independent ten-page Android APK passes Home, ToDo and Clipboard/Share
regressions after disconnecting dev transport and cold-starting. Device/System
checks cover physical screen rotation, network-listener cleanup, host identity,
settings, capability gating, cache and notification schedule/cancel. Wi-Fi checks
cover module/listener lifetime and scan results. Location verifies visible error
and reset only; a successful fix remains pending. Keep the Device page's
orientation inherited: explicit page `auto` overrides application API requests.
These checks establish the separate host's baseline, not full product acceptance.

The independent File page also verifies managed chooser cancel/selection,
download/fetch failure feedback, pause/recreation/resume, durable fetch/write and
native PDF preview/back on Pixel 3. A local HTTP fixture supplies the document and
failure responses while dev transport is disconnected. View state and pending
action completion survive the GPU-to-native viewer transition. Native Office
handlers and remaining file input/presentation cases are not yet verified.

## Runtime contracts

GPU replaces page content previously rendered by WebView. Host navigation bars,
tab bars, page stacks and navigation actions remain shared native infrastructure.
Preserve each reference page's `navigationStyle` / `navigationBar` configuration
and host tab configuration; do not hide native chrome to redraw it in TSX.
A page explicitly using custom navigation may author its own content header.


Android GPU pages share the host pull-to-refresh indicator and page-instance
checks with WebView pages. Feed surface touch events to the host helper, cancel
GPU press/scroll/slider ownership when the helper claims a pull, and swallow the
rest of that gesture. Selection-handle, long-press and claimed slider gestures
keep ownership. Determine the top boundary from the touched scroll region and
its ancestors using desired UI-thread offsets, including unpresented deltas.
Dragging through scrolled content does not count that consumed distance toward
the refresh threshold. Cancellation and extra pointers cannot trigger refresh.

Programmatic start/stop must compare the request's webtag with the current page
regardless of renderer. Switching pages aborts the old indicator and detaches its
touch callback. Activity teardown clears only its own callback; a newer activity
may already have attached the same retained page. Pixel 3 verifies API start/stop,
physical pull, short/horizontal/canceled gestures, scrolling and leaving while
refreshing. Preserve the helper-owned page offset after host layout updates;
rotation must not leave Logic refreshing with the native indicator covered. Device
checks assert the exposed indicator space and restored page position, and inspect
the rotated screenshot. Robolectric covers nested/pending scroll boundaries and
slider behavior.

`useChannel(name, topic, { params, onData })` declares a page-channel resource
owned by a scoped local cell. It opens only after its candidate commits, outside
render; abandoned descriptions never open connections. Reuse the connection
across redraws and retained list-row windows. A committed topic/JSON-parameter
or bridge replacement closes the old connection before opening its successor.
Update the data callback only on commit. Removing the owning cell or resetting
the View disposes the resource; a late acknowledgement closes without delivering
data to the removed owner. Explicit close stays closed until `connect()` or a
committed configuration replacement. Resource operations are forbidden during
render, like raw channel operations. Components remain pure descriptions; this
is a runtime-owned channel resource, not a general component effect hook.

Android drains requests after a render commit as well as after input/bridge
callbacks. The post-render drain requests another frame only for pending local
updates; unconditional invalidation would create an idle rendering loop. Keep
resource connection state authoritative across a discarded local-state candidate.
Tests cover failed candidates, rejection/retry, late acknowledgement, scoped
ownership, parameter replacement, list refinement and quiescent redraw.
Pixel 3 offline APK acceptance verifies automatic entry, live subscriptions
retained across rotation, manual reconnect and page exit/reentry. After explicit
close, both ticker data and the app's GPU presentation count stop increasing.

Android host tab-bar occlusion is measured after layout from the visible native
bar and current GPU page rectangles. Pass physical-pixel edge overlap through
`GpuPage` to `GpuView`; merge it with system-bar/cutout overlap using maxima,
then convert to View points. Already reserved host margins must not be counted
again. Hidden or non-overlapping chrome clears its contribution. Observe native
pre-draw geometry so show/hide, page changes and rotation update the environment
even when the GPU surface size stays unchanged; unchanged insets request no frame.
This supplies edge insets, not the proposed full chrome-rectangle environment or
modal focus/back arbitration.
Pixel 3 offline acceptance verifies bottom-tab show/hide viewport restoration,
portrait/landscape and return navigation. The badge-clear button stays wholly
above the measured native tab and clears the badge through physical coordinate
taps. Side-position tab bars and the full proposed chrome API remain separate gates.

Colors accept theme tokens, `#rrggbb` and `#rrggbbaa` (alpha last). Six-digit
colors and tokens stay opaque. Validate every hex digit before accepting a
candidate; shorthand, signs and malformed lengths are rejected. Retain straight
RGBA bytes in paints, then pass alpha to CPU Vello, sparse GPU and legacy Vello
scene brushes. Do not premultiply the color before constructing those brushes.
Color alpha affects the individual paint, not the subtree's opacity, layout,
hit testing or accessibility. Input selection contrast uses RGB only.
CPU tests cover ordered blending, fully transparent text/search strokes and
strict color decoding. Pixel 3 screenshots verify 20 backdrop samples of
`#00000080` over the page's `#f6f7fb` background in the independent GPU APK,
with dev transport disconnected. This does not supply modal focus/back policy
for `stack`; those remain separate from color support.

`text.lines` optionally limits visible text to 1..1024 complete Parley lines.
Omission is unlimited; the cap is a text prop, append-only field 50, validated
independently in JS and native decoding. Measure only visible line extents and
emit only their glyphs, preserving the complete source string in snapshots for
accessibility. This line-boundary truncation adds no ellipsis. Explicit sizes
still take precedence. Rebreak the full cached unbroken layout at the current
width before applying the cap; never cache a truncated source or editor layout.
Pixel 3 validates two-line geometry, expansion/collapse, complete accessibility
text, identity across rotation and light/dark presentation. The same probe passes
with dev transport disconnected after cold start from the embedded APK; input
UTF-16/focus/stale-reference and stacked-hit regressions remain green.


`stack` (append-only tag 17) places children in the same layout area and paints
in source order. Its natural size is the largest unshifted child extent, with
normal padding/min/max constraints. `style.offset: { x?, y? }` shifts a direct
stack child from its laid-out position without reflowing siblings; omitted axes
are zero. Lower to signed finite offsetX/Y wire fields 48/49 bounded to 16384dp.
Reject offsets at the root or below any other parent, including through a
component; reject author use of the private wire names. Native decoding repeats
parent/range validation. Share the same shifted geometry for painting, clipping,
snapshots and input. Stack is not an overlay and adds no modal focus/back policy.
Pixel 3 checks cover frontmost overlap taps into Logic, accessibility activation,
rotation with stable identity and light/dark theme captures. Keep labels readable
and at least 44dp of an interactive back card exposed in a stacked product layout.


Window refinement shares ordinary resolved JS nodes. Copy only list-bearing
paths and keep each candidate's list windows, row caches and state maps separate;
encoding must not mutate shared ordinary nodes. Nested-list failures/discards
must preserve committed windows and callbacks. A full render still resolves
fresh nodes and callbacks. Pixel 3 search/window recovery and idle workloads
pass; list CPU P95 29.395 -> 29.092ms is too small to establish a speedup.

When a changed list anchor lies outside its previous window and count is
unchanged, the first candidate may reuse the old row descriptions for native
anchor reconciliation. Do not publish this intermediate candidate. Clear its
seeded row cache before refining the destination so even overlapping rows
resolve against current props. Keep row locals and failed-frame rollback intact.
An empty placeholder instead of old geometry causes extra layouts and is not
used. Count changes and anchors within the window keep ordinary row evaluation.
Pixel 3 sampling with the packaged JS checked against the rebuilt bundle records
list View evaluation P95 12.493 -> 9.287 ms and total CPU preparation
29.396 -> 27.965 ms (60 frames each, thermal status 0, unchanged layout-pass
distribution). Evidence: `target/gpu-anchor-retained-performance-evidence`.
Search, empty results, keyboard, theme and rotation regression passes.
This reduces redundant row work; it does not establish 60 fps or React-relative
performance. The default parallel Rust suite hit an existing editor budget
timeout; its serial rerun passed, including the anchor rollback/layout test.


`grid` uses row-major auto-placement into 1..64 equal columns. `columns` is a
required integer prop, append-only tag 16/field 47; reject it on other tags and
validate it independently in native decoding. Taffy tracks use minmax(0, 1fr)
so long text cannot enlarge one column. Gap applies between rows and columns;
padding and explicit child sizes retain existing semantics. Rows size to content.
Grid `align`/`alignSelf` align items vertically; `justify` distributes tracks
horizontally. Keyed children retain their identities when column counts change.
Nested scroll/list children still use the shared clipping and reveal contracts.
Pixel 3 checks cover portrait single-column/wide two-column cards, stable keyed
identity across rotation, accessibility and physical touch activation into Logic,
and light/dark clipping within the containing scroll region.


`slider` is a controlled numeric leaf: value/onChange or a live Local<number>
binding, min/max/step (defaults 0/100/1), enabled and a required accessibilityLabel.
Tag 15 and fields 42..46 are append-only; field 41 also labels sliders. Native
validation requires finite values within +/-1e9, min < max, a positive resolvable
step no larger than the range, and at most 1e6 intervals. Clamp and snap displayed
values and proposals to the nearest step or final endpoint; reselecting the
current value does not call onChange. Numeric callbacks reject strings, booleans,
non-finite values and stale generations. Disabled controls expose no change handle.
Snapshots publish the normalized range and a relative track inset; shared CPU/GPU
paint uses a 24dp thumb, 4dp track and default 48dp height. Rendering a failed
candidate preserves committed values and callbacks. Android resolves horizontal drags
against the presented track; vertical motion yields to the containing scroll region.
Keep one numeric callback in flight and only the latest pending desired value.
Resolve its identity/range against the next presented frame before using a fresh
callback handle; frame preparation waits for the in-flight callback. Pause, blur,
page replacement and gesture geometry changes invalidate queued proposals.
Tab reaches enabled sliders; arrows step, Home/End choose endpoints. Decrementing
an unaligned maximum reaches the last full step first. SeekBar accessibility nodes
publish the presented range and support set-progress and increment/decrement.
Emit range-change events only after presentation; disabled sliders reject changes.
Pixel 3 verification covers continuous drag versus page scrolling, endpoint steps,
keyboard navigation, stale/disabled rejection, light/dark rendering, background
resume and rotation with stable range identity. TalkBack speech remains unverified.

`search` is a single-line editor with the same value/bind, committed onInput,
onSubmit, placeholder, autofocus and enabled contracts as input. It owns the
search keyboard configuration and rejects secure/keyboard overrides. Wire tag
14 is append-only; native validation rejects children and password fields, and
defaults absent keyboard metadata to search. Reserve 32dp before editable content
for the magnifier; measurement, hit testing, selection, caret reveal and clips all
use that content origin. Keep icon paints outside the editable paint range so
selection and horizontal text scrolling cannot cover them. Android uses the shared
EditText accessibility node and IME_ACTION_SEARCH; submission keeps editing active.
Filtering, result state and a separate clear action belong to the author view. When
list count shrinks, accept a still-valid previous-count native window and seed a
bounded in-range row before native measurement refines it. Refined windows must
obey the new count; failed presentation preserves the previous committed window.

`checkbox` and `switch` are labelled leaves with a controlled boolean `value`,
`onChange(nextValue)` and optional `bind={useLocal(...)}`. Bind accepts a live
boolean local cell and excludes explicit value/onChange. Activation uses the
existing generation-scoped callback path; it proposes the inverse displayed
value and does not mutate native state. Disabled controls expose no activation
handle. Keep wire tags/fields append-only and validate them natively. Layout
reserves indicator space before shaping the label and defaults to at least 44dp
height. Checked geometry uses the same display list for CPU and GPU. Android
exposes CheckBox/Switch roles and checked state from the presented snapshot,
including disabled controls; empty labels do not hide the accessibility node.
After presentation, content-change events invalidate the virtual subtree so
Android does not retain stale checked/text/geometry data. Keyboard focus changes
emit TYPE_VIEW_FOCUSED with the stable virtual ID.


`radio` takes 1..64 `{ value, label, enabled?, testId? }` options and one
controlled string `value`/`onChange(value)` or `bind` to a live string local cell.
`accessibilityLabel` names the group. Keys are nonempty unique option values;
reordering preserves identity. Activating the selected option is a no-op; an
unknown/absent value selects none. A disabled group disables every option.
The View expands groups into private `radio-option` leaves; reject direct author
use and reject other child/parent shapes natively. Native validation independently
enforces the option limit, unique keys and at most one checked child. Snapshots
carry each option's group identity. Circle paint, label shaping and target sizing
share the checkbox rendering path. Android exposes RadioGroup/RadioButton virtual
parents and single-selection collection metadata. Tab enters the selected enabled
option (otherwise the first enabled one); arrows wrap within the group and skip
disabled options. Tab leaves the group rather than visiting its remaining options.
Show-on-screen accessibility actions reveal semantic nodes, including ordinary
buttons and text, without activating them or changing input focus. Disabled
controls can still be revealed; removed identities and inactive pages reject it.

Snapshots expose `scroll_parent`, the nearest scroll-region ID. Vertical reveal requires
a positive horizontal intersection with every scroll ancestor and the outer
visible viewport; Tab skips side-clipped targets that scrolling cannot recover. Keyboard navigation
uses bounded ancestor traversal to reveal targets with minimal vertical offsets,
accounting for already queued UI offsets. Oversized targets align at the start.
Defer focus/activation until the revealing frame is presented; retain only one
pending identity and page generation and revalidate the target before acting.
Viewport changes (including IME dismissal) recompute the reveal from the new frame.
New touch/accessibility actions, blur, pause, replacement and surface teardown
cancel pending activation. Ordinary keyed focus restoration emits no new focus
event. Keep scroll-only navigation on retained layout frames.

A rebuilt View may reuse retained geometry only when the viewport matches and
all decoded structure, keys, text and fields match, except callback handle values
and list anchor metadata. Callback presence and enablement still have to match.
Rebind every event family into a separate candidate before composing scroll offsets;
list anchors still run through measurement/reconciliation. Reconcile editor values
before comparison and bypass this path while input layout is dirty. A failed
candidate must preserve the old geometry, generation and callable handles.
Count this work in compose time, while View execution remains visible separately.
On Pixel 3, the same list-window workload changes 19 frames from two layouts to
one and 20 from one to zero; total preparation P95 32.013 -> 29.198ms and layout
P95 9.309 -> 5.270ms (60 frames, thermal status 0). Input preparation P95 varies
15.425 -> 15.801ms. End process PSS increases 825 KiB in this single comparison.
Grid application pixels remain identical across rotation and light/dark themes.
These CPU observations do not satisfy the complete performance gates.

`ViewSession::frame_metrics()` describes the last successful frame; failed
candidates preserve it and reset clears it. Android snapshots include the same
`metrics` object. All `*Us` fields are CPU wall microseconds. `viewUs` covers
description/row evaluation, wire decoding and candidate commit; `viewPasses`
counts description/row evaluations. Its subsets `viewEvalUs`, `viewTransferUs`
and `viewDecodeUs` cover JS invocation, primitive-array extraction (including
getters and JS handle release), and native schema decoding respectively. All
three accumulate across list convergence passes; `wireValues` counts primitive
values transferred across those passes. Retained frames report zero for these
fields. Budget checks, microtasks, native buffer cleanup and candidate commit
remain outside the subsets but inside `viewUs`; do not sum subsets with their
parent. `layoutUs` includes tree construction,
measurement and initial display composition. Its `textUs` subset covers Parley
shaping, line breaking and alignment; do not add it to layout time. `composeUs`
covers retained-layout recomposition. `buildUs` is the backend callback, including
GPU preparation or CPU rasterization, not GPU execution. `totalUs` also includes
unattributed reconciliation, input decoration and state bookkeeping. Counts use
the final displayed node/paint set; layout/view pass counts include convergence.
Only one metrics record is retained, with no text or frame-history allocation.
Pixel 3 counter/input/list/retained-blink/background workloads validate these
subsets and zero transfer on retained frames. Across list groups with one, two
and three View passes, JS invocation accounts for 80..82% of aggregate View
CPU time, primitive extraction 8..12%, and schema decoding 3..4%. List total
preparation P95 is 29.395ms across 60 frames; this instrumentation establishes
where to optimize next, not an improvement or a GPU/FPS performance gate.
GPU snapshots also include `metrics.prepare`: `sceneUs` covers sparse-scene and
glyph preparation, `imagesUs` texture eviction/upload/binding, `encodeUs` renderer
encoding and command-buffer completion, and `submitUs` queue submission. These
are CPU subsets of `buildUs`; resize/setup and snapshot assembly remain outside
these subsets. Do not add preparation or presentation subsets to their parent.
`sceneCacheHit` reports exact sparse-scene reuse. Keep at most two scene entries
for recurring caret phases; compare viewport/scale, every paint (including font
identity, variation coordinates, glyph positions and clips), and immutable image
identity. Semantic nodes and input state still come from the new frame. Texture
bindings are rebuilt for the selected scene, including retained bitmap glyphs.
Suspend drops both entries. Entries are eligible only up to 256 paints, 4096
glyphs, 16 images, 4 Mi viewport pixels and 256dp font size, with separate 8 MiB
limits for referenced decoded images and bitmap glyphs. These bound cache inputs,
not the renderer library's total allocation size. Oversized scenes bypass reuse
and retire cached entries; do not keep their allocations as a spare scene.
Pixel 3 verification compares portrait/landscape pixels with the uncached build,
checks input/caret pixels, and exercises bitmap emoji across cached phases,
image eviction/reload and surface resume. Preserve these checks when changing keys.

Android presentation logs add worker queue, worker-to-vsync wait and UI callback
wall times. The UI time ends before log emission and pending-click dispatch.
Match `GPU_PRESENT` and accepted frames by process, surface and generation;
surface acquire/encode/submit/present timings are CPU calls, not display receipts.
Collect workloads separately, report sample counts with P50/P95/max, and retain
memory and thermal samples. Retained blink frames must report zero View/layout
passes; background idle must produce no presentation samples. These diagnostics
do not establish GPU timestamps, display FPS or the product performance gates.

Current independent-host Pixel 3 sampling records counter/input/list CPU P95
14.392/16.297/29.396 ms, with 60 list frames and thermal status 0. List View
evaluation P95 is 12.493 ms; layout is 5.691 ms and scene preparation 6.714 ms
(percentiles are not additive). Focused idle has seven frames with no View or
layout passes; background has none. Evidence: `target/gpu-current-performance-evidence`.
A second baseline run measured list P95 30.354 ms and View evaluation 13.278 ms
at thermal status 0. It reused the same cached bundle, so it cannot evaluate
the proposed per-tag property lookup table. That experiment was not retained.
Host cache keys now include source-exported linked packages. Check the packaged
runtime when comparing implementations; host cache reuse alone is not evidence.
Preserve primitive-specific property rejection, including prototype names,
when optimizing validation. These observations do not close the performance gate.

Layout sizes are logical pixels. `minWidth`/`minHeight` and `maxWidth`/`maxHeight`
constrain Taffy sizing; `shrink` overrides the existing flex-shrink default.
`align` and `alignSelf` accept start/center/end/stretch on the cross axis;
`justify` accepts start/center/end/between on the main axis. Center/end use safe
alignment so overflowing content retains an accessible start. These fields do
not change text alignment. Virtual lists require stretch/start for their own
row layout; reject other align/justify values rather than desynchronizing row
measurement and window offsets. Descendant row contents can align normally.
Keep style enums in the Rust schema and generate both JS validation options and
TypeScript unions from it. Native wire validation remains authoritative.
Pixel 3 checks cover centered header controls, minimum touch height, a 720dp
content-width limit in landscape and Logic state retained through rotation.
Formal-host regression covers Vulkan presentation, actions, theme, tabs, disabled
controls and Home/resume. Build with current CLI binaries: an older compiler can
bundle an empty action catalog even though the page renders successfully.

Non-editor text shaping may reuse unbroken Parley layouts within one Renderer.
Match exact source text, font size/weight, absolute line height and generic family;
line breaking and alignment always operate on a clone at the new available width.
Input, textarea and search bypass this cache. Retain at most 128 entries, 2 KiB
source bytes per entry and 64 KiB total source bytes; these bound cache inputs,
not Parley's allocated byte size or referenced font allocations. Reset clears it.
The renderer owns its font collection; any future font mapping mutation must also
clear shaped layouts. Failed candidates can warm this pure cache without changing
committed geometry, callbacks or editor state.
Pixel 3 same-workload observation reduces list shaping P95 6.601 -> 3.801ms,
layout 12.155 -> 9.309ms and total CPU preparation 34.870 -> 32.013ms (60 frames
per build, thermal status 0). End-of-run process PSS increases about 2.9 MiB;
this is a whole-process observation, not an allocation measurement of the cache.
Portrait, landscape and dark grid pixels match the uncached build outside system
status chrome; UTF-16/editing accessibility regression also passes. These results
do not establish display FPS, WebView-relative memory or the full performance gate.

Typography uses the same Parley layout for measurement, paint and input geometry.
`fontFamily` accepts `sans`/`mono` and inherits; map these to the platform generic
sans-serif/monospace families without bypassing fallback. `lineHeight` is positive
logical pixels (up to 16384), inherits as an absolute value, and defaults to font
metrics when absent. `fontWeight` requests a numeric weight from 1 to 1000 on the
current text/button/input/textarea; it does not inherit from containers. Preserve
the selected face and variation coordinates in both CPU and GPU glyph paints.
On Android, Fontique 0.12 does not resolve the fonts.xml monospace alias to a
scanned family. If its generic mapping is empty, use an available Droid Sans
Mono, Noto Sans Mono or Roboto Mono family. Keep an existing platform mapping.
Pixel 3 GPU captures verify equal mono advances for narrow/wide Latin glyphs.
Gboard checks cover composition, submit, resume, long-text scrolling and deletion;
pixel checks verify caret visibility and blink at the new absolute line height.

`backgroundGradient` (wire field 72) is `none` or bounded JSON describing a linear
gradient: optional direction or clockwise CSS angle, sRGB/Oklab interpolation,
and 2..16 ordered hex-color stops. Offsets are bounded to -16..16; extend the
gradient line before normalizing offsets for Peniko. Reject unknown fields,
ambiguous direction/angle and malformed stops before accepting the frame.
Corner directions follow [CSS Images](https://www.w3.org/TR/css-images-3/#linear-gradients),
including aspect-dependent magic corners. CPU, sparse GPU and Vello share brush
construction, rounded bounds and retained-scroll translation. Paint gradients
above the background color and below borders; editor foreground clips exclude
both background paints. Pixel tests cover interpolation, alpha, borders and
scrolling. Pixel 3 screen checks cover non-square corner directions, sRGB/Oklab,
premultiplied alpha, hard stops, rounded borders and reset/reuse. Home and Device
regressions cover remote-photo overlays, light/dark gradients and rotation.

Bitmap decoding accepts host-supplied PNG/JPEG bytes on a worker, never paths or
network URLs. Encoded input is limited to 8 MiB, dimensions to 4096 per axis and
source area to 12 Mi pixels, downsampled to 4 Mi retained pixels. Decoder allocation limits are best effort; these
are per-image bounds, not a process memory guarantee. EXIF orientation precedes
layout; shared pixels are immutable premultiplied RGBA. CPU/GPU fit geometry is
shared, and cover placement requires clipping to the image node.

GPU remote image resources use `PageInstance::load_gpu_image_bytes`: package and
managed URIs retain the bounded local reader; HTTP(S) uses the owning app's
managed URL cache. The shared network scope applies the same domain, DNS and
redirect policy as Logic transfers, with a grant check before cache reuse and a
session check before publishing bytes. Android invokes it on its bounded image
IO pool; no network or filesystem access belongs in the renderer.

Image transfers request an 8 MiB persisted-byte limit, including cache hits and
resumed/chunked responses. PNG/JPEG decoding accepts at most 4096 pixels per axis
and 12 Mi pixels per source, then downsamples to the existing 4 Mi pixel retained
bitmap limit after applying orientation. This admits the showcase UHD background
without increasing per-image retained texture memory. These limits do not imply
support for arbitrary image formats or unbounded originals.

Image sources come from visible paint commands, not list overscan. A successful
frame publishes its wanted sources and evicts others; failed candidates retain
the old set. Completion binds decoded pixels to retained layout without running
View. Shared `ImageRequests` bounds in-flight loads to four and owns request
identities across eviction/reload/parking. Retain the queue for the surface owner,
not a single ViewSession: clear the wanted set on reset but count obsolete
transfers until completion. Complete each result once, including failures;
accept bytes only while its token is current. Repeated frames do not retry a
finished source; removing and reintroducing it allocates a new token. Android
keeps only a main-thread JNI adapter and closes the queue when its GpuView closes.
Its worker passes queue/token with image bytes. Decode outside the queue registry
lock, then check the token and bind the bitmap atomically with queue invalidation;
a main-thread preflight alone cannot reject a result invalidated during decode.
Its host resolves resources using the live endpoint's
owning page; native paths and foreign app URIs are rejected. HTTP(S) sources
use the owning app network policy and managed cache.
GPU textures are shared by immutable bitmap identity and released on eviction
or surface suspension. CPU and GPU frames use the same image fit and clipping.
Glifo's bitmap glyph paints must also become external GPU textures before scene
rendering; Vello GPU rejects raw pixmap paints. The glyph sink preserves Glifo's
transforms, clipping, sampling and premultiplied pixels, and shares the presenter's
image upload/eviction path. Do not rely on the experimental glyph atlas to make
bitmap fonts safe: an uncached glyph can still produce a pixmap.

Scroll/list descendants inherit rounded masks in original translated container
bounds, never in their rectangular visible intersection. Adjacent paints share
the common clip stack; rectangular masks retain the GPU rectangle fast path.
Fully masked nodes do not contribute paint or image requests. Presented node
snapshots carry the same ancestor masks for Android hit testing and press/focus
overlays; overlays retain original node geometry beneath those masks.
Android feedback belongs to a noninteractive sibling above the SurfaceView;
its own overlay can draw without appearing in the composed device frame.
Reattach both views after a failed module load, and keep the sibling out of
keyboard focus and accessibility navigation.
On keyboard focus entry, select a visible enabled button in the entry direction
and suppress the platform's whole-surface focus highlight. Touch entry does not
select a keyboard target. Snapshot node identity includes ancestry, key (or
sibling position when unkeyed), and tag; test IDs and callback handles are not
identity. On successful presentation, remap keyboard focus to the same visible
enabled identity and refresh its geometry. Removal, hiding, disabling, reload
and touch entry clear it. Enter without a focused target must not pick another
button. Dynamic siblings need keys to distinguish replacement from reuse.

The generated schema includes `input`/`textarea` descriptions with controlled
value, placeholder, keyboard, secure/autofocus flags and text callbacks. `bind`
requires a live `useLocal<string>` cell in the current candidate and cannot be
combined with value/onInput. Text callbacks accept only bounded valid Unicode
strings; click callbacks reject text payloads, and stale frame generations are
ignored. Input previews clip to their own bounds, mask secure display text and
flatten hard breaks for single-line presentation without changing the value.
ViewSession connects these descriptions to native editors and typed callbacks.

`text_input::Editor` is worker-owned and stores text in Parley's `PlainEditor`.
Its IME selection/composition offsets remain exact UTF-16 positions even when
Parley visual cursors snap to a grapheme boundary. Invalid surrogate boundaries
are rejected atomically. Text is bounded to 64 KiB of UTF-8 and batches to 64
edits; one accepted batch yields one snapshot and at most one committed-text
notification. Preedit stays private. Empty preedit or an empty composing region
can finish a real deletion/commit and must not leave the controlled value stale.
Focused editors retain their editing buffer across value echoes; blur reconciles
the latest controlled value. Replacement cursors and surrounding deletion follow
the [Android InputConnection contract](https://developer.android.com/reference/android/view/inputmethod/InputConnection).
The session retains editors by node identity. Focus requests carry the presented
frame generation; subsequent edits carry a focus token that survives frames but
expires on blur, hiding, disabling, removal or reset. Autofocus is consumed once
when the node becomes visible. Host edits cannot change controlled values or
focus directly. Keep at most 128 editors and 1 MiB of combined raw, controlled
and published UTF-8 text plus editing history; these bounds do not cover shaping
or process memory.

Before decorating inputs after viewport, inset or keyboard-occlusion changes,
reveal the existing focused editor through its scroll ancestors using minimal
bounded offsets. Commit these offsets with the frame; failed candidates preserve
focus and previous scrolling. This does not initiate autofocus or override user
scrolling on an unchanged viewport. Hidden/disabled/removed editors and editors
with no recoverable visible scroll area still lose focus. Pages must keep their
editable content in a scroll region that retains space when the keyboard opens;
fixed headers must not consume the entire remaining viewport.

Undo/redo history belongs to the worker editor and rolls back with failed batches
and frames. Keep at most 64 steps and 256 KiB of retained history text per editor,
counting both stacks and the pre-composition baseline; evict oldest steps first.
Contiguous single-character non-whitespace insertions within one second coalesce.
Selection changes, composition and focus boundaries split typing groups; paste and
replacement are separate steps. Preedit updates share one baseline. Undo during
composition cancels modified preedit before undoing committed text; redo is
unavailable while that preedit differs from its baseline. An IME marking unchanged
text as composing must not intercept undo/redo: clear that region and apply the
history command. New text edits discard redo. Restore exact logical
selection offsets with text, never a visual grapheme approximation. Matching
controlled echoes preserve history; a differing installed value clears it. Blur
can cancel preedit back to its controlled baseline without losing older history.
Input replies expose `can_undo`/`can_redo`; host `undo`/`redo` use the same focus
token, batch limits and committed onInput path.
Android floating-menu actions and Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y dispatch history
commands only from a settled mirror with the matching capability. The mirror
does not predict history: reject edits and submit until the reply installs the
worker state. A stale frame/reply cannot release this barrier. On completion,
invalidate the old IME wrapper and restart input so late composition callbacks
cannot write to the restored selection. Focus teardown cancels the operation's
UI callback. Menu availability follows acknowledged state without inspecting the
clipboard or invalidating an unchanged toolbar.

Keep canonical View records separate from the live editing overlay. Preedit and
selection-only frames reuse those records without executing View; committed text
dispatches onInput and forces View reconciliation. Submit finishes composition
before dispatching onSubmit. Failed frame preparation restores the prior editor
baseline and advances its token: platform adapters must replace the old input
connection. Callback side effects are not rolled back. Timed-out pages reject
further editing until disposal.

Caret, selection and composition paints use the retained displayed Parley layout,
including its actual font, wrapping and alignment. Convert logical UTF-16 offsets
through password masking and single-line break replacement before querying it.
Selection paints below glyphs; composition underlines and the collapsed caret
paint above them, within the input and ancestor clips. Retained scroll translates
geometry with text; blurred or hidden inputs publish no editing geometry.
Geometry exposes the logical `anchor` and `caret` (selection focus) in surface dp,
plus the `content_bounds` viewport. Both endpoints use the displayed offset mapping
and text scroll; never infer direction from their x coordinates or selection-box
order. Endpoint rectangles remain unclamped, including when scrolled outside the
content viewport. Platform handles must hide clipped endpoints and respect the
node's rounded ancestor clips rather than pinning handles to a viewport edge.
Focused inputs retain an internal scroll offset by editor identity. Reveal the
selection's focus with minimal movement, clamp after text/viewport changes, and
translate glyphs and editing geometry together beneath the content-box clip.
Do not cull input glyphs before this translation. Frame rollback restores the
offset with editor state; blur resets it. This scroll does not run View or move
ancestor containers.

Input hit queries share the successfully built frame's Parley layout and retain
only its input nodes. Convert surface dp through ancestor placement, content
padding and internal scroll, then map displayed byte offsets back to logical
UTF-16. Empty placeholders hit offset zero; password word queries select the
whole value without exposing its underlying word boundaries. Reject points
outside the visible rounded clips, stale frame generations and queries while
edits await a frame. Failed frame builds do not publish candidate hit geometry.
The `hit_test` input request returns a selection without changing focus or text;
platform adapters must still guard asynchronous replies against newer local edits.
Android taps request collapsed positions; stationary long presses request words.
Movement beyond touch slop, another pointer, changed content and lifecycle
teardown cancel a pending long press. A completed long press consumes its release
so the following up event cannot collapse the selection. Defer touch queries
behind an in-flight frame under the same semantic checks as clicks. Ignore hit
replies after a newer touch, focus epoch, local edit or unfinished edit batch.
Apply accepted positions by atomically finishing composition and selecting; focus
new inputs before applying the result. Do not synchronously query the View worker.
Do not drain View actions for read-only input requests or selection/preedit-only
edits: the Android drain adapter invalidates View and can supersede the queried
frame before focus. Drain after edits requiring View reconciliation, including
failed edits whose callbacks may already have queued actions.

Android draws selection and insertion handles in the sibling above SurfaceView.
Their tips use presented endpoint geometry; ancestor masks constrain both paint
and touch targets. Text-line taps must not be consumed by handle hit areas.
Keep the opposite endpoint fixed and make the dragged endpoint the logical focus,
including when crossing it, so existing caret reveal follows the moving end.
Coalesce pointer moves behind one hit query; edit only while its focus epoch,
connection revision and pointer position still match. Wait for the acknowledged
selection to appear in a frame before the next query or release menu. A stale
frame query waits for a newer frame rather than looping. Focus, text or scale
changes, cancellation and another pointer invalidate the drag. A newer IME edit
also cancels it even when a newer pointer position is queued.
For edge dragging, clamp queries to the visible content viewport and carry the
outside-edge direction separately. `drag_hit_test` also validates the focus token.
While text remains outside that edge, advance using the displayed layout's visual
cursor or line navigation; nearest-point snapping alone can stall on a large glyph.
Repeat only after an accepted selection is presented, and stop at an unchanged
position or pointer release. Never estimate character widths in the adapter.
Pixel 3 / Android 12 device probes cover selection extension, endpoint crossing,
reverse selections, insertion-handle positioning, multiline dragging and held
edge drags through horizontal/vertical text scroll. Offscreen endpoints remain
hidden; insertion after dragging uses the resulting logical selection.

Android drives collapsed-caret visibility with one 500 ms main-thread timer.
Text/selection/composition changes restart the visible phase; an unchanged frame
does not restart it. Noncollapsed selection, window focus loss, surface teardown,
pause and disposal stop the timer. Capture visibility with each frame request;
CPU/GPU adapters suppress only the caret command, retaining geometry and input
state. Phase frames reuse layout without executing View. Unchanged semantic
snapshots do not emit accessibility content-change events.
Identical semantic snapshots also preserve an active press and accessibility
focus. Defer clicks while a frame awaits presentation; dispatch only if the
surface generation, scale and semantic snapshot still match. Focus teardown
must not queue a phase frame ahead of its triggering click.

Android's `nativeInput(page, request)` runs on the owning View worker. Requests
are strict tagged JSON (`focus`, `query`, `edit`, `blur`, `hit_test`), capped at 512 KiB;
JNI also bounds UTF-16 length before copying and rejects unpaired surrogates.
Edit batches contain only selection, composition, replacement, finish, surrounding
deletion and undo/redo; submit counts as one finish edit against the batch bound.
Expired focus/query/edit requests return null, blur returns whether it matched,
and accepted edits return the snapshot, token and View-rebuild requirement.
The UI-thread IME adapter must consume these replies asynchronously; this entry
point does not grant host capabilities or make synchronous View calls safe.

`GpuInputConnection` keeps a UI-thread mirror for synchronous IME queries using
the [BaseInputConnection contract](https://developer.android.com/reference/android/view/inputmethod/BaseInputConnection).
The outermost batch sends once; retain at most 64 outstanding batches and
512 KiB of queued/in-flight command bytes. Reject edits before mutating the
mirror when text, batch or backlog limits would be exceeded. Acknowledgements
release backlog but replace the mirror only when no later local edit exists.
Physical character insertion and multiline Enter finish composition before
inserting at the selection. Reserve both commands atomically and send them in
one batch; normal IME commitText still replaces the composing region.
Backspace and forward-delete key events use Android BaseKeyListener on a temporary
text/selection copy to determine the device's deletion range (including supported
emoji sequences). Reserve finish-composition, selection and replacement together
before changing the mirror. A selection takes precedence over the composing
region; capacity rejection leaves both untouched. IME deleteSurroundingText and
deleteSurroundingTextInCodePoints retain their explicit UTF-16/code-point units.
Android context-menu commands and Ctrl+A/C/X/V support select-all, copy, cut and
plain-text paste. Cut and paste finish composition and replace the selection in
one bounded batch. Reserve cut edits before writing the clipboard; a denied
clipboard write restores the reservation and leaves text/selection unchanged.
Secure inputs cannot copy or cut. Read clipboard contents only for explicit
paste; accept at most 128 plain-text items within the editor's Unicode/UTF-8
budget, without URI coercion. Closed connections cannot access the clipboard.
Accepted Android long presses open a native floating ActionMode toolbar anchored
to the focused input's visible bounds. Its commands use the same bounded input
mirror; showing the toolbar does not inspect clipboard contents. Hide copy/cut
for secure or collapsed selections. Menu callbacks belong to one ActionMode and
focus epoch; old callbacks cannot operate a replacement menu/editor. Typing,
another surface touch, focus loss, pause and teardown dismiss it. Update toolbar
items and bounds only when they change: repeated invalidation can relayout the
host and create a frame loop even with a stationary noncollapsed selection.
Frames must carry the revision captured when scheduled; stale frames cannot
overwrite the mirror. Native rejection requires closing and replacing the
connection. GpuIme binds presented focus tokens to the Surface editor, advertises
keyboard/password/multiline metadata and synchronizes IME selection/extracted
text. Each Android connection creation returns a separate wrapper: closing an
old wrapper must not close the current editor. Pause/reload/focus loss invalidate
local callbacks; a late cancelled focus reply is blurred before reuse. A native
edit failure blurs its token on the worker before later queued edits can run.
Input nodes participate in hit testing, keyboard traversal and accessibility
click focus. Android accessibility set-text and set-selection actions use the
same bounded editor, focusing asynchronously when needed. Set-text atomically
finishes composition, selects all and replaces; a rejected reservation changes
nothing. Successful external text replacement invalidates the old IME wrapper
and restarts input so cached keyboard composition cannot edit the new text.
When focusing a new editor, apply requested text/selection before requesting
Android focus and starting the keyboard; its first snapshot must include them.
Pending focus, unsettled mirrors, invalid UTF-16 boundaries, hidden or
inactive surfaces reject editing. Epoch cancellation prevents a late focus reply
from editing a replacement page. Accessibility values exclude placeholder text;
selection offsets stay logical UTF-16, including one mask character per code unit
for secure fields. Presented text/selection changes emit virtual-node events;
unchanged caret-phase frames do not. Send events only while AccessibilityManager
is enabled; UIAutomator may disconnect between frames. Password event text and
before-text are masked.
Android virtual accessibility IDs follow presented node identity, not array
position. Retain IDs across reordering; prune removed nodes and never reuse their
IDs, including after a page reset. Resolve every action through the current map,
and use the same IDs for text events and input-focus queries. Preserve accessible
focus across content frames while its node remains visible; removal or hiding
clears it. A clear-focus request for another node must not clear the current one.
Pixel 3 checks retain accessible focus and a cached node through text edits, then
replace the input with a textarea and verify the old node rejects edits and
refresh. Reordering and ID retirement across reset are covered by JVM tests.
When Android enables touch exploration, route hover enter/move/exit to visible
semantic nodes through the same scale and ancestor clips as touch hit testing.
Emit virtual-node hover transitions only when the target changes; hovering must
not click, focus the editor or open the keyboard. Re-hit the last hover position
after changed frames, and end hover on pause, focus loss and reset. Accessible
focus remains controlled by the service's explicit actions.
Pixel 3 hover injection with a temporary exploration-enabled UiAutomation service
verifies input-node enter/exit and no IME activation. Its ordinary injected touch
bypassed the system exploration transform, so this is hover-routing evidence,
not a TalkBack finger-gesture or speech acceptance result. Screen-reader traversal,
movement granularity and announcement UX remain gates.
Physical Pixel 3 validation covers English Gboard preedit, Done commit/submit,
deletion, keyboard dismissal and Home/resume composition cancellation. Screenshot
checks cover the composing underline and caret, plus horizontal caret visibility
after entering 80 characters and deleting back to four. Ten-line textarea entry
and deletion verify vertical caret visibility and reset. Phase samples verify
on/off pixels, native-only frames while focused and no frames after pause/blur.
Physical Ctrl shortcut checks cover selection paint, copy/cut, consecutive paste,
selection replacement and submit. Android 12's shell `input keycombination`
injects zero modifier state; device probes must inject KeyEvents carrying Ctrl.
Physical touch checks cover first focus, tapping before/after text, long-press word
replacement, movement cancellation, submit and placement on an earlier textarea
line. Selection screenshots and subsequent inserted text verify the hit positions.
Physical floating-toolbar checks cover copy/cut/paste/select-all, dismissal after
typing and Home/resume, and no stationary selection frame loop. The system toolbar
is a separate accessibility window; device probes must inspect interactive windows.
Physical Pixel 3 / Android 12 checks with Gboard 10.8.03.382062350 verify Simplified
Chinese Pinyin candidates: `nihao` commits `你好`, pending `shijie` is cancelled by
Home/resume without changing the committed value, and choosing `世界` then submits
the edited Chinese value. Pinyin stays inside this IME before candidate selection;
this does not establish inline CJK composition coverage. Screenshots verify glyphs,
caret and the bound preview. In this configuration the first soft Delete clears
the suggestion strip without changing text; the second removes one character.
The same two-step text result was reproduced in Android Settings' native EditText.
Use real candidate/soft-key taps, assert both states, and restore keyboard layouts
after testing. This evidence covers this Gboard configuration, not all CJK IMEs.
Physical accessibility checks cover unfocused set-text, virtual input focus,
UTF-16 emoji selection, split-surrogate rejection, keyboard replacement, clearing
and text/selection events. The colored emoji is rendered as an uploaded glyph
texture in both the input and its bound preview. Ordinary touch/IME regression
also runs with no persistent accessibility service; this catches event dispatch
after UIAutomator disconnects. Physical Pixel 3 / Android 12 checks cover backward
and forward key deletion of family ZWJ, skin-tone, regional-indicator flag, keycap
and simple emoji sequences, asserting both text and UTF-16 caret positions. A
Gboard soft Delete tap also removes the complete family sequence. These checks
cover this device's deletion rules, not all Unicode sequences or Android versions.
Physical Pixel 3 checks cover Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y, exact selection
restoration after replacement, the native Undo/Redo toolbar, modified Gboard
preedit cancellation without publishing it, history after Home/refocus and redo
invalidation after new typing. Gboard may mark unchanged committed text as a
composition region immediately after restart; this must not consume undo or
disable redo. Draggable selection handles and other CJK IMEs remain device gates.

`ViewApp` owns one Rong isolate and bounded per-page contexts on its caller's
View executor. Page identities are never reused. Reset clears local state and
handlers while retaining module globals and replicated data; discard releases
the context. Neither operation restores saved local state. The Android harness
and standard Android SDK use this owner on one executor per app session.

QuickJS isolates have one sleeping watchdog: module evaluation gets 1 second;
render and input evaluations get 100 ms. These are fault-containment deadlines,
not frame-performance targets. Interrupts are scoped and disarmed before another
page can execute. A timed-out context is quarantined until discarded, retaining
the last accepted frame. Apple and ArkJS do not use this watchdog; Apple private
SPI preemption is deliberately excluded. Their stuck-worker recovery is pending.

- Rust generates the JSX vocabulary and wire field IDs. Update the generator
  and generated files together (`cargo run -p lingxia-ui --example generate`).
- Candidate validation/layout precede callback and local-state commit.
  Failed candidates preserve the last committed handlers and generation.
- Input carries the displayed frame generation; stale input is ignored.
- Logic-enabled contexts wait for the first snapshot. State replication applies
  patches atomically and returns requested acknowledgements before rendering.
  Send those acknowledgements even if the subsequent render fails.
- Revision gaps or invalid patches request one snapshot and suspend patch
  application until recovery. The recovery response carries `{ rev, state }`.
- View data and environment are deeply immutable. Functions remain in the
  context; only primitive records and event handles cross to native.
- GPU unary actions use the compiled `__names`/`__modes` catalog and LegacyV2
  `req`/`res` frames with `cap: "page"`. Configure only after bridge ready;
  streams are rejected. Drain requests after input and response continuations,
  including when a later render fails. Reset cancels pending Promises and
  invalidates captured action functions; request IDs are not reused.
- The CLI extracts the catalog from the sibling Logic file using the same
  parser as WebView builds and embeds it in `.ui.js`. `ViewSession::begin_bridge`
  starts an inert replica with a native nonce. A matching V2 helloAck and ready
  enable actions; rendering also waits for the initial snapshot. State may arrive
  before ready, but cannot produce a frame until both have arrived. Session IDs
  are handshake-only; ordinary GPU frames remain unbound LegacyV2 envelopes.
- QuickJS drains microtasks inside the View execution budget. On failure,
  drain remaining jobs with interrupts armed and the stack limit temporarily
  reduced to reject JS calls: branch-free Promise chains may never poll an
  interrupt. Restore the renderer-owned default stack limit before dropping the context or
  executing a sibling. Async callback rejections are exposed through the bounded
  action-error queue. Apple/Harmony action adapters remain pending; the action
  client alone grants no host authority.
- Android initializes GPU resources and prepares textures on its worker.
  The UI thread presents a matching revision. Surface suspension drops the
  native-window reference but retains the device and pipelines.

`tools/lingxia-ui-android` is an isolated device harness. Its JNI registry is
not a bridge ingress and grants no host capabilities. `PageInstance` now owns a
`PageView` (WebView or native GPU endpoint). Outbound messages, environment
updates, parking, reload and detach use that endpoint. Browser-document-bound
outbound traffic remains WebView-only. GPU endpoint calls must enqueue without
blocking, and dispose runs after releasing the page's View lock.

Native SDKs register one `GpuPageViewFactory` before opening apps. Page creation
and recreation select it from the validated renderer; a missing factory fails
setup rather than opening a WebView. Creation has a 30-second deadline. A late
endpoint is disposed if its page departed; failed or cancelled setup detaches
only the endpoint it attached, never a replacement. Factory futures must clean
up resources when dropped.

Android's `com.lingxia.ui.GpuView` is the shared SDK content component, also used
by the device harness. Its host owns one single-thread executor per app session;
JNI keeps one `ViewApp` on that thread and addresses contexts with process-unique
page handles. Releasing a page preserves its siblings. Async UI callbacks use
the main Handler, not View.post, so detached views still release late surfaces.
The component does not own Activity chrome, resource loading, or worker shutdown.

Android samples system appearance, locale, keyboard and system/cutout insets on
the UI thread with each requested frame. Insets subtract the space already
reserved outside the content surface. Environment changes invalidate pending
presentations; the worker installs the snapshot before layout. Viewport and size
class derive from the actual render dimensions, not host JSON. Reset retains the
environment; reload samples it again. These native measurements do not yet apply
the app's display-language override or desktop surface-context revisions.

The Android facade links the renderer and registers its JNI methods during SDK
initialization using the app classloader. Keep registration's function pointers:
they retain the renderer when linked into a host library. The isolated harness
calls `GpuView.initializeNative()` after loading its renderer library, exercising
the same registration table on-device.

Android/QuickJS evaluates the bundled `.ui.js` as ESM on the View executor under
the module watchdog. Initialization errors, rejected/pending module promises and
missing compiled runtime exports fail context creation. The compiler targets
ES2020 and bundles imports; asynchronous module initialization is unsupported.
The harness loads this same ESM artifact. `PageInstance::gpu_module_source` reads
only the validated catalog entry through the app's resource source, with strict
UTF-8 decoding. Apple/Harmony ESM adapters remain pending.

Android registers a factory and presents its `GpuPage` through the regular
Activity container. Creation waits for module load, and a cancellation lease
disposes the Java page even when the factory future is dropped. Java retains
one executor per app session until its last page is disposed. Logic-enabled GPU
pages connect after PageSvc creation. App display-language and surface-context
transport remain pending. Attaching an endpoint does not mark setup or bridge
ready; setup completion and the bridge handshake acknowledge those milestones.
The component's first successful presentation after each load reports through
the factory's native endpoint token. Only the attached endpoint may finish the
page load and advance the loaded watch; that receipt drives the home splash
and dev reload completion. Module load alone must not dismiss the splash.

GPU bridge connections use runtime-issued `GpuViewGeneration`, never a browser
document identity. Ingress verifies the attached endpoint and captured session
work before dispatch; caller class comes from the owning LxApp. GPU hello keeps
that native-created connection. Outbound work captures a weak endpoint plus its
generation, and Android rechecks the generation on the View worker. Reload reads
the current catalog module and replaces the connection; parking and disposal
invalidate delivery. First-frame receipts carry the same generation. GPU View
host requests return `E_PERMISSION_DENIED`; host APIs run through Logic actions.
GPU actions preserve compiler modes: ordinary actions return promises; generator
actions return `Stream<T, R>` with `on("data" | "end" | "error")`, `result`,
`cancel()` and one async iterator. Events start at sequence zero and increase
monotonically. Terminal responses settle once; cancellation settles locally and
sends LegacyV2 `cancel`, with late frames ignored. Reset rejects retained streams
and readers. Listener failures enter the bounded action-error diagnostic queue.
Only iterator consumers buffer events, bounded to 256 items and 1 MiB of
serialized UTF-16 storage; overflow cancels the request. At most 256 operations
and 256 listeners per event are live; outbound cancellation has a reserved slot
per admitted request.
GPU Views open bidirectional channels through `view.channels.open(topic, params)`.
Topics must name a current-page Logic handler; dotted topics and host channels
are denied at native ingress. The native endpoint, generation and captured
session work checks also apply to channel frames. Handles expose `send`, `close`,
`on("data" | "close" | "error")` and one async iterator. Each direction starts at
sequence zero and increases monotonically. Opening resolves only on `ch.ack`;
rejected opens and reset settle pending consumers, and late closed-channel
frames are ignored. Before a data listener attaches, or while using an iterator,
the queue is bounded to 256 items and 1 MiB; overflow closes the producer.
Remote close drains accepted iterator messages before EOF; local close discards
unread messages. Channels share the 256-operation admission limit with actions.
Pixel 3 verifies channel acknowledgement/initial state, live ticker updates,
bidirectional symbol subscription, stopped delivery after close, reconnection
and exit/reentry against the independent APK with dev transport disconnected.
The page's automatic initial connection and remaining error/presentation cases
still require parity work.
Pixel 3 verifies live token rendering, terminal Logic state, explicit cancel,
clear, navigation teardown and a fresh stream from the repacked APK with dev
transport disconnected. The showcase's chart presentation and remaining stream
error/input cases still need parity verification.

The runtime page catalog preserves `renderer` and `module`. A compiled GPU entry
must use a `.ui.js` path and the identical module path inside the bundle; it must
agree with the sibling `index.json` renderer. Source and compiled UI extensions
resolve to the same route, but an ambiguous extensionless alias is rejected.
`PageInstance::renderer()` exposes the validated selection to native containers;
selection alone does not create a GPU surface or grant bridge authority.

Run `cargo test -p lingxia-ui --features gpu`, the UI package's Node tests,
and the harness's `verify.mjs` on a physical Android device. CPU wall-time
logs do not establish GPU timestamps or the draft's workload performance gates.


Explicit `height: 'auto'` / Tailwind `h-auto` appends wire field 82 with value 1.
Omission keeps primitive default heights; auto delegates height to Taffy content
measurement. Reject combinations with absolute/percentage height. Min/max height
still constrain layout, allowing a textarea to grow with soft wraps and scroll
its editor content at the cap. This is not a JS character-count approximation.


Textarea `submitOnEnter` (field83, boolean encoded0/1) opts into hardware Enter
submission and Android IME Send; omission preserves multiline newline behavior.
Shift+Enter still inserts a newline. Hardware Enter during preedit finishes
composition without submitting; explicit IME Send commits and submits as usual.
Propagate the flag in input snapshots and restart IME configuration when it
changes. Keep multiline input type and keyboard focus after submitting.


`fontStyle` (field 84) accepts normal/italic and inherits through layout parents;
explicit normal resets inheritance. Tailwind italic/not-italic lower to this field.
Include family and slant in shaping-cache identity. Request the matching Parley
face; preserve its synthesis skew per glyph through retained paint, CPU, sparse
GPU and Vello encoding. Store the font-space skew; negate it for glyph transforms in Y-down layout
coordinates and include transformed ink extents when culling. Pixel 3 verifies
normal/italic raster differences, inherited italic, explicit normal reset and
repeated style changes without stale shaping-cache results.

Pixel 3 verifies configured pie radius pixels, ring/hole/outside physical taps,
and short/wrapped chart legends in both themes. CPU pixels cover the reference
300x210 geometry; Android unit tests cover changed-layout queued-click rejection.

`letterSpacing` (field 89) accepts signed logical lengths within +/-1024 and
inherits through layout parents; zero resets it. Feed the resolved value into
Parley before measurement, breaking and input geometry, and include it in the
shaping-cache key. Tailwind tracking utilities lower through the official CSS
compiler. Resolve em spacing against the owner's computed font size once;
descendants inherit the computed length, including when they change font size.
Pixel 3 verifies expanded glyph advances, direct/inherited/reset raster matches
and repeated cache toggles. Chart-title tracking and chart interactions pass in
light/dark; input spacing and wrap behavior currently have CPU coverage.

Independent corner geometry is carried internally by solid/gradient/border
paints and optional snapshot radii. Retained scroll translation preserves it;
input/scroll hit masks and Android highlights use the same contour. Corner overrides use the same geometry for backgrounds, borders, content clips
and input snapshots.


Corner radius overrides append fields90?93 (top-left, top-right, bottom-right,
bottom-left), finite logical lengths0..16384. Omitted corners use uniform radius,
including control defaults. Normalize adjacent corners together before deriving
border/content contours. Equal corner overrides collapse to the uniform path.
Snapshots transport resolved radii for native clipping and highlighting.

Independent shadow masks key size, normalized corners, offsets, spread, blur,
resolved color and display scale; position is excluded. Single-threaded CPU
Gaussian filtering generates immutable premultiplied pixels once per cache miss.
Keep uniform shadows on the native fast path. Bound masks to4096 per axis/4M
pixels; keep at most64 entries/16MiB in the LRU and16MiB of unique mask pixels per
retained layout. Reject excess before painting. Reset clears cache ownership;
existing frames keep their immutable pixels. Masks are internal resources, never
host image requests, and remain bound when external image bindings refresh.
Retained scrolling translates their bounds; owner knockout, ancestor clipping
and opacity apply consistently. This fallback trades bounded texture memory for
cross-backend contour parity; it is not a new performance claim.

### Backdrop filter backend feasibility (not an authored feature)

`src/backdrop_backend_tests.rs` verifies a captured parent image can be blurred,
rounded-clipped, tinted and composited beneath sharp foreground content. CPU
uses an immutable Pixmap; sparse GPU copies the rendered target into an external
texture and continues the target with `TargetInit::SrcOver`. The GPU path does
not read pixels back for rendering; readback is only used by test assertions.
Run the hardware test explicitly with
`cargo test -p lingxia-ui --features gpu --lib backdrop_backend_tests -- --include-ignored`.
It requires an adapter and fails if one is unavailable; ordinary CI skips it.

`gpu_pass::SceneRenderer` owns the renderer/resources and submits each scene
before returning; Presenter and the offscreen tests use this shared path. Staged
`queue.write_*` uploads share internal buffers and otherwise replace earlier
scene inputs before execution. Submission orders work without a CPU completion
wait. Capture textures need COPY_DST/TEXTURE_BINDING and source targets need
COPY_SRC; the production Presenter target does not yet enable this path.

The prototype covers one glass card within an opacity root, composited once at
50% over a distinct outer background. Pixel assertions check root isolation,
group opacity, foreground sharpness, clipping and retained parent pixels.
The standalone backend tests choose roots explicitly; production still uses one
scene. The shared compositor below now selects capture roots from group structure.
Production integration still needs backdrop record emission, backdrop geometry
translation and cache invalidation
when the sampled background changes. Capture the parent once at each boundary;
do not recursively replay its display prefix for every card. Keep foreground
content outside the filter input. Vello 0.3 supports Gaussian blur; saturation
uses the separate shared color-filter backend below. Do not expose Tailwind
backdrop utilities as supported until their authored semantics work end to end.

`color_filter::Saturation` transforms caller-owned premultiplied sRGB pixels in
place; never mutate a shared decoded image. Its factor is finite in 0..16, with
1 unchanged and 1.8 corresponding to ToDo's 180%. The RGB-only linear matrix can
operate on premultiplied values if its result is clamped to [0, alpha]; alpha is
unchanged. This is algebraically equivalent to the non-premultiplied
[saturation matrix](https://www.w3.org/TR/filter-effects-1/#feColorMatrixElement)
followed by clamping and premultiplication, without division near zero alpha.

`gpu_color_filter::SaturationPass` retains a render pipeline and encodes an
overwrite pass between distinct same-sized RGBA8Unorm textures, sampling exact
texels. These textures contain sRGB values; sRGB texture formats would introduce
an unwanted transfer conversion. Inputs/outputs are single-layer, single-sample,
single-mip 2D textures with TEXTURE_BINDING/RENDER_ATTACHMENT respectively.
Each encoded pass owns its uniform so multiple factors can be batched safely.
It allocates no intermediate image; the caller owns texture lifetime/budgets.
No queue submission or CPU wait is performed by this filter pass.

CPU fixed-value checks cover gray/identity/180%, primary colors and translucent
pixels. Desktop GPU tests compare all pixels at six factors, cover alpha 0/1/17/
128/255, reject alias/size mismatch and exercise Gaussian -> saturation without
readback between stages. This backend is not yet connected to authored styles.

`composite::Plan` is the shared execution order for CPU/GPU group composition.
Draw spans must be monotonic, non-overlapping and within the immutable display
list. Push/pop scopes contain overflow clips, opacity or backdrop groups. An
opacity below 1 or a backdrop starts a sampling root. Ordinary overflow clips
do not. At a backdrop boundary, snapshot the current surface and merge unfinished
clip groups from inner to outer, stopping at the nearest root. Never replay
earlier draw spans. Filter the capture, clip that result to the owner, then draw
the owner's foreground; composite the completed group with its opacity once.

`composite_cpu::Cpu` uses immutable Arc<Pixmap> snapshots; writes produce new
pixels. `composite_gpu::Compositor` snapshots textures, uses the shared SceneRenderer
for ordered submissions and applies saturation between blur and owner clipping.
Its rendering has no CPU pixel readback or GPU-completion wait. The caller's draw
callback encodes a span into a fresh CPU context or GPU scene. GPU pipelines and
depth resources persist across calls; only the callback/display borrow is per-frame.
Groups currently use full-frame intermediates; filter intermediates are cropped
to the visible owner border box. Presenter selects composition when a display
contains backdrop scopes; ordinary displays retain the cached single-scene path.

`Compositor::render_display` owns reusable glyph preparation and image textures
for the display-list path. Keep all frame images and discovered bitmap glyphs
bound across spans. Restore caches after failed draws and clear them on suspend.
`Presenter::prepare_composited` prepares this path explicitly, retaining the
output lease until it is replaced or suspended. Presentation blits that lease's
view directly; do not release it after preparation or copy through CPU pixels.
Its encoding timing slot includes the entire multipass operation. `prepare`
selects this path automatically for authored backdrop styles. CPU `rasterize`
uses the same plan. Legacy single-scene encoding rejects backdrop scopes;
`DisplayList::scene` now returns Result rather than silently omitting filters.

Keep one sparse renderer and its paired Vello resources per compatible device
in `gpu::Context`, initialized on first draw. Ordinary and composited draws from
all its presenters share pipelines/upload buffers; page images, glyph preparation,
composition pools and retained outputs remain independent. Hold the renderer
mutex from resource mutation through queue submission: another page must not
replace staged buffer uploads before the preceding scene is submitted. Clearing
or destroying a page must not clear another page's renderer or retained output.
Include first-use initialization in encoding time.

`DisplayList::composition_plan` lowers existing clip/opacity scopes, verifies
matching closes and resolves clip coordinates/radii to device pixels once. Span
index0 is the surface background; remaining indices address paint commands plus1.
`rasterize_composited` runs the CPU reference path. GPU `sparse_span` and CPU
`paint_cpu` share the existing primitive encoders with the single-scene paths.
Do not duplicate text/image/vector drawing in the compositor or scale a clip twice.
`gpu_images::ImageTextures` owns image/bitmap-glyph texture bindings for both paths;
its input must include all images live in the frame, not just the current span.

Real View records with text, translucent images, vectors, separate opacity groups
and rounded scrolling compare against single-scene output at1x/1.5x/2x and three
retained scroll offsets. CPU differs by at most2 byte values, GPU by at most3.
The GPU test reuses one Compositor across frames. Retained composition places
each node's opacity/backdrop scope inside its ancestors' clips and outside its
own scroll-content clip. Backdrop and opacity on one node form one scope, so
its opacity applies after capture, filtering and foreground drawing. Transparent
owners still emit backdrop scopes. Scroll translates the owner bounds before
device scaling; ancestor scroll masks stay fixed. Root geometry and foreground
color are checked in authored CPU/GPU fixtures at three scales/scroll positions.

Preflight bounds the program to4096 steps/32 live levels,4096 per axis/4M pixels,
and64MiB of concurrent RGBA working storage, including capture merging and CPU
copy-on-write outputs. The GPU pool independently bounds cached plus leased RGBA
storage to64MiB. A surface returns only after its last lease drops; every internal
use is submitted first, so subsequent clears/writes execute after prior reads.
Keep a Surface lease (or clone) through the last submission; cloning a raw wgpu
texture does not extend that lease. Reuse always starts with a transparent clear.
Compositor::clear drops idle textures/depth and invalidates returns from old
leases. Live old leases remain charged until dropped. TextureStats reports owned
RGBA bytes and depth counts, not driver memory. Depth24Plus views use an LRU of
at most4 sizes/4M total pixels; their byte representation is backend-dependent.

`backdrop_region::Region` intersects the owner border box with the capture,
snaps its raster extent outward, then crops before blur/saturation. The original
fractional/rounded clip applies to the output. Reflect the cropped image across
the complete padded Gaussian input; setting only Vello's filter edge mode would
reflect its padded layer boundary instead. Obtain tile alignment and source
padding from pinned vello_common::FilterData, not duplicated constants. Bound
the padded source to4096 per axis/4M pixels before allocation. This bounds source
geometry, not Vello's total scratch/atlas reservation; profile that on-device.
This follows the mirror intent of the
[backdrop draft](https://drafts.csswg.org/filter-effects-2/#backdrop-filter-operation),
using raster-aligned capture edges. Fractional edge phase still needs visual
comparison against the browser; do not claim full CSS parity from backend tests.

CPU assertions cover opacity/foreground isolation, nested overflow capture,
transparent sampling roots and earlier sibling content. Desktop GPU executes the
same plans and compares every pixel against CPU with a3-byte-value tolerance;
draw counters ensure capture does not re-run paints. Validation rejects unbalanced
groups, invalid factors/clips/spans, replayed spans and oversized working sets.

Pool tests submit multiple frames without CPU waits, retain an old output lease,
and confirm unchanged old pixels plus zero additional RGBA/depth allocations
after warmup. Clear invalidates old returns. Mirror tests cover4px/20px blur,
translucent edge alpha, excluded exterior colors and a red/blue pattern that
distinguishes reflection from clamp/wrap. Authored Home/ToDo filters also pass
Pixel3 interaction/rotation/theme checks with inspected screenshots. This does
not establish fractional-edge browser equivalence or total GPU scratch usage.

Android keeps the SurfaceView at its last unoccluded measured extent while IME
is visible; the host clips it and View.env.keyboard carries the overlap. Do not
resize the compositor buffer to the IME-reduced parent: Android stretches the
previous GPU frame before the asynchronous replacement is ready. Width or window
configuration size changes invalidate the retained extent.


Android tab transitions capture the visible GPU buffer before dispatching the
native tab event or synchronizing chrome: either can invalidate the outgoing
surface. The activity owns one transient opaque cover, never a per-page bitmap
cache. Compose the copied buffer with the host chrome into a root-sized snapshot;
keep its pixel size and position fixed when navbar visibility changes. The cover
stays above the live navbar so its title/background change with the captured
content, not ahead of it. Live tabbar and capsule stay above it so a newer tab
selection can supersede the pending transition. Commit that cover before changing
visibility. After the incoming frame,
PixelCopy waits for its submitted buffer; commit the copied pixels before
uncovering. Navigation tickets reject stale capture/presentation callbacks.
Release the cover on failed admission/presentation, activity destruction or app
replacement. SurfaceHolder.Callback2 redraw completion follows actual GPU
presentation; complete outstanding callbacks on suspension/destruction too.

GPU push/pop prepares opaque outgoing/incoming snapshots before their synchronized
slide. Retiring a visible runtime page can precede host navigation: remove its
registry identity immediately, but cover its pixels before releasing the surface.
Pending retirement callbacks must also drain on activity destruction. The
PixelCopy buffer is consumed by a synchronous software composition and recycled
immediately afterwards; displayed snapshots remain render-thread-owned until
their ImageViews are removed and their drawable references cleared.
Runtime tabbar visibility updates can arrive before the navigation callback.
Capture and commit the outgoing cover before applying that geometry change;
a following navigation takes ownership via its version. A same-page chrome
change removes the cover after its replacement GPU frame is copied and committed.
If a surface was already resized, wait for one successful presentation before
retrying the outgoing copy, rather than falling back to an unprepared slide.
