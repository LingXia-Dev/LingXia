# Android UI renderer smoke host

First Android milestone for `docs/draft/gpu-renderer.md`. This isolated host
loads `examples/hello-ui` and renders its `index.ui.tsx` on a `SurfaceView`.
Rong evaluation, Taffy layout, Parley shaping and Vello GPU preparation run on
a dedicated worker. The UI thread blits and presents the prepared texture on
demand; surface generations reject stale frames. The worker retains the View
and font contexts. Sparse Strips preprocessing runs on the worker; Vulkan vertex
and fragment passes rasterize and composite the frame. GPU device/pipelines survive
surface suspension; viewport attachments are released. CPU reference rendering remains available.

From the repository root on Windows:

```powershell
npm --prefix packages install
npm --prefix examples/hello-ui install
cargo build -p lingxia-cli
./tools/lingxia-ui-android/run.ps1 -Sdk C:/Android/Sdk -Device <adb-serial>
node tools/lingxia-ui-android/verify.mjs C:/Android/Sdk/platform-tools/adb.exe <adb-serial>
```

Requires Rust's `aarch64-linux-android` target, JDK 17, Android SDK 35 and an
installed NDK. The script installs `com.lingxia.ui.smoke` and launches it.

Headless checks:

```powershell
cargo run -p lingxia-ui --example generate
node packages/lingxia-ui/build.mjs examples/hello-ui
cargo run -p lingxia-ui --example render -- examples/hello-ui/dist/pages/home/index.ui.js examples/hello-ui/dist/hello.png
cargo test -p lingxia-ui
node --test packages/lingxia-ui/tests/*.test.mjs
```

The Rust schema generates wire field IDs and style types; the authoring package
owns JSX and hooks. The compiler emits
an ES2020 ESM page, evaluated as a module on Android/QuickJS. Candidate primitives
cross into Rust directly, without JSON
serialization. Node bounds are exported beside the headless PNG.

Current primitives include `column`, `row`, `text`, clickable `button`, `spacer`
clipped `scroll` viewports and windowed `list` rows. Keyed state, transactional candidates and
generation-checked input share the CPU and GPU display list. Scroll frames reuse
layout and shaping and cull offscreen paint. A list window change calls only
entering row callbacks; it does not rerun the page render or ordinary components.
Unsupported props fail compilation.

`image` accepts a package-root `src` (for example `public/avatar.png`) or an
owned managed `lx://` URI, and `fit="cover" | "contain" | "fill"`. Set its
width/height explicitly; the default is 64 by 64 logical pixels. The background
is the placeholder, and radius clips the image. PNG/JPEG decode and resource IO
run off the UI thread, with at most four outstanding loads per visible page.
Late or failed loads preserve layout and do not rerun View JavaScript. Invisible
sources release decoded pixels and GPU textures; surface suspension also drops
image textures, recreating them from decoded pixels on resume. Direct remote
URLs are not supported yet; Logic can download into managed storage first.
Each image is limited to 8 MiB encoded, 4096 pixels per axis and 4 Mi pixels;
a frame admits at most 128 distinct sources and 64 MiB of decoded image data.
The SDK release showcase passed physical Pixel 3 image checks: screenshot pixels
at rows 1 and 1000, unload/reload, Home/resume and idle stopping. Its image-bearing
1000-row regression also passed rotation, retained row state and Logic actions,
with at most 87 layout nodes. This is functional verification, not the matched
WebView benchmark or a memory/frame-rate gate.


`list` accepts `count`, `row(index)` and optional `height(index)`. Give each returned
row a stable key. The renderer measures visible rows plus a screen of overscan;
unknown heights are estimated, so the scroll extent adjusts as rows are measured.
Offscreen row layout and glyphs are released, while row local state survives
window eviction. Shrinking `count` removes state beyond the new end; removing the
list releases all its row state. Candidate failures keep the last committed window.
`anchor={{ index, align: "start" | "end", seq }}` or `anchor="end"` moves only when
its value changes. Increment `seq` to repeat a jump without taking over user scrolling.
Both `scroll` and `list` accept `onReachStart` and `onReachEnd`. Android delivers
each callback once upon entering that edge in a visible, successfully presented
frame, including the initial frame. Leaving the edge arms it again; changing the
callback alone does not. Content that fits the viewport can reach both edges. Hidden regions
do not emit events, and unmounting clears edge state. Callbacks use the same
generation-checked dispatch and local state/Logic actions as buttons.
The SDK release fixture passed repeated first/end jumps on Pixel 3: callbacks
updated local status and settled without additional idle frames. The 1000-row
regression with callbacks retained at most 69 layout nodes.
The draft's complete list performance gate remains pending.
The SDK release showcase's 1000-row fixture passed on Pixel 3: first/end anchors,
row state after window eviction, drag/fling, Home/resume, rotation and Logic actions.
The run exposed at most 68 layout nodes. Its 171 scroll frames measured worker
median 9 ms / p95 18 ms; UI presentation median 1 ms / p95 2 ms across 190 frames.
These CPU wall times do not establish the draft's frame-drop or image-list gates.
Evidence is under `target/showcase-gpu-evidence/list-*` (ignored).

This isolated harness requires `logic: false`. The standard Android SDK hosts
Logic-enabled GPU pages, supplies theme/language/surface environment updates and
routes actions through the existing trusted bridge. Use that host for Logic and
host-control verification. IME, AccessKit and the draft's full performance gates
remain incomplete.

Verified on 2026-10-15 with a connected Pixel 3 (Android 12): Hello World is
visible in portrait and landscape, survives Home/resume and activity recreation,
and emits zero additional `present` log entries during a 79-second idle window.
Screenshots and local build logs are under `target/hello-ui-smoke-evidence/` (ignored).
This does not establish the draft's GPU timing, memory, IME or fuzz gates.

Interactive verification on the same device: two taps increment to two, Home/resume
preserves the count, and Android's accessibility hierarchy exposes the button and
updated text. The harness supplies virtual accessibility nodes from frame bounds.
CPU worker time was 48 ms for the first frame and 27–29 ms for updates; this does
not meet the GPU frame budget. Activity destruction releases the View state.

The Vulkan path passes `verify.mjs` on that device: real GPU backend, accessible
button, two state updates, Home/resume and 15 seconds without another frame.
After pipeline reuse, click updates measured 9–10 ms on the worker plus about
2 ms for UI presentation; resume measured 5 ms plus 2 ms. These are CPU wall
times, not GPU timestamps or the draft's p95 workload gate. Cold pipeline
initialization remains separate from frame preparation. Unlike the draft's
initial submission placement, raster encoding/submission runs on the worker:
the first UI-thread version measured up to 343 ms, so only final presentation
remains on the UI thread.

The Hello page adapts to short viewports and uses shared rounded geometry on
both backends. Android exposes keyboard activation and rounded press/focus
feedback. Configuration changes update the existing View viewport, preserving
local state; actual Activity destruction releases it. `verify.mjs` also checks
keyboard input, rotation state retention, and that every landscape node fits.

`lingxia build` now routes pages with `renderer: "gpu"` through `@lingxia/ui/compiler`,
including mixed WebView/UI projects. GPU modules retain `.ui.js` paths and renderer
metadata in the built manifest. Debug and release outputs use the same language.

Pass `-Project <lxapp directory>` to `run.ps1` to preview another standalone
GPU lxapp with `logic: false`; it uses the formal CLI build and packages that
project's output. The default remains `examples/hello-ui`. Logic-enabled showcase projects must run through the standard SDK host; they
cannot run unchanged in this static harness.

Android publishes hit targets and accessibility metadata only after presentation
succeeds. The on-device snapshot file is replaced atomically on the worker after
that presentation. Accessibility resource IDs expose the source `testId`, allowing
device tests to locate controls without fixed coordinates or translated labels.

The harness now compiles the SDK's `com.lingxia.ui.GpuView` source directly.
`SmokeActivity` supplies assets, system insets, lifecycle and evidence capture;
rendering, input and accessibility live in the reusable SDK component. JNI page
handles address isolated contexts in one app isolate on the shared View executor.
Run `node tools/lingxia-ui-android/verify-pages.mjs <adb> <serial>` to exercise two
GPU pages on that executor, switching surfaces while retaining independent local
state. It requires the Hello UI or showcase GPU sample and restores single-page
mode at the end. The standard Android SDK also connects GPU pages to their
Logic service through generation-bound LegacyV2 messages. This isolated harness
still loads static pages; use the SDK host to verify Logic actions and state.

The Android presenter now uses `vello_gpu` 0.3 Sparse Strips with direct rectangular
clips. On the same Pixel 3, the SDK release showcase's 20-card scroll run produced
228 scroll frames: worker median 12 ms / p95 16 ms. Across 238 presented frames,
UI presentation totaled median 1 ms / p95 2 ms; the present call itself measured
median 390 us / p95 749 us. Scroll, Logic clicks, rotation, appearance changes and
Home/resume passed. Logs/screenshots are in `target/showcase-gpu-evidence/`.
These are CPU wall times for this workload, not GPU timestamps or proof of the
1000-row frame-rate gate. `GPU_PRESENT` logs split lock, acquire, encode, submit
and present time so later regressions can be attributed to a stage.

Rounded scroll/list masks now use original container geometry across GPU paint,
hit testing and native feedback geometry. A high-contrast probe in the standard
Pixel 3 release host verified white outside-corner pixels, blue interior pixels,
rejected corner taps and accepted interior taps. Rust tests cover nested masks
and shared clip stacks. Feedback now draws in a noninteractive sibling above
the SurfaceView: the physical probe verifies darkening during press, unchanged
outside-corner pixels and restoration after release. Probe captures are
`target/showcase-gpu-evidence/rounded-*.png`.

With a stable filled button visible, run `node tools/lingxia-ui-android/verify-feedback.mjs
<adb> <serial> <package> <button testId>` to check press/release pixels. It requires
Python with Pillow, samples away from a centered label and activates the button.
Keyboard entry selects the first available button (last when entering backward)
and disables Android's whole-surface focus highlight. The physical rounded probe
also verifies the first Tab's clipped focus stroke and Enter activation.
Presented frames retain a visible enabled keyboard target by node identity,
including changes to its callback, label, position and scale. A missing target
clears focus; Enter does not implicitly activate another button.
The Pixel 3 release-host focus probe passed sibling insertion, label/callback
updates, portrait/landscape changes, repeated Enter, and target removal without
activating its replacement. Focus captures are
`target/showcase-gpu-evidence/focus-{initial,reordered,landscape}.png`.
