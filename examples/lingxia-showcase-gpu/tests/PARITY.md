# React showcase → GPU UI

Target: migrate the existing React showcase except the native-component gallery
and its component demos. Keep this app and its probes local. Renderer milestones
are committed independently. This checklist is not a completion claim.

This is the independent host at `examples/lingxia-showcase-gpu`, Android package
`com.lingxia.example.gpu.dev`. Leave the original React example unchanged.

| Page / workflow | Implementation | Physical Android evidence / remaining checks |
| --- | --- | --- |
| Home | GPU View; original greeting, appearance/language watchers, version Logic; app network state; managed logo | Pixel 3: greeting/flush, Chinese/English, theme, rotation, covered-page local/data retention. Managed remote background loading is verified (see later image notes). Backdrop blur/shadows and full app chrome/link/update integration remain. |
| ToDo | GPU View; snapshot persistence with legacy-key migration; atomic bulk toggle and load retry | Pixel 3: physical Add tap, toggle one/all, filters, delete, clear, empty state, page recreation/storage, dark/landscape, cold-process storage, physical text input and Enter submission. Empty-list Active/Completed filters survive page recreation. Five injected Logic tests cover read/write failures, retry without duplication, concurrent-load exclusion and disposal. Physical storage-failure UI remains unverified. |
| Clipboard | GPU View; original Logic and all actions | Pixel 3: text/typed/image roundtrip, managed image visible, types, clear and empty read. Real album image selection/write/read, picker cancellation preserving clipboard, centered preview, rotation and clear pass. Failure and keyboard flows remain. |
| Share | GPU View; original Logic and all actions | Pixel 3: system text-share sheet opened/dismissed; file picker opened/canceled. Local receiver verifies exact text, GPU page-link path/query and a readable 1024x1024 selected image. System-file selection and PDF sharing pass with byte count/SHA-256 checked by the receiver. Browser App Link association remains pending. |
| UI playground | Previous Hello/Explore/Discover page moved to `lab` | Host/performance scripts enter `lab` through Home. Other historical renderer probes still require entering `lab` first; old direct-home launch assumptions no longer apply. |
| API index | GPU navigation to Device, System, Wi-Fi, Location, Clipboard and Share | Independent Pixel 3 APK: forward/back routes pass. |
| Device and System | GPU Views with original Logic/actions | Independent Pixel 3 APK: device/screen/network, network-listener cleanup, actual landscape/portrait, host identity/settings, capability gating, cache and notification schedule/cancel pass. Device page inherits application orientation; explicit `auto` would override API requests. Phone/vibration, denied permissions and remaining failure paths pending. |
| Wi-Fi and Location | GPU Views with original Logic/actions and visible progress/results | Independent Pixel 3 APK: permission flow, Wi-Fi start/stop, listener cleanup, scan results; location visible error and reset pass. Successful location fix, denied permissions and actual Wi-Fi connection remain unverified. |
| UI APIs | GPU navigation/lifecycle, Toast, Modal, Action Sheet, navigation-bar, tab-bar and appearance Views; corresponding original Logic handlers | Independent Pixel 3 APK: navigation/state restoration, popup lifetime, native action sheet/modal results, toast, navigation title/colors, appearance and tab label/badge/visibility pass. Measured immersive-tab insets now pass show/hide, portrait/landscape and physical badge-clear taps above native chrome; side-position bars remain unverified. Surface controls now include float/window capability checks, hide/show/close, messages and URL aside entry. View-popup focus/back arbitration, accessibility speech, physical keyboard, rotation and remaining failure cases are still separate gates. |
| Pull-to-refresh | GPU View and original refresh Logic; shared native host indicator | Pixel 3: API start/stop, physical pull, short/horizontal/cancel gestures, scrolled-content boundary and leave/reentry pass. Rotation retains the native indicator and count; stop and reentry restore page position. Nested boundary and slider unit tests pass. Theme, system gesture cancellation and accessibility speech remain separate checks. |
| Leave guard | TS View with committed GPU leave guard; native Back delivery | Pixel 3: keyboard Back priority, repeated Back, keep editing, discard, save, Home and fresh reentry pass. Bridge reset invalidation and exact host-route admission have unit coverage. Native navigation-bar taps, rotation while asking and accessibility speech remain acceptance checks. |
| Feedback | TS GPU form; original Logic submit and floating surface entry; More menu entry | Pixel 3: modal excludes background accessibility; blank gating, category, hardware Enter/newline, email, success toast, programmatic/native close, fresh reentry and no idle rendering after close pass. More-menu entry, rotation, hide/show, failures and accessibility speech remain separate checks. |
| File | GPU View with PDF transfer, document fetch/write/open, managed file chooser and all original actions | Independent Pixel 3 APK: chooser cancel/selection, download/fetch errors, pause then page recreation/resume, durable fetch/write, PDF preview/back pass. Native Office handler, network timeout, rotation/theme and physical keyboard acceptance remain pending. |
| Cloud | TS Logic and GPU Auth/MQTT/functions Views; pending subscriptions invalidated on stop/unload; stale function results ignored | Pixel 3 cold embedded provider APK: login, tenant/user identity, echo/whoami, intentional failure/recovery, MQTT receive/stop/restart and broker-confirmed leave cleanup pass. Tenant A/B switching and automatic fallback after logout also pass. Seven Logic tests cover subscription and function lifecycle races. Reconnect and permission failures remain open. |
| Media API portions | TS Logic and GPU Pictures, Videos, Scan code, Image tools, Video tools and Save to album Views; original ordinary media API actions | Pixel 3 cold embedded APK: all six modes, count/source/code-type native sheets, default fields and empty-source action gates pass (`target/ui-media-device.log`). Real album permission/selection, 1024×1024 image info, nonempty 256×256 compression, visually inspected native preview and retained result on return pass (`target/ui-media-image-device.log`, `target/gpu-independent-media-preview.png`). Video selection, thumbnail, compression cancel/restart and preview return pass. Lifecycle disposal/late results have injected Logic coverage; album QR decoding and nested scanner/picker Back pass. Physical mid-task navigation, camera QR recognition, album writes, denied permissions and rotation remain unverified. Native video component controls are excluded. |
| Stream | GPU View and original generator Logic; renderer supports events/result/cancel/iterator | Independent Pixel 3 APK: live tokens, terminal result/state, explicit cancel, clear, navigation teardown and fresh stream pass. Native GPU bar/smooth-area/donut artifacts, accessible summaries, TS axes, direct selection hints and legend selection pass on the cold embedded APK in light/dark with rotation. Chart animation, exact axis styling and remaining error/input cases are pending. |
| Channel | GPU View and ticker Logic; renderer supports page-scoped bidirectional channels | Independent Pixel 3 APK: ack/initial state, live ticks, AAPL/MSFT/TSLA subscription, close delivery stop, reconnect and exit/reentry pass. Per-tick changes are rendered. Automatic entry connection, subscription retained across physical rotation, manual reconnect, exit/reentry and no idle frames after disconnect now pass. Device error/theme and remaining presentation cases are pending. |
| Bridge reproduction | GPU bootstrap/initial snapshot, unary Echo, continuous tick audit and transport capability feedback; scoped request/stream resources | Pixel 3 cold embedded APK: bootstrap, snapshot, Echo, gap-free ticks, accessibility and held-touch stop across redraws, no idle frames, restart and active-stream exit/fresh entry pass (`target/ui-stream-resource-device.log`). Bootstrap deadline, rotation and transport-replacement stress remain pending. |
| Surface | GPU child page and UI controller; original float/window/URL actions; scoped opener subscriptions | Pixel 3: desktop window capability refusal, hide/show lifecycle counts, local counter/input retention, messages delivered while hidden, reply then close, fresh state after close, no hidden idle frames, landscape width and retained state pass. URL aside error document/Retry/Back and invalid HTTP rejection pass. HTTPS success remains unverified because the device browser reports ERR_PROXY_CONNECTION_FAILED. Composed-lxapp UI, multiple surfaces, hidden rotation, remaining geometry cases and accessibility speech remain. Android results do not verify desktop windows. |
| Components, video, picker, navigator, swiper | Excluded | Native-component demonstrations per agreed scope. |

Video evidence: `target/ui-media-bars-after.log` verifies native selection of the
640×1280 fixture, duration, a 160×320 thumbnail, nonempty compressed output and
return from both previews. Screenshots confirm the thumbnail/video content.
`target/ui-media-bars-before.log` reproduces white system icons left on the light
GPU page after preview; the fixed run verifies restored contrast.
`target/ui-media-bars-unit.log` covers icon appearance combinations on API 31/35;
`target/ui-media-bars-surface-regression.log` retains float hide/show/rotation and
message behavior. Video motion, compression ratio/cancellation, denied permissions,
scanner results and album writes remain unverified.

Earlier independent-host evidence (Pixel 3 `914X1UMW2`, Android 12): all fifteen routes are GPU
pages in the installed APK. Cold-started with dev transport disconnected.
`target/ui-independent-home.log`, `target/ui-independent-todo.log`,
`target/ui-independent-device-system.log`, and
`target/ui-independent-wifi-location.log` record the new package's checks.
`target/ui-independent-clipboard-share.log` verifies text/typed/image clipboard
round trips, types, clear/empty, the system share sheet and file-picker cancel.
`target/gpu-independent-{device,orientation,system}.png` records presentation.
View and Logic both pass `npm run typecheck`.

Stream evidence: `target/ui-independent-stream.log` and
`target/gpu-independent-stream-{live,complete}.png`. Android probe sources and
drivers live under `lingxia-sdk/android/tests/gpu/`, outside the mini-app. APK inspection confirms
no Java/Python test sources or test directories in the bundled lxapp.

Channel evidence: `target/ui-independent-channel.log` and
`target/gpu-independent-channel-live.png`. The channel-stage APK's thirteen catalog
entries all select GPU modules; its 31 mini-app resource entries contain no
Android probes or Java/Python test sources.

File flow evidence: `target/ui-independent-file.log`,
`target/gpu-independent-file-{paused,preview}.png`. The local `android/scripts/file-fixture.py`
server supplies a generated PDF, throttled transfer and 404 responses on port
39871 (adb reverse); dev transport on 39782 stays disconnected during the probe.
`GpuFileProbe` checks actual PDF page count and native file selection, including
resuming a paused download from a recreated GPU page. Recreated pages retain the
transfer URL and attach a completion waiter before resuming.

Automatic-channel evidence: `target/ui-channel-resource-device.log` and
`target/gpu-independent-channel-{live,landscape}.png`. The committed-resource
runtime starts the ticker after the first successful View commit; no Connect
click is needed on entry. The probe verifies MSFT survives rotation and checks
that both incoming data and GPU presentation stop after disconnect.

UI API evidence: `target/ui-chrome-device.log` verifies navigation/state, native
dialog results, toast, navigation title/colors, theme and tab updates.
`target/ui-chrome-rotation-device.log` verifies actual button bounds and coordinate
taps above immersive chrome, hide/show viewport restoration, landscape and return.
Screenshots: `target/gpu-independent-ui-tabbar.png` and
`target/gpu-independent-chrome-landscape.png`. Both runs use the embedded APK
with dev transport disconnected. The UI API-stage APK has 14 GPU routes and 36
mini-app resources; no Android test sources are bundled.

Refresh evidence: `target/ui-refresh-rotation-device.log` verifies API/gesture
refresh, short/horizontal/canceled drags, scrolled content, rotation with exposed
native indicator space, position restoration and fresh page instances.
`target/gpu-independent-refresh-{gesture,landscape}.png` records presentation.
`target/ui-refresh-keyboard-regression.log` covers physical text input and Enter
submission on a non-refresh page. `target/ui-refresh-android-unit.log` runs 17
scroll/slider tests. The current APK has 15 GPU pages and 38 mini-app resources.

Historical evidence below uses the earlier host and must not be attributed to
the independent package without rerunning its probe.
Device: Pixel 3 `914X1UMW2`, Android 12. Repacked and installed the release dev APK,
removed the dev-server adb reverse mapping and force-stopped before testing.
Home, ToDo and Clipboard/Share probes pass against the embedded bundle.
Evidence is under repository `target/`: `ui-home-parity-probe.log`,
`ui-todo-parity-probe.log`, `ui-todo-presentation-probe.log`,
`ui-api-parity-probe.log`, `ui-{home,todo,api}-parity-offline.log`,
`ui-todo-cold-{seed,restore}.log`, `ui-todo-keyboard-parity.log` and
`gpu-{home,todo,clipboard,share}-*.png`.

Retain performance comparison, GPU tooling, accessibility speech and full
navigation/input acceptance as separate gates. Passing these workflow probes
does not establish those gates or overall product readiness.

After the multi-page migration, `verify-performance.mjs` passes through the
product Home → playground route. Evidence: `target/gpu-parity-performance-evidence`.
Counter/input/list CPU P95 totals: 14.318/16.162/29.939 ms; seven focused-idle
frames execute no View/layout, background has zero frames. This is a regression
sample, not a matched WebView comparison or a performance improvement claim.

GPU plot evidence: `target/ui-plot-device.log` and
`target/gpu-independent-chart-{bar,line,pie}.png` verify the cold embedded Pixel 3
APK, all three streamed plot kinds, colored pixels, accessible data summaries
and legend selection/reset. Screenshots were visually inspected.
`target/ui-plot-stream-regression.log` covers live data, completion, cancellation,
clear and teardown/reentry after this change; `target/ui-plot-app-errors.log`
contains no renderer/runtime errors. CPU reference pixels check donut interiors
for tessellation seams. This does not establish full chart styling parity.

ToDo recovery evidence: `target/ui-todo-recovery-final-device.log` verifies the
final embedded APK on Pixel 3 through add/toggle/filter/delete, dark mode,
rotation and empty-list filter restoration. `target/ui-todo-recovery-logic.log`
contains five injected storage/lifecycle tests. Read failure disables mutation
and exposes Retry; write failure propagates to the View without clearing its
draft or changing persisted/visible tasks. These failure paths have Logic-level
coverage, not a simulated physical-device disk failure.

Media lifecycle evidence: `target/ui-media-lifecycle-video.log` verifies native
video selection, 160x320 thumbnail, compression cancel/restart, successful output,
preview return and status-icon contrast on the cold embedded Pixel 3 APK.
`target/ui-media-lifecycle-image.log` repeats image selection, 1024x1024 metadata,
256x256 compression and preview return. Five tests in
`target/ui-media-lifecycle-unit.log` cover unload cancellation, old/new task
ownership, synchronous startup failure, late image output and preview listener
release. Compression and preview handles are invalidated on hide/unload.
This evidence does not cover every picker/scanner callback after navigation.

Scanner evidence: `target/ui-scan-back-final-device.log` and
`target/gpu-independent-scan-result.png` cover the embedded Pixel 3 APK:
scanner cancel/reopen, nested album cancellation, album QR result/type, and
system Back through the underlying GPU pages after overlays are removed.
The fixture lives in `lingxia-sdk/android/tests/gpu/fixtures/scanner-qr.png`; scanner decoding
from a live camera image remains a separate check.

GPU bootstrap waiting: Android now shows a recoverable waiting message after
five seconds without ready plus initial state, with `UI_BOOTSTRAP_TIMEOUT` in
logs. Three Android unit tests exercise the deadline and actual GpuView
replacement/late-ready/disposal behavior (`target/ui-bootstrap-unit.log`).
`target/ui-bootstrap-device.log` verifies normal bootstrap, echo, retry, stream
stop/restart and page recreation on the cold Pixel 3 APK;
`target/ui-bootstrap-errors.log` is empty. Artificial transport delay has not
yet been injected on the physical device.

Selected-image clipboard evidence: `target/ui-clipboard-image-final-device.log`
verifies the cold embedded Pixel 3 APK: album selection, image clipboard
write/read, image MIME type, centered square preview geometry, picker cancel
preserving clipboard, rotation and clear. The actual logo pixels were visually
inspected in `target/gpu-clipboard-selected-image.png`;
`target/ui-clipboard-image-errors.log` contains no renderer/runtime errors.

Share receiver evidence: `target/ui-share-receiver-final-device.log` verifies
text/page/image delivery and return to the GPU page.
`target/ui-share-receiver-payload-check.log` checks exact text, the GPU appId,
page path and query in the generated HTTPS URL, plus a readable 1024x1024 image
from the granted content URI. The album picker converts the PNG fixture to JPEG;
its recorded SHA-256 describes the received bytes, not the source PNG.
The independent host now declares the original showcase's `appLinks.hosts`.
No browser/domain association claim is made. The local receiver was uninstalled
after testing; its sources and build/check scripts remain under `lingxia-sdk/android/tests/gpu`.

File-share evidence: `target/ui-share-file-device.log` verifies Android system
file selection, receiver delivery and return.
`target/ui-share-file-payload-check.log` compares MIME type, length and SHA-256
with the deterministic PDF generated by `file-fixture.py`; all match. This
exercises imported system files, not only files already in lxapp userdata.
The temporary local receiver was uninstalled after validation.

App Link OS check: `target/ui-gpu-app-link-verification.log` reports domain
verification state `1024` for `applink.lingxia.app` on the independent dev
package. Browser auto-opening is therefore not verified. Generated URL payload
checks must not be used as evidence of domain association.

Cloud provider evidence: `target/ui-cloud-provider-final-device.log` verifies
logout/login through the authorization browser, Tenant A/User A, echo/whoami,
intentional remote function failure and recovery. `target/ui-cloud-mqtt-final-device.log`
verifies three UTF-8 messages, full topic/payload, stop without further delivery,
restart and page-leave cleanup. Broker assertions include both client ID and
topic and check presence before removal. The provider runs in the cold embedded
Pixel 3 APK against a local test daemon, not a production cloud service.
HTTP uses USB loopback; MQTT uses the LAN. This does not verify reconnect,
multi-identity switching or all payload formats.

Multi-identity and function lifecycle follow-up:
`target/ui-cloud-lifecycle-provider-device.log` and
`target/ui-cloud-lifecycle-identity-device.log` verify the rebuilt embedded APK,
including adding B, A/B activation, server whoami matching each active tenant,
and logout of B automatically restoring A. `target/gpu-cloud-identities.png`
records the identity selector. Seven tests in
`target/ui-cloud-lifecycle-final-unit.log` include old function results arriving
after a newer call and both success/failure after page unload. New calls clear
the prior result; stale completions do not overwrite the latest call.
`target/ui-cloud-lifecycle-typecheck.log` verifies both View and Logic types.
`target/ui-cloud-lifecycle-mqtt-device.log` repeats message and cleanup checks
against this APK. HTTP probes now exit after successful cleanup instead of
leaving HTTP worker threads in app_process.

Current performance resampling: `target/gpu-current-performance-evidence`
records counter/input/list CPU P95 14.392/16.297/29.396 ms. The property-table
experiment reused a cached bundle and therefore did not measure that change;
it was not retained. The installed APK kept the baseline runtime
(`target/ui-performance-restored-build.log`).
`target/ui-performance-restored-search.log` passes soft-keyboard Search,
continued editing, empty/filtered results, disabled empty-list navigation,
clear, tab/theme retention and rotation. Each edit now explicitly reopens the
keyboard before testing Back. These measurements do not establish 60 fps,
GPU execution time or a performance improvement over React.

Distant-anchor optimization: `target/ui-anchor-retained-artifact-check.log`
verifies that APK and rebuilt lab JS match. The candidate reuses old window
geometry until native determines the destination, then reevaluates all target
rows (including overlap). `target/gpu-anchor-retained-performance-evidence`
records list View evaluation P95 9.287 ms and total CPU 27.965 ms versus
12.493/29.396 ms in the baseline, with identical layout-pass distribution and
thermal status 0. `target/ui-anchor-retained-search-device.log` verifies search,
empty results, clear, tab/theme retention and rotation on the new APK.
97 UI tests pass; the Rust suite passes serially after a parallel editor-budget
timeout. These remain CPU observations, not a completed performance gate.

Linked UI package build-cache verification:
`target/ui-package-cache-integration.log` checks warm reuse, a source-only package
edit triggering a bundle rebuild, source restoration and packaged JS matching
the current dist. `target/ui-source-package-cache-tests.log` covers six cache
tests. Both CLIs were rebuilt and copied to the local tool directory.
`target/ui-source-cache-home-device.log` verifies the resulting cold embedded APK:
greeting/flush, language, theme, rotation and navigation state retention.

Smooth line and gradient area: `target/ui-smooth-chart-device.log` verifies
bar/line/donut streaming, native colored pixels, a broad gradient-filled line
area, accessible summaries, legend selection and selection retained through
landscape and back. The cold embedded APK's Stream JS matches current dist.
`target/gpu-smooth-line.png` and `target/gpu-smooth-line-landscape.png` were
visually inspected. The renderer has 127 passing GPU-enabled library tests;
98 JS UI tests and both example type checks pass. This covers monotone curves
and gradient fills, not chart animation, direct point tooltips or exact axis
styling parity.

TS chart axes: `target/ui-chart-axis-device.log` and
`target/ui-chart-axis-dark-final-device.log` verify the packaged axes, gradient
pixels, legend selection and retained selection across rotation in light/dark.
Category-label centers are checked against plot sample positions in both
orientations; five tick labels are checked against the generated data range.
The first dark run exposed a six-sample assumption in the probe; it now derives
sample count and range from the actual accessible chart summary, covering the
five-point growth sample as well as the six-point latency sample.
`target/gpu-chart-axis-line.png`, `target/gpu-chart-axis-landscape.png` and
`target/gpu-chart-axis-line-dark.png` were visually inspected.
Both example type checks pass. Axes remain TS UI composition; no native
chart library or Java code was added to the lxapp.

Direct chart selection: `target/ui-plot-selection-final-light-device.log` and
`target/ui-plot-selection-final-dark-device.log` run real touch taps on bars,
line categories and donut slices, assert the selected value and TS hint, select
another item through an accessibility action, reject donut-hole taps and verify
scrolling leaves selection unchanged. Existing rotation/legend checks also pass.
The first visual review found the hint covered a curve endpoint; the final
bundle reserves hint space and highlights the selected legend instead.
Final line/light, line/dark and donut screenshots under
`target/gpu-plot-selection-final-*.png` were visually inspected.
`target/ui-plot-selection-artifact.log` checks final APK JS against current dist.
99 JS tests, 128 Rust library tests plus the additional wire-admission test,
151 Android UI unit tests, workspace check, Windows clippy and example View/Logic
type checks pass. Speech and physical-keyboard selection remain separate gates.

Plot callback layout reuse: `target/ui-plot-retained-before.log` reproduces an
unnecessary layout pass when only the selection callback changes.
`target/ui-plot-retained-native.log` passes all 130 Rust library tests after the
fix: unchanged bar/line/pie frames rebind callbacks with zero layout passes,
preserve pixels, reject stale callbacks and retain committed callbacks after a
failed candidate. Changed data and selected indices still invalidate reuse.
This is a layout-work assertion, not an Android FPS or GPU-time measurement.
`target/ui-plot-retained-light-device.log` and
`target/ui-plot-retained-dark-device.log` pass the rebuilt cold embedded APK's
direct/accessibility selection, hint, hole rejection, scroll, rotation, axis and
clear checks. `target/ui-plot-retained-errors.log` contains no app renderer errors.
Workspace check, Windows browser-shell clippy and formatting also pass.

Subtree opacity Android acceptance (2026-10-22): `style.opacity` validates
finite unit values on JS/native sides and isolates each painted subtree. The
133-test renderer suite and the added transparent-input regression pass; all
100 JS tests and example View/Logic type checks pass. `GpuOpacityProbe` passes
on the cold embedded Pixel 3 APK with dev transport disconnected: overlapping
children, separate sibling groups, local opacity updates, scrolling, rotation
retention and light/dark GPU pixels. `target/gpu-opacity-{faded,dark}.png` were
visually inspected. The first two probe attempts exposed test-driver assumptions
(nonsemantic decorative containers and hardware screenshot pixel access); the
final probe fixes both without runtime changes.

Evidence: `target/ui-opacity-device-final.log`,
`target/ui-opacity-chart-{light,dark}.log`, `target/ui-opacity-artifact.log` and
`target/ui-opacity-app-errors.log`. Both chart regressions pass with no errors in
the app process; the embedded lab bundle matches current dist. Device settings
are restored to accessibility null, rotation 1/0; USB dev transport is restored.
Transition timing/lifecycle and remaining product/performance gates remain open.

Workspace all-target check and Windows browser-shell clippy pass after restoring
the GPU-only test guard. Logs: `target/ui-opacity-workspace-final.log` and
`target/ui-opacity-windows-clippy.log`. Formatting passes.

## Tailwind adapter foundation (2026-10-22)

The lab opacity sample now uses actual Tailwind utilities in TSX for dimensions,
colors and conditional opacity. The probe requires the new
`opacity-class-toggle` identifier so a cached pre-class page cannot pass.
`target/ui-tailwind-cold-device-final.log` verifies this page on Pixel 3 after
installing the updated dev APK, disconnecting dev transport and cold-starting.
Overlap/sibling pixels, toggles, scroll, rotation retention and light/dark pass.
Screenshots: `target/gpu-tailwind-faded.png`, `target/gpu-tailwind-dark.png`.
Earlier attempts with stale cached Views are not evidence for this milestone.
A normal `build --skip-native` did not refresh installed same-version home content;
relaunching through `lingxia dev --skip-native` installed the current dev source.

Validation: 108 JS tests; 134 GPU-feature Rust tests (two test threads to avoid
execution-budget contention); View/Logic typecheck; workspace all-targets check;
Windows browser-shell clippy; rustfmt. Logs use the `target/ui-tailwind-` prefix.
App-process error logs retain startup WebView messages and the existing update
network failure, with no UI_CLASS or GPU renderer errors observed.

This is authoring infrastructure, not full Tailwind CSS support or visual parity.
The shared theme, missing style primitives and reference page layout migration
remain open under `docs/internal/ui-styling.md`. Responsive/dark rule selection
has JS coverage; device tests here verify the sample in both themes, not those
variant conditions themselves.

## Semantic theme tokens (2026-10-22)

Home now uses semantic Tailwind color classes from `theme/index.css`, which
imports `theme/tokens.css`. The static token values match the React reference;
`tests/theme-parity.test.mjs` compares all retained declarations. Animation
tokens/keyframes and the browser pre-stamp fallback are deliberately not copied
yet; they are outside the supported token grammar, not silently discarded.

`target/ui-theme-cold-device.log` verifies reference page/surface/primary pixels
in light/dark, repeated switches and rotation on Pixel 3 after disconnected
cold startup. `target/ui-theme-home-device.log` passes greeting/flush, language,
appearance, navigation/retention, version and network-state regressions.
The installed APK Home module matches current dist. `target/gpu-theme-dark.png`
was visually inspected. Earlier `ui-theme-device.log` detected the old palette
and is a stale-bundle failure, not acceptance evidence. Reinstall through
`lingxia dev --skip-native` before relying on a watcher reload after offline runs.

111 JS tests, 134 GPU-feature Rust tests, token-parity test, View/Logic typecheck,
workspace all-target check and Windows browser-shell clippy pass. App-process
errors contain the preexisting startup WebView/update-network messages, with
no UI_CLASS or GPU errors. Dev transport is restored.

This aligns Home colors only. Layout, scenic background, glass-card effects,
segmented-control styling, navigation entry placement and other pages remain
in the visual-parity backlog. Do not treat this as completed Home parity.


## Edge padding and control text alignment (2026-10-22)

The native schema now accepts physical padding edges. Tailwind padding shorthand,
axis and edge declarations lower in CSS cascade order; inline padding overrides
class-derived edges. Buttons and single-line editors center text in the padded
content height. Input paint, caret and hit testing share the same text offset;
textareas retain top alignment. Home uses the reference 44dp input height and
16dp horizontal padding through classes.

Validation: 113 JS tests; 137 GPU-feature Rust tests plus the subsequent button
centering test; workspace all-targets check; Windows browser-shell clippy;
View/Logic typecheck; rustfmt. The installed APK Home equals current dist.
Pixel 3 disconnected-start regression passes Home greeting, language, appearance,
rotation and retention, plus ToDo physical typing and Enter submission.
`target/ui-padding-portrait-acceptance.log` verifies 44dp height, 16dp text inset,
vertical text centering and physical caret insertion in light/dark portrait.
The light screenshot was inspected. App errors retain the existing startup
WebView/update-network messages, with no UI_CLASS or GPU renderer errors.

**Failure at this milestone (resolved below):** the full `GpuPaddingProbe` failed landscape physical input.
Geometry/pixel checks pass, but opening the keyboard reduces the surface from
2160x1080 to 2160x440; Home's editor becomes completely clipped and loses focus,
then the keyboard closes. See `target/ui-padding-focus-device.log`. The explicit
`--portrait-only` acceptance does not replace the default rotation test. Preserve
the focus-on-hiding contract; follow up with viewport-aware editor reveal and
responsive page layout. Do not count this as full keyboard or Home parity.
Dev reverse transport is restored. The example and Java probes remain local.


## Viewport focus reveal and native chrome boundary (2026-10-22)

GPU replaces the WebView content region. Navbar/tabbar remain the host controls.
Page navigationStyle/navigationBar settings now match the React reference, and
shared Screen no longer draws a duplicate Back/title row. Host tabs match the
reference except the excluded Components gallery, including Media and mobile
Device entries and the reference colors/icons. `navigation-parity.test.mjs`
checks these configurations. Home/ToDo retain their reference custom navigation.

The renderer now reveals an already focused editor through scroll ancestors
before validating visibility after viewport/inset/keyboard changes. Candidate
failure rolls back offsets and focus. User scrolling with an unchanged viewport
still blurs hidden editors. Home's title joins its scroll content so a landscape
keyboard cannot consume all of its editor's available scroll area.
`target/ui-reveal-cold-device.log` and `target/ui-retained-padding-device.log`
pass full portrait/light/dark/landscape dimensions, ink centering, physical caret
insertion and keyboard persistence on Pixel 3. This resolves the preceding
landscape failure without dropping the default rotation case.

Restoring real host tab navigation exposed an independent presentation bug:
retained GPU pages could remain composited over the selected page even though
its accessibility state was correct. Retired GPU pages now pause, explicitly
hide their SurfaceView, and reject hidden surface attachment/presentation.
`target/ui-chrome-pixels-cold.log` passes native Back plus three rounds of
physical ToDo/Home/Media/Home tab taps with Home pixel checks after every return.
The fixed navbar and tabbar screenshots were inspected; the earlier
`gpu-native-tabbar.png`/initial semantic-only acceptance was insufficient and
is superseded by the pixel check and `gpu-native-tabbar-fixed.png`.

Validation: 141 Rust library tests, workspace all-targets check, Windows
browser-shell clippy, rustfmt, 152 Android UI JVM tests, View/Logic typecheck,
and two navigation parity checks. Broader Rust integration-test linking was
attempted but failed with Windows LNK1104 on output executables; those runs
are not recorded as passes. Physical Home greeting/language/theme/rotation/
retention and ToDo keyboard submission regressions pass on the final SDK.
App error logs retain the known startup WebView/update-network messages, with
no new GPU/UI_CLASS error. Examples and Java probes remain uncommitted.

This does not complete page visual parity or full Tailwind CSS capability.
Remaining work includes the shared visual primitives and reference page layouts.

`target/ui-chrome-api-regression.log` also passes navigation instance/local-state
retention, redirect/switchTab, native action sheets/modals, toast, native navbar
title/show/hide, appearance and native tab labels/badges/show/hide. Its badge-clear
button is physically tapped above the native tab bar. Dev reverse is restored.

### Home content/layout correction in progress

The previous Home did not match the React page: it omitted the full-screen
remote background and added navigation shortcuts and separate settings cards.
The View now uses one compact card over an image stack, matching the reference
content order, with small appearance/language segments and no added shortcuts.
TypeScript and the View compiler pass. This is NOT device-accepted: the current
watcher reports reload success, but the captured device frame still shows the
old layout (`target/gpu-home-layout.png`). Reinstall/reload validation is pending.
Existing probes that navigate through removed Home shortcuts or assert blue
selected appearance buttons must be updated to use native tabs and the reference
selected-segment palette; their previous results do not validate this new View.

Remote image loading is a prerequisite: `LxApp::read_gpu_image` currently only
accepts package paths and managed lx:// resources. Reuse the existing managed
URL cache (`lingxia-transfer::user_cache::download_to_user_cache`) behind the
host resource boundary, keeping resource IO out of the renderer. Also address
the decoder's 4-megapixel limit for the reference UHD background with bounded
downsampling, rather than silently rejecting a valid background or adding a
page-specific image download workaround. Blur, gradients, borders and shadows
remain pending shared rendering capabilities.

API entry corrections add cloud gating, desktop-only banner gating, separate
file demos, other-app launch, website destination selection and confirmed exit.
Six Logic tests and View/Logic typechecks pass; device interaction verification
and the reference accordion layout are still pending.

### Remote-image implementation and host checks

The host now resolves HTTP(S) image sources through the managed transfer cache,
with the shared Logic network guard. Downloads and cache hits obey the 8 MiB
bound; UHD JPEGs are downsampled to the retained 4 Mi-pixel bitmap budget. The
reference background was fetched for inspection: JPEG, 3840x2160, 3,638,104 bytes.
This supersedes the missing-loader prerequisite above, but device acceptance is
still pending the native build/install in `target/ui-image-native-dev.log`.

Verified: five bitmap tests (`ui-image-decode-test.log`), fixed/chunked/cached
transfer bounds (`ui-image-download-test.log`), local resource/owner checks
(`ui-image-owner-test.log`), four network guard tests (`ui-image-network-test.log`),
workspace all-target check (`ui-image-workspace.log`), and Windows browser-shell
clippy (`ui-image-clippy.log`). Home/Chrome probes now use native tab navigation;
Home additionally requires non-flat pixels outside the card to prove the remote
background is presented, rather than accepting an image node alone.

### Remote background accepted on Pixel 3

The full native build/install completed (`ui-image-native-dev.log`). The updated
Home greeting, language, appearance, rotation and native-tab retention probe
passed (`ui-image-home-device.log`). Inspected `gpu-home-background.png` and
`gpu-home-landscape-dark.png`: the reference photograph is rendered behind the
single card; added Home navigation shortcuts are absent.

Cold-cache acceptance uses the SAME installed channel for online priming and
offline restart. Draft and installed channels have separate storage; switching
between them is not a cache-hit test. An initial color-variance probe also
mistook the startup transition for an image; its result was discarded. The
strengthened probe requires varied saturated photograph pixels, and the final
screenshots were inspected. `ui-image-cold-online.log` and
`ui-image-cold-offline.log` pass; `gpu-home-cached-background.png` visibly shows
the photograph with the airplane-mode indicator and unavailable network text.
Airplane mode was restored to off and adb reverse 39590 restored afterward.

`ui-image-chrome-device.log` passes the native API title, clipboard native Back,
and three rounds of ToDo/Home/Media/Home with Home selected-segment pixels.
The tab-root API page correctly returns Home through the native Home tab, not a
nonexistent Back button. Borders, shadows, background blur and gradient overlay
remain missing; these screenshots do NOT establish full React visual parity.

### API catalog and accordion behavior

API now follows the React reference's eight collapsed-by-default groups and
ordered destinations, with Location as a separate entry. Cloud is provider-gated,
Desktop Banner remains desktop-only, and the three network entries carry the
reference's distinct query values. Feedback is no longer an extra API entry.
Styles use the shared Tailwind semantic palette. Entry labels/descriptions use
plain user-facing language.

`api-catalog-parity.test.mjs` extracts destinations/query values and group order
from the current reference View rather than assuming the GPU list is correct.
Together with `api-logic.test.mjs`, eight tests pass (`ui-api-catalog-tests.log`);
View/Logic typechecks pass (`ui-api-catalog-typecheck.log`). This proves entry
routing, not parity of every destination page (the Device View still combines
network demos after accepting their distinct queries).

Pixel 3 `GpuApiCatalogProbe` passes initial collapse, expand/collapse, mobile
banner gating, clipboard/native-Back expansion retention, distinct file demo
selection, and retained expansion across Home/theme/tab changes
(`ui-api-catalog-device.log`). Inspected `gpu-api-collapsed.png`,
`gpu-api-bridge.png` and `gpu-api-dark.png`. A source-encoding issue in the
expanded marker was corrected before the final run.

This is content/navigation progress, NOT visual acceptance: section/entry text
still uses centered native buttons, descriptions are not part of the clickable
row, and reference icons, separators and shadows are missing. Next shared UI
work needs composable pressable rows with explicit accessible labels/state,
followed by native icon/vector and border/shadow capabilities. Do not fake the
reference rows with invisible overlays or page-specific paint tricks.

The API probe helper opens an entry's actual accordion header when required;
assertions about collapsed absence use direct lookup to avoid doing so. Older
probes that still navigate through deleted Home shortcuts must be migrated to
native tabs; previous passes are not acceptance for this changed API layout.

### Composed API rows: implementation checks

Buttons now accept noninteractive layout/text/image children with an explicit
accessible name. The API View uses these for left-aligned section labels and
entry title/description rows. Android groups the descendants into one accessible
button; child-content changes invalidate held-press rebinding. Native decoding
also rejects nested controls and unnamed composed buttons.

Checks: 115 UI JS tests, 138 serial Rust library tests, 153 Android JVM tests,
View/Logic typechecks, eight API catalog/Logic tests, workspace all-target check,
Windows browser-shell clippy and formatting pass (`target/ui-composed-*.log`).
The first concurrent Rust run had one editor-budget timeout under build load;
the full serial rerun passed without changing its deadline or implementation.

Device acceptance remains pending the full native build/install in
`ui-composed-native.log`. `GpuApiCatalogProbe` now physically taps the description
area and checks that a composed row exposes one accessible control. Its new
screenshots use `gpu-api-composed-*` so earlier evidence is preserved. The
clipboard/share probe now enters through native API navigation instead of the
removed Home shortcuts. Reference icons, separators and shadows remain pending.

### Composed API rows accepted on Pixel 3

The full native APK was installed and launched (`ui-composed-native.log`).
`ui-composed-catalog-device.log` passes physical description-area navigation,
one accessible button per row, collapse/expand, native Back retention, distinct
file demos and theme/tab retention. Inspected `gpu-api-composed-collapsed.png`,
`gpu-api-composed-bridge.png` and `gpu-api-composed-dark.png`: labels and
descriptions align left, with markers at the right edge and no overlaid text.
`ui-composed-clipboard-share-device.log` also passes clipboard text/typed/image
round trips, types, clear/empty, native share sheet and file-picker cancellation
through the updated API navigation.

This supersedes the pending device acceptance above. Reference vector icons,
separators, shadows and centered header illustration remain missing. Device
network destinations still require separate content/layout, so neither this
API milestone nor the passing route inventory proves whole-showcase parity.

### Device content and Tailwind layout

Device now renders the reference mode selected by its API entry, without the
extra section menu. Network type, local IP and network-listener views are
distinct. Device/screen results use labeled fields; the other modes retain
separate vibration, dial and orientation controls. TSX uses the shared Tailwind
palette, card hierarchy and reference emoji. Gradients, borders and shadows
still await renderer capabilities.

View/Logic typechecks and two routing/listener lifecycle tests pass
(`ui-device-layout-typecheck.log`, `ui-device-layout-logic-tests.log`). Pixel 3
`ui-device-layout-device.log` verifies distinct route titles/result controls,
actual device/network results, listener start/stop/reentry, physical landscape
and portrait, empty dial gating and dark IP presentation. Inspected
`gpu-device-layout-{info,ip,dark-ip,landscape}.png`. This was the live dev build,
not a repacked cold embedded APK.

A separate injected documentation-address fixture exposes a remaining layout
defect: unbroken IPv6 text overflows the result card. The real network currently
has no IPv6 address, so its passing result cannot prove wrapping.
`ui-device-layout-long-ip.log` FAILS and `gpu-device-long-ip-before.png` confirms
the overflow. Add shared word-break support (reference uses `break-all`) through
Tailwind lowering, schema, native Parley shaping and text-cache identity; do not
insert invisible characters into addresses. Re-run the long-IP probe and normal
Device probe after the native build. The injected page is removed afterward.

### Shared word breaking implemented; device build pending

Native styles append `wordBreak` (63) and `overflowWrap` (64), inherited through
parents and included in shaped-text cache identity. Tailwind lowers `break-all`,
`break-keep`, `break-normal`, `wrap-anywhere`, `wrap-break-word`, `wrap-normal`.
The reset utility must preserve both declarations. Device result values now use
`break-all`; no invisible characters are inserted into the original addresses.

141 Rust library tests and 116 JS tests pass (`ui-wordbreak-rust-all.log`,
`ui-wordbreak-js-all.log`). Native tests exercise inherited wrapping and repeated
mode switches at two scales, asserting unchanged address text, increased line
height, containment and CPU rasterization. Wire tests reject invalid enum values;
cache tests distinguish each mode. Workspace/all-target check, Windows
browser-shell clippy and View/Logic typecheck pass.

The full native build is running in `ui-wordbreak-native.log`; physical long-IP
acceptance is still pending. The updated `GpuDeviceLayoutProbe long-ip` compares
IPv6 bounds against the short IPv4 result edge and height and requires exact
unchanged text. After install, re-run ordinary Device flow, then inject the
documentation-address fixture into a fresh localIP page, run `long-ip`, inspect
its screenshot and close the fixture page. Do not attribute the prior failure
log to the new renderer until this run completes.

### Long-address wrapping accepted on Pixel 3

The native build/install completed (`ui-wordbreak-native.log`). Device flow
regression passes (`ui-wordbreak-device.log`), including distinct modes, network
listener stop/reentry, actual orientation, empty dial gating and dark IP layout.
The identical documentation-address fixture that previously overflowed now
passes `ui-wordbreak-long-ip.log`: IPv6 stays within the result edge, its height
exceeds the single-line IPv4 value, and accessible text is byte-for-byte unchanged.
Inspected `gpu-device-layout-long-ip.png`: the address visibly wraps onto two
lines inside the card. The probe closes the fixture page afterward. This
supersedes the pending physical word-breaking acceptance above; it does not
prove a real IPv6 network connection.

### Shared borders: geometry/paint foundation in progress

`lingxia-ui/src/border.rs` builds a single nonzero-fill ring with reversed inner
contour, unequal edge widths, normalized radii and elliptical inner corners.
`Paint::Border` is handled by CPU, sparse GPU and Vello encoders and retained
scroll translation. Four geometry/raster tests pass (`ui-border-geometry.log`):
transparent center, unequal widths, rounded corners, zero/oversized widths, and
single alpha coverage at corners. GPU-feature compilation passes
(`ui-border-gpu-check.log`). Windows test linking transiently failed with LNK1104
on the output exe; the subsequent run linked and passed.

This is UNCOMMITTED and not exposed as a public style yet: no source creates
`Paint::Border`, so the dead-code warning is expected until integration. Next:
append width/color/edge fields after current field 64; lower real Tailwind
border declarations and their custom-property defaults; account for border
in Taffy sizing, text/editor origins, and scroll clipping/hit geometry. Reuse
the inner rounded geometry for clipping rather than treating a border as
extra padding. Cover unequal inner radii, scrolling and translucent corners
in physical-device checks before adding separators/card borders to the Views.
No screenshot or current APK proves this capability yet.

### Elliptical clip transport and Android masks

`ClipSnapshot` now has optional per-corner `radii` pairs (TL/TR/BR/BL); existing
uniform-radius snapshots retain their old paint path. Elliptical clips use the
shared rounded-box path, hit test and intersection geometry. Android GpuClip
uses matching normalized radii for touches and native highlight paths, including
malformed-radius rejection. Native-graphics JVM testing verifies the actual
highlight mask rather than relying on Robolectric's legacy Path approximation.

146 Rust library tests, 155 Android UI tests and GPU-feature compilation pass
(`ui-border-clips-rust.log`, `ui-border-clips-android.log`,
`ui-border-clips-gpu.log`). Border styles remain unexposed and uncommitted.

Integration still required: a translated inner content clip for bordered scroll
containers, Taffy border edges plus text/editor content origins, and Tailwind
width/color/solid-style lowering. Input background/border painting must stay
outside its inner text clip: simply replacing the input's existing outer mask
would erase the border. Split background from text layers while preserving
`input_paints` ranges and editor foreground replacement. Native min-size rules
for controls must include border edges; plot/slider custom geometry and image
content bounds also need explicit treatment before admitting styles on them.

### Border style integration (device acceptance pending)

Fields 65-70 now expose solid border width/color and physical edge widths.
Taffy sizes borders; editor origins include them; scroll descendants retain
elliptical inner clips. Image/plot/slider content uses inner bounds, including
Android slider mapping, plot selection and held-gesture invalidation.
Tailwind utilities preserve generated defaults, cascade and currentColor.
Device cards/results/phone input now use reference line-100/line-200 tokens.

149 Rust library tests pass serially; an earlier concurrent run hit the existing
View execution-budget test timeout. 117 JS and 158 Android UI tests pass;
GPU-feature compilation, native wire color regression and example typecheck
pass. Logs: target/ui-border-layout-{rust-serial,js,android,gpu}.log,
ui-border-wire.log, ui-border-typecheck.log. The corner raster test samples the
solid border between outer/inner curves rather than an antialiased outer edge.

Full Android build started in target/ui-border-native.log (exec session 47357).
No current screenshot proves border acceptance. Next: inspect build completion,
exercise Device and long-IP regressions on Pixel 3, add physical asymmetric
border/scroll/input/slider/plot/image acceptance, then workspace and browser-shell
gates before committing renderer changes. Example remains uncommitted.

Border continuation: API accordion rows now use first-row line-100 and later
line-200 separators. Added lab/border-fixture.tsx behind data.rendererFixture
=== 'border' (normal lab unchanged), plus GpuBorderProbe. It checks actual GPU
pixels and taps for translucent corners, asymmetric scroll clipping, currentColor,
slider/plot content bounds, image border and long input foreground. Probe jar
compiled successfully; fixture and API example typecheck pass. API catalog/Logic
and Device route regressions pass (10 tests, ui-border-page-regressions.log).

Review caught rounded plot hit detection ignoring its own inner content clip;
GpuPlot now checks that clip, with an elliptical-corner regression. All 158
Android UI JVM tests pass after this fix. Workspace all-target check and Windows
browser-shell clippy pass (ui-border-workspace.log, ui-border-clippy.log).

Android native build session 47357 is still live, rustc PID 3536 observed with
increasing CPU (~384 seconds at last check). Do not restart solely for slow LTO.
The added fixture/API edits happened after initial JS bundling: after install,
trigger the existing dev watcher rebuild before navigating to lab and setting
getCurrentPages()[getCurrentPages().length-1].setData({rendererFixture:'border'}).
Then push target/gpu-independent-probe.jar and run GpuBorderProbe. Run ordinary
GpuDeviceLayoutProbe and the existing long-IP fixture as well; inspect screenshots.
No physical border results yet, no renderer commit yet.

### Solid border milestone accepted on Pixel 3

Renderer committed as 33e5a01ba (example/probes remain uncommitted).
Full Android build installed successfully; dev watcher remains session 47357.
After a View rebuild, GpuBorderProbe passed actual GPU pixels and touches:
translucent corners blend once, asymmetric scroll clipping excludes border
hits while scrolled content changes blue-to-green, currentColor resolves,
slider/plot inner geometry maps taps correctly (including rounded plot corner
rejection), image borders persist, and long editor content cannot overwrite
borders. Extended probe resets to MMMM, taps the 14px content origin, physically
types Z, checks ZMMMM, hides keyboard, then captures a settled screenshot.

GpuDeviceLayoutProbe passes ordinary modes/listener cleanup/orientation/dark IP;
long IPv6 documentation fixture wraps inside the bordered result card unchanged.
GpuApiCatalogProbe passes after fresh app startup: initial collapsed state,
expansion, description-area taps, navigation/file routes and theme/tab retention.
Two earlier API attempts failed preconditions (Device group retained expanded,
then reload transition had no visible native tab); fresh startup resolves them.
No app storage was erased. The final UI_ log query was empty.

Evidence: target/ui-border-device.log, ui-border-device-layout.log,
ui-border-long-ip.log, ui-border-api-device.log; screenshots gpu-border-fixture.png,
gpu-border-device-info.png, gpu-border-device-dark-ip.png,
gpu-border-device-long-ip.png, gpu-border-api-bridge.png. Inspected all images.
This is live dev / persisted development resource validation, not a disconnected
fresh embedded APK fixture test. Solid borders are verified; overall showcase
visual parity is still incomplete (remaining per-page structure, icons, shadows,
gradients, text alignment and other styling/workflow requirements remain).

### Device result hierarchy and full-radius utility

Device/screen results now include the reference title and colored capsule;
query headers have dividing lines; result rows have line-200 separators except
on the final row. Restored result padding, 48px header icon boxes, 16px header
gaps and 10px query-button vertical padding. Screen/network/IP icon colors now
follow the reference families. Removed the arbitrary 12-line result-value cap.
Still pending: gradients/shadows, right-aligned wrapped values/percentage max
width, and remaining mode-specific details; this is not full Device acceptance.

Initial physical Get-info rendering exposed unsupported rounded-full output
(calc(infinity * 1px)); fixed in the shared Tailwind adapter and committed as
8e9039888. Only the radius lowers to the native bound; infinite width still
fails. 118 JS tests pass. Example typecheck and physical Device mode/listener/
orientation/dark regressions plus long IPv6 wrapping pass after the fix.
Evidence: ui-rounded-js.log, ui-device-details-{typecheck,device,long-ip,errors}.log;
screenshots gpu-device-details-info.png, gpu-device-details-dark-ip.png,
gpu-device-details-long-ip.png. Device info screenshot inspected: title capsule,
header separator and four result dividers present, no trailing result divider.
Hot reload while a pushed page was active again left no clickable API tab;
process restart restores the expected root chrome for the probe. Track this
reload lifecycle issue separately rather than counting it as a page pass.

### Inherited text alignment in progress (uncommitted)

Appended wire field 71 textAlign: start/end/left/center/right/justify. Native
layout inherits it through parents, retaining the existing centered button
default unless explicitly overridden. Parley alignment runs after final line
breaking and before glyph/editor geometry. Alignment-neutral shaped-cache
entries are reused safely; cache/update raster regression passes. Single-line
input/search use NoWrap and an alignment box at least the full text width,
keeping long text origins nonnegative for caret scrolling. Right-aligned short
inputs may scroll 1.5 logical pixels to reserve the caret itself.

Tailwind text-* alignment utilities now lower directly. Generated JS schema and
TS types updated. Native tests cover inherited multi-line LTR/RTL start/end vs
physical edges, overrides, cached positions/pixels, and input/search caret hit
geometry at 1x/2x with short and overflowing text. 119 JS tests pass. Workspace
all-target, GPU feature, Windows browser-shell clippy and example typecheck pass.
Logs: target/ui-text-align-{rust-all,cache,js-all,workspace,gpu,clippy,typecheck}.log.

Device InfoRow now uses text-right (must install the new SDK before testing).
Added lab/alignment-fixture.tsx behind data.rendererFixture === 'alignment' and
GpuTextAlignProbe; probe jar compiled. It measures physical glyph bounds across
six alignments, RTL behavior and explicit overrides; then physically types into
right/center editors and checks exact values. Enter lab via lxdev, set the last
page data to {rendererFixture:'alignment'}, push the latest jar, run the probe.
Also rerun ordinary Device and long-IP probes and inspect updated screenshots.

Old dev watcher stopped. New full Android build is LIVE exec session 84391,
log target/ui-text-align-native.log. Fixture and Device edits were made BEFORE
this build, so initial assets should include them. No physical alignment result
or commit yet. Do not restart slow native LTO without confirming terminal state.

Text-alignment continuation while native build runs:
- Device vibration/phone headers now use the reference compact 40px icon layout,
  mode colors and p-6 sections. Orientation has its missing descriptive header,
  violet/indigo lock buttons and two equal grid columns. Other paired action
  rows now use two-column grids. Event logs use a 224px max-height nested scroll,
  intact joined text and break-all, with reference Clear button spacing.
- API composed buttons explicitly use text-left. This is necessary because the
  new inherited textAlign correctly propagates a button's default center value
  into descendant labels; ordinary action buttons retain centered labels.
- Added GpuDeviceLayoutProbe orientation-logs mode. Navigate to orientation and
  inject orientationEvents = Array.from({length:80},(_,i)=>'Event '+String(i).padStart(3,'0')+' portrait');
  it verifies bounded independently scrollable content, all 80 events retained,
  scroll-back availability after scrolling, captures the final log and clears it.
- Typecheck and 10 API/Device Logic/catalog regressions pass. Updated probe jar
  compiles (ui-device-modes-probe-compile.log). No physical results yet for these
  final View changes.

Build 84391 remains live, native rustc PID 10332 observed at ~337 CPU seconds,
347 elapsed. Latest View edits above occurred AFTER its initial bundling;
trigger dev watcher rebuild once install is complete. Then restart the app
process (no data erase) to establish normal root tabs before probes. The earlier
reload issue with pushed-page/native tab state remains separately pending.

### Text alignment accepted on Pixel 3

Renderer/adapter committed as 673efbe6e; example and probes remain uncommitted.
New native build installed; live dev watcher is exec session 84391. Latest
Device/API View edits were rebuilt after installation. GpuTextAlignProbe passes
multiline physical glyph-edge checks, RTL start/end versus physical edges,
explicit left overrides, mode/cache cycling and actual right/center input taps
and typing (MMMMZ then AMMMMZ). Screenshots inspected: gpu-text-align-right.png,
gpu-text-align-input.png. Native/Rust/JS gates remain 153 Rust tests, 119 JS tests,
workspace all-target, GPU and Windows browser-shell clippy passing.

Ordinary Device probe passes including listener cleanup, actual orientation,
empty dial gating and dark IP. Long IPv6 wraps with the second line now correctly
right-aligned (gpu-text-align-device-ip.png). New orientation-logs probe passes
with 80 injected fixture events: independent max-height scroll, final Event 079
visible and clear restores --. Screenshot gpu-device-orientation-log-end.png
inspected. These 80 events are a layout fixture, not 80 physical sensor events.
API catalog regression passes after full startup; description-area taps, file
routes, theme/tab and native Back retention remain intact. Inspected
 gpu-text-align-api.png: descriptions remain left-aligned via explicit classes.
Current process LingXia.UI error log is empty.

Logs: ui-text-align-{device,device-layout,long-ip,api-device,runtime-errors}.log,
ui-device-orientation-logs.log. One early fixture attempt raced View rebuild and
lost fixture data; reinjection after reload resolves it. An early Device attempt
raced cold app startup and stayed Home; rerun after settled startup passes.
No repeated failed build or blocked dependency remains. Next visual priorities:
shared gradient backgrounds/shadows and remaining per-page reference details;
full showcase scope remains active and incomplete.


### Linear gradients: implementation and desktop checks; device verification pending

Shared native `backgroundGradient` (field 72) now reaches CPU, sparse GPU and
Vello paints. CSS corner geometry, sRGB/Oklab interpolation, transparent colors,
ordered stops outside 0..1, rounded backgrounds and retained scroll translation
share one implementation. Tailwind compiler output lowers to typed per-rule
metadata; the View combines direction/from/via/to in CSS cascade order. Supports
conditional colors, percentage positions, stop fixup, `bg-none` and inline reset.
No runtime CSS parser or page-specific simulated gradient is introduced.

Device now uses the reference gradients for background, results, buttons and
icon panels. Lab `rendererFixture: 'gradient'` and `GpuGradientProbe` cover screen
pixels for horizontal/corner gradients, Oklab, alpha, hard stops, border/radius
and reset/reuse. The probe has compiled but has NOT yet run on the new native APK.

Desktop evidence: ui-gradient-rust-all.log (158 pass), ui-gradient-js-all.log
(122 pass), ui-gradient-workspace.log, ui-gradient-gpu-check.log,
ui-gradient-windows.log, ui-gradient-ui-clippy.log (warnings),
ui-gradient-typecheck.log and ui-gradient-bundle-check.log. The bundled TSX
fixture emits five gradient fields. Native Android build is running in
ui-gradient-native.log; wait for installation, then force a View rebuild and
wait for reload before injecting fixture data. Run GpuGradientProbe followed by
GpuDeviceLayoutProbe, inspect captures and current-process renderer errors.
This is implementation progress, not physical-device or full showcase acceptance.

Home now uses the reference three-stop photo overlay, translucent card border,
centered header and IP status dot/monospace text. Standard Tailwind system font
stacks now lower to existing native generic families; arbitrary custom faces
remain rejected. Tailwind tests: 17 pass; Home bundle and TS typecheck pass.
These Home visual edits await the same gradient APK/device pass.


### Linear gradients: physical Pixel 3 verification complete for this primitive

New native APK installed; latest View resources reloaded before fixture injection.
GpuGradientProbe passes pixel assertions for horizontal gradients, non-square
corner directions, sRGB vs Oklab, premultiplied alpha, CSS hard-stop fixup,
rounded borders and inline reset/reuse. First fixture used decorative containers
which are absent from accessibility; changed probe fixtures to labeled text
nodes, then reran successfully. No renderer failure was involved.

GpuDeviceLayoutProbe passes separate routes/results, network listener cleanup,
physical orientation, empty-phone gating and dark IP layout. GpuHomeParityProbe
passes greeting/flush, language/theme preferences, rotation, covered-page local
state retention, version and network status. Home remote bitmap check also passes.
Current process renderer-error log is empty. Inspected gpu-gradient-fixture.png,
gpu-gradient-home.png, gpu-gradient-device-info.png and
 gpu-gradient-device-dark-ip.png. Home now has the reference photo overlay,
card border, centered header and IP status indicator/monospace text.

Logs: ui-gradient-device.log, ui-gradient-device-layout.log,
ui-gradient-home-parity.log, ui-gradient-home-device.log,
ui-gradient-runtime-errors.log. Evidence uses live/persisted dev resources,
not a disconnected fresh embedded-bundle installation. Backdrop blur, shadows,
remaining page details and full showcase acceptance remain outstanding.


### Outer shadows: implementation complete; new Android build in progress

Shared boxShadow field 73 supports bounded outer layers, inherited currentColor,
signed spread, CSS radius adjustment, owner knockout and Gaussian blur. The three
backends use native blurred-rectangle paints; shadows do not change hit geometry,
layout or scroll extent. Offscreen owners can still cast visible shadows.
Editor clipping starts after shadow paints, preserving native foreground redraw.

Official Tailwind shadow output lowers to typed layers plus cross-class color
metadata, with CSS order, variants, shadow-none and inline overrides. Insets,
rings, unsupported variable compositions and out-of-budget values fail visibly.
Supported color/gradient @supports branches now work around conditional rules
as well as inside a rule. Nested transparent color mixes retain precision until
final byte conversion.

Home card and selected appearance/language segments, Device panels/buttons and
API cards now use their reference shadows; API introduction is centered. Lab
rendererFixture='shadow' and GpuShadowProbe are ready for screen pixels and real
input, compiled into the independent probe jar. Native build log ui-shadow-native.log
must reach installation before reloading page resources and injecting this fixture.

Desktop checks: 163 Rust tests, 126 JS tests, workspace all-targets, GPU check,
Windows browser-shell clippy, TS typecheck and four-page bundle build pass.
Logs use ui-shadow-* names. Physical shadow verification and full showcase
acceptance remain pending; these edits are not a completion claim.


### Outer shadows: Pixel 3 verification passed

New native APK installed (streaming install fell back successfully to adb install),
then latest View resources reloaded before fixture injection. GpuShadowProbe
passes screen pixels for transparent-owner knockout, ordered translucent layers,
blur falloff outside input bounds, Tailwind color override and reset/reuse.
Real typing changes MMMM to MMMMZ; the external shadow survives keyboard open,
native editing and keyboard dismissal.

HomeParity, ApiCatalog and DeviceLayout probes pass on the new build. The existing
GpuBorderProbe also passes translucent border corners, asymmetric scroll clips,
slider/plot geometry, bordered images, long input foreground and real typing.
Current process renderer-error log is empty. Inspected gpu-shadow-fixture.png,
gpu-shadow-home.png, gpu-shadow-api.png, gpu-shadow-device-info.png and
 gpu-shadow-device-dark-ip.png. Home network lookup returned its unavailable
status during one capture; the cached remote image remains visible.

Evidence: ui-shadow-{device,home-device,api-device,device-layout,border-regression,
runtime-errors}.log. This verifies the shared outer-shadow primitive and these
page regressions with live/persisted dev resources, not full showcase parity or
an offline fresh embedded bundle. Backdrop blur, vector icons, percentage sizing,
per-corner radii and remaining page-by-page content/layout alignment stay open.

### Percentage dimensions: Pixel 3 verification passed

Shared width/height and min/max percentage support now lowers official Tailwind
output to append-only native fields, preserving existing numeric lengths.
Device InfoRow uses max-w-[72%] and justify-between to match the React value
constraint while retaining the right edge for short values.

GpuPercentProbe checks padded/bordered content-box sizing, w-full, w-1/2,
percentage min/max widths, fractional/max heights, right-aligned full IPv6 and
320-to-240-to-320 logical-pixel parent resizing. The initial probe incorrectly
required intrinsic text width to equal its maximum; a separate w-full/max-width
node now checks exact clamping, while text checks its upper bound and right edge.
DeviceLayout long-ip, full DeviceLayout, HomeParity and ApiCatalog pass after the
new native APK installation and View reload. Inspected gpu-percent-narrow.png,
gpu-percent-device-long-ip.png and gpu-percent-device-info.png. Current renderer
error log is empty. Navigation/tab bars remain native.

167 Rust tests pass serially; the initial parallel run hit the existing editor
JavaScript execution deadline under Android build load, and its isolated retry
passes. 128 JS tests, workspace all-targets, GPU check, Windows browser-shell
clippy, example typecheck and four-page compilation pass. Evidence logs use
ui-percent-* names. These checks use live/persisted dev resources. Backdrop blur,
vector icons, per-corner rounding and remaining page content/layout parity are
still open; this is not full showcase acceptance.

### Vector paths and reference icons: Pixel 3 final verification passed

Added a shared decorative vector primitive with bounded SVG path data, inherited
fill/stroke color, centered viewBox fit and shared CPU/sparse-GPU/Vello outlines.
API now uses the reference bolt, category, location, exit and chevron paths.
Home greeting now includes the reference filled success icon, green border and
untruncated wrapping text. Navbar/tabbar remain native.

First native build passes GpuVectorProbe pixels for fill, stroke holes, color
updates, layered paths, centered viewBox and retained scrolling; composed buttons
still expose one accessible control. ApiCatalog and HomeParity pass. Inspected
gpu-vector-{fixture,api,home}-first.png; current process renderer errors are empty.
Evidence logs: ui-vector-{device,api,home}-first.log and ui-vector-runtime-first.log.

Final code adds a coordinate-budget check after viewBox transformation and its
regression test. 173 Rust tests pass serially, 130 JS tests pass, and workspace,
GPU, Windows browser-shell, example typecheck and four-page compilation pass.
One serial Rust attempt during Android linking hit the existing editor JS deadline;
isolated retry and the complete run after linking both pass.

The final native rebuild and APK installation succeeded (ui-vector-native-final.log).
Normal named-session startup then failed because another workspace owns a 0.27.0
broker while the frozen GPU CLI is 0.25.1. The other workspace's 0.27.0 CLI can
connect to this host but cannot compile its GPU page format. Do not terminate
that workspace's broker or assume a showcase-gpu watcher is still running.

Final verification used the installed APK with persisted dev resources and a
short-lived loopback endpoint at 127.0.0.1:39590 via adb reverse. The local helper
(target/gpu-vector-session.py) accepts protocol-v2 runtime hello and sends two
fixed lxapp.eval requests for this app: navigate to Lab, select vector fixture.
It does not modify project protocol/auth code or register with the broker.
GpuVectorProbe, HomeParity and ApiCatalog pass again on the final native binary.
Renderer-error log is empty. Inspected gpu-vector-{fixture,home}-final.png and
 gpu-vector-api-dark-final.png. Evidence: ui-vector-{device,home,api}-final.log,
ui-vector-runtime-final.log and ui-vector-direct-session.log. The helper and its
adb reverse mapping are stopped after verification. This is not a fresh offline
embedded-resource acceptance run. Full showcase parity, backdrop blur,
per-corner rounding and page-by-page layout work remain pending.

### Next visual priority: ToDo structure

Current source audit: the GPU ToDo still has no Tailwind stylesheet/classes,
uses its own blue palette, extra Back/title/Add controls, separate task cards
with status rows, and always-visible filters/clear action. The React reference
uses a centered translucent task panel over a muted red wordmark, an Enter-submit
input, contiguous task rows, and a conditional count/filter/clear footer.
Functional ToDo probes therefore do not establish visual alignment. Prioritize
this page structure after the vector milestone; preserve persistence, atomic
toggle-all, load retry and keyboard behavior. Reference styles also need round
check indicators and completed-label strikethrough; do not silently count those
as supported. The old GpuTodoProbe still assumes Lab shortcuts and the added
controls, so update its navigation/assertions alongside the actual page.

### ToDo TSX/Tailwind structure (in progress; not device-verified)

The View now follows the React task panel: Enter-only input, contiguous task
rows, exact no-task copy, conditional footer and completed-only clear action.
Removed the extra Back/Add controls and status subtitles. Preserved the existing
atomic Logic actions and draft/error/retry behavior. Typecheck and direct
Tailwind page compilation passed. Still outstanding: circular checkbox styling,
completed-text strikethrough, desktop horizontal footer, backdrop blur and final
wordmark/layout fidelity. Existing ToDo probes target the old invented controls
and must be updated to native-tab navigation and Enter submission.

Device verification attempt: target/gpu-todo-session.py serves the existing dev
HTTP manifest/file protocol on loopback 39590 (ADB reverse), avoiding the other
workspace broker. Manifest uses sorted paths and the CLI SHA256 hash format.
Initial HTTP responses needed Content-Length; added it. Restart timed out and
reconnected; disable repeated restart on reconnect. After a cold launch the
runtime selected the dev bundle, but rendered a blank/WebView timeout rather
than GPU content, so target/todo-parity.png is failure evidence, NOT a passing
visual screenshot. Investigate installed native/runtime vs dev bundle pairing
before counting this change as verified. No user storage was cleared.
The helper and ADB reverse were stopped after investigation.

### ToDo device recovery and keyboard verification

The blank/startup failure was the temporary endpoint missing the dev bootstrap
message `session.network.mock.ready` with `{baseline:null}`. This app has no
mocks directory; send the same ready message as the official dev server after
the runtime hello. Without it, the native mock gate waits 30 seconds and shuts
down Logic. Earlier apparent recovery was connection teardown releasing that
gate. No runtime gate or authorization changes were needed.

The helper now serves the complete frozen-CLI build output, with Content-Length
and Connection: close; never overwrite a built module separately from its
lxapp.integrity.json. Runtime connection is not page readiness.

Pixel 3 now shows the new ToDo panel. Updated GpuTodoKeyboardProbe navigates via
the native ToDo tab, physically types, submits with Enter, verifies draft clear,
deletes the created task and returns via the native Home tab. It removes only
its exact GPUKeyboard fixture on retry. Final run passes; renderer error log is
empty. Evidence: target/ui-todo-keyboard-device.log, ui-todo-runtime.log and
gpu-todo-empty-parity.png. Increased delete and filter hit regions to 44dp after
the physical probe caught the smaller reference-sized delete target. Typecheck
and full lxapp build pass. This verifies the basic interaction and empty layout,
not the remaining checkbox/strikethrough/theme/desktop/full-parity work.
Helper and reverse mapping stopped after verification; installed app remains
on Home, user storage preserved and probe tasks cleaned up.

### ToDo footer, shadow and interaction coverage

The panel now uses the reference three-layer light/dark shadow through Tailwind
arbitrary shadow utilities. Mobile filters fill the footer width; above 768
logical pixels the count and filters share a row. Clear-completed occupies the
next row (the reference wraps it at the 550px panel limit). The desktop branch
is typechecked/built but still needs a viewport wider than 768px for inspection;
Pixel 3 landscape does not establish that branch.

GpuTodoParityProbe passes on Pixel 3: add two tasks; bulk complete/reactivate;
individual toggle; active/completed filters; count and clear visibility; native
tab navigation; dark theme; physical rotation; clearing completed preserves
active; filtered-empty does not show the no-tasks message; deleting the last
task removes the footer. It requires an empty baseline before bulk mutations
and cleans only its named fixture tasks. Native rotation is restored afterward.
The initial probe needed to wait for loaded empty/count content, and Home theme
controls use their own helper rather than the ToDo 44dp assertion.

Evidence: target/ui-todo-parity-device.log (PASS), ui-todo-parity-runtime.log
(empty), gpu-todo-parity-{dark,landscape}.png (inspected). Full lxapp build and
typecheck pass. Remaining gaps include checkbox circle/tint, strikethrough,
backdrop blur, precise typography/wordmark behavior and desktop visual proof.
The old GpuTodoProbe main still targets the previous layout; use the new parity
probe and updated keyboard probe for current checks.

### Shared strikethrough implementation (pending native/device verification)

Added append-only style field 81 textDecorationLine (none | line-through),
generated schema/types and Tailwind lowering for line-through/no-underline.
Unsupported underline still fails. Renderer emits an ordinary retained Rect
per visible shaped run using font strike offset/thickness and aligned run
advance, so existing CPU/GPU paint, clip and scroll paths apply. Descendant
none cannot remove ancestor decoration; the outermost decorating ancestor
provides color. Multiple nested independent strikes remain unsupported.

ToDo completed checkbox labels now use line-through in source. DO NOT sync the
new bundle to the previously installed native APK until rebuilding native UI: it
does not know field 81. The on-device bundle remains the previously verified
ToDo without strike. This milestone is uncommitted pending physical validation.

174 Rust tests pass, including multiline/right-aligned ancestor decoration and
local toggle/cached-shape pixel restoration. 131 JS tests pass, GPU cargo check,
example typecheck and ToDo page compilation pass. Evidence logs:
target/ui-decoration-{rust,js,gpu-check,test}.log. Remaining next steps: native
Android rebuild/install, wrapped completed-task visual probe, scroll/scale
verification and platform gates before committing shared renderer files.

### Strikethrough native verification preparation

Added Lab rendererFixture=strike (strike-fixture.tsx) and GpuStrikeProbe. The
probe counts black bands through whitespace in two 32px mono text lines:
none before toggle, two afterward, one initially clipped in a 96px scroll
viewport, two after retained scroll, and none after disabling decoration.
65 probes compile. Fixture/page typecheck and full lxapp build pass. Workspace
all-target check and Windows browser-shell clippy pass as well.

Android native build is ACTIVE at this checkpoint: exec session 36576, log
target/ui-decoration-native.log; last observed rustc PID 19976 performing
release LTO (CPU increasing, no error). Do not restart it based on timeout.
Built directly with cargo rustc --lib --crate-type=cdylib --target
aarch64-linux-android --release --features standard,browser-shell,cloud,devtools
from the GPU native project, NDK28.2/API29 target compiler and bindgen includes.
This preserves the existing assets/app.json dev endpoints and avoids the
other workspace's CLI broker. After completion copy target/aarch64-linux-android/
release/liblingxia.so into android/app/src/main/jniLibs/arm64-v8a, then assemble
release with applicationId=com.lingxia.example.gpu, suffix=.dev, versionName
0.2.7, versionCode2007, overlay D:/lingxia/target/lingxia/android/overlay/dev/res,
appName=@string/lx_app_name, icon=@mipmap/ic_launcher_lingxia_env, round icon
ic_launcher_lingxia_env_round, splashTheme=@style/Theme.LingXia.Splash.
Install via adb -r; preserve user data. The target/gpu-todo-session.py helper
now accepts --strike to navigate Lab and set rendererFixture after mock.ready;
start it with adb reverse39590 only AFTER installing the new native binary.
Then GpuStrikeProbe, ToDoParity and keyboard regressions, screenshots and
current-PID renderer logs. No helper is currently running.

### Strikethrough native verification complete

Native cargo release finished successfully in 10m20s. Direct Gradle assembly
required ANDROID_HOME/ANDROID_SDK_ROOT=C:/Android/Sdk; final APK assembled and
installed with adb -r, preserving app data and dev config. Source native library
was copied from target/aarch64-linux-android/release/liblingxia.so.

GpuStrikeProbe passes on Pixel 3: two lines across whitespace, descendant none
propagation, clipped/retained scrolling, and cached toggle restoration. Inspected
target/gpu-strike-fixture.png and gpu-todo-strike-dark.png. GpuTodoParityProbe
and GpuTodoKeyboardProbe pass on this native binary; current-process renderer
error log is empty. Evidence: target/ui-decoration-{native,gradle,device,todo,
keyboard,runtime}.log. The installed artifact is android/app/build/outputs/apk/
release/app-release.apk (the version-named APK remains the older build).
Helper and adb reverse are stopped afterward, app Home/light, fixtures removed
by the probes. No full-app parity claim: circular checks, backdrop blur, further
page alignment and desktop visual proof remain outstanding.

### ToDo wide viewport verification

GpuTodoWideProbe exercises >768 logical pixels on Pixel 3 using temporary
220dpi, then restores the original density/rotation settings in finally. It
verifies count/filter horizontal alignment, panel width cap, long task wrapping
without delete overlap, and completed dark state. This is real GPU wide-viewport
coverage on Android, not macOS/Windows host acceptance. Screenshots remained
portrait, so do not label this as a landscape desktop screenshot.

Corrected wordmark top to 30px in the wide branch and added the reference
intermediate 160px font/12%-top breakpoint above 480px. Source typecheck and full
build pass; reloaded complete dev bundle and reran the wide probe successfully.
Inspected target/gpu-todo-wide-light.png; prior dark capture also showed wrapped
strikes. Logs ui-todo-wide-device.log pass, ui-todo-wide-runtime.log empty.
Verified device restored physical density440 (no override), user_rotation0,
accelerometer_rotation1. Probe task removed, app Home/light. Helper and reverse
mapping stopped afterward. Circle checks and blur remain missing.


### Composed ToDo checkbox (pending device verification)

The UI runtime and wire decoder now admit noninteractive checkbox children with
an explicit accessible label. The renderer suppresses its default square only
for composed checkboxes, groups descendant accessibility, fingerprints child
content for gesture rebinding, and preserves the checked state/change callback.
ToDo uses the reference 24px circle/check paths with Tailwind layout and colors.
Rust tests: 176 passed; JS tests before the additional compiler audit: 132 passed.
Example typecheck and complete release lxapp build passed. New native library,
physical Pixel 3 layout/tap/keyboard checks and shared platform gates remain;
this entry does not establish visual parity or device acceptance.


### Composed checkbox platform gates and live Android build

All 133 JS tests pass after adding the TSX compiler audit. Workspace/all-targets
check, GPU-feature check and Windows browser-shell clippy completed successfully
(logs target/ui-checkbox-{workspace,gpu,windows,js-final}.log).
GpuTodoParityProbe now asserts checkable/checked/class semantics and no accessible
children, physically taps the label area, then tests Space/Enter on the focused
checkbox. All 66 probes compile; execution remains pending new native binary.

Native Android release build remains LIVE at this checkpoint, exec session 91637,
log target/ui-checkbox-native.log. Last polling the same handle waited 50 seconds
and returned that live session; rustc is performing final release compilation.
Do not restart due to output silence. No dev HTTP/WebSocket helper has started;
the currently installed binary predates composed checkbox support. Finish this
build, copy liblingxia.so, assemble/install app-release.apk with the existing GPU
dev overlay, then start target/gpu-todo-session.py and adb reverse39590. Run
updated ToDoParity, Keyboard and Wide probes sequentially, inspect screenshots,
and check current-process renderer errors. Preserve existing app data.


### Composed checkbox verified; Clipboard Tailwind alignment

Shared milestone committed as 7277ebf6e. Native Android build finished in 8m36s;
Gradle assembly and adb install -r succeeded. Pixel 3 ToDoParity, Keyboard and
Wide probes pass. Inspected gpu-todo-circle-{light,dark}.png: reference circle,
white check and completed-label strike render correctly. Tab/Space/Enter checks
reacquire focus between operations because the ToDo busy state disables controls
during persistence. No claim of uninterrupted keyboard focus; improve it later.
Original ACTION_FOCUS probe failed because that action is not provided; the
corrected probe uses actual hardware Tab navigation.
Evidence: target/ui-checkbox-{native,gradle,todo-device,keyboard-device,
wide-device}.log. The wide check is Android viewport evidence, not desktop-host
acceptance. No full-app visual parity claim.

Clipboard now uses TSX/Tailwind and the reference four cards: status/types,
Text, Image and Inspect, with reference text, spacing, separators and action
colors. Dynamic styles are limited to viewport/insets/keyboard geometry.
Reference default draft restored to Hello from lx.clipboard. During physical
regression repeated sample PNG writes failed because fs.write defaults to
no overwrite. The example now explicitly overwrites its own sample cache file;
this leaves the reference project untouched. Probe verifies two consecutive
writes and reads the image back. Text/typed/image roundtrips, types, clear/empty,
share sheet and picker cancellation pass on the device. Evidence:
target/ui-checkbox-clipboard-device.log. Inspected gpu-clipboard-layout-light.png.
Typecheck and complete lxapp build pass after the fix. Dark Clipboard layout,
keyboard/rotation and real image selection with the new layout remain pending.
Renderer current-process error log target/ui-checkbox-runtime.log is empty.
Examples and probes remain uncommitted. Full goal remains incomplete.


### Clipboard dark, keyboard and real-image verification

Updated GpuClipboardImageProbe uses the current native API tab route and waits
for Home's resolved Dark state before entry. It additionally verifies the actual
rendered dark surface pixels (copy hardware screenshots to ARGB_8888 before pixel
access). A first capture before that synchronization was light and is not dark
acceptance evidence. No runtime theme code was changed in this verification.

Final Pixel 3 run passes: dark surface, textarea edit plus hardware Enter newline,
real album selection/write/read, centered square contain preview, cancellation
preserving clipboard data, actual landscape rotation and clear/empty. Inspected
target/gpu-clipboard-layout-dark.png, gpu-clipboard-keyboard-dark.png and the final
gpu-clipboard-image-landscape.png. The keyboard screenshot shows the editor above
the IME; landscape shows the chosen image letterboxed without distortion.
Evidence: target/ui-clipboard-image-{compile,device}.log. The final device log's
PASS summary predates the added dark/keyboard assertions, but those assertions
execute before the image workflow and the process exits successfully.
Rotation settings are restored in finally and the successful run returns Home
and requests Light. This verifies Android behavior, not desktop hosts, screen
reader speech, denied permissions, or full-app parity. Example/probes remain
uncommitted. Next visual migration candidate: Share still uses the legacy Screen
wrapper and inline style objects rather than the reference card layout/classes.


### Share reference layout and Tailwind migration

Share now follows the React page's five cards: status, Text, Page, Image and
File. It restores reference descriptions/capitalization, No selection/path
panels and paired choose/share actions, using TSX Card composition and theme
Tailwind classes. Dynamic inline styles only carry viewport/inset/keyboard
geometry. The old invented image preview is removed to match the reference.
Image/File sharing is available before selection, matching the existing Logic
prompt to choose first; actions remain temporarily disabled while busy.
No native chrome or sharing Logic changed. The reference app remains untouched.

Typecheck and complete release lxapp build pass. New GpuShareLayoutProbe passes
on Pixel 3: light/dark background pixels, same-row nonoverlapping action pairs,
native text share-sheet opening/dismissal, and file-picker cancellation in both
themes. Inspected target/gpu-share-layout-light.png and gpu-share-files-dark.png.
Logs target/ui-share-{tailwind-build,layout-compile,layout-device}.log.
This run does not re-establish actual receiver payload/image/PDF delivery,
selected-path wrapping, rotation, desktop behavior or denied-permission cases.
Earlier receiver evidence remains historical; rerun those flows when extending
Share acceptance. Example/probes remain uncommitted; full goal is incomplete.


### Wi-Fi reference card migration

Wi-Fi View now uses TSX/Tailwind, reference gradient background, title, module,
connected-info, events, scan and connection cards. Start/Stop and listener On/Off
are separate controls. Conditional module sections, typed information rows,
event metadata, signal badges, frequency/security details and a capped nested
scan list replace the legacy Screen wrapper/raw JSON. Status/error feedback and
existing Logic cleanup are retained. Network cards use composed buttons with
explicit labels and text-left; selecting a row fills SSID. Password bytes remain
unchanged (no trimming). No actual network connection was requested by tests.

Typecheck/full build pass. Executed compiled View for enabled/disabled and both
themes with connected-info/events/networks fixtures, catching unsupported classes
before device work. An intermediate overflow-hidden class failed at runtime and
was removed: this renderer does not support it. Empty connected cards omit their
bottom separator so it cannot extend through rounded corners; no clipping claim.

Final GpuWifiLayoutProbe passes on Pixel 3: disabled/enabled control states,
module start/stop, current-info action, separate listener controls, cleanup on
hide/reentry, real scan (37 networks in final run), network selection filling
SSID and enabling Connect, and dark rendered pixels. Final screenshots inspected:
target/gpu-wifi-scan-light.png and gpu-wifi-cards-dark.png. Initial centered
network labels were corrected and rerun. Evidence logs:
target/ui-wifi-{tailwind-build,layout-compile,layout-device}.log.
Old GpuWifiLocationProbe updated to the new control IDs/status labels but not
rerun as a combined Location regression. Successful connection, permission denial,
rotation/keyboard, populated event visual capture and desktop hosts remain pending.
The final probe returns Home/light with module stopped; helper/reverse removed.
Examples/probes remain uncommitted, full-app visual parity remains incomplete.


### Location reference layout and result presentation

Location View migrated from Screen/raw JSON to TSX/Tailwind reference title,
Current Location card, centered empty/error/loading states, coordinate heading,
responsive 1/2/3-column detail grid and Get Location/Clear actions. Dynamic inline
styles only carry viewport/insets/keyboard geometry. Numeric zero is retained for
accuracy/altitude/speed; nonfinite/missing values show --. Loading uses a static
vector arc and text: animated spinner parity remains pending. Clear stays disabled
while the existing request is pending to avoid an old completion refilling it.

Typecheck/full build and execution of compiled View light/dark empty/loading/
error/populated branches pass. GpuLocationLayoutProbe passes on physical Pixel 3:
light/dark empty surface, actual request settling to visible error, and clear.
No successful real GPS fix was obtained. A separate helper flag
--location-fixture navigates to Location and injects deterministic page data only
for presentation verification; it is not shipped app data or a real fix. Its
probe passes exact west/north degree-minute headings, six decimal coordinates,
zero numeric details and clear. Inspected gpu-location-empty-{light,dark}.png and
gpu-location-result-debug.png/result-fixture.png. Logs target/ui-location-
{tailwind-build,layout-compile,layout-device,fixture-device}.log.

Android probe compiler now explicitly passes -encoding UTF-8 to javac; the prior
Windows default corrupted degree symbols in expected strings. All probes compile.
Historical GpuWifiLocationProbe Location status expectations are stale after this
copy/layout migration; use the dedicated Location probe for current acceptance.
Wide-grid/rotation and real location success remain unverified; desktop hosts and
full-app visual parity remain incomplete. Example/probes remain uncommitted.


### Location wide-grid and combined regression verification

Extended the deterministic Location fixture probe to temporarily use 220dpi and
160dpi on Pixel 3. It asserts Longitude/Latitude share a row, Accuracy/Altitude
form the second row in the two-column branch, and Longitude/Latitude/Accuracy
share the first row in the three-column branch. Both pass, source fixture values
remain exact, and original density is restored in finally. Inspected
 target/gpu-location-grid-three.png; both two/three screenshots retained.
This is Android wide-viewport coverage with injected presentation data, not a
real GPS fix or Windows/macOS acceptance. Log ui-location-wide-device.log passes;
adb wm density confirms physical440 with no override after cleanup.

Updated historical GpuWifiLocationProbe to new Location readiness/copy semantics,
using enabled request control to await settlement and admitting either a real
coordinate result or a visible error. Reran it successfully after the fixture:
real Wi-Fi scan returned results, module/listener lifecycle passes, actual
Location request returns a visible error, and Clear restores reference empty copy.
Log target/ui-wifi-location-regression.log passes. The previous stale-status
warning is superseded for this probe. No user network connection was requested.

Remaining legacy Screen/colors page migrations found in the current worktree:
channel, cloud, bridge-repro, leaveguard, media, surface, system, pulltorefresh,
feedback, stream, file and ui. This is a migration queue, not the entire completion
audit; native-component gallery exclusion, all page fidelity, shared animation/
blur gaps and cross-platform verification still apply to the full objective.

### Pull-down refresh Tailwind layout and device verification

Replaced the legacy Screen/colors presentation with TSX and theme Tailwind
classes matching the React reference's introduction, status and API cards.
Restored the reference vector, copy, labeled status rows, button labels and
refreshing disabled state. Native navbar and refresh lifecycle are unchanged.
Typecheck and full bundle build passed (target/ui-refresh-tailwind-build.log).

GpuRefreshLayoutProbe passed on physical Pixel 3 in light and dark themes:
API start/stop, timestamp/count updates, disabled start, short/horizontal/canceled
pull rejection, actual downward gesture, state retention in actual landscape,
and exit/reentry reset. Log: target/ui-refresh-layout-device.log. Inspected
 target/gpu-refresh-layout-light.png and dark.png; reference cards fit portrait.
Landscape screenshot confirms retained refreshing state but is scrolled and has
the native refresh indicator visible; it does not prove complete landscape visual
parity. Renderer error log was empty. Probe restored Home/light and rotation.
The old GpuRefreshProbe main entry still assumes obsolete navigation IDs; only
its gesture helpers are reused by the new probe.

Pulltorefresh is removed from the legacy page migration queue above. Other
pages, animation/blur gaps and cross-platform acceptance remain outstanding.
Example and probes remain uncommitted.

### Leave guard reference layout and device verification

Migrated leaveguard from Screen/colors to TSX + Tailwind theme classes. Matches
React empty initial draft, Saved/Unsaved status, inline request count, Save
control and conditional confirmation card with equal-width Keep editing/Discard
buttons. Removed the extra in-page Home control and explanatory content absent
from the reference. Retains useLocal/useLeaveGuard and native chrome. Explicit
bg-surface on the outlined button avoids the native button default blue fill.
Used grow w-0 for equal-width buttons; flex-1 is currently rejected by lowering.

Typecheck/full release bundle passed (target/ui-leave-tailwind-build.log).
Updated GpuLeaveGuardProbe passes on physical Pixel 3 in both themes: initial
save disabled, edit, keyboard dismissal before native Back, request deduplication,
keep preserving draft, discard, fresh empty page, save and unguarded native Back.
Inspected target/gpu-leave-confirm-light.png and dark.png after the background
fix. Log target/ui-leave-device.log passes; ui-leave-runtime.log has no errors.
Probe restores Home/light. Native Home requests, rotation and desktop acceptance
were not exercised by this revised probe; historical Home checks are not current
layout evidence. Leaveguard leaves the legacy styling queue; full parity remains
incomplete. Example/probe edits remain uncommitted.

### Feedback form Tailwind migration and physical surface verification

Feedback now follows the React form: Help us improve header, decorative star,
three equal-width category buttons, labeled multiline/email fields, reference
submit styling and demo footer. Uses theme Tailwind classes and max-w-lg in TSX;
only viewport/inset geometry remains inline. A vector replaces the reference
star character because Pixel 3 rendered the glyph as tofu. Radio list becomes
buttons as in the reference; selected color is visible in captured Bug state.
Input disabling while submitting and visible action errors remain supported.

Typecheck and full release build pass (target/ui-feedback-tailwind-build.log).
Updated GpuFeedbackProbe passes in light/dark on physical Pixel 3: modal excludes
background accessibility, whitespace gating, multiline Enter, email input,
submission toast/close, fresh reentry, Android Back and native close button.
Inspected final target/gpu-feedback-initial-light.png and layout-dark.png; star
renders correctly and Bug highlights in the three-column row. Runtime error log
is empty. Final run: target/ui-feedback-final-device.log. Probe restores Home/light.
The local HTTP verification connection kept app_process alive after PASS and
UiAutomation disconnect; completed probe processes were explicitly cleaned up.
Probe now exits after cleanup, and recompiles successfully.

Entry caveat: api-feedback is obsolete and current Android host exposes no
More menu in the observed tree. Test helper /__probe/feedback invokes the API
page's existing openFeedback action through the dev session, opening the actual
80% native float. This verifies the form and surface, not discoverable user entry.
Investigate missing native More entry before claiming full app parity. Category
payload assertions, focus ring/hover transitions, rotation/wide and desktop
acceptance remain outstanding. Example/probes remain uncommitted.

### Feedback entry investigation correction

Android LxAppActivity.shouldShowCapsuleButton explicitly returns false for the
host homeAppId. Both original showcase and GPU showcase configure their embedded
app as homeAppId. docs/skill/cli/lxdev.md also specifies no capsule on a home
lxapp. Thus the absent More menu observed above is existing host behavior, not
an identified GPU regression. Preserve native chrome; do not add a GPU replica
or change host policy to make the test pass. Feedback form evidence remains dev
invocation of the existing openFeedback action; guest capsule interaction is not
verified by that test.

### Channel Tailwind reference layout and device verification

Migrated Channel from Screen/colors to theme Tailwind TSX. Restored status dot,
inline symbol buttons, large price/change card, timestamp, bounded recent-ticks
card, bottom Disconnect/Reconnect and error text. Removed extra explanatory and
Logic/debug fields absent from React. Auto-connect, subscription filtering,
30-item history and channel lifecycle are retained. Final-row divider removed
so a single tick does not draw through the card's rounded bottom corners.

Typecheck/full release build passes (target/ui-channel-tailwind-build.log).
Updated GpuChannelProbe passes on physical Pixel 3 in both themes: auto-connect,
numeric ticks, MSFT selection across actual rotation, disconnect freezes price
and stops idle frames, disabled subscriptions while disconnected, reconnect,
TSLA selection and fresh AAPL on page reentry. Final target/ui-channel-device.log
passes; renderer error log empty. Inspected gpu-channel-layout-light.png/dark.png
with positive/negative change colors and corrected rounded card boundary.
Probe restores Home/light and rotation. This is a simulated local Logic feed,
not external financial data. Exact price-to-symbol payload binding, full 30-row
scrolling, narrow overflow, baseline/tracking typography and desktop acceptance
remain outside this run. Buttons use equal grow widths instead of horizontal
scroll (the four reference symbols fit the measured portrait viewport).

Remaining Screen/colors migrations: bridge-repro, file, cloud, media, stream,
surface, ui, system. This is not the full parity audit; shared animation/blur and
cross-platform verification remain open. Example/probes stay uncommitted.

### Stream chat structure migration (partial parity)

Replaced legacy top-form/sample-radio/debug-status presentation with reference
chat structure: empty-state icon/copy/hints, user/assistant message bubbles,
vector assistant avatar, bottom composer and icon Send/Stop, conditional Clear.
Uses TSX/theme Tailwind, list anchor=end for changing messages and live tokens,
and explicit viewport/inset/keyboard geometry. Composer height currently follows
explicit newline count (42..120dp). Removed invented sample picker; chartKind auto
keeps reference random replies. Reused exact empty-state SVG path after tiny
open paths failed to show the three dots on device.

Typecheck/full release bundle passes (target/ui-stream-tailwind-build.log).
New GpuStreamLayoutProbe passes physical Pixel 3 light/dark empty state, live
tokens, completion, cancellation, clear, leave-while-streaming and fresh entry.
Final target/ui-stream-layout-device.log passes; runtime error log empty.
Inspected gpu-stream-empty-light.png (including corrected icon), completion-light
with existing pie chart, and completion-dark with wrapped text. Input remains at
the bottom. Probe restores Home/light; local simulation only, no external chat.

This page is NOT fully aligned: shared/chart.ui.tsx still has oversized legacy
heading/legend/buttons and inline colors; diagram primitives/animations still
need reference fidelity. Enter-to-send versus Shift+Enter, soft-wrap textarea
autosize, placeholder tint, per-corner bubble radii, overlay Clear (currently an
in-flow top row), blinking stream cursor, keyboard-visible/rotation/long-history
and desktop behavior remain unverified or missing. Old GpuStreamProbe main and
chart sample probes expect removed stream-status/count/sample nodes and are stale;
only its input/token helper methods are reused. Keep these gaps in the active
completion scope. Example/probes stay uncommitted.

### Stream chart card Tailwind presentation

Replaced shared/chart.ui.tsx legacy Screen/colors styling with theme Tailwind
card/title/axes/legend. Chart accepts data only; palette and plot geometry remain
explicit primitive inputs. Compact uppercase 10px heading, surface-50 inset card,
smaller chart, 11px axes and compact colored-dot pie legend replace oversized
heading, generic series buttons and always-present instruction text. Non-pie
selection uses the plot; tooltip text appears only after selection. Four fixed
fixture values verify axis positions and accessible data. This is closer to the
reference presentation, not full ECharts parity.

Typecheck/full release build passed (target/ui-chart-tailwind-build.log).
GpuChartLayoutProbe passes physical Pixel 3 light/dark bar/line/pie rendering,
exact accessible series values, native plot selection and pie legend select/
deselect. Loopback helper /__probe/chart/{kind} injects deterministic completed
messages into the actual Stream page; this is presentation fixture evidence,
not new stream-delivery coverage. Inspected target/gpu-chart-bar-light.png,
gpu-chart-line-light.png and gpu-chart-pie-dark.png. Renderer errors empty;
ui-chart-layout-device.log passes. Fixture messages cleared and Home/light restored.

Remaining chart differences: axis nice-tick calculation/dashed grid, exact bar
width/radii and dimensions, compact legend wrapping, reference legend visibility
toggle (current selection highlights), overlay tooltip instead of layout-changing
text, chart entrance/emphasis animations, wide/rotation and longer labels. Stream
keyboard semantics, per-corner bubbles, Clear overlay and blink remain as listed
above. Original showcase untouched; GPU example/probes uncommitted.

### Pie legend visibility parity

Pie legend buttons now hide/show their original series, retaining stable palette
indices and muted labels/dots when hidden. Visible indices drive plot values and
selection mapping, so keyboard/accessibility selection cannot select a hidden
slice. Toggling clears the old tooltip, percentages use the visible total, and
all-hidden renders an empty plot while retaining every legend action. Plot and
button accessibility labels report hidden/visible state.

Typecheck/build pass (target/ui-chart-legend-build.log). Updated chart fixture
probe passes physical Pixel 3 light/dark: hide Q2, Q1 percentage becomes 22%, hide
all, restore all, Q1 returns to 17%; bar/line selection remains covered. Inspected
 gpu-chart-pie-hidden-light.png and all-hidden-light.png. Final device log
ui-chart-legend-device.log passes; renderer error log empty. Home/light restored.
This supersedes the earlier legend-highlight-only gap; animation/emphasis,
overlay tooltip, exact axes/geometry and Stream keyboard/other pending page work
remain open. No native chrome or shared renderer changes; example uncommitted.

### Bridge repro reference card layout

Migrated bridge-repro to theme Tailwind TSX: colored Bootstrap/Stream verdict
banners, snapshot and stream statistics cards, paired Reconnect/Echo and
Restart/Stop controls, reference footer. Removed extra subtitle, repeated-probe
button and standalone stream state. Unavailable GPU reconnect reports in echo
log and keeps reconnect count at zero, following the reference fallback shape.
No transport reconnection is simulated. Existing nonce verification and gap
tracking remain. Start/restart resets the current audit as before.

Typecheck/full bundle passes (target/ui-bridge-tailwind-build.log). Revised
GpuBridgeProbe passes Pixel 3 light/dark: bootstrap/snapshot/echo, explicit
unavailable reconnect, at least ten contiguous ticks, stop freezes counters and
idle frames, held-touch stop during redraw, restart and active exit/reentry.
Final ui-bridge-layout-device.log passes; runtime error log empty. Inspected
 gpu-bridge-layout-light.png/dark.png. Home/light restored. The revised test uses
visible counters and verdict instead of removed debug state nodes.

Remaining: reference five-second bootstrap deadline is not implemented here;
failure/delayed bootstrap visual paths, rotation/desktop and actual supported
transport reconnect remain unverified. Shared style/keyboard/animation and other
page gaps still apply. Remaining Screen/colors page migrations: file, cloud,
media, surface, ui, system. Original reference untouched, GPU files uncommitted.

### File page reference cards and route separation

Migrated file to TSX/theme Tailwind: Options, PDF download, Office fetch cards,
and separate Choose File branch driven by existing API routes. Removed invented
in-page section tabs and raw transfer-state text. Restored reference labels,
empty selected path/type, status panels, conditional PDF progress with percentage
bar or unknown-length indicator, and pause/continue presentation. Kept blank
input/paused duplicate-start gating. File/download Logic unchanged.

Typecheck/full build pass (target/ui-file-tailwind-build.log). New
GpuFileLayoutProbe passes physical Pixel 3 light/dark routes, actual share-option
toggle, native managed picker open/cancel and URL extension detection. Local
helper /__probe/file/{mode} supplies downloading/paused/completed/unknown-length
presentation states; checks percentage text and pause availability including
unknown-size active download. These are UI fixtures, not completed transfers.
Final ui-file-layout-device.log passes; runtime error log empty. Inspected
 gpu-file-choose-light.png, paused-light.png and streaming-dark.png. Home/light
restored; injected state is page-local and no download was initiated.

Remaining: real download pause/resume/errors/native preview and selected-path
layout need rerun on this presentation, checkbox description alignment differs
from the reference composed label, progress pulse/transition and focus rings
remain absent, and rotation/desktop remain unverified. Old GpuFileProbe main
expects removed section/state IDs and must be updated before reuse. Remaining
Screen/colors migrations: cloud, media, surface, ui, system. Full goal also
includes earlier shared/style/interaction gaps. Original reference untouched;
GPU example/probes remain uncommitted.

### File composed option and real transfer regression

Share option now uses a composed semantic checkbox with one accessible label,
20dp indicator and aligned title/description column. Both checked state and
whole-label activation remain intact. Inspected final gpu-file-final-choose-light
screenshot; the earlier description-alignment gap is resolved. Typecheck/full
release build passes (ui-file-final-build.log).

Updated GpuFileProbe to current separate API routes and visible progress/button
states. Physical Pixel 3 passes actual local-server download/fetch 404 errors,
pause then page exit/reentry/continue, PDF viewer page count and return, fetch
write with overwrite to lx://userdata/sample.pdf, managed selection/type detection
and reopening that file. ui-file-transfer-device.log passes. The loopback server
served generated one-page PDFs; no external document was downloaded. Server
observed HTTP 200 on continue, not 206: this proves pause/continue UX, not byte
range continuation. Expected connection abort on pause appears in server log,
not a renderer failure. Test-created sample.pdf remains in app userdata for the
managed chooser (same longstanding fixture name).

Reran GpuFileLayoutProbe after checkbox change: light/dark routes, option toggle,
picker cancel/type and four transfer fixtures pass in ui-file-final-layout-device.
Renderer error log empty; Home/light restored. This supersedes the prior stale
GpuFileProbe warning and unverified current download/preview flow. True Office
formats, long selected filenames, rotation/desktop, progress animations and focus
rings remain open. Example/probes remain uncommitted.

### Surface page reference layout and physical-device regression

Surface content now uses TSX/theme Tailwind with the reference header and six
cards, centered max-w-md width, two-column lifecycle counters and distinct
hide/close actions. Retained local counter/message and existing Logic contracts.
Typecheck/full release build passes (target/ui-surface-tailwind-build.log).

GpuSurfaceProbe passes physical Pixel 3 in light and dark: unsupported desktop
window reporting, float open, lifecycle counts, counter/draft preservation across
hide/show, parent delivery while hidden, actual landscape state preservation,
reply/close, parent close and fresh-instance reset, and hidden idle rendering.
Evidence: target/ui-surface-layout-device.log. Inspected dark portrait and
landscape screenshots; landscape demonstrates centered width at the preserved
scroll position, not a complete top-to-bottom visual audit. Renderer error log
empty at inspection; probe restores Home/light and automatic rotation.

Remaining: dark float has a white Android system-navigation area in portrait;
parent Surface Demo still uses the legacy UI layout. Hover/focus/transition and
tracking details, long query/message wrapping, desktop and other platforms remain
unverified. Remaining legacy Screen/colors page migrations: cloud, media, ui,
system. This is not whole-showcase parity. Original reference untouched; GPU
example and probes remain uncommitted.

### System reference routes, cards and state presentation

Replaced legacy Screen/colors and invented section-picker grid with TSX/theme
Tailwind. Existing API routes now show the reference mode heading and relevant
card: app identity, system settings, startup, notifications, cache or desktop
banner. Results use named rows, Yes/No/unknown and human-readable cache estimates.
Restored banner reread and busy-state dismiss gating; banner content is desktop
only. Logic unchanged. Notification targets use two rows below 440dp to prevent
observed phone overflow; shared intrinsic flex wrapping remains unavailable.

Final typecheck/full release build passes (target/ui-system-final-build.log).
GpuSystemLayoutProbe passes physical Pixel 3 light/dark separate API routes,
actual identity/settings reads, absent Android autostart and reread, cache size
read, target button activation, notification scheduling and cancellation.
Evidence: ui-system-final-device.log; ui-system-final-runtime.log empty. Inspected
base-light, final notification-light and cache-dark screenshots. Notification
buttons now remain inside the card and inter-panel spacing corrected. Probe
restored Home/light; helper stopped and reverse removed.

Remaining: cache Clear/busy/error paths not exercised on this presentation;
notification destination delivery/selection payload not asserted (schedule/cancel
uses activate), denied permission/error paths, rotation, extreme widths and
long result values not verified. Supported startup uses a labeled semantic switch
rather than reference's compact unlabeled toggle; desktop startup/banner layout
and interaction require desktop verification. Banner background buttons still
need natural wrapping. Hover/focus/transitions and shared style gaps remain.
Historical GpuDeviceSystemProbe main is stale; use the new System probe for these
routes. Remaining legacy Screen/colors pages: cloud, media, ui. Full goal remains
incomplete; reference untouched and GPU example/probes uncommitted.

### Cloud MQTT reference cards

MQTT branch now uses TSX/theme Tailwind with three reference cards: runtime
connection, topic subscription and latest message. Restored state dot/color,
active badge, two equal action buttons with inactive unsubscribe disabled,
message count, actual empty-message branch and topic/time/payload presentation.
Auth/functions remain legacy and are the next Cloud migration work. Logic intact.

Typecheck/full release build passes (target/ui-cloud-mqtt-build.log). Physical
Pixel 3 GpuMqttLayoutProbe passes light/dark unreachable-service feedback,
empty state, connected/message presentation fixture, button gating, stop and
fresh entry reset. Helper /__probe/mqtt injects page-local state; no cloud publish
or successful subscription is claimed. Current device reports Cannot reach the
cloud service, not the historical missing-provider message. The original stale
assertion failed and was corrected to the observed explicit service error.
Final ui-cloud-mqtt-device.log passes; runtime renderer error log empty. Inspected
empty-light and message-dark screenshots (scrolled to lower cards). Home/light
restored; helper stopped/reverse removed.

Remaining: real connected MQTT/delivery, reconnect/error transitions, long payload,
rotation/wide/desktop and full top-card screenshot audit. Empty-message dashed
border currently solid; tracking and hover/animation gaps remain. Existing
GpuCloudProbe main expects old route/status text and inactive Stop behavior;
use new MQTT probe for this branch. Auth/functions, Media/UI and broader parity
work remain; original React example untouched, GPU work uncommitted.

MQTT follow-up: screenshot review caught unreadable dark payload despite passing
accessibility-text assertions. Set the code-block foreground explicitly to
#f1f5f9 with a Tailwind arbitrary color, preserving its fixed dark background.
Final typecheck/build and full light/dark probe rerun pass in
ui-cloud-mqtt-final-{build,device}.log; renderer error log empty. Inspected both
final message screenshots and confirmed JSON text visible in both themes. This
supersedes the earlier message-dark visual evidence. Helpers stopped; Home/light.

### Cloud functions reference cards

Functions branch now uses TSX/theme Tailwind: current-app function card with
status/action area and demo-name badges, plus separate Last Result card with
function label and minimum-height readable dark JSON block. Removed unrelated
authentication status from this route. Two action columns on narrow screens
preserve reference wrapping for the three sample names; badges fit one row.
Logic unchanged. Auth remains legacy; Cloud migration is not complete.

Typecheck/full release build passes (target/ui-cloud-functions-build.log).
GpuFunctionsLayoutProbe passes physical Pixel 3 in light/dark: all three actual
calls surface Cannot reach the cloud service and update last function; injected
success result and empty demo list render; fresh entry resets output and restores
demo buttons. Success data comes from /__probe/functions/result, not a successful
cloud invocation. Final ui-cloud-functions-device.log passes and renderer error
log empty. Inspected initial-light and result-dark screenshots; JSON visibly
readable in dark. Screenshots are scrolled and do not audit the entire top card.
Home/light restored; helper stopped and reverse removed.

Remaining: real cloud results and intentional server failure, loading/racing calls,
long names/results, natural wrap rather than width-based action grouping, rotation
and desktop, hover/animation. Auth, Media, UI and shared fidelity gaps remain.
GPU example/probes uncommitted; original React reference untouched.

### Cloud auth reference cards and identity presentation

Auth now uses TSX/theme Tailwind; Cloud no longer imports Screen/colors. Restored
Cloud Authentication action grid, Current State and Tenants cards. Login visibility
uses tenant presence, matching React. Added displayName precedence, compact round
identity images, status colors, identity badges/count, whole-row semantic tenant
buttons with active/missing-id gating. Tenant text explicitly left aligned after
screenshot review caught inherited button centering. Logic unchanged.

Final typecheck/release build passes (ui-cloud-auth-final-build.log). Physical
Pixel 3 GpuAuthLayoutProbe light/dark passes empty-state and identity/image fixtures,
displayName, current user/id, tenant count, disabled active/missing-id entries,
actual missing-identity activation feedback and fresh-entry original-state restore.
Device initially has Tenant A; the stale no-identity assumption failed. Tests now
inject page-local empty state and preserve original provider identity, without
logging it out. Fixtures use packaged public/AppIcon.png; this verifies local image
layout, not new remote/cache behavior or real authentication. Final device log
passes; renderer error log empty. Inspected current-light and final tenants-dark:
images visible, longer workspace name wraps, tenant text left aligned. Home/light
restored; helper stopped/reverse removed.

Remaining: login/add/logout and successful identity switching not exercised on
this presentation, status transition variants, remote avatars/loading failures,
rotation/desktop and extreme widths. Identity short-name/id badges are stacked
beneath names rather than inline as in React; natural wrap/shared style fidelity
still needs work. Cloud's source migration is complete but overall behavioral and
visual parity is not. Remaining legacy Screen/colors pages: Media and UI. Original
reference untouched; GPU example and probes uncommitted.

### Media scan and save-to-album reference branches

Scan/save branches now use TSX/theme Tailwind with reference title cards,
settings rows, scan result/type card, album instruction panel and two capture
buttons. Existing native API actions preserved. Screenshot review caught album
text overflowing its padded background; moving panel padding/background to an
outer column fixed the visible bounds. Final typecheck/full build passes in
ui-media-capture-final-build.log.

Physical Pixel 3 GpuCaptureLayoutProbe passes light/dark settings sheets,
native scanner open (Album visible) and cancel, result/type fixture, and save
busy/idle button gates. Result and save-busy states are helper-injected, not QR
decoding or actual capture/save. No device media created. Final device log passes;
renderer error log empty. Inspected scan-empty-light and final save-album-dark.
Home/light restored, helper stopped and reverse removed.

Initial runtime failure identified unsupported italic on the conditional empty
result class. Removed italic for now; this is a shared fidelity gap, not completed
parity. Header ornaments are solid rather than fading, setting separators solid
rather than dashed; loading spinner/hover/animations remain. Actual QR decode and
photo/video capture/save need rerun on this presentation; rotation/desktop not
verified. Old GpuScanProbe main and GpuMediaProbe scan expectations are stale;
new probe reuses only scanner-open helper. Media image/video/gallery/tools and UI
still use legacy presentation; original reference untouched, GPU work uncommitted.

### Media image tools reference layout and real compression

Image tools now use TSX/theme Tailwind with reference header, Pick Image action,
conditional source info, three-column quality/width/height fields, compression
result card and native preview action. Restored dimensions/type/human-readable
file size/path presentation; parameters only appear after selection. Added
reusable ImageInfoCard and MediaError. Logic unchanged.

First device run caught a null-returning component (UI_ROOT expects one node).
Fixed by conditionally mounting MediaError in the parent. Final typecheck/full
release build passes in ui-media-image-final-build.log. New
GpuImageToolsLayoutProbe passes physical Pixel 3 light/dark: actual native album
selection, positive source dimensions, editing width/height, real 256x256 JPEG
compression with nonzero displayed size, native preview and retained return
state. Selected the first existing album image; no 1024px source assumption.
Generated outputs are managed temp files, not saved to album. Final device log
passes and renderer error log empty. Inspected compressed-light and dark images;
result/card/three-column inputs readable. Home/light restored, helper stopped,
reverse removed.

Remaining: numeric keyboard/min/max semantics (current input is text), focus
rings, loading spinners/transition, pick cancel/error/compression failure paths,
rotation/extreme widths, other image formats and desktop. Screenshots are scrolled
and do not fully audit header/source card. Old GpuMediaImageProbe expects raw-byte
size and obsolete entry; new probe covers this presentation. Media gallery/video
tools and UI still need migration; broader fidelity work remains. Original React
reference untouched, GPU example/probes uncommitted.

### Media video tools reference layout and real processing

VideoTools now uses TSX/theme Tailwind: reference header, selected path, preflight
metadata/source cards, thumbnail guidance and 2-column fields, inline thumbnail
image/results, compression guidance/fields, busy progress/cancel and compressed
result/preview. Human-readable duration/size/bitrate and optional metadata labels
restored. Actions remain existing Logic, with reference busy gating. Shared
MediaHeader now constrains long subtitles so API names wrap within the card.

Typecheck/full release build passes (ui-media-video-final-build.log). Physical
Pixel 3 GpuVideoToolsLayoutProbe passes both themes: actual native video selection,
metadata, thumbnail bounded by 160x320, thumbnail native preview/return,
compression cancel/restart, nonempty compressed MP4 and native preview/return.
Current fixture source/result observed 640x1280 and 3.84s; no fresh capture or
album write. Outputs remain managed temp files. Final device log passes, renderer
error log empty. Inspected final header-light and thumbnail-light plus compressed
dark; corrected default narrow image width with w-full and subtitle overflow.
Initial header screenshot was taken before readiness; new probe waits title.
Home/light restored, helper stopped/reverse removed.

Remaining: numeric keyboard/min/max, focus/spinners/animations, cancellation and
error presentations beyond tested compression flow, full preflight metadata visual
audit, landscape/wide/desktop, other codecs and tall image ratio sizing. Final
thumbnail screenshot has Max Height placeholder (input setting not explicitly
asserted); 160x320 output demonstrates proportional width bound, not independent
height-input binding. Thumbnail image is full width but screenshot sees only its
top, so full aspect/layout remains to inspect. Old GpuMediaVideoProbe main expects
raw fields; use new layout probe. Media gallery and UI remain legacy; overall
parity still incomplete. Original React reference untouched, GPU work uncommitted.


### Video input follow-up: not resolved

Strengthened GpuVideoToolsLayoutProbe to assert width 160 and height 100 both
before and after keyboard dismissal, and require thumbnail height <=100. The
previous 160x320 check could pass using only the width bound. Physical Pixel 3
fails the stronger assertion: height becomes empty after Back. A 1200 ms wait
previously produced 100100 instead; waiting is not a fix.

Found and fixed an independently reproducible Android IME defect: accessibility
text replacement left the old keyboard wrapper able to submit stale composition.
GpuIme now invalidates that wrapper and restarts input after successful replacement.
The new regression fails before the fix; all 28 GpuImeTest and 36
GpuInputConnectionTest cases pass afterward (ui-ime-replacement-after.log).
Release APK builds and installs with the existing 2007/0.2.7 version.

The actual video failure remains on the new APK (ui-ime-replacement-device.log).
A temporary read-only Logic query observed height=100 before keyboard dismissal
and height="" afterward (ui-ime-state-session.log), so this is not merely stale
accessibility text. The diagnostic delay/query was removed from the tracked probe;
the stronger assertions remain. Next: trace IME edits across keyboard dismissal
and controlled state replication. No full video parity or overall completion claim.


### Video input diagnosis corrected; final device regression passes

The preceding "height changed to empty after Back" diagnosis was not established.
Per-theme phase logging and Logic event tracing showed that the light page accepted
100 and completed; the new dark page never received a height event. Queries made
during the two-theme run had compared different page instances. The native log
ui-ime-focus-events.log records rejected height focus at generation 12. This was
an asynchronous focus rejection while targeting the next field during keyboard
layout changes, not a demonstrated setData overwrite or Back-induced clearing.

GpuVideoToolsLayoutProbe now reveals/clicks each input and asserts focus before
replacing text. It retains exact 160/100 assertions before and after keyboard
dismissal and independent output bounds. Final clean APK passes both themes:
real video selection, metadata, 50x100 thumbnail from a 640x1280 video, thumbnail
preview, compression cancel/restart/result, native video preview return and light
status icon restoration. Logs: ui-ime-final-{build,pages,device,runtime}.log;
runtime errors empty. GpuTodoKeyboardProbe also passes actual touch/keyboard text,
Enter submission, input clearing and test-task cleanup (ui-ime-final-keyboard.log).

Shared Android fixes retained: replace text on an already focused editor by
invalidating its old keyboard wrapper and restarting input; initialize external
text/selection on a newly focused editor before attaching the keyboard. Regression
tests cover stale wrapper rejection and initial text at Android focus. All 30
GpuImeTest and 36 GpuInputConnectionTest cases pass (ui-ime-initialize-build.log).
GpuVideoInputProbe adds 12-cycle direct accessibility replacement coverage; it
passed the earlier clean intermediate build on an empty video-tools page, not
the long selected-video layout, and is not proof of arbitrary in-flight focus
reliability. Temporary Java/Logic tracing removed.

Full media gallery/UI migration, shared styling gaps, other platforms and overall
showcase parity remain incomplete. No reference showcase source edits in this work.
Final screenshots inspected: thumbnail light and compressed dark. Home/light restored; verification helper stopped and reverse 39590 removed.


### Media gallery TSX/Tailwind migration

Replaced the remaining legacy Screen/colors gallery with the React reference's
Media Manager header, source/count settings, video camera/duration settings,
selection badge, compact preview options, image-duration input, preview action,
result card and two-column image cards. Gallery no longer displays raw paths
beneath each image. Camera-only source fixes count to 1 and disables its picker.
Add tile is hidden once the selection reaches the limit. Index indicator uses
the reference's button behavior rather than the old checkbox. Fullscreen native
preview remains wired to the existing Logic. MediaInfo values now cap at 72% and
wrap long identifiers, including the selected source path.

The reference's embedded LxVideo and its component-only Rotate/Fit controls are
not implemented here; these belong to the excluded native component work. Video
metadata cards and fullscreen preview are available. No original showcase edits.

GpuMediaGalleryProbe passes on physical Pixel 3 in both themes: source Camera/Album,
count 1 gate and count 2 selection, rotation 90, contain fit, hidden index setting,
actual two-photo selection (2/2 with second thumbnail and no Add tile), one video
selection (1/2), native preview/Back, retained selection and last-result source.
Result path bounds and multiline height are asserted. Screenshot review covers
selected image light, video result dark, and final wrapped result light; native
picker screenshot confirms actual selections. Test restores Home/light.

Evidence: ui-gallery-wrap-build.log (typecheck and release), ui-gallery-render.log
(compiled runtime empty/selected image/video descriptions), ui-gallery-wrap-device.log
and ui-gallery-wrap-runtime.log (pass / no renderer errors). Initial attempts
selected media correctly but rendering failed on unsupported overflow-hidden;
removed it. A composed option button additionally needed an explicit accessibility
label; fixed. An earlier overflow of Auto Next text was fixed with composed text
content so layout measures its wrapped height. Native picker selection was not
the cause of those failed attempts.

Remaining style gaps: dashed borders/header fade, image-card corner clipping,
disabled/loading presentation and focus/hover/animation. Auto Next/Loop sequencing,
image-duration editing, preview cancellation/errors, capture/camera options,
landscape/wide/desktop and other platforms need fuller verification. UI API pages
still use legacy layouts; overall showcase parity remains incomplete.
Verification helper stopped and reverse 39590 removed. GPU example/probes remain uncommitted.


### UI Toast / Action Sheet / Modal layout migration

Added scoped TSX/Tailwind DialogControls for three UI routes; removed their old
inline-style branches and legacy local fields. Toast now has the reference's
parameter card, labeled Change selectors and separate Show/Hide action card.
Modal has parameter/action/result cards, pretty JSON and conditional Clear Result.
Action Sheet shows its spacious action card and only renders a result after a
selection/cancellation. Native toast/dialog/sheet implementations remain intact;
no changes to original showcase or host navbar/tabbar.

GpuDialogLayoutProbe passes on physical Pixel 3, light and dark: focus-confirmed
text edits, duration 10000, None/Top toast settings, mask checkbox toggling, native
toast show/hide; action id 1 and cancellation; native modal Cancel/OK, result clear,
custom Proceed label with Cancel disabled. Local fields remain present through
dialog return. Home/light restored. Screenshots inspected: toast-light and
modal-result-dark. Renderer error log empty.

Evidence: ui-dialog-build.log (typecheck), ui-dialog-final-build.log (release),
ui-dialog-render.log (all six compiled empty/result render cases),
ui-dialog-final-device.log and ui-dialog-final-runtime.log. Initial Action Sheet
mt-8 failed runtime mapping; replaced with an equivalent h-8 spacer.
Old GpuUiProbe main has stale API entry navigation and compact JSON/empty-result
expectations; use GpuDialogLayoutProbe for these routes.

Remaining: numeric input min/max/step and browser-like empty-duration behavior,
focus/hover styles, checkbox sizing/color, exact card clipping/gutters, mask
interaction blocking (only checkbox state exercised), timeout/loading variants,
empty title/multiline text/native errors, landscape/wide/desktop and other OSes.
Navigation, parent Surface, Navbar, Appearance and Tabbar UI routes remain legacy;
overall showcase parity is not complete. GPU changes remain uncommitted.
Verification helper stopped and reverse 39590 removed.

### Appearance controls: TSX/Tailwind layout and device verification

Replaced the legacy inline-style Appearance branch with a themed card matching the React section hierarchy: heading, Light / Dark description, three preference choices with selected colors, divided Preference/Resolved rows, and supporting copy. Copy describes the user-visible behavior without presenting WebView-only HTML implementation details. Native chrome remains owned by the host.

Verification: TypeScript checks and release build passed (`target/ui-appearance-build.log`); compiled View rendered all six light/dark environment and auto/light/dark preference combinations (`target/appearance-render.cjs`). Physical Pixel 3 probe `GpuAppearanceLayoutProbe` passed dark/light/auto selection, preference retention after page reentry, and dark-state propagation to Home (`target/ui-appearance-device.log`). Light/dark screenshots were inspected (`target/ui-appearance-{light,dark}.png`): content fits, selected options remain distinct, and native navbar/status text changes with the theme. Renderer error log is empty (`target/ui-appearance-runtime.log`). Restored Home/light.

Limits: this does not prove preference persistence across process restart, automatic theme updates when the host changes, accessibility selected-state semantics, wide-layout/cross-platform equivalence, or full React visual parity. Remaining legacy UI branches include navigation, parent Surface, navbar and tabbar; shared rendering/style gaps remain. No changes to the reference showcase.

### Navbar controls: reference layout and preset behavior

Migrated the navbar View branch from legacy inline styles to TSX/Tailwind: title/input action row, paired color fields, full-width color action, two-column colored presets, and home/reset controls in the reference card hierarchy. Initial fields are empty like React; empty color values use white/black and empty titles do not update chrome. Corrected preset behavior to update both title and exact native colors (previous GPU presets changed colors only). Native navbar implementation is unchanged.

Verification: TypeScript and release build passed (`target/ui-navbar-build.log`). Compiled View harness renders in both themes (`target/navbar-render.cjs`). Physical Pixel 3 `GpuNavbarLayoutProbe` passed in light/dark: edited title, all four preset titles and sampled native background pixels, custom color input, and reset to configured User Interface title/default theme color (`target/ui-navbar-device.log`). Inspected light reset and dark custom screenshots; card fits with readable fields and controls (`target/ui-navbar-{light,dark}.png`). No renderer errors (`target/ui-navbar-runtime.log`). Restored Home/light.

Limits: home hide/auto actions were invoked, but this pushed-page scenario has Back rather than Home, so it does not prove root Home visibility behavior. Empty-field defaults were source-aligned but not independently exercised on device. Hover/focus-ring support, responsive wide layouts and cross-platform checks remain open, as do navigation, parent Surface and tabbar legacy View branches. Full parity remains incomplete.

### Tabbar controls: reference cards and completion feedback

Migrated the TabBar branch to TSX/Tailwind with the React Visibility, Red Dot and Badge card hierarchy, input/action rows, description text and matching action colors. Removed extra icon-path fields absent from the reference page. Kept native tabbar rendering intact. Logic now emits the reference shown/hidden toast after awaiting the native visibility update (inside the existing error-reporting wrapper).

Verification: TypeScript, release build and compiled View rendering passed (`target/ui-tabbar-build.log`, `target/tabbar-render.cjs`). Physical Pixel 3 `GpuTabbarLayoutProbe` passed in light/dark: native label changes, badge 42 set/removal, hide/show visibility and completion toasts (`target/ui-tabbar-device.log`). Red-dot actions were exercised; inspected dark screenshot confirms the dot is visible. Inspected light badge/dark dot screenshots show separated cards, readable controls and native tabbar (`target/ui-tabbar-{light,dark}.png`). Renderer error log empty (`target/ui-tabbar-runtime.log`). Restored label API, no badge/dot, visible tabbar and Home/light.

Limits: red-dot removal has no independent pixel assertion in this probe; completion toast/visibility checks do not measure animation timing frame-by-frame. Narrow/wide layout, hover/focus, platform parity and remaining shared rendering gaps are still open. Legacy UI branches now remain for navigation and parent Surface. Full objective remains incomplete.

### Navigation View: reference content hierarchy and instance exercises

Replaced the legacy navigation grid/flat text with TSX/Tailwind action rows, top-first page-stack card and depth badge, current-entry highlight, instance identity card, composed colored state counters, gradient popup action, individual lifecycle-event rows and centered View popup. Copy names GPU View concepts (useLocal/View popup) where the React reference describes useState/H5. Navigation rows explicitly align text left. Removed the old inline-style navigation/popup branches; parent Surface is the remaining legacy branch in this View file.

Initial device run exposed non-scrollable long content with percentage-sized stack roots. Explicit viewport dimensions on the root stack and content column restore bounded scrolling; this is an environment-dependent geometry binding, with visual styling still Tailwind. This does not resolve general percentage-height behavior in the shared renderer.

Verification: typecheck, final release build and compiled View harness passed (`target/ui-navigation-build.log`, `target/navigation-render.cjs`). Physical Pixel 3 final `GpuNavigationLayoutProbe` passed both themes (`target/ui-navigation-device.log`): push changes instance identity/depth and resets local/Logic counters while retaining module count; back restores earlier local/Logic state; redirect changes tag without changing depth; popup closes, disappears after exit/reentry; switchTab returns Home. Inspected light/dark state cards and light popup screenshots (`target/ui-navigation-state-{light,dark}.png`, `target/ui-navigation-popup-light.png`). Renderer error log empty (`target/ui-navigation-runtime.log`). Restored Home/light.

Limits: same-route redirect counter/lifecycle reset is not proven by this probe (the existing runtime can rerun onLoad on the same state; the prior probe explicitly expected retained counters). Stack-limit behavior, background/foreground lifecycle, popup backdrop hit testing, responsive/cross-platform checks, exact clipping and general percentage sizing remain open. Composed counter accessibility presents one labeled button with its value; decorative child test IDs are not independently exposed. Full parity remains incomplete.

### Parent Surface View: reference controls and size handling

Replaced the last legacy UI View branch with TSX/Tailwind. Parent Surface now includes all four kinds, conditional aside edges/float positions/window chrome, hints, optional px/% dimensions with validation, active-state disabling, conditional Show/Hide/Close and status. Removed the extra URL/opener-message fields absent from React's parent demo. Logic size typing now uses OverlaySurfaceSizeValue, preserving percentage inputs through the existing native surface API. Empty size/chrome fields are omitted from action payloads; explicit undefined members initially threw at native action dispatch. Hint backgrounds now live on a padded column so multiline text stays inside the card. All UI demo branches now use Tailwind visual styling; dynamic viewport/inset/keyboard geometry remains explicit.

Verification: final TypeScript checks, release build, compiled default Surface render in both themes passed (`target/ui-surface-build.log`, `target/surface-parent-render.cjs`). Physical Pixel 3 `GpuSurfaceParentLayoutProbe` passed both themes (`target/ui-surface-device.log`): kind/edge/chrome selection, unsupported desktop-window result, invalid 101% rejected, corrected 90% width and 620 px height opens a float, child hide/parent show preserves its show counter, parent close/reopen resets the counter, child close clears active controls. The probe uses the child surface-scroll helper when reading/clicking within the float; generic page-scroll lookup was insufficient after restored scroll position. Inspected light/dark parent form screenshots (`target/ui-surface-parent-{light,dark}.png`); renderer error log empty (`target/ui-surface-runtime.log`). Restored Home/light and closed floats.

Limits: screenshots retain the previous window unsupported status before opening the float; this is last-operation status, not a float failure. Exact 90% geometry and all position/edge permutations are not asserted. Actual aside URL/lxapp opening and desktop window behavior are not verified here; blank/negative/malformed/height boundary inputs need broader checks. Selected colors are blue for all chooser groups rather than the reference's mixed neutral/indigo styles. Other shared renderer/style/lifecycle/cross-platform gaps remain. Full parity is not complete merely because this View file no longer imports legacy Screen/colors.

### Shared text measurement: padded multiline height

Followed the Surface hint overflow into the shared renderer. Added a differential layout regression comparing a padded/bordered text leaf with identical text inside an equivalent padded/bordered container. Before the fix, width 318 allocated height 36 for the leaf versus 56 for the container (`target/ui-padded-text-test.log`), explaining why painting a second line escaped the background.

Fixed `crates/lingxia-ui/src/render.rs` to wrap using Taffy's content-box available width, not its border-box known width. Painting already subtracts padding/borders, so layout and painting now agree. Added the invariant to ui-renderer.md without replacing existing uncommitted IME documentation. `cargo fmt -p lingxia-ui` completed.

Verification: original regression passed after the fix (`target/ui-padded-text-fixed.log`); all 177 lingxia-ui library tests passed (`target/ui-text-layout-suite.log`). Expanded final regression covers 41 widths times 1x/2x scale times no/2px borders, all passed (`target/ui-padded-text-matrix.log`). Windows linker intermittently could not open the test executable; retries after confirming the command terminated completed successfully. PowerShell may report exit 1 because native compiler warnings are redirected as errors; terminal test summaries above show actual success.

The installed Android APK still contains the prior native renderer. This change has host-library test coverage only: rebuild/install the GPU dev native library and verify padded text plus page/input regressions on Pixel before claiming device verification or committing the renderer milestone. The Surface wrapper workaround remains until that verification. Full visual/behavior parity and the broader goal are incomplete.

### Padded text Android build and verified failing baseline (in progress)

Added Lab rendererFixture=padded-text and GpuPaddedTextProbe. The fixture compares directly padded text to identical text inside a padded container, sweeping widths 240..340 in steps of 4 with/without 2px borders. Probe measures the child text plus padding/border (noninteractive containers are not standalone accessibility nodes). On the existing Android renderer, the final baseline fails at a line-break boundary: direct bounds height 99 physical pixels versus inner text height 110 plus required padding (`target/ui-text-device-before.log`, `target/ui-text-before.png`). The earlier single-width 318 check did not reproduce on Android fonts, so it was expanded. Lab has no native Back button; probe now exits with Android Back.

Fixture release build and probe compilation completed. Current test artifact: target/gpu-independent-probe.jar. Fixture helper target/gpu-todo-session.py supports --padded-text, active exec session 73133 with log target/ui-text-width-session.log; adb reverse39590 is active. Device currently displays failed baseline fixture.

Android native rebuild is ACTIVE, not blocked: exec session 98286 running target/build-gpu-text-native.ps1, log target/ui-text-native.log. Last confirmed rustc PID 22080 is linking native release; CPU increased through 361 seconds. Do not restart merely because output is quiet. Initial failed attempts needed ANDROID_NDK_ROOT and explicit NDK clang19 include path for bindgen; the script now includes both. Builds GPU native cdylib with standard,browser-shell,cloud,devtools into D:/lingxia/target. Existing liblingxia.so still dated 10/22 11:04 at last check and must not be copied before terminal success.

Next: wait on session98286; on success copy target/aarch64-linux-android/release/liblingxia.so to GPU/android/app/src/main/jniLibs/arm64-v8a/liblingxia.so. Run target/assemble-gpu-text.ps1 (prepared exact version2007/0.2.7, .dev suffix and existing dev overlay settings), then adb install -r APK. Force-stop/launch with existing fixture helper, rerun GpuPaddedTextProbe to target/ui-text-device-after.log, inspect screenshots. Then stop fixture helper and start it without fixture for SurfaceParentLayout, NavigationLayout and TodoKeyboard regressions. Check current-PID renderer errors. Native fix remains uncommitted and not yet verified on the device.

### Padded text native verification complete

Native rebuild finished successfully (10m21s, target/ui-text-native.log), copied the new liblingxia.so, assembled dev APK (target/ui-text-apk.log), and installed -r preserving data. Same GpuPaddedTextProbe that failed on the prior native renderer now passes all 52 width/border combinations (target/ui-text-device-after.log). Inspected target/ui-text-after.png: directly padded and container-wrapped text have matching two-line geometry and backgrounds. Fixture lives only in the local GPU example/Lab.

Final native regression runs passed: SurfaceParentLayout in both themes (target/ui-text-surface-regression.log), NavigationLayout in both themes (target/ui-text-navigation-regression.log), real ToDo keyboard typing/Enter/clear/cleanup (target/ui-text-keyboard-regression.log). Current-PID renderer error log is empty (target/ui-text-final-runtime.log). Workspace all-target cargo check and Windows browser-shell clippy completed successfully (target/ui-text-workspace-check.log, target/ui-text-windows-check.log); existing warnings remain. Shared Rust fix, regression and its three-line invariant were committed as a1ab3a770. Previously uncommitted IME docs/code were left untouched by the commit; GPU example/probes remain uncommitted.

Restored Home/light via regression cleanup. Stopped fixture/dev helper and removed reverse39590. Installed APK now includes the padded text fix and earlier IME changes. Broader styling, page lifecycle, accessibility, remaining workflows and cross-platform parity remain incomplete; this milestone only closes the measured padded-text wrapping bug.

### Stream long-content wrapping

Reference Stream bubbles apply break-word wrapping; GPU bubbles previously omitted an overflow-wrap policy. Added Tailwind break-words to the shared bubble text, affecting user, completed assistant and live assistant text without altering accessible content.

Typecheck and release build passed (target/ui-stream-wrap-build.log). Physical Pixel 3 GpuStreamWrapProbe passes both themes with a long unbroken URL: exact accessible text retained, bounds inside screen and multiline height, cancel/clear return to empty (target/ui-stream-wrap-device.log). Inspected dark screenshot target/ui-stream-wrap-dark.png confirms the full URL wraps within the user bubble. Existing GpuStreamLayoutProbe passes light/dark empty/live/completed, cancellation, clear and teardown/reentry (target/ui-stream-wrap-regression.log). Renderer error log was empty at the targeted check (target/ui-stream-wrap-runtime.log). Restored Home/light and stopped helper/reverse39590.

Limits: new boundary assertion covers user text; assistant uses the same bubble component, but no dedicated long assistant payload was injected. Composer auto-height for soft wraps, Enter/Shift+Enter parity, streaming cursor animation, italic placeholders, individual bubble corner radii and chart details remain unresolved. API index source review did not identify a new missing entry; it was not a comprehensive end-to-end route audit. Full goal remains active.

### Stream composer intrinsic height implementation (native verification pending)

Implemented explicit height:'auto' / Tailwind h-auto through JS validation/encoding, CSS lowering, TypeScript generation and native Taffy sizing. Append-only internal wire field82 must be exactly1 and cannot coexist with absolute height1 or percentage height75. Omitted height preserves primitive defaults. Width/min/max auto remain unsupported. Stream textarea now uses h-auto min-h-[42px] max-h-[120px] instead of counting newline characters in JS.

Tests: 178 Rust library tests passed, including intrinsic textarea growth with soft-wrap content and cap (target/ui-autoheight-rust.log); generated schema drift check passed. Added wire conflict/value regression separately, passes (target/ui-autoheight-schema.log). 135 JS tests passed including Tailwind lowering/encoding and rejecting unsupported auto dimensions (target/ui-autoheight-js.log). Full workspace all-target check completed (target/ui-autoheight-workspace.log). GPU View/Logic typecheck, release build (target/ui-autoheight-build.log), and GpuStreamAutoHeightProbe compile (target/ui-autoheight-probe-build.log) passed. The new probe checks short/soft-wrap/long-cap/shrink/newline/empty in both themes. Changes remain uncommitted.

Android native rebuild is ACTIVE: exec session32768, rustc PID11620 last observed. It runs target/build-gpu-text-native.ps1 and writes target/ui-text-native.log (overwriting prior build log); latest output is Compiling native with no new error. Do not restart based on quiet output. Previous installed APK has padded-text fix but does NOT understand field82. Do not load current Stream dist into it before installing new native.

Next: wait session32768. On successful terminal build copy target/aarch64-linux-android/release/liblingxia.so into GPU/android/app/src/main/jniLibs/arm64-v8a/liblingxia.so; run target/assemble-gpu-text.ps1, install release APK -r, then start target/gpu-todo-session.py normally with reverse39590. Push current target/gpu-independent-probe.jar and run GpuStreamAutoHeightProbe. Inspect screenshots and current-PID errors. Run StreamLayout/StreamWrap and actual keyboard regressions as appropriate. No helper currently runs and reverse39590 was removed by prior turn. Further physical keyboard Enter/Shift+Enter, animation/italic/corners/chart and broader parity remain open.


### Stream composer intrinsic height verified on Pixel 3

The pending native build completed successfully (8m27s, target/ui-text-native.log). Copied the new library, assembled the Android release APK (target/ui-text-apk.log), and installed it preserving data. GpuStreamAutoHeightProbe passes both themes: soft wrapping and explicit newlines grow the composer, long content remains capped, deletion shrinks it, and clearing restores empty state (target/ui-autoheight-device.log). Inspected target/ui-autoheight-dark.png: capped editor shows the final lines/caret above the keyboard with internal scrolling.

Regressions pass: Stream empty/live/completed/cancel/clear/teardown/reentry (target/ui-autoheight-stream.log), unbroken URL wrapping in both themes (target/ui-autoheight-wrap.log), actual Android text input and Todo Enter submission/cleanup (target/ui-autoheight-keyboard.log). Current-process LingXia.UI error log is empty (target/ui-autoheight-runtime.log). Windows browser-shell clippy completed (target/ui-autoheight-windows.log). Prior Rust/JS/workspace results above remain applicable; no implementation changes were made during device verification.

This verifies intrinsic sizing, not complete Stream or showcase parity. Stream Enter/Shift+Enter, cursor animation, italic placeholders, per-corner radii, chart details, and broader cross-platform visual verification remain open. Native navbar/tabbar remain preserved; native-component gallery remains excluded. Device restored to Home/light.


### Stream reference composer padding and empty-state gutters

A closer React source comparison corrected the previous combined textarea box: React applies 14px horizontal/10px vertical padding on an outer rounded wrapper, while the editor itself has min-height22/max-height120. GPU now uses the same wrapper and a transparent p-0 h-auto editor with those constraints. Empty-state horizontal gutters now include both the reference scroll padding16 and inner padding32 (48 total). Updated ui-styling.md to document supported h-auto instead of its stale blanket rejection.

Typecheck/release build passed (target/ui-stream-insets-build.log). Pixel 3 autoheight checks pass light/dark (target/ui-stream-insets-height.log): observed editor heights63 short,190 wrapped,330 capped physical pixels. Initial test incorrectly treated min-height22 as an exact short height; actual font metrics make it63px. Corrected cap test calibrates using the fixed36px send button instead, checking120px within2physical pixels. Independent wm density440 and accessibility bounds also confirmed the330px cap. This was a test assumption correction, not a relaxed cap requirement.

Stream layout/streaming/cancel/clear/teardown regressions pass (target/ui-stream-insets-layout.log). Inspected target/ui-stream-insets-empty-light.png and target/ui-stream-insets-dark.png. Current process renderer error log empty (target/ui-stream-insets-runtime.log). Restored Home/light; stopped local helper and removed reverse39590. GPU example remains uncommitted.

Next input parity gap: GpuInputConnection.sendKeyEvent currently hardcodes multiline Enter to newline; React sends on Enter and inserts newline on Shift+Enter. Needs a shared opt-in submit behavior, preserving normal multiline controls and composition semantics, plus physical-device checks. Full objective remains incomplete.


### Stream Enter submission implemented; native device verification pending

Added explicit textarea-only submitOnEnter (append-only wire field83, numeric boolean), generated TextareaProps and snapshot submit_on_enter. Stream enables it and binds onSubmit to its existing send function. Android preserves multiline input, advertises IME Send, updates configuration when the flag changes, sends on hardware Enter, and retains Shift+Enter newline. Hardware Enter during active preedit only finishes composition; explicit IME Send submits normally. Ordinary textareas retain Enter newline behavior. No reference-showcase source was changed.

136 JS tests pass (target/ui-submit-js.log). Rust suite:179 passed,1 failed due UI_TIMEOUT in editor_budget_failure_preserves_the_previous_input_connection while concurrent builds ran (target/ui-submit-rust.log); that exact test passed when rerun alone (target/ui-submit-rust-retry.log). Generated schema drift and new invalid wire flag/tag cases passed. Android Robolectric:38 InputConnection and32 Ime cases, zero failures/errors, SDK23/31; covers opt-in Enter/Shift+Enter/preedit and multiline IME action configuration (target/ui-submit-android-tests.log). Workspace all-target check and Windows browser-shell clippy completed (target/ui-submit-workspace.log, target/ui-submit-windows.log). GPU typecheck/release build passed (target/ui-submit-build.log).92 probes compiled including new GpuStreamKeyboardProbe (target/ui-submit-probe.log).

Android native rebuild ACTIVE in exec session48646, target/build-gpu-text-native.ps1, log target/ui-text-native.log. Latest polled live processes were rustc35116/15596 compiling lingxia and control-runtime. Do not restart merely on quiet output. Installed APK does NOT support new field83; do not serve current Stream dist until new APK installed. No helper/reverse currently running.

Next: poll session48646 to terminal success, copy target/aarch64-linux-android/release/liblingxia.so to GPU/android/app/src/main/jniLibs/arm64-v8a/liblingxia.so, assemble using target/assemble-gpu-text.ps1, install -r. Start target/gpu-todo-session.py normally, reverse39590, force-stop/launch, push target/gpu-independent-probe.jar. Run GpuStreamKeyboardProbe (Shift+Enter draft newline then physical typing and Enter submission, both themes), inspect screenshots; check Gboard Send too. Regress StreamAutoHeight/Layout and TodoKeyboard, current-PID errors. New probe is compiled but not yet device-run. Full objective remains incomplete; changes uncommitted.


### Enter verification preparation and full Rust rerun

Full Rust library suite rerun serially passes all180 tests (target/ui-submit-rust-serial.log), including the previously timed-out editor budget test. GpuStreamKeyboardProbe now also locates and taps Gboard's accessible Send action after its hardware Enter/Shift+Enter case; this runs in both themes with exact message and cleared-draft assertions. All92 probes compile (target/ui-submit-probe.log). This extension has not been device-run yet.

Native build session48646 remains live, rustc21836 compiling/linking native; observed CPU increased32.19 to80.42 seconds. Continue polling this existing session, do not restart. No device install or helper has started. Next steps remain install new field83-capable native, then run keyboard/autosizing/layout/Todo regressions. Full objective remains active.


### Stream keyboard submission verified on Pixel 3

Native build session48646 completed successfully (8m15s); copied the library, assembled/installed new APK. Final installed APK has temporary GpuSubmitProbe logs removed; source search and diff whitespace check clean. GpuStreamKeyboardProbe passes light/dark hardware Enter submission of a committed multiline draft, Shift+Enter newline, and Gboard IME Send (target/ui-submit-device.log). Inspected target/ui-submit-dark.png: exact two-line user message and active assistant response. It preserves native navbar/tabbar.

Diagnostics corrected three probe assumptions, without weakening production composition protection: Gboard may mark English words as preedit after shell typing, so hardware test types a space to commit its last word before Enter; preedit Enter only finishes composition (observed compose11:17 and no submit, target/ui-submit-preedit-diagnostic.log). Sending disables the editor and can hide the keyboard, so probe dismisses only an existing TYPE_INPUT_METHOD window rather than navigating Back unconditionally. Finally the text 'Keyboard Send' made Gboard offer a candidate also named Send; the probe had clicked that candidate. Screenshot target/ui-soft-send-before.png proved this. Changed payload to 'Soft keyboard message' and invoked the actual Send key's accessibility click. Final run is on the non-diagnostic APK. Hardware Enter during active English preedit is intentionally not claimed to submit; CJK device coverage for this new option remains unverified.

Regressions pass: StreamAutoHeight light/dark (target/ui-submit-autoheight.log), Stream empty/live/completed/cancel/clear/teardown (target/ui-submit-layout.log), and Todo real text input/Enter/cleanup (target/ui-submit-todo.log). Current-process renderer error log empty (target/ui-submit-runtime.log). Prior180 serial Rust,136 JS,70 Android unit tests, workspace check and Windows clippy apply to final production implementation. Default textarea newline behavior is covered by Android unit tests, not a separate new device case this round. Restored Home/light; helper/reverse39590 stopped after verification.

Changes remain uncommitted, including shared h-auto and submitOnEnter work plus earlier IME changes. Full objective remains incomplete: Stream cursor animation/italic/per-corner radii/chart details, broader page visual parity and cross-platform verification still open.


### Chart tooltip overlay removes selection layout jumps

React ECharts tooltips overlay the chart; GPU previously inserted a text row above it. Added a plot-bounds invariant to GpuChartLayoutProbe and reproduced the old behavior on Pixel3: bar moved from Rect(302,728-717,1080) to Rect(302,794-717,1146), a66physical-pixel shift (target/ui-chart-overlay-before.log). Moved tooltip into the existing plot stack as a top-right themed, bordered shadowed overlay in shared/chart.ui.tsx, using TSX/Tailwind. Initial screenshot exposed vertical stretching; items-start fixed it and the probe now also bounds tooltip height below half plot height.

GPU typecheck/release build pass (target/ui-chart-overlay-build.log). Updated GpuChartLayoutProbe passes bar/line/pie in both themes, exact accessible data, plot selection, pie hide/show/all-hidden/restored percentages, unchanged plot bounds and compact tooltip height (target/ui-chart-overlay-after.log). Inspected target/ui-chart-overlay-bar.png and target/ui-chart-overlay-pie-dark.png. Current-PID renderer errors empty (target/ui-chart-overlay-runtime.log). Fixture data cleared, Home/light restored, helper/reverse39590 stopped. No native rebuild needed; reference showcase untouched and example remains uncommitted.

Tooltip anchors top-right; pointer-follow positioning, dismissal parity and animation are not implemented by this change. Existing chart gaps (nice ticks/dashed grid, exact bar/line/pie geometry, title tracking, responsive legend and long labels) remain. Full objective remains active.


### Stream Clear overlay and stable message viewport

Moved Clear from a conditional layout row into a top-right overlay within the message stack, matching React's positioning model. Added fixed list pt-12/pb-4 so the button cannot cover the first bubble and its appearance does not reserve/release layout height. Empty-state gutters remain unchanged. This intentionally improves the collision seen when using the reference's small top padding literally.

New chart-fixture assertion fails before the change (target/ui-stream-clear-before.log): Clear lies above the message viewport. Final light/dark bar/line/pie checks pass Clear containment and first-message non-overlap, plus all prior tooltip stability/legend checks (target/ui-stream-clear-after.log). Inspected target/ui-stream-clear-light.png after adding collision-free spacing; initial overlay-only screenshot showed overlap and was corrected. GPU typecheck/release build pass (target/ui-stream-clear-build.log), Stream light/dark empty/live/completed/cancel/clear/teardown regression passes (target/ui-stream-clear-regression.log). Current-process renderer errors empty (target/ui-stream-clear-runtime.log).

Home/light restored, helper/reverse39590 stopped. Reference showcase untouched, GPU changes uncommitted. This does not establish long-history scroll coverage or full visual parity; original objective remains active.


### Home failure feedback and localized retry

Home source review found action rejection rendered the success checkmark/green card and a hardcoded English error. Changed local error state to boolean, translate its copy at render time, use red themed feedback and an alert vector, and preserve the existing input/retry action. Switching language while the error is visible updates its text. Successful retry clears the error and restores the normal greeting card. This improves failure recovery beyond the reference's success-only View without changing the reference project.

GPU typecheck/release build pass (target/ui-home-error-build.log). New GpuHomeErrorProbe passes English failure, enabled retry, live Chinese localization/dark theme and successful retry (target/ui-home-error-device.log). Test helper temporarily overrides the Home instance's flush to throw, then restores it; direct replacement of greet did not work because action registration captures the original method. Earlier unsuccessful probes were injection failures, not successful rejection-path verification. Final fixture response logs confirm application. Inspected target/ui-home-error-dark.png; light screenshot at target/ui-home-error-light.png.

Restarted app after fixture cleanup and ran existing GpuHomeParityProbe: real cached/network background pixels, greeting/flush, English/Chinese, light/dark, rotation, native tab navigation/local-state retention, version and network rows pass (target/ui-home-error-regression.log). Current-process renderer errors empty (target/ui-home-error-runtime.log). Device Home/light with language Auto restored by the regression. Helper/reverse39590 stopped after verification. All changes remain uncommitted; no reference-showcase edits. Remaining Home gaps include backdrop blur and focus/hover visual states; full objective remains active.


### Shared italic text capability implemented; Android verification pending

Added fontStyle normal/italic as append-only wire field84, generated styles/types and Tailwind italic/not-italic lowering. Renderer inherits style, honors explicit normal reset, selects Parley FontStyle and includes slant alongside family in shaping cache identity. Paint::Text now retains synthesis skew through scrolling and all CPU/sparse-GPU/Vello glyph encoders; glyph ink bounds account for transformed extents. Local Glifo source confirmed glyph transforms apply in Y-down layout coordinates, requiring negative font-space skew. Corrected the initial sign before any device build/install.

Pixel regression confirms inherited italic differs from normal, explicit reset matches normal, local italic equals inherited italic, and switching back reuses correct cached shaping. Full181 Rust tests passed before adding final synthetic test/sign correction (target/ui-italic-rust-all.log). Dedicated synthetic-italic test then passed: forced12degree slope leans right and ink bounds contain all painted black pixels (target/ui-italic-synthetic.log).137 JS tests passed including official Tailwind lowering/normal override/invalid style rejection (target/ui-italic-js.log). All-target GPU feature check completed before the sign-only correction; final workspace all-target check and Windows browser-shell clippy completed (target/ui-italic-workspace.log, target/ui-italic-windows.log). Generated files match schema; diff whitespace check passes. No page applies italic yet.

Native rebuild ACTIVE: exec session93882 runs target/build-gpu-text-native.ps1, log target/ui-text-native.log. Last output Compiling lingxia-ui. Do not restart on quiet output. No helper/reverse running. Installed native supports field83 but NOT new field84. Next: add Lab italic fixture/probe for actual-vs-normal/reset, wrapping/input/scroll/theme on Pixel3, apply italic+muted text to empty assistant placeholder as React does, rebuild GPU dist/typecheck. Wait native build terminal success, copyso, assemble via target/assemble-gpu-text.ps1, install -r, serve helper and verify fixture plus Stream/Home regressions. Synthetic pixel test is CPU reference evidence, not proof of device GPU parity. Full objective remains active and all changes uncommitted.

### Italic rendering verified on Pixel 3

Added the Lab italic TSX fixture and GpuItalicProbe: normal, direct italic, inherited italic, explicit normal reset and a dynamic style compared across four frames. Stream's empty assistant placeholder now uses italic text-gray-400, matching the React text treatment. Final serial Rust suite passes all182 tests (target/ui-italic-final-rust.log); GPU typecheck/release build and all94 probe compilation pass (target/ui-italic-build.log, target/ui-italic-probe.log).

Native build session93882 completed successfully in8m22s. Copied the library, assembled and installed the field84-capable APK. Pixel3 probe passes actual raster difference, inheritance/reset equality and repeated cache toggles (target/ui-italic-device.log). Inspected target/ui-italic-device.png: right-leaning italic and correctly restored upright samples. This fixture covers single-line display text; wrapped text, editable italic and retained-scroll italic do not have separate device assertions in this round. CPU synthetic skew/ink-bound evidence remains separate from device rendering evidence.

Stream light/dark empty/live/completed, cancel, clear, teardown and fresh entry regression passes (target/ui-italic-stream.log); inspected target/ui-italic-stream-dark.png. Home remote background pixel assertion, greeting/flush, language/theme, rotation and local state retention pass (target/ui-italic-home.log). Current-process renderer error log empty (target/ui-italic-runtime.log), diff whitespace check passed. Home/light restored. Reference showcase untouched by this work; shared and GPU-example changes remain uncommitted. Full objective remains incomplete, including per-corner bubble geometry, animations, chart details, wider visual parity and cross-platform verification.

### Readable chart scales implemented; device verification pending

Added paired domainMin/domainMax to the shared plot primitive (append-only fields85/86), generated TS/JS props, precommit and native wire validation, and f64 domain snapshots. Domains must contain zero/all values, be strictly ordered within +/-1e9, and apply only to bar/line. Omitted domains retain prior geometry. Both CPU/GPU use the same domain-based shapes. Native tests assert signed bar/line geometry, exact data preservation, decimal bound precision and malformed wire rejection.

GPU chart-scale.ts computes readable ticks including zero, signed, decimal, constant, bounded extreme and subnormal data. Quarterly24/38/31/52 now maps to0..60 with10-unit ticks. Labels/grid positions derive from the same domain passed to plot; source values and accessibility summaries remain exact. Decimal tick rounding avoids duplicate endpoint labels. Chart styling stays TSX/Tailwind.

183 full Rust tests passed before adding the final wire-validation-only test (target/ui-domain-rust.log). All11 plot tests including that new test pass (target/ui-domain-plot-tests.log); initial rerun hit transient Windows LNK1104, retry succeeded.138 JS tests and2 chart-scale tests pass (target/ui-domain-js.log, target/ui-domain-scale.log). Workspace/all-target GPU checks and Windows browser-shell clippy finish successfully (target/ui-domain-workspace.log, target/ui-domain-gpu-check.log, target/ui-domain-windows.log). Final GPU typecheck/releasebuild and94 probe compilation pass (target/ui-domain-build.log, target/ui-domain-probe.log). GpuChartLayoutProbe adds exact60..0 tick assertions and screenshot measurement of Q4 bar height against52/60, in both themes.

Native build ACTIVE session88255, target/build-gpu-text-native.ps1, log target/ui-text-native.log. Latest live process rustc13752 compiling/linking native. Installed APK supports only field84; do not serve new dist until new APK installed. No helper/reverse active. Next: await existing build terminal success, copyso, assemble/install, start normal target/gpu-todo-session.py, reverse39590, relaunch, pushjar, run GpuChartLayoutProbe, inspect screenshots and errors, regress Stream if indicated. Full goal remains active and changes uncommitted. Dashed grid, precise ECharts geometry and animations remain open.

### Readable chart scales verified against reference and Pixel 3

Native build session88255 finished successfully in8m08s; copiedso, assembled and installed the field85/86-capable APK. Updated GpuChartLayoutProbe passes bar/line/pie in both themes, exact60..0 tick labels, measured Q4 bar pixel height for52 on0..60 scale, accessible exact data, selection/tooltip bounds and pie hide/all-hide/restore interactions (target/ui-domain-device.log). Inspected target/ui-domain-bar-light.png and target/ui-domain-line-dark.png: aligned ticks/grid with no label collisions. Current-PID renderer error log empty (target/ui-domain-runtime.log).

Ran the reference project's installed ECharts in SVG SSR mode for quarterly, mixed-sign, decimal and zero datasets; its extents/ticks match the new helper (target/ui-domain-reference.log). An additional80 deterministic positive/mixed datasets produced no tick mismatches (target/ui-domain-reference-sweep.log). This is numerical parity evidence for those cases, not proof of all ECharts options or geometry. Device pixel coverage here is positive quarterly data; signed/decimal geometry is covered by Rust tests, not a new device fixture.

Home/light restored and fixture messages cleared by the probe. Helper/reverse39590 stopped after verification. Shared changes and GPU example remain uncommitted. The overall goal remains incomplete: dashed grid, precise chart marker/bar/pie geometry, responsive legends, animation and broader page/cross-platform visual parity are still open.

### Line marker shape aligned; native build active

Source inspection corrected an initial empty-state assumption: React's outer scroll is a block container, so EmptyState's flex-1/justify-center alone does not establish vertical centering. No empty-state layout change was made. A confirmed chart difference was fixed instead: reference ECharts LineSeries defaults to emptyCircle, symbolSize6, with a2px centered stroke and neutral00 (#fff) fill (local ECharts symbol.js). GPU used filled rounded rectangles.

Added a shared Circle plot shape, emitted as colored outer8px and white inner4px circles per line sample, shrinking proportionally when plot padding is constrained. Render maps circles to existing rounded paint, shared by CPU/sparseGPU/Vello. JS and Rust geometry budgets now count both circles per marker. No new wire fields; existing domain85/86 API remains. CPU high-density raster regression confirms white center, exact blue ring and untouched dark outside corner.185 Rust tests and138 JS tests pass (target/ui-marker-all-rust.log, target/ui-marker-js.log); initial targeted test link hit transient Windows LNK1104, retry passed (target/ui-marker-pixels.log). Workspace/all-target GPU checks and Windows browser-shell clippy finish successfully (target/ui-marker-workspace.log, target/ui-marker-gpu.log, target/ui-marker-windows.log). GPU typecheck/release build and94 probe compilation pass (target/ui-marker-build.log, target/ui-marker-probe.log). GpuChartLayoutProbe now checks final line marker white center and blue ring pixels in both themes, in addition to previous domains/selection/legend checks.

Native build ACTIVE session87422, target/build-gpu-text-native.ps1, log target/ui-text-native.log. Last confirmed live rustc26508 compiling/linking native. Do not restart on quiet output. No helper/reverse active. Installed APK still has old filled line markers. Next: await existing build success; copyso, assemble via target/assemble-gpu-text.ps1, install -r, start normal helper, reverse39590, force-stop/launch, pushjar, run GpuChartLayoutProbe, inspect line screenshots and current-PID renderer errors. Restore Home/light and stop helper/reverse. Shared/GPU work remains uncommitted. Full goal active; marker device parity is not yet established.

### Line marker circular appearance verified on Pixel 3

Ran the new marker probe against the previously installed APK while native build87422 was active. It failed at the intended assertion: line marker white center got ff3b82f6 (target/ui-marker-before.log), demonstrating the old solid marker is detected. Existing build then completed successfully in8m40s. Copiedso, assembled (BUILD SUCCESSFUL) and installed the new APK; a relative log-read after the assemble script failed because that script changes its shell directory, then the absolute log path confirmed success. No build was restarted.

The identical GpuChartLayoutProbe now passes light/dark white-center and blue-ring pixels plus bar-domain height, exact ticks/data, all three plot selection/tooltip stability and pie hide/all-hide/restore (target/ui-marker-after.log). Pulled both line screenshots; inspected target/ui-marker-line-dark.png, confirming circular white-centered markers along the curve. Current-PID renderer error log empty (target/ui-marker-runtime.log). Prior185 Rust/138 JS and workspace/GPU/Windows checks apply to final implementation.

Home/light restored and fixture messages cleared by probe. Helper session78520 and reverse39590 stopped after verification. Shared/GPU work remains uncommitted. Full objective remains active; dashed grid, per-corner radii/bar geometry, chart animation/legend responsiveness and broader page/cross-platform parity remain open.

### Bar top-only corners implemented; native build active

Confirmed reference ECharts SVG SSR output for positive and negative bars uses [4,4,0,0]: geometric top-left/top-right rounded, bottom square (target/ui-bar-reference.svg). Replaced plot bar rounded-all-corners rectangles with a single shared filled path using radius min(4,width/2,height). Eight path elements are counted in both JS/native plot budgets. Existing Paint::Vector handles CPU/sparseGPU/Vello and retained scroll. No wire change. This does not add generic per-corner container styles or fix Stream bubble corners.

CPU pixels verify both upper corners are background and both lower corners filled for positive and negative bars. Short/narrow/zero path bounds are tested; first bounds test exposed only f64 curve-extrema noise (19.999999999999996 vs20), so bound comparisons use1e-9 tolerance; raster expectations remain exact. Initial link encountered transient Windows LNK1104, retry succeeded. Final187 Rust tests and139 JS tests pass (target/ui-bar-corners-final-rust.log, target/ui-bar-corners-js.log), including shared budget rejection for17 maximum-size bar plots. Workspace/all-target GPU check and Windows browser-shell clippy complete (target/ui-bar-corners-workspace.log, target/ui-bar-corners-gpu.log, target/ui-bar-corners-windows.log). GPU typecheck/release build and94 probe compilation pass (target/ui-bar-corners-build.log, target/ui-bar-corners-probe.log). Diff whitespace check clean.

Updated GpuChartLayoutProbe checks first bar top-corner pixels against nearby background, in addition to previous scale/marker/selection/legend assertions. Old installed APK fails exactly at bar top corner follows four-pixel radius (target/ui-bar-corners-before.log), before any new native install.

Native build ACTIVE session90094, target/build-gpu-text-native.ps1, log target/ui-text-native.log. Last confirmed live rustc29112 compiling/linking native. Do not restart on quiet output. Helper session18014 ACTIVE, log target/ui-bar-corners-helper.log; reverse39590 active. Device left in Stream bar fixture/light from intentional failing baseline. Next: await existing native build success, copyso, assemble using target/assemble-gpu-text.ps1 (it changes shell cwd; read logs via absolute paths), install-r, force-stop/launch with existing normal helper, run same GpuChartLayoutProbe to target/ui-bar-corners-after.log, inspect screenshots/errors. Restore Home/light and stophelper/reverse. Device corner verification still pending; all work remains uncommitted and full goal active.

### Bar top-only corners verified on Pixel 3

Existing native build90094 completed in8m40s, copiedso, assembled and installed successfully. Extended GpuChartLayoutProbe with exact blue pixels at both lower corners;94 probes compiled and updated jar pushed. The same top-corner check that failed on the old APK now passes, along with new bottom-left/right checks in both themes and previous bar scale, line marker, all plot selection/tooltip and pie legend interactions (target/ui-bar-corners-after.log). Pulled light/dark screenshots; inspected target/ui-bar-corners-light.png, showing rounded tops and square baseline corners. Current-PID renderer error log empty (target/ui-bar-corners-runtime.log).

Final187 Rust/139 JS and workspace/GPU/Windows checks from implementation turn apply. Device fixture covers positive quarterly bars; negative and tiny bars are covered by CPU/path tests, not a new device fixture. Probe restored Home/light and cleared messages. Helper18014 and reverse39590 stopped after verification. All shared/GPU changes remain uncommitted. Overall parity remains incomplete, including generic per-corner bubble styles, dashed chart grid, animation, responsive legends and broader page/cross-platform checks.

### Shared flex-wrap and responsive pie legend implemented; device build pending

Added append-only style field87 flexWrap (nowrap/wrap/wrap-reverse), generated schema/types, official Tailwind flex-wrap/flex-nowrap/flex-wrap-reverse lowering, and Taffy flex_wrap mapping. Omission keeps nowrap. Native layout regression verifies two rows with gaps, reverse cross-axis order, nowrap overflow, and relayout to three rows at narrower width. Native wire tests reject unknown/nonstring values; JS tests cover utility mapping and inline override.

Pie legend changed from fixed two-column grid to content-sized centered flex-wrap row in TSX/Tailwind. Buttons use max-w-full/min-w-0 and shrinkable labels; short Q1..Q4 should fit one row, longer labels wrap. Added chart-legend test ID. Helper target/gpu-todo-session.py now accepts __probe/chart/pie-long with North America/South America/Asia Pacific/Europe labels. GpuChartLayoutProbe verifies one row for short labels, multiple for long labels, item containment/nonoverlap and long-label hide action in both themes, alongside existing chart scale/marker/corner/selection checks. These new device assertions are compiled, NOT yet run.

188 full Rust tests pass before the final wire-test-only addition (target/ui-wrap-all-rust.log); both final flex_wrap tests pass (target/ui-wrap-final-tests.log).140 JS tests pass (target/ui-wrap-js.log). Workspace/all-target GPU and Windows browser-shell checks succeed (target/ui-wrap-workspace.log, target/ui-wrap-gpu.log, target/ui-wrap-windows.log). GPU typecheck/release build and94 probes compile (target/ui-wrap-build.log, target/ui-wrap-probe.log). Diff whitespace check passes.

Native build ACTIVE session76392, target/build-gpu-text-native.ps1, log target/ui-text-native.log. No helper/reverse active. Installed APK supports fields through86, NOT flexWrap87; do not load new dist until new native installed. Next: poll existing build to success, copyso, assemble/install, start normal helper, reverse39590, relaunch, push latest probejar, run GpuChartLayoutProbe to target/ui-wrap-device.log. Inspect short/long legends and clipping in screenshots; fix measured layout if assumptions fail. Check current-PID renderer errors, restore Home/light, stop helper/reverse. All shared/GPU changes remain uncommitted. Full objective active; precise pie geometry, dashed grid, generic per-corner bubbles, animation and broader visual/cross-platform parity remain open.

### Responsive pie legend verified on Pixel 3

Existing native build76392 finished in7m20s. Copiedso, assembled and installed field87-capable APK. First probe failed to find chart-legend because decorative layout rows are excluded from the Android semantic tree; screenshot showed correctly aligned single-row Q1..Q4 and no renderer errors. Corrected only the probe: check actual button bounds inside page-scroll, horizontally within the plot and vertically below it, along with nonoverlap and distinct row counts. No production accessibility nodes were added for testing.

Final GpuChartLayoutProbe passes short labels on one row, North/South America/Asia Pacific/Europe on multiple rows, containment/nonoverlap, and wrapped South America hide interaction, both themes (target/ui-wrap-device.log). Existing bar corner/domain, line marker, selection/tooltip and pie legend regressions also pass. Inspected target/ui-wrap-diagnostic.png (short labels) and target/ui-wrap-legend-dark.png (long labels), confirming readable centered layout and hidden series styling. Current-PID renderer errors empty (target/ui-wrap-runtime.log). CPU tests cover resized viewport and wrap-reverse; this round's device checks vary label lengths, not device orientation.

Home/light restored and messages cleared by probe. Helper84924 and reverse39590 stopped after verification. Shared and GPU example changes remain uncommitted. Full objective remains active: precise pie geometry, dashed grid, generic per-corner bubbles, animation and broader visual/cross-platform parity remain open.

### Bounded vector dash capability implemented; chart integration still pending

Added optional VectorPath.strokeDasharray to generated types and JS/native validation. Supports0..16 positive viewBox-unit lengths in1/64..1024; omission/empty means solid. Kurbo handles odd pattern repetition. Before stroke expansion, native checks perimeter/shortest dash + path elements <=4096, preventing unbounded subdivision before allocation; expanded outlines still count against vector/frame budgets. Rendering continues through shared filled vector paths across CPU/sparseGPU/Vello. This is a reusable primitive capability, not page-generated rectangle fragments.

Native tests check painted/gap geometry, odd-vs-doubled pattern equality, empty solid pattern, invalid values and excessive subdivision rejection.190 full Rust tests pass before adding final raster-only case (target/ui-dash-all-rust.log); final CPU raster test passes exact black/white dash/gap pixels after4x viewBox scaling (target/ui-dash-raster.log).141 JS tests pass including encoded arrays and invalid patterns (target/ui-dash-js.log). Workspace/all-target GPU check and Windows browser-shell clippy finish successfully (target/ui-dash-workspace.log, target/ui-dash-gpu.log, target/ui-dash-windows.log). Generated types match schema, diff whitespace check clean.

No Android rebuild/install or device test this turn; no helper/reverse active. Installed native still rejects strokeDasharray in vector JSON. No showcase page uses it yet; chart grid remains solid. Next work must finish chart integration rather than claim dashed grid parity: vector currently fits viewBox with preserved aspect and scales stroke/dash lengths. A full-width grid needs a reusable width/fit solution and should preserve suitable logical-pixel dash spacing across sizes; do not settle for page-side fragments or silently stretched icons. Implement/verify the necessary shared sizing behavior, then replace chart grid, rebuild/install native, add device gap/width/theme checks and regress charts. All changes remain uncommitted, and full goal remains active.

### Full-width fixed-size vector strokes and chart dashed grid integrated; native build active

Added vector fit=contain|fill (default contain) and path vectorEffect=none|non-scaling-stroke. Fixed strokes expand after the viewBox transform, preserving logical-pixel width/dash spacing; fill and stroke order stays source order. Deferred outlines recheck subdivision, per-vector16384 and aggregate-frame32768 geometry budgets after layout. Transformed fixed-stroke spans above32768 or coordinates beyond1e12 reject before expansion. Existing ordinary icons retain centered contain fitting and scaled strokes. Generated TS types, compiler prop audit and JS/native validation updated.

Chart grid now uses a full-width vector with fit=fill, one-pixel non-scaling stroke,4/2 dash pattern, Tailwind theme tint; bottom zero axis remains solid with line-200 tint. Reference installed ECharts SSR confirms stroke-dasharray4,2 (target/ui-grid-dash-reference.svg).193 Rust tests pass (target/ui-vector-fit-all-rust.log), including exact pixels at two widths, fixed stroke thickness, deferred subdivision/frame-budget rejection and prior vectors.142 JS tests pass (target/ui-vector-fit-js.log). Workspace/all-target GPU and Windows browser-shell checks complete (target/ui-vector-fit-workspace.log, target/ui-vector-fit-gpu.log, target/ui-vector-fit-windows.log). GPU typecheck/release build and94 probes compile (target/ui-vector-fit-build.log, target/ui-vector-fit-probe.log). Diff whitespace check passes.

GpuChartLayoutProbe adds dash/gap pixel samples near start/middle/end of top grid in bar/line and both themes, alongside existing scales/markers/corners/legend/selection checks. These new assertions are compiled, not yet device-run.

Native build ACTIVE session42254, target/build-gpu-text-native.ps1, log target/ui-text-native.log. Last confirmed live rustc17388 compiling/linking native. No helper/reverse active. Installed APK supports flexWrap87 but NOT new vector JSON fit/effect/dash properties; do not serve current dist until new native installed. Next: poll existing build, copyso, assemble/install, start normal helper, reverse39590, relaunch, pushjar, run GpuChartLayoutProbe to target/ui-vector-fit-device.log. Inspect actual grid gaps/thickness in screenshots; investigate sample alignment if probe fails rather than weakening intended visual checks. Check errors, restore Home/light, stop helper/reverse. Changes uncommitted and full goal active.

### Dashed chart grids verified on Pixel 3

Existing native build42254 completed successfully in7m05s. Copiedso, assembled and installed the new vector-capable APK. GpuChartLayoutProbe passes dash/gap pixel assertions at left/middle/right in both bar/line plots and both themes (target/ui-vector-fit-device.log), together with prior scale/bar corners/line rings/tooltip/legend checks including long-label wrap. Inspected target/ui-vector-fit-line-dark.png: grid is dashed across its width and bottom axis remains solid. Also saved target/ui-vector-fit-bar-light.png. Current-PID renderer error log empty (target/ui-vector-fit-runtime.log).

Prior193 Rust/142 JS and workspace/GPU/Windows checks apply. Device tests sample the showcase's current plot sizes; fixed spacing at two different widths and stroke thickness are established by CPU raster tests, not a new rotated-device case. Home/light restored and fixture messages cleared. Helper42836 and reverse39590 stopped. All changes remain uncommitted. Full objective stays active: per-corner message bubbles, exact pie geometry, animations and broader page/cross-platform parity remain open.

### Signed chart zero axis aligned and verified

Changed chart.ui.tsx to draw the solid axis at tick value zero rather than the final tick. Mixed-sign data now has a central zero axis; all-negative data has its zero axis at the top, and negative bottom ticks remain dashed. The React reference's installed ECharts model confirms domains [-40,60] and [-60,0] for the new fixtures and automatic on-zero axis positioning.

GPU typecheck/release build and chart-scale tests (2) pass. GpuChartLayoutProbe now checks dash/solid pixels for mixed-sign and all-negative bars in both themes, alongside existing bar/line/pie selection, grid, markers, corners and wrapped-legend regressions. Pixel 3 run passes (target/ui-zero-axis-device.log); inspected target/ui-zero-axis-signed-light.png. Renderer error log target/ui-zero-axis-runtime.log is empty. No native code changed or native rebuild required. Home/light restored and fixtures cleared. Changes remain uncommitted; overall parity remains incomplete, including per-corner bubbles, exact pie geometry, animation and broader cross-platform verification.

### Configurable pie layout and reference-sized chart (native build in progress)

Added pieLayout (append-only field 88): center fractions and inner/outer radius fractions of half the smaller content dimension. JS/native reject nonfinite/out-of-range/unknown values, malformed bounded JSON, reversed radii and use on non-pies. Default retains legacy geometry. Snapshot includes resolved layout; Android uses it for hole/outside hit rejection and queued-click identity. GPU Chart uses reference center [0.5,0.44], radii0.4/0.68 in a210-high plot, with wrapped legend at the bottom. Installed reference ECharts confirms center(150,92.4), radii42/71.4 at300x210 (target/ui-pie-layout-reference.log).

193 existing Rust tests plus three new targeted raster/invalid-layout/wire tests pass.143 JS tests pass. Android GpuPlotTest passes including configured hole/ring/outside and stale queued selection. Workspace all-target, GPU all-target and Windows browser-shell clippy pass. GPU typecheck/release build and updated Android probes compile. Earlier build attempt rejected unsupported axis-specific gap utilities; final page uses supported uniform gap and explicit legend line height.

Native build95980 remains live; poll that handle, do not start another build. Logs target/ui-text-native.log. No helper/reverse active. Installed APK predates field88; don't serve new dist until installing new native. Next copyso, assemble/install, start helper, run GpuChartLayoutProbe with new reference-radius pixel/touch assertions and legend containment checks in both themes. Existing full objective remains active; selection emphasis/animation and other visual parity remain pending. Changes uncommitted.

### Reference pie layout verified on Pixel 3

Native build95980 completed successfully in8m28s; copiedso, assembled and installed the new APK. GpuChartLayoutProbe passes in light/dark (target/ui-pie-layout-device.log): reference inner/outer radius pixels, physical ring selection, ignored hole/outside taps, short/wrapped legends below the ring, hide/restore, tooltips, plus bar/line/dashed-grid/signed-zero-axis regressions. Inspected target/ui-pie-layout-light.png and target/ui-pie-layout-legend-dark.png. Current-PID renderer error log target/ui-pie-layout-runtime.log is empty. Home/light restored and fixture cleared. This verifies current Pixel portrait sizes, not all viewport/platform combinations; pie selection emphasis/animation and remaining overall parity are still open. All changes remain uncommitted.

### Shared letter spacing and Tailwind tracking implemented; native build active

Added signed letterSpacing field89, bounded +/-1024, inherited in native layout with explicit zero reset. Parley receives spacing before measure/line break/editor geometry; cache identity includes spacing. Official Tailwind lowering supports letter-spacing normal and px/rem/em lengths, including negative values. Em resolves against owner computed font size once; native descendants inherit pixels. Inline spacing remains numeric. Chart title now uses tracking-widest (10px font =>1px spacing).

199 Rust tests pass serially (target/ui-tracking-rust.log), including spacing inheritance/reset/negative input widths, changed wrapping/cache restoration and signed wire validation. Initial wrapping test incorrectly measured the viewport-sized root; changed fixture to a text child with intrinsic height. Initial parallel suite also hit the existing editor budget timeout; serial full suite passes without loosening deadlines.144 JS tests pass (target/ui-tracking-js.log), covering official classes, em resolution, overrides and invalid values. Workspace/all-target GPU and Windows browser-shell checks pass (ui-tracking-workspace/gpu/windows.log). GPU typecheck/release build and95 Android probes compile (ui-tracking-build/probe.log).

Added Lab tracking-fixture.tsx and helper --tracking. GpuTrackingProbe compares exact raster for normal/direct/inherited/reset/dynamic rows across4 toggles, checks increased glyph advance and returns Home. These device assertions are not run yet. Native build ACTIVE session65611 (target/build-gpu-text-native.ps1, target/ui-text-native.log), last confirmed compiling/linking native. No helper/reverse active. Installed APK supports pieLayout88 but not letterSpacing89; do not serve current dist before installing new native.

Next poll65611, copyso, assemble/install, launch helper --tracking, reverse39590, relaunch/pushjar/run GpuTrackingProbe; inspect screenshot and renderer errors. Stop/restart helper without flag for GpuChartLayoutProbe regression including title. Restore Home/light, stop helper/reverse. All changes remain uncommitted. Full objective active, with per-corner bubbles, selection emphasis/animation and broader page/platform parity open.

### Tracking and pie percentage presentation verified on Pixel 3

Native build65611 completed successfully in8m26s; copiedso, assembled and installed. GpuTrackingProbe passes expanded glyph extent, direct/inherited/reset exact raster comparisons and4 dynamic toggles (target/ui-tracking-device.log). Inspected target/ui-tracking-device.png. Restarted helper without fixture and GpuChartLayoutProbe passes both themes, including pie radius/touch/legend and signed axis regressions (target/ui-tracking-chart-device.log). Inspected target/ui-tracking-pie-tooltip-light.png: title tracking is visible and tooltip is Q1:16.55%. Renderer error log target/ui-tracking-runtime.log empty. Home/light restored and fixtures cleared; helper99243 and reverse39590 stopped.

Also aligned pie tooltip to reference '{b}: {d}%' instead of raw value plus integer percentage. New chart-percent.ts uses largest-remainder allocation of hundredths; raw series values remain in accessible description. chart-percent.test.mjs compares136 datasets against installed reference ECharts, including hidden-series subsets, equal thirds, zeros, subnormal values and128-entry data. Test passes; GPU typecheck/build and95-probe compile pass after change. Device checks expect Q1:16.55%, Q2:26.21%, and Q1:22.43% after hiding Q2. This does not complete selection emphasis, pointer-follow/dismissal, animation or overall page/cross-platform parity. All changes uncommitted and full goal active.

### Independent corner border geometry prepared (not yet page-enabled)

Started the remaining Stream bubble corner requirement. React uses rounded-3xl with rounded-br-md for user and rounded-bl-md for assistant; GPU still has uniform corners. RoundedBox already represents independent elliptical corners, but Border::new previously flattened all outer corners to one radius. Added Border::with_radii and routed the existing uniform constructor through it. Normalize outer corners using one CSS overlap scale before subtracting per-edge widths; inner corners retain independent elliptic geometry and collapse sharp where needed.

Eight border tests pass, including three new tests for bubble small-corner contour, unequal border widths, overlapping radii, zero-width cancellation and exact translucent CPU pixels. Full202 Rust tests pass serially (target/ui-corner-border-all-rust.log); workspace/all-target GPU and Windows browser-shell clippy finish successfully (ui-corner-border-workspace/gpu/windows.log). Scoped diff whitespace check passes. No schema/style props changed, no native rebuild/install this turn, no helper or reverse active. Device still runs the previously verified tracking/pie build.

This is preparatory geometry, not completed bubble parity. Next must propagate corner geometry through backgrounds/gradients, border commands, outer and inner clips, snapshots/hit geometry, retained scrolling and shadows before enabling Tailwind rounded-*-* on Stream. Shadow backends currently use uniform-radius blurred rectangles; do not silently retain uniform shadows or use a page-side decorative approximation. Keep the shared rendering invariant and establish an implementation for independent-corner shadow geometry. Full goal remains active and all changes uncommitted.

### Independent-corner shadow geometry and CPU filter feasibility verified

Added CornerShadowGeometry with normalized owner contour, separately spread per-corner silhouette, blur sigma/extent and shared owner knockout. Legacy uniform BoxShadow::geometry delegates to this calculation and retains its uniform fast-path output. CSS sharp-corner positive-spread correction runs per corner; negative spread clamps each independently, and owner overlap normalization happens before spread. Tests cover offset/extent, positive/negative spread, sharp corners, asymmetric owner knockout and overlapping large radii.

204 full Rust tests pass serially before adding one final CPU-filter feasibility test (target/ui-corner-shadow-all-rust.log); the added test separately passes (ui-corner-shadow-filter.log). It uses vello_cpu0.3 Filter::from_function(FilterFunction::Blur{radius:sigma}) around the independent path, with owner knockout outside the filter, checking small-corner outside alpha versus large corner and zero alpha in owner interior. Workspace/all-target GPU and Windows browser-shell checks finish successfully (ui-corner-shadow-workspace/gpu/windows.log). Scoped diff check passes.

Inspection of installed dependencies establishes a concrete next path: vello_cpu0.3 and vello_gpu0.3 expose push_filter_layer and Gaussian blur. CPU filters are experimental and panic with multithreaded contexts, so preserve single-threaded context use for that path. vello0.11 Scene only exposes uniform draw_blurred_rounded_rect, requiring an equivalent fallback or implementation; don't feed independent geometry through a scalar-radius shortcut. No page-facing corner fields/Tailwind utilities or rendering commands added yet; Stream remains uniform. No Android build/install/helper/reverse this turn, changes uncommitted and full objective active. Next integrate independent-corner rendering across backends, background/gradient/border/clips/hit/retained scroll, then schema/Tailwind and Stream styles, with CPU/GPU/device verification.

### Corner paint commands, retained scrolling and native hit/highlight transport integrated

Rect, Gradient and Border paint commands now carry optional independent elliptical corner radii. CPU, sparse GPU and Vello consume the same ClipSnapshot/RoundedBox contour; borders use Border::with_radii. Retained composition copies corners while translating bounds. A CPU pixel test manually supplies nonuniform paint commands for solid/gradient/border cases, verifies the small bottom-right versus large top-right contour, and repeats after scrolling.

NodeSnapshot adds optional radii (omitted for existing uniform nodes). InputHitMap, scroll descendant clips and editor outer clips consume them. Android GpuClip own hit tests and Highlight use the same radii. Removed GpuView's subsequent scalar-radius overrides so they cannot overwrite per-corner highlights. Highlight defaults missing radius to zero (required by existing tests). Added Android unit tests for small/large corner hits, highlight arrays and reset to uniform; GpuClipTest and GpuPlotTest pass (target/ui-corner-paints-android.log).

206 full Rust tests pass serially before final hit-snapshot test; final targeted test separately passes own editor and ancestor scroll masks (ui-corner-paints-all-rust.log, ui-corner-paints-hit.log). Workspace/all-target GPU and Windows browser-shell clippy pass (ui-corner-paints-workspace/gpu/windows.log). Scoped whitespace check passes. No author-facing fields or Tailwind classes yet; layout still emits corners/radii=None. No Android rebuild/install/device run this turn; no helper/reverse active. This is compile/CPU/unit verified, not device parity. Next: finish independent shadow paint across backends, then wire corner styles/cascade and layout snapshots consistently and apply Stream classes; verify border/content/image/editor/scroll clips and real-device bubbles. All changes uncommitted, full objective active.

### Independent corner styles and bounded cross-backend shadow masks connected; native build active

Added fields90–93 radiusTopLeft/TopRight/BottomRight/BottomLeft (0..16384). Native resolves omitted corners from uniform/default radius, normalizes contours, collapses equal corners to uniform fast path, passes corners into solid/gradient/border paints and snapshots, and supplies inner clips even without borders where independent corners need content clipping. Tailwind official declarations expand radius shorthand to all four fields; physical corner/side utilities map to corresponding fields; inline radius resets class corners before explicit inline corner overrides. Stream now uses rounded-br-md for user and rounded-bl-md for assistant alongside rounded-3xl.

Added MaskCache/RasterShadow in shadow.rs and immutable Bitmap identity/from_pixels support. Independent corner shadows rasterize once via single-threaded CPU Gaussian filter with owner knockout, then share premultiplied bitmap across CPU/sparseGPU/Vello. Position-independent key includes dimensions, normalized corners, offsets/spread/blur, resolved RGBA and display scale. Max4096 per axis/4M pixels per mask;64 entries/16MiB LRU; layout caps unique mask bytes at16MiB. Uniform shadows retain direct native rendering. Reset clears cache. Paint::RasterShadow preserves outer-shadow ordering outside editor clips, ancestor masks/opacity and retained-scroll translation; bitmaps participate in existing GPU texture binding but never host image requests or external bind_images replacement. This deliberately trades bounded texture memory for consistent arbitrary corner blur; no new performance claim.

209 full Rust tests pass serially (target/ui-corner-mask-rust.log), including authored bubble pixels, nearby shadow contour, bitmap reuse across layout, external-bind preservation, cache scale/eviction/budget checks.145 JS tests pass (ui-corner-mask-js.log), including Tailwind corner cascade, order independence and inline reset. GPU typecheck/release build passes (ui-corner-mask-build.log),95 probes compile (ui-corner-mask-probe.log). Workspace/all-target GPU/Windows browser-shell checks pass before the final image fix (ui-corner-mask-workspace/gpu/windows.log). GpuChartLayoutProbe now samples user small bottom-right/large bottom-left and assistant small bottom-left/large bottom-right pixels in both themes, deriving scale from plot and interior colors from padding. Probe not run yet.

IMPORTANT ACTIVE BUILD: native session83023 (target/build-gpu-text-native.ps1, target/ui-text-native.log) confirmed live rustc18832. After this build started, fixed image clipping for equal per-corner overrides (all8 without uniform radius): image paint now uses resolved radius instead of rereading field10. New targeted test passes (ui-corner-mask-image.log); total test inventory210. This final source change requires a FOLLOW-UP native build after83023 reaches terminal; do not restart it while live or claim its artifact includes the fix. Can optionally use first artifact to diagnose bubble-only behavior, but final verification/install must use rebuilt current code. No helper/reverse active; installed APK still predates corner fields90–93. Do not serve current dist until a corner-capable APK is installed.

Next poll83023 to terminal, run native build again for latest source, copyso/assemble/install, start normal helper/reverse39590/relaunch, push probejar and run GpuChartLayoutProbe. Inspect bubbles/shadows in light/dark screenshots, investigate probe geometry without weakening intended corner checks, check current-PID renderer errors. Additional image/editor/scroll/device coverage may be needed for the shared feature. All changes uncommitted; full objective remains active with emphasis/animation and broader parity still open.

### Final corner image regression included in full unit verification; follow-up native build running

Confirmed previous native build83023 terminal from release completion (8m24s) and no live cargo/rustc. Started required follow-up native build for current source: exec session97230, target/ui-text-native.log. Latest live rustc23860 confirms progress; do not restart this build. Full current lingxia-ui lib suite passes210 tests serially, including final equal-corner image clipping regression (target/ui-corner-final-rust.log). PowerShell wrapper reported exit1 from redirected stderr; test terminal summary explicitly has210 passed/0 failed.

Pixel3 serial914X1UMW2 remains connected. APK install, bubble corner pixel probes, light/dark screenshot inspection and runtime errors are still pending until97230 completes. No helper or reverse was started. Next poll97230, copy current liblingxia.so, run target/assemble-gpu-text.ps1, install and use normal target/gpu-todo-session.py plus GpuChartLayoutProbe as described above. Full objective remains active; unit success is not device visual parity.

### Independent corners and centered Stream empty state verified on Pixel 3

Follow-up native97230 completed successfully in8m05s and includes final equal-corner image fix. Copiedso, assembled (35s), installed on Pixel3 serial914X1UMW2. GpuChartLayoutProbe now passes both themes including all four bubble corner comparisons and bar/line/pie, signed-axis, tooltip and legend regressions (target/ui-corner-chart-device.log). Inspected light bubble screenshot target/ui-corner-failed.png and final dark target/ui-corner-bar-dark.png. Initial probe failures were sampling errors: border pixels, brief dark-shadow surface-color matches and nonsemantic containers absent from Android accessibility. Final probe locates a continuous bottom-padding run and samples clear of the border; it still requires small corners filled and large corners empty in both themes. Ordinary layout containers intentionally remain absent from accessibility; no renderer workaround was introduced.

Also aligned Stream empty content to React centering and px-8 spacing using grow shrink-0 min-h-full justify-center on its scroll child, with no fixed viewport/composer deduction. GpuStreamLayoutProbe checks centering from visible text bounds plus the authored icon/gap geometry and passes both themes, live/completed response, cancel, clear, teardown and fresh entry (ui-corner-stream-device.log). Inspected ui-stream-centered-light.png. GpuStreamAutoHeightProbe passes soft wraps/newlines,120px cap, deletion shrink and empty in both themes (ui-corner-composer-device.log; physical heights63/190/330). Latest page release build and95-probe compilation pass; typecheck passed before the final testId-only edit. Runtime error log ui-corner-runtime.log empty after chart/stream runs.

Home/light restored, helper48358 stopped and reverse39590 removed. No active build remains. Latest page content was served through dev resource transport; APK native is current, embedded page bundle predates the final centering/testId updates. All changes remain uncommitted and full goal active. Corner behavior outside these bubble fixtures, shortened/landscape empty-state scrolling, chart emphasis/animation/dismissal and broader page/platform parity remain separate acceptance work.

### Stream empty-state shortened viewport acceptance verified

Added GpuStreamViewportProbe (96 compiled probes). Pixel3 verifies portrait centering, keyboard reducing message viewport, scrolling to the last hint and back to title, physical landscape, and portrait recentering in both themes. Last hint must retain its full40CSS-pixel height as well as lie inside the viewport, avoiding false success from clipped accessibility bounds. Final run passes (target/ui-stream-viewport-device.log). Inspected ui-stream-landscape-last-hint.png and ui-stream-keyboard-last-hint-dark.png showing complete last hint above composer; starting-scroll screenshots also inspected. Runtime error log ui-stream-viewport-runtime.log empty. Rotation settings restored, Home/light restored, helper42993 and reverse39590 stopped. No native/page source changes or builds this turn. This closes the prior shortened/landscape empty-state acceptance item; full goal remains active with chart emphasis/animation/dismissal and broader page/platform parity open.

### Chart category shadow pointer aligned and verified

Removed page-side dimming of unselected bar/pie colors, which does not match reference default emphasis. Added a decorative TSX grid overlay for selected bar/line category, aligned to plot padding/category bands with height120 and y12. Tailwind bg-[#81828833] matches installed reference ECharts AxisPointerModel shadowStyle token rgba(129,130,136,0.2). Overlay paints after plot and before tooltip, without adding layout height or input handling. This is category pointer alignment, not completion of per-item emphasis or pie scale/labels.

Typecheck and release page build pass.96 probes compile. Pixel3 GpuChartLayoutProbe passes full light/dark regressions including exact pointer blend against neighboring background, other bars retaining original color, physical coordinate tap from Q1 to Q3 updating tooltip and pointer through the decorative overlay, unchanged plot bounds, and existing pie/legend/axis/bubble checks (target/ui-axis-pointer-device.log). Pixel sampling moved below tooltip shadow after a diagnostic failure; it remains above fixture data and between grid lines. Inspected ui-axis-pointer-line-dark.png and diagnostic bar screenshot ui-pointer-fail.png; final light screenshot ui-axis-pointer-bar-light.png saved. Runtime error log ui-axis-pointer-runtime.log empty. Home/light restored; helper30186 and reverse39590 stopped. Native unchanged and no build active. All changes remain uncommitted; full goal active. Next: actual item emphasis (pie5px growth/bold label, line/bar emphasis), tooltip positioning/dismissal, animations, broader page/platform parity. Latest pages remain dev-served rather than refreshed embedded APK.

### Pie emphasis outer-radius capability implemented; native build running

Added optional pieLayout.emphasisScale (0..64 logical px, default0) to existing bounded layout JSON, generated TS declaration and JS/native validation. Selected slice alone expands its outer radius; inner radius/center/angles and other slices stay unchanged. Shared Plot shapes serve every rendering backend. Android GpuPlot hit uses the same per-slice expanded edge and preserves owner/content/ancestor clips. Snapshot transports emphasisScale; queued selection already compares layout. Reference installed ECharts PieView expands only shape.r by scaleSize, matching this implementation. Showcase config now requests5px. No emphasis label, lightened color or animation added yet.

211 full Rust tests pass serially (target/ui-pie-emphasis-all-rust.log), including exact selected/unselected outer-edge, fixed-hole and outer-limit pixels at1x/2x for none/first/second selection.145 JS tests pass (ui-pie-emphasis-js.log), covering valid0/5/64 and invalid negative/oversized/nonfinite/type values. Android GpuPlotTest passes old geometry plus selected edge, switching, hole and unselected rejection (ui-pie-emphasis-android.log). Workspace all-target check passes (ui-pie-emphasis-workspace.log). Generator initially hit a transient locked exe; retry succeeded, generated-schema drift test passes. Page typecheck/release build passes.96 probes compile; GpuChartLayoutProbe adds5CSSpx growth pixel checks after Q2 selection and ensures hole/unselected edge/outer limit stay blank. Not device-run yet.

ACTIVE native build exec61907 (target/build-gpu-text-native.ps1, log target/ui-text-native.log), confirmed live rustc31144. Do not restart while live. Installed APK predates emphasisScale; do not serve current dist until new native is installed. No helper/reverse active. Next poll61907, copyso, assemble/install, normal helper and full chart probe, inspect screenshots/errors, restore Home/light and stop helper/reverse. Need GPU feature/Windows browser-shell compile gates for shared change as well. All changes uncommitted; full objective active. New pie emphasis labels, selection color/line-marker emphasis, tooltip dismissal/positioning, animation and broader parity remain open.

### Pie expansion clipping corrected while native build remains live

GPU all-target and Windows browser-shell checks passed, then code inspection found unbordered plot content lacked its own clip: large emphasis could paint outside the same rectangle Android hit testing enforces. Renderer now supplies content_clip for all plots (existing border/corner geometry and retained-composition inner clipping), keeping draw/input extents consistent. New CPU regression uses64px emphasis in a100x100 unbordered plot inside a larger white parent, checks red immediately inside and white past right/top/bottom edges. Full212 Rust tests pass (target/ui-pie-emphasis-clip-rust.log); post-fix GPU all-target and Windows browser-shell checks pass (ui-pie-emphasis-clip-gpu/windows.log).

ACTIVE native61907 still live, rustc22264 confirmed CPU341s at latest poll. IMPORTANT: clipping fix was made after61907 started, so follow-up native build is required after it reaches terminal. Never restart it while live. Do not attribute clip fix to first artifact. No helper/reverse active, no install/device run this turn; current dist includes emphasisScale and must wait for new APK. Next poll61907, follow-up native current source, then copyso/assemble/install and GpuChartLayoutProbe. Full goal active, all changes uncommitted; pie label/color emphasis and broader parity remain unfinished.

### Final clipping build started; bar/pie emphasis color aligned

Confirmed native61907 terminal success8m22s, then started required follow-up native20428 containing plot clip fix. It remains ACTIVE (rustc15488, target/ui-text-native.log), so poll same handle and never restart while live. No further native source edits after this build started.

While waiting, inspected installed reference zrender liftColor/ECharts default state: selected bar/pie fill channels multiply by1.1, truncate and saturate. Added shared/chart-emphasis.ts and selected-item color mapping in chart.ui.tsx; other colors unchanged, line skipped because current native line uses its first color globally and point-only emphasis requires separate support. chart-emphasis.test.mjs compares six palette colors and all256 grayscale values against installed reference liftColor/toHex; passes (target/ui-chart-emphasis-color.log). Page typecheck/release build passes.96 probes compile; chart probe now expects selected Q2 expanded edge#9865ff and selected bar plus pointer composite#4d8ce7, still checking other bars retain#3b82f6. Device assertions not run yet.

Pixel3 remains connected; installed APK predates emphasisScale. No helper/reverse active. Next wait20428 terminal, copyso, assemble/install, run normal helper/full GpuChartLayoutProbe and inspect new emphasis screenshots/runtime errors. Build current native does include final clip fix; no additional native rebuild needed unless new native changes occur. Latest TSX dist must not be served before new APK. Full goal active, uncommitted; pie emphasis label, line-marker emphasis, tooltip positioning/dismissal, animations and broader parity remain open.

### Pie outer expansion and bar/pie emphasis verified on Pixel 3

Final native20428 completed8m51s, copiedso, assembled30s and installed. This native includes plot viewport clipping fix. GpuChartLayoutProbe passes full light/dark suite on current native and dev-served latest dist (target/ui-pie-emphasis-device.log): Q2 outer edge expands5CSSpx with emphasized#9865ff, inner hole/unselected edge/6px outer limit unchanged; selected bars compose emphasized fill under category shadow#4d8ce7 while other bars remain#3b82f6; physical Q1-to-Q3 selection, original pie/legend/axis/bubble regressions pass. Inspected ui-pie-emphasis-light.png and ui-pie-emphasis-dark.png: Q1 visibly grows and brightens, remaining slices retain original colors. Runtime error log ui-pie-emphasis-runtime.log empty. Expanded hit boundary and clipping have unit/CPU coverage from earlier entries; the full device probe verifies existing selection interactions and expansion pixels.

Home/light restored, helper38632 stopped and reverse39590 removed. No active build/helper remains. Native installed is current; most recent page changes were dev-served, embedded-bundle freshness remains separate. All changes uncommitted and full goal active. Next implement actual pie emphasis label and line-marker emphasis, tooltip dismissal/positioning, animation; broader page/platform parity remains pending.

### Line marker emphasis implemented; native build active

Added append-only field94 markerEmphasisScale (line-only1..4, default1), generated schema/TS declarations and JS/native validation. Shared Plot circle shapes scale only selected empty-circle body while retaining stroke width, line, area and other points. Inspected installed ECharts Symbol.js: six-pixel symbol uses1.1 default ratio and strokeNoScale=true, so showcase now sets1.1 and selected diameters are8.6/4.6 (outer/inner), rather than scaling the two-pixel stroke. Plot content clipping from previous work also bounds enlarged markers. Category hit geometry stays unchanged; no Android input protocol change needed.

214 full Rust tests pass (target/ui-marker-all-rust.log): exact1.1 diameter geometry/unchanged other circles, scale2 raster ring/white-center movement and selected reset, invalid native tag/range.146 JS tests pass (ui-marker-js.log). Workspace/all-target GPU/Windows browser-shell checks pass (ui-marker-workspace/gpu/windows.log). Page typecheck/release build passes.96 probes compile (ui-marker-probe.log). GpuChartLayoutProbe captures Q3 before selection, physically selects it, then checks additional blue coverage at its lower outer edge after subtracting known category tint; not device-run yet. AA ring coverage uses bounded blue-dominance assertions in CPU raster test, while geometry test checks exact sizes.

ACTIVE native60488, target/ui-text-native.log, latest confirmed live rustc14036. No native changes after build start. Installed APK supports pie emphasis but predates field94; do not serve current dist before new APK installed. No helper/reverse active. Next poll60488 to terminal, copyso/assemble/install, run normal helper and full chart probe, investigate marker pixel assertion if needed without weakening actual growth requirement, inspect screenshots/errors and restore Home/light. Full goal active; pie emphasis label, tooltip position/dismissal, animation and broader parity remain open. All changes uncommitted.

### Line marker emphasis verified on Pixel 3

Native60488 completed7m46s, copiedso, assembled and installed. GpuChartLayoutProbe passes full light/dark regressions with selected marker growth measured against Q3 before physical selection (target/ui-marker-device.log). Initial test passed light but failed dark because red-channel decrease is not a theme-independent signal of new blue coverage. Final assertion subtracts the expected category tint and measures additional blue-minus-red contrast at the lower circular edge, clear of the curve; threshold remains15. It verifies growth instead of accepting only state or tooltip changes. Inspected target/ui-marker-line-dark.png. Renderer error log ui-marker-runtime.log empty. Home/light restored; helper67143 and reverse39590 stopped. No active build/helper remains; installed native supports field94, latest pages still dev-served. All changes uncommitted and full goal active.

Re-read React and GPU Home while waiting: primary content and managed image chain exist, but backdrop blur and focus/hover interaction visuals remain genuine parity work. Do not equate chart milestones with whole-app completion. Pie emphasis label, tooltip position/dismissal, animations, Home visual details and broader page/platform acceptance remain open.

### Plot clear-selection events and in-plot dismissal verified

Plot onSelect now admits -1 (matching selectedIndex's existing unselected state) while still rejecting less than-1, out-of-range/noninteger/type values and stale/discarded handles. Android selected plots expose standard accessibility ACTION_CLEAR_SELECTION; pie empty hits within its viewport dispatch-1 only when a selection exists. Existing click generation/identity and queued rebind checks remain in force. Chart TSX already maps a missing visible index to-1, so clearing removes tooltip/axis pointer and restores normal emphasis without separate page-side flags.

146 JS tests pass (target/ui-plot-clear-js.log), targeted Rust committed-generation selection/clear test passes (ui-plot-clear-rust.log), Android GpuPlotTest passes clear-action availability, empty hit and disabled/unselected cases (ui-plot-clear-android.log). Typecheck/release page build and96-probe compile pass. Reassembled Java APK23s using previously verified current native (production Rust unchanged) and installed. GpuChartLayoutProbe passes full light/dark suite including physical taps on donut hole/outside ring clearing tooltip and allowing reselection; all three chart kinds clear via accessibility, remove the clear action when unselected, then reselect (ui-plot-clear-device.log). Runtime error log ui-plot-clear-runtime.log empty; scoped diff check passes. Home/light restored, helper31215 and reverse39590 stopped. No active build/helper remains.

This handles empty areas INSIDE the pie viewport and explicit accessible clear. Clicks outside the plot viewport, tooltip following/placement, pie emphasis label, animations, Home backdrop blur and broader page/platform parity remain open. No overall completion claim; all changes uncommitted. Latest pages were dev-served, embedded freshness separate.

### Shared plot selection and outside dismissal verified

Rust PlotSnapshot owns category/pie hit geometry and next-item selection;
NodeSnapshot applies the renderer's content and ancestor clips. ViewSession::select_plot
resolves the committed node/callback and rejects stale generation, missing identity,
invalid index and pending text geometry. Android forwards point, next and explicit
selection through nativePlot; removed Java angle/radius/category and next algorithms.
Android still builds accessibility actions and collects outside-dismissal targets.
Stationary taps outside a selected plot clear it; motion/long press do not enter
the new blank-tap dismissal path. This is a partial platform-boundary migration,
not completed iOS/Harmony support.

217 Rust tests pass (target/ui-shared-plot-session.log); workspace all-targets and
GPU feature checks pass; Android adapter tests and all 96 probes compile.
Native build72961 completed in10m42s, APK assembled33s and installed on Pixel3.
GpuChartLayoutProbe passes both themes, including existing physical selection,
emphasis pixels, accessible clear/reselection and new physical title-tap dismissal
for bar/line/pie (ui-shared-plot-device.log). UI runtime log is empty.
Full showcase visual/behavior parity remains open; no overall completion claim.

### Android device tools moved to SDK ownership

The 127 driver/fixture files now live in `lingxia-sdk/android/tests/gpu/`.
The 126 non-document files are byte-identical to the pre-move manifest;
README commands and parity links use the new location. All96 Java probes
compile through the relocated runner. GpuChromeProbe passes on Pixel3 from
that entrypoint (target/ui-test-move-device.log); UI error log is empty.
Showcase retains its TS/JS page tests and this checklist. Device scenarios
still depend on showcase IDs; this move does not establish reusable scenarios
or iOS/Harmony coverage.

### Home spacing follows React section structure

Replaced the card-wide12px gap with TSX sections and Tailwind spacing matching
React: logo/title12px, header/form24px, form controls12px, greeting/appearance16px,
language12px and version4px. Version uses leading-none; the IP caption uses
tracking-wide. Typecheck/release page build pass. Pixel3 GpuHomeParityProbe
passes greeting, Chinese/English, theme, rotation, navigation/state retention,
version/network and remote background checks (target/ui-home-spacing-device.log).
GpuHomeErrorProbe passes localized failure and retry recovery. Inspected
ui-home-spacing-light.png and ui-home-spacing-landscape.png; UI error log empty.
Pages were dev-served on the previously verified native APK. Backdrop blur,
interaction styling, chart emphasis labels/tooltip positioning and broader
visual parity remain open.

### ToDo responsive typography and footer layout

Added the missing481–768px input/row typography and padding, intermediate14px
panel corners, wordmark tracking, reference empty-state spacing/colors and
completed bulk-toggle accent. Desktop footer now places count, filters and clear
in one wrapping row rather than forcing clear onto a full-width second row.
Typecheck and release page build pass. GpuTodoWideProbe passes long-task wrapping,
capped panel, desktop footer alignment and dark completed state on Pixel3
(target/ui-todo-layout-wide.log). Inspected ui-todo-wide-layout.png and the normal
density empty-state screenshot ui-todo-empty-layout.png. Empty-state fixture
changed only page memory and restored it; the existing non-test task was verified
afterwards, with no test tasks remaining. UI error log is empty.

The full GpuTodoParityProbe did NOT run past its empty-baseline assertion because
the device contains a non-test task; its shell reports Killed, while AndroidRuntime
records that assertion. Do not count these attempts as a functional pass. The
481–768px breakpoint is compiled but not separately measured on-device this turn.
Emoji grayscale, backdrop filtering, hover/focus visuals and animations remain.

### Image request scheduling shared in Rust

`ImageRequests` now owns deduplication, the four-transfer bound, source/token
identity and completion across eviction/reset/reload. Obsolete transfers keep
their slots until completion; finished sources do not restart on every frame.
The queue belongs to GpuView rather than the replaced ViewSession. Android keeps
a JNI adapter, closes its queue on disposal and sends queue/token with bytes.
Decode occurs outside the registry lock; final token validation and bitmap
binding are atomic with invalidation. Downloads, policy and disk cache continue
through lxapp's managed image loader; no new downloader/cache dependency.
The former Java queue tests moved to the shared Rust module.

220 Rust tests, Android UI unit tests, workspace all-targets and GPU feature
checks pass (target/ui-shared-images-final-*.log). Final native54986 completed
7m55s, APK assembled25s and installed. Pixel3 Home parity passes remote background,
local icon, themes/rotation/navigation (ui-shared-images-home.log). The updated
SDK verify-images driver passes first/end list pixels, shared source, eviction/
reload, actual task-stack resume and idle (ui-shared-images-list-final.log).
Its initial run failed because the historical resume command launched Home;
the driver now focuses the original task instead. Inspected the last-row image
screenshot; LingXia.UI warning/error log is empty. Pages remain dev-served.
This adds reusable lifecycle policy, not iOS/Harmony surface implementations
or full showcase visual parity.

### Backdrop backend feasibility

Added shared CPU/GPU backend tests for captured-background blur, rounded clipping,
translucent white card tint and an unblurred two-pixel foreground stem. Both pass
on the Windows host (Intel Iris Xe, Vulkan), recorded in
target/ui-backdrop-backend-final.log. GPU composition uses texture-to-texture
copy and SrcOver; only the final assertion reads pixels back. Separate queue
submissions preserve Vello's reused scene upload buffers.

This is backend feasibility evidence only. No authoring property, Tailwind class,
production multipass renderer or Home/ToDo glass effect was added. Nested opacity,
sampling boundaries, saturation, allocation limits, cache/scroll invalidation
and Android device verification remain before page integration.

The backend tests now live in crates/lingxia-ui/src/backdrop_backend_tests.rs
and use gpu_pass::SceneRenderer, also used by the production Presenter. The
shared renderer submits each scene together with its staged resource uploads;
encode/submit timing remains available. Added CPU/GPU assertions for a glass card
inside a 50%-opacity root over a blue outer background: sampling stays within the
chosen root and group opacity applies once. These explicitly constructed roots
do not establish automatic display-list partitioning or authored filter support.
Both backend tests pass on Intel Iris Xe/Vulkan (target/ui-backdrop-passes.log).
The GPU-feature library regression passes 227 tests with the hardware test
explicitly ignored in that run; the hardware test was executed separately above.
Workspace all-targets check passes (ui-backdrop-passes-regression.log and
ui-backdrop-passes-workspace.log). This Presenter refactor has desktop GPU
execution coverage; the Android APK has not been rebuilt or reverified for it.

### Shared saturation backend

The reference ToDo panel requests blur(20px) saturate(180%). Added a shared CPU
premultiplied-sRGB transform and a reusable GPU saturation pipeline (factor0..16).
GPU writes distinct same-sized RGBA8 textures without changing alpha; per-pass
uniforms preserve independent factors when batching. CPU fixed-value tests and
desktop GPU parity pass across six factors and alpha0/1/17/128/255. The actual
Gaussian -> saturation GPU chain also passes without a CPU readback between its
rendering stages (target/ui-saturation-gpu-final.log).

This removes the backend saturation gap; it does not implement backdrop style
lowering, automatic sampling roots, texture budgets/cache invalidation or page
integration. Android device execution remains unverified for this new shader.
Workspace all-targets check and GPU all-targets clippy complete successfully
(target/ui-saturation-workspace.log, target/ui-saturation-clippy.log). Clippy
still reports existing warnings outside the new filter modules; this is not a
warning-free gate claim.

### Shared group compositor

Added composite::Plan with CPU/GPU backends. It selects the nearest opacity or
backdrop root from the group stack, merges unfinished overflow clips inner-to-
outer, filters a snapshot, then applies owner opacity once when the group closes.
Draw spans cannot overlap/replay. CPU pixels cover sharp foreground, transparent
roots, nested clipping and prior sibling content. Desktop GPU runs the same plans;
all pixels match CPU within3 byte values and paint callbacks run only once
(target/ui-compositor-final.log).

Preflight validates structure/geometry and bounds full-frame RGBA allocations,
including queued GPU work. These are execution backends, not a completed page
feature: DisplayList lowering, cropped/reused targets, filter scratch accounting,
CSS blur-edge semantics, cache/scroll invalidation, TSX/Tailwind integration and
Android device verification remain. Existing page rendering still uses its
single-scene Presenter path.
All237 GPU-feature library tests, including explicitly enabled hardware tests,
pass (target/ui-compositor-regression.log). Workspace all-targets check and final
GPU all-targets clippy pass; existing unrelated clippy warnings remain
(ui-compositor-workspace.log, ui-compositor-clippy-final.log). The first clippy
attempt rejected the intentionally reversed range literal in a validation test;
the fixture now constructs that invalid Range explicitly.

### Display-list composition lowering

DisplayList now lowers existing clip/opacity scopes into the shared Plan with
device-coordinate clipping, a single surface background and non-replayed paint
spans. CPU/GPU span execution reuses existing primitive encoders. GPU image and
bitmap-glyph bindings were extracted into ImageTextures, used by Presenter and
the multipass tests. Compositor pipelines/depth persist across frames while the
draw callback borrows only the current display.

Real View nodes containing text, translucent images, vectors, opacity groups and
rounded scrolling match single-scene pixels at1x/1.5x/2x and offsets0/12/30;
CPU tolerance2 and GPU tolerance3 byte values (ui-display-compositor-final.log).
All239 GPU-feature library tests pass including enabled hardware tests;
workspace all-targets and GPU all-targets clippy complete with existing warnings
(target/ui-display-compositor-{regression,workspace,clippy}.log).

No backdrop paint/style is emitted yet. Presenter continues its single-scene
path using the shared encoders/image bindings. Cropped/reused intermediate
textures, blur-edge semantics, filter scratch accounting and Android verification
remain before Home/ToDo filter integration. This turn has desktop execution
coverage; the installed Android APK has not been rebuilt for these refactors.

### Cropped mirrored filters and pooled GPU surfaces

The compositor now crops blur/saturation intermediates to the owner's visible
raster border box. Reflected image sampling fills Vello's complete padded source
region before Gaussian filtering; original fractional/rounded clipping remains
at output. vello_common0.3 is now a direct pinned dependency for its public padding
and tile geometry (already present transitively). Padded source geometry is bounded
before allocation; Vello atlas/scratch reservations still need device profiling.

GPU RGBA surfaces use leases and a bounded pool. Submitted passes can reuse dropped
leases in queue order; retained outputs/clones remain immutable. Clear discards
idle entries and prevents older leases from refilling the pool. Depth textures
cache up to4 sizes/4M pixels. Preflight now counts concurrent working storage,
consistent with pooling, rather than rejecting repeated sequential allocations.

243 full GPU-feature tests pass including hardware tests
(target/ui-backdrop-pool-region-final.log). Three mirror tests then pass after
adding a pattern assertion that distinguishes reflect from clamp/wrap
(ui-backdrop-mirror-pattern.log). Tests cover4px/20px blur, excluded exterior
colors, translucent alpha, no new RGBA/depth allocations after warmup, held old
frames and clear/late-return behavior. Initial mirror linking hit transient
LNK1104; no test process remained and retry passed.

Workspace all-targets check and GPU all-targets clippy pass with warnings
(target/ui-backdrop-pool-region-{workspace,clippy}.log).

Ordinary groups still use full-frame buffers. Authoring/schema, backdrop paint
emission, Presenter selection, fractional-edge browser comparison and Android
verification remain; Home/ToDo do not yet use the filter path.

### Shared display composition and Presenter entry

Compositor::render_display now owns the image/glyph preparation previously
implemented only in the GPU regression callback. Presenter::prepare_composited
renders through it and retains the pooled output through direct GPU presentation.
Suspend releases output leases and clears composition texture/glyph caches.
The ordinary Presenter entry still uses its cached single-scene path.

The real-display CPU/GPU regression passes through the new shared entry at
three scales and three retained scroll positions (target/ui-presenter-composite.log).
This executes the compositor on desktop GPU; native surface presentation and
Android execution of the new Presenter entry have not been verified. Authored
backdrop groups, automatic path selection and Home/ToDo filters remain pending.

Workspace all-targets check and GPU all-targets clippy pass with existing warnings
(target/ui-presenter-composite-{workspace,clippy}.log); rustfmt and diff checks pass.

### Authored backdrop filters and automatic composition

Tailwind's generated blur/saturation variables now lower into shared backdrop
styles (append-only fields95/96), preserving CSS order, variants and disabling.
Home's card/status pill use blur-xl/md; ToDo's panel uses20px blur and180%
saturation. Unsupported filter functions/order and excessive parameters fail.
CPU rasterization and Presenter select composition automatically for these paints.
Retained scopes preserve ancestor clips and combine the owner's backdrop/opacity;
transparent owners emit filters even without foreground paints.

247 Rust tests, including desktop GPU execution, pass
(target/ui-backdrop-authoring-verified.log);148 JS tests pass
(target/ui-backdrop-authoring-js-final.log). Actual authored filters compare CPU
and GPU at three scales and three scroll offsets, preserving sharp/saturated
foreground. The new readback comparison was corrected to skip GPU row padding.
The scroll-mask structure test now expects one ancestor clip instead of duplicate
rectangle/rounded clips; its fixed-edge pixel assertions still pass.
Typecheck, showcase release build, workspace check and GPU clippy pass (warnings
remain). Android release native build completed in8m30s; rebuilt APK installed
on Pixel3/Android12. GpuHomeParityProbe and GpuTodoWideProbe pass with automatic
composition (target/ui-backdrop-{home,todo}-device.log). Home covers remote image,
input/greeting, language/theme changes, rotation and navigation retention. ToDo
covers a wide logical viewport, long task wrapping, footer alignment and completed
dark state; its temporary task was removed and the existing dddd task preserved.
Home light/dark and ToDo light/dark screenshots were inspected
(target/ui-backdrop-{home,home-dark,todo-light,todo-dark}.png). Current app PID8463
has no LingXia.UI warning/error or AndroidRuntime error output
(target/ui-backdrop-device-errors.log). Device returned Home/light; density and
rotation restored. Full visual parity, fractional-edge browser comparison and
GPU scratch/latency profiling remain open; this verifies the authored filter path.

### ToDo empty-state visual alignment

The empty state now uses a shared monochrome paper/pencil vector instead of the
unfiltered color emoji. This is an intentional icon treatment within the existing
outline system; copy and responsive56/64px sizing remain. Added the missing empty
input separator, corrected the wordmark's desktop breakpoint to769px, and applied
the reference dark accent to completed checkmarks.

Typecheck and release page build pass. Pixel3 light/dark empty-state screenshots
were inspected (target/ui-todo-empty-note-{light,dark}.png). A temporary in-memory
fixture restored the existing task count after capture without storage writes
(target/ui-todo-empty-note-device.log); the existing dddd task remains. Initial
probe retries corrected startup timing and an invalid accessibility assertion
for a decorative vector. Current app PID8944 has no renderer warning/error output
(target/ui-todo-empty-note-errors.log). This does not close broader interaction
styling, typography or animation parity.

GpuTodoWideProbe also passes after the changes (target/ui-todo-note-wide-device.log),
covering long task wrapping, footer alignment and dark completed state. Its own
temporary task was removed; device returned Home/light with density restored.

### Shared placeholder color

Tailwind's generated ::placeholder color now lowers to placeholderColor(field97),
separate from body text. Native glyph painting uses it only for an empty editor's
hint; value text, whitespace values, caret and search adornments keep normal color.
It is non-inherited and defaults to the existing text color when omitted. Other
placeholder declarations still fail compilation. Home/Stream use the reference
gray400; ToDo uses its light/dark tertiary label colors.

249 Rust tests (including desktop GPU) and150 JS tests pass
(target/ui-placeholder-{regression,js}.log). Tests cover input/textarea/search,
value/caret isolation, non-inheritance, wire validation, theme/viewport cascade,
class order and inline overrides. Typecheck, release page build, workspace check,
GPU clippy and rustfmt pass with existing warnings. Android release build completed
in8m09s; rebuilt APK installed on Pixel3/Android12. GpuHomeParityProbe,
GpuStreamLayoutProbe and GpuTodoWideProbe pass
(target/ui-placeholder-{home,stream,todo}-device.log). Screenshots inspected:
ui-placeholder-home.png, ui-placeholder-stream-{light,dark}.png and
ui-placeholder-todo-{light,dark}.png under target/. Hints are subdued separately
from entered text; theme, input/clear/reentry and existing interaction checks pass.
Current PID9774 renderer/runtime error log is empty (ui-placeholder-device-errors.log).
Device restored Home/light; ToDo temporary task removed, existing dddd preserved,
and density/rotation restored. This closes placeholder color, not focus/hover,
transition or general pseudo-element support.

### Shared editor focus paint and Home ring

Tailwind editor focus rules now resolve paint deltas in the shared View runtime
and encode private field98. Native editor focus applies them before layout while
preserving base records for blur. Autofocus, scrolling out of visibility, disable
and failed-frame rollback use the same shared state. Supported properties are
color/background/opacity/border color/gradient/shadow/placeholder color; geometry,
typography, backdrop and non-editor focus remain unsupported. Outer ring width and
color compose with ordinary shadows; ring offsets and insets remain unsupported.
Home uses focus:ring-2 focus:ring-blue-500/30; ToDo uses the reference subtle
light/dark focus backgrounds.

252 Rust tests including desktop GPU pass (target/ui-focus-final-regression.log),
and152 JS tests pass (target/ui-focus-ring-js.log). Typecheck, release page build,
workspace check, GPU clippy and rustfmt check pass; clippy has warnings including
the focus validator's manual parity check. Android release native build completed
in8m50s and the rebuilt APK was installed on Pixel3/Android12.

GpuFocusStyleProbe passes (target/ui-focus-device.log): Home blue ring sample
counts0 ->734 ->0 across base/focus/blur; ToDo red-channel background samples
255 ->252 ->255 in light and31 ->38 ->31 in dark. Physical taps focus editors;
Tab blurs them. Focus screenshots ui-focus-home.png and ui-focus-todo-{light,dark}.png
under target/ were inspected. This probe edits no tasks.
GpuHomeParityProbe and GpuTodoWideProbe also pass (ui-focus-{home,todo}-device.log).
The latter removes its own temporary task and restores density/rotation; existing
dddd remains. Device returned Home/light. Current app PID11064 has no renderer or
runtime error output (ui-focus-device-errors.log). General control states, hover,
transitions and broader typography/visual parity remain open.

### ToDo task insets and filter shadows

Composed task checkboxes now explicitly set px-0, removing the native primitive's
extra12px horizontal padding. Icon-to-label gaps14/18/22px at phone/tablet/desktop
widths place labels50/54/58px from the panel edge, matching the reference CSS.
Selected filter chips use the reference light/dark two-layer shadow instead of
generic shadow-sm. Delete controls use the reference default55% opacity while
retaining their existing touch target. Hover/pressed transitions remain open.

Typecheck and release page build pass. Pixel3 GpuFocusStyleProbe and
GpuTodoWideProbe pass (target/ui-todo-inset-{focus,wide}-device.log), covering both
themes, focus/blur, wide footer alignment, long-task wrapping, completion and
temporary-task removal. Phone light/dark and wide dark screenshots were inspected
(target/ui-todo-inset-{light,dark,wide-dark}.png). Renderer/runtime error output is
empty (ui-todo-inset-errors.log). Existing dddd preserved; Home/light and device
density/rotation restored. No native renderer change was needed.

### ToDo footer content sizing

Filter buttons now use intrinsic widths and no shrink, with reference13px desktop
type, medium weight, tracking and horizontal padding. The footer wraps Clear
completed instead of squeezing Completed against its edge. Below360px,13px type,
8px padding and64px minimum widths keep all three filters on one row; larger phone
widths use the reference14px/20px/80px values. Controls retain44px touch height.

Typecheck/release build pass. GpuTodoWideProbe now asserts Completed has at least
90logical pixels, filters stay inside the panel, and Clear completed does not
overlap (target/ui-todo-footer-wide-device.log). GpuFocusStyleProbe now checks
filter alignment/containment and passes at320logical pixels in both themes
(ui-todo-footer-narrow-device.log). Wide dark and final narrow light screenshots
were inspected (ui-todo-footer-{wide-dark,narrow-light}.png). Earlier failures
identified an inaccessible container test target and genuine label squeezing;
the final checks use interactive-node bounds. Existing tasks are preserved and
temporary tasks removed; device density/rotation and Home/light restored.

Shared layout follow-up: render.rs currently assigns zero flex basis whenever
grow>0. This couples growth and basis, unlike CSS grow alone. Removing grow from
these intrinsic controls resolves their presentation, but does not close the
general Tailwind flex-basis semantic gap. Add an explicit shared basis contract
and audit current grow callers before claiming full flex semantics.


### Explicit flex basis

Field99 carries auto, logical lengths and percentages through TSX/Tailwind to
shared Taffy layout. Grow alone now keeps auto basis; flex-1 supplies 0%, while
existing showcase grow callers explicitly use basis-0 to retain their intended
fill behavior. GpuFlexBasisProbe passes row/column, content/zero basis, percentage
and parent resize checks (target/ui-flex-device.log). JS154 and Rust255 tests
pass, including desktop GPU tests in the serial Rust run
(ui-flex-final-serial.log). The parallel run hit one JavaScript execution-budget
timeout; the complete serial run passes. Workspace check, GPU clippy, page
typecheck and release build pass. Indefinite-parent percentage/intrinsic sizing
still needs browser comparison; this is not a full CSS flex parity claim.

### Home IME font distortion

Android now retains the SurfaceView's unoccluded extent while the IME is visible,
with parent clipping and View.env.keyboard handling usable content space. Width
and window configuration size changes invalidate that extent. This prevents the
old GPU buffer being stretched vertically during the asynchronous resize.

Pixel3 Android12 before/after recordings reproduce the distortion and verify
normal glyph proportions during show/hide/refocus after the fix. Evidence:
target/ui-home-font.mp4 and ui-home-font-final.mp4; transition sheets
ui-home-font-final-frames-04 through08. GpuHomeFontProbe preserves the input value
and reports CAPTURED, requiring visual inspection. ToDo wide and Stream layout
regressions pass (ui-ime-todo-wide.log, ui-ime-stream.log), preserving existing
ToDo tasks and restoring density/rotation. Other Android versions and split
window behavior are not device-verified in this pass.

Final Home parity/rotation/navigation regression passes (ui-ime-home-final.log);
renderer/runtime error log is empty (ui-ime-final-errors.log).


### Clipboard and Share control proportions

These pages now use the React reference's inherited16px/24px button typography
instead of the GPU migration's explicit14px/20px. Clipboard's three-row editor
uses78px (3?20px lines +16px vertical padding +2px border), replacing96px.
All changes remain TSX/Tailwind authoring.

Typecheck/release page build pass. Pixel3 GpuClipboardLayoutProbe passes at
normal density and320logical pixels in both themes, checking48px single-line
buttons,78px editor, aligned two-column actions and multiline input with IME.
Screenshots were inspected: target/ui-clipboard-sizing-light.png,
ui-clipboard-sizing-ime-dark.png, ui-share-sizing-dark.png and
ui-clipboard-sizing-image-narrow.png. Narrow sample-image text wraps to two lines
without clipping or overlapping its neighboring action. Device density restored.
Evidence: ui-clipboard-sizing-device.log and ui-clipboard-sizing-narrow.log.
This closes these control-size differences, not overall visual parity.

GpuApiProbe also passes text/typed/image clipboard roundtrips, clear/empty,
share-sheet dismissal and file-picker cancellation (ui-clipboard-sizing-actions.log).
Renderer/runtime error output is empty (ui-clipboard-sizing-errors.log).

### Native tab GPU presentation

The reproduced API failure was UI_TIMEOUT during View JavaScript preparation.
The shared class resolver now caches up to 256 class cascades, invalidating on
theme, viewport width and compiled-class changes. Inline and inherited values
remain per-node; nested cached paints are copied. The 100ms watchdog is unchanged.
All 155 JS tests pass, including cache mutation and environment invalidation;
page typecheck/release build and Android release assembly pass.

Android captures the outgoing GPU frame before dispatching a native tab event.
A transient root-sized snapshot combines it with host chrome, retaining original
pixel geometry when navbar visibility changes. The incoming GPU buffer is copied
after presentation and committed before removing the cover. Native navbar/tabbar
remain live above it. Navigation versions reject stale callbacks; no per-page
bitmap cache is retained.

Pixel 3 / Android 12, packaged app with dev server disconnected: the five-tab
probe passes 47 switches in each theme, including rapid taps, final target and
IME dismissal. Final light recording has 373 decoded frames and dark has 479,
with no black content frames. Transition sheets also show no exposed top black
strip. Evidence under target/: ui-tabs-composed-light-device.log,
ui-tabs-composed-light-video.log, ui-tabs-verified-dark-device.log,
ui-tabs-verified-dark-video.log and corresponding MP4s. App renderer/runtime
error output is empty (ui-tabs-composed-errors.log).

Home font focus/typing/blur/refocus was recorded and visually inspected after
this change (ui-tabs-font.mp4, ui-tabs-font-device.log); glyph proportions remain
stable during keyboard transitions. This verifies these tab scenarios on this
device, not a 60fps guarantee or animated push/pop transitions. An earlier dark
recording (ui-tabs-composed-dark.mp4) includes black frames during a separate
push/pop before the tab probe starts; it is not passing evidence for navigation.

### Navigation preparation and memory-pressure investigation

GPU push/pop now prepares both snapshots before sliding them with equal progress.
Visible-page disposal/parking preserves pixels until the cover commits; pending
retirements drain on host destruction. Navigation lifecycle/state probes passed
in light/dark, but ui-nav-prepared.mp4 and ui-nav-disposal.mp4 still contain black
frames, so neither establishes visual completion.

The reported freeze has recorded app and system ANRs. The app trace shows
filemap_fault / heavy major faults, 64% kswapd CPU and 40% aggregate I/O wait;
this establishes memory pressure, not a single proven allocation root cause.
Evidence: target/ui-freeze-app-anr.log, ui-freeze-events.log, ui-freeze-lastanr.log.
The temporary PixelCopy bitmap is now recycled immediately after software
composition, stale captures are recycled, and retired ImageViews clear drawable
references. The finished but lingering GpuTodoEmptyVisual test process was stopped.

Two consecutive 47-switch runs pass after the buffer-reclamation change:
ui-memory-tabs-first.log and ui-memory-tabs-second.log. End-of-run PSS is
434208 / 445676 KiB, graphics 296815 / 300951 KiB; ui-memory-errors.log is empty.
This is a short repeatability check, not proof of leak freedom or bounded total
GPU memory. The build subsequently also recycles captures superseded by runtime
retirement. Keep the overall visual/memory audit open.

Final installed build also passes GpuNavigationLayoutProbe in both themes
(ui-nav-memory-final-device.log), with no renderer errors or new ANR event;
end-of-run PSS is 378572 KiB (ui-nav-memory-during.log). However the recording
still contains 12 black content frames out of 415 (ui-nav-memory-final-video.log).
Cold push and programmatic switchTab from a non-tab page remain visual gaps.

### Chrome-before-navigation handoff

Tracing the remaining gaps showed runtime tabbar visibility updates resizing
the outgoing surface before host navigation could capture it. The SDK now
covers that geometry change first; navigation owns the cover if it follows,
while same-page show/hide waits for its own copied/committed replacement frame.
An already-resized outgoing surface gets one presentation/copy retry before
falling back. Temporary trace logs were removed from the final build.

Pixel 3 final installed release: GpuNavigationLayoutProbe passes in light/dark;
ui-nav-chrome.mp4 contains 453 decoded frames with zero black content frames.
Light/dark slide sheets were inspected, including cold push and the return to
Home. GpuTabbarLayoutProbe also passes label/badge changes and same-page
show/hide, confirming its cover is removed and controls remain usable.
Evidence: target/ui-nav-chrome-device.log, ui-nav-chrome-video.log,
ui-nav-tabbar-regression.log. Renderer error output is empty; the ANR event log
contains only the previously investigated events, with no new ANR. End-of-run
PSS was 421645 KiB. This supersedes the visual failures above for these tested
navigation cases; it does not close overall showcase parity or memory auditing.

Final five-tab regression passes another 47 switches, including rapid final
target and IME dismissal. Its 246-frame recording has no black content frames
(ui-nav-tabs-final-device.log, ui-nav-tabs-final-video.log); final renderer error
output is empty (ui-nav-final-errors.log). Device is left on Home/light.

### Shared text baseline alignment

Tailwind items-baseline / self-baseline now map to shared Rust layout. Text
baselines include padding, borders and vertically centered control labels;
fixed dimensions reflow text at final content width. Channel uses this for
its mixed-size price/change row. Portrait light/dark and landscape screenshots
were inspected on Pixel 3 (target/ui-baseline-channel-*.png).

Validation: 267 Rust unit/integration tests, 156 JS tests, lxapp typecheck,
workspace all-targets check, Android release build and GpuChannelProbe pass.
Device evidence: target/ui-baseline-channel-final.log. The first two probe runs
failed disconnect while growing history moved the button; a manual touch worked.
The probe now waits for bounded history height before a physical disconnect tap,
preserving the renderer's rule that movement cancels queued clicks. Final app
error output is empty (ui-baseline-final-errors.log); ui-baseline-final-anr.log
contains only the earlier investigated ANRs. No iOS/Harmony device validation.
Channel horizontal scrolling at narrow widths and long-run memory auditing
remain open.

Horizontal-scroll groundwork: retained composition now supports orthogonal
ancestor translations, including paints, glyphs, clips and control bounds.
Shared reveal stages offsets until the full ancestor chain succeeds. A core
test exercises a horizontal region nested inside a vertical region at 1x/2x,
checks clipping and CPU pixels, reveals an offscreen control, and restores zero
offsets without geometry drift. Existing authoring still creates vertical
regions; this does not yet resolve Channel's narrow-width overflow. Android
gesture/accessibility integration and physical verification are outstanding.
Core regression: 244 unit and 24 integration tests pass
(target/ui-scroll-axes-rust.log). The initial targeted invocation failed at
Windows linking with LNK1104; the subsequent complete test invocation exited
successfully after confirming no prior test/compiler process remained.

Horizontal authoring and Android adapter are now connected in source: field100
selects one scroll axis, `overflow-x-auto` lowers onto scroll containers, and
Channel wraps its intrinsic symbol row with that container. Drag/fling routing
selects the matching ancestor axis; accessibility uses width for horizontal
forward/backward and stages reveal corrections transactionally. Two prior reveal
test fixtures placed targets beyond their inner region's maximum recoverable
extent; they now use reachable geometry, with separate unrecoverable/rollback
coverage. Do not report success for targets still fully clipped after clamping.

269 Rust tests, 157 JS tests and 165 Android UI tests pass. Workspace all-targets
check, lxapp typecheck/release build and 102-probe compilation pass. Logs:
target/ui-scroll-authoring-rust-final.log, ui-scroll-authoring-js.log,
ui-scroll-android-all.log, ui-scroll-authoring-workspace.log. The initial Rust
invocation hit LNK1104; the complete retry exited successfully. Native release
compilation is in progress (ui-scroll-native.log); installed-device acceptance
for this change is still pending. GpuChannelScrollProbe is the new 320dp
physical/touch/accessibility acceptance probe, followed by the ordinary Channel
and tab/navigation regressions. Overall visual parity and memory audit stay open.

Installed Pixel 3 acceptance now passes at 320dp in light/dark: physical
horizontal swipe, trailing TSLA tap, accessible backward/forward, and vertical
swipe on the horizontal strip reaching the outer page. Both screenshots were
inspected (target/ui-scroll-channel-light.png, ui-scroll-channel-dark.png);
ui-scroll-channel-device.log records the passing run. Normal-density Channel
connection/subscription, rotation, disconnect idle-frame stop, reconnect and
teardown regression also passes (ui-scroll-channel-regression.log). Density
was restored to physical440 with no override.

Early probe runs failed before acceptance: its reflection constructor used the
concrete class instead of IUiAutomationConnection; the origin assertion compared
a button whose preceding selected-state border changed width; the history-height
wait measured clipped accessibility bounds on the narrow viewport. The probe now
uses the interface constructor, verifies the leading edge against the viewport,
and waits for actual outer scrollability before testing vertical routing. These
were probe corrections; keep failed logs distinct from passing device evidence.

The installed build passes 47 tab switches, rapid final-target selection and
switching after IME dismissal (ui-scroll-tabs-regression.log). App-process error
output is empty (ui-scroll-app-errors.log); ui-scroll-final-anr.log contains only
the two previously investigated ANRs. This run did not record video, so it adds
functional regression evidence, not a new black-frame or frame-rate measurement.

### ToDo narrow footer acceptance

The reference uses a separate mobile footer row rather than horizontal scrolling.
Keep the GPU footer's existing responsive sizes. GpuTodoParityProbe now checks
all three filters remain within the inset footer, retain full widths and 44dp
touch heights, and share one row with separate hit targets. Pixel 3 at320dp passes
these checks in light/dark together with physical Enter creation, checkbox
label/keyboard toggles, filtering, clearing, rotation and empty-state restoration.
Screenshots ui-todo-narrow-light.png and ui-todo-narrow-dark.png were inspected;
passing evidence is target/ui-todo-narrow-parity.log. The initial probe requested
non-accessible layout containers and failed; it now measures exposed controls
against the scroll viewport. Failed-run screenshot pulls are not acceptance
evidence. Owned test tasks were removed and physical440 density restored.
Default-density rerun also passes (ui-todo-default-parity.log); app-process
renderer/runtime error output is empty (ui-todo-narrow-app-errors.log). This
closes the checked narrow-footer layout concern, not all ToDo visual behavior
or the broader showcase parity audit.

### Animation timing core (not enabled in showcase)

Added shared animation timing with bounded duration/delay/iterations and
per-node/aggregate definition budgets. Prepared instances commit only on the
matching page-owned presentation ticket; failed/superseded candidates retain
committed identities. First presentation starts time, visibility pauses active
time, and reset/removal/definition changes have separate lifetime semantics.
Six deterministic tests cover these boundaries, large clock jumps, invalid
inputs and foreign-page callbacks. The first test run used exact float equality
for0.7; its assertion now uses a small numeric tolerance.

251 unit plus24 integration tests pass (target/ui-animation-timeline-all.log).
This module is not connected to ViewSession or native presentation yet; it does
not add visible cursor blinking or chart entrance effects. Keyframe easing,
transformed paint/hit geometry, Tailwind animation lowering, platform wakeups,
virtual-list lifetime integration and real-device animation acceptance remain.
Follow docs/internal/ui-animation.md before extending this path.

Animation numeric core now includes bounded linear/step-end/cubic-Bezier easing,
numeric tracks with explicit increasing keyframes, next-change hints that skip
constant spans, and ordered scale/translate matrices around an explicit origin.
Five new tests cover the reference900ms blink boundaries, analytic Bezier
inversion/overshoot, the chart's center-origin/order-sensitive endpoints,
constant spans and invalid inputs. This does not yet animate rendered pixels.

Initial test linking hit LNK1104. The subsequent complete run passed the new
tests but exited with STATUS_ACCESS_VIOLATION near the existing subtree-opacity
test; that test passes in isolation (ui-animation-interpolation-opacity.log).
Preserve ui-animation-interpolation-all.log as failure evidence rather than
calling it a passing full gate. Workspace all-targets check passes
(ui-animation-interpolation-workspace.log). Native presentation integration and
device animation acceptance remain open.
The complete repeat exits successfully with256 unit and24 integration tests
passing (ui-animation-interpolation-all-repeat.log). The earlier native crash
was not reproduced by the targeted or complete repeat; its cause is unresolved.

Freeze follow-up: captured freeze-current-logcat.log and freeze-current-input.log
before restarting the app. Device top showed heavy reclaim/I/O wait; app meminfo
was about489MiB PSS including287MiB graphics. No ANR since boot was reported;
this does not prove absence of a hang. Native stack capture was denied by Android.
Restart recovered interaction; freeze-recovery-tabs.log passes47 switches and
IME dismissal, with no matching render/fatal errors in the final captured log.
The freeze root cause and sustained memory behavior remain unproven.

Sparse renderers now initialize their paired Vello renderer/resources on first
draw. A composition-only page therefore avoids allocating the unused single-scene
renderer. First-use initialization remains included in encode timing. Seven GPU
pixel/retained-pool tests pass (ui-lazy-renderer-gpu-repeat.log); the first attempt
failed linking with LNK1104 (ui-lazy-renderer-gpu-tests.log). Workspace all-targets
check and259 unit plus24 integration tests pass (ui-lazy-renderer-workspace.log,
ui-lazy-renderer-tests.log). Android release build is in progress in
ui-lazy-renderer-native.log; this optimization has not yet been installed or
measured on device. Do not treat prior device passes as validation of this change.

Animation effects now use validated typed opacity/transform tracks, expose active
prepared visuals and schedule against actually presented values. Three additional
tests cover blink deadlines, late presentation/final cleanup and transactional
sampling failures. The259 unit tests above include these. Rendering integration,
Tailwind animation lowering and actual Stream cursor/chart animation remain open.

Controlled pre-install baseline for lazy renderer: installed version2007/0.2.7,
force-stop then launch, wait5s, sample Home, run GpuTabTransitionProbe, sample
again. ui-lazy-cold-baseline-home.log reports215314KiB PSS/92147KiB graphics;
ui-lazy-cold-baseline-alltabs.log reports450536KiB PSS/278043KiB graphics.
ui-lazy-cold-baseline-tabs.log passes47 switches. A prior warm-process run also
passed (ui-lazy-baseline-tabs.log); its differing swap/native residency is not a
controlled comparison to the cold samples. New native build remains running;
no post-change device memory or visual result is claimed yet.

Lazy-renderer Android acceptance completed: native release succeeded in10m34s,
APK assembly passed and version2008/0.2.8 installed on Pixel3. Built and packaged
liblingxia.so SHA-256 matches306C09A761886656B5AAEFAB01D8C92E597B5A3AE7E5CB5AE571E3945D93B59C.
Cold repeat reports232015KiB PSS/113087KiB graphics on Home and472155KiB PSS/
277651KiB graphics after the same tab probe (ui-lazy-cold-new-{home,alltabs}.log).
Against the previous215314/92147 and450536/278043 samples, this does not establish
a material memory improvement. Startup readiness/residency varies; no leak or
freeze fix is claimed. Per-page RenderContext/device and retained renderer
resources remain the next memory investigation, with switching latency measured
before choosing eviction or sharing.

ui-lazy-cold-new-tabs.log passes47 tab switches; ui-lazy-home-device.log and
ui-lazy-todo-device.log pass greeting/language/theme/rotation/retention and task
entry/toggle/filter/clear workflows. Inspected ui-lazy-home.png,
ui-lazy-home-dark.png and ui-lazy-todo-{light,dark}.png: content renders through
both ordinary and composition paths. Home portrait is intentionally scrolled
by ACTION_SHOW_ON_SCREEN; it is not a full-page capture. The app-PID log has no
matching render/fatal/OOM errors; ui-lazy-device-anr.log reports no ANR since boot.
No transition video was captured, so this run does not establish zero black
frames or frame pacing. Test tasks were removed and Home/light restored.

Shared GPU device domain implementation: gpu::Context owns a cloneable device
context; Presenter::with_context shares compatible devices/queues while keeping
page caches separate. Android attaches under the registry lock using an existing
presenter's domain; removing the last presenter releases the domain. Resume
retains the original-device check. Lock order and lifecycle are documented in
ui-renderer.md; gpuDevices is included in native frame metrics for acceptance.
Eight desktop GPU tests pass (ui-shared-context-gpu.log), including shared-owner
lifetime and interleaved independent compositor outputs after one page clears.
Workspace all-targets check passes (ui-shared-context-workspace.log). Native
Android release build is running (ui-shared-context-native.log); not installed
or device-verified yet, and no memory/latency improvement is claimed.

Saved installed version2008 APK/native library under target/ui-shared-context-
baseline-2008.{apk,so}. New baseline waits for GpuHomeImageProbe to confirm actual
background pixels before sampling, then runs the same47-switch probe. Home:
231547KiB PSS/113371KiB graphics; after tabs:470457KiB PSS/285347KiB graphics.
Evidence: ui-shared-baseline-{ready,home,tabs,alltabs}.log. Use this readiness
condition on the new build too; do not compare incomplete startup frames.

Shared-context physical acceptance: release completed in10m18s; APK version2009/
0.2.9 installed. Built/packaged native SHA-256 matches
C034E1F52137C78AD20AA2156EFF718857195AF9D677960AC99CBBA045031576.
ui-shared-new-app.log reports gpuDevices=1 throughout the five-tab run. Both
ui-shared-new-tabs.log and ui-shared-video-tabs.log pass the47-switch probe.
Home readiness-confirmed PSS/graphics:236631/113143KiB; after tabs:443600/287011KiB.
Baseline was231547/113371 and470457/285347KiB respectively. Total PSS was lower
after tabs in this sample, but graphics did not fall and swap/residency differs;
this single pair does not prove a stable memory reduction or fix the freeze.

ui-shared-timings.json compares25/29 matched first-frame samples: click to logged
submitted frame p50=200/197ms, p95=804/671ms. This excludes cover removal and
overtaken rapid clicks; it is not user-visible latency or a controlled performance
claim. Further work must reduce retained graphics and first-frame costs without
changing page state or causing blank transitions.

Recording wrapper initially rejected a null Windows Process.ExitCode even though
screenrecord had terminated and produced a valid file. Corrected that check and
independently pulled/decoded the existing recording; did not rerun it or call the
wrapper successful. ui-shared-tabs.mp4 decodes373 frames; no central-content frame
meets the near-black threshold in ui-shared-tabs.analysis.json. Inspected contact
sheet and darkest frame. This only rejects near-full black content, not partial
flashes, frame pacing or delayed chrome/content agreement; a transition can show
the outgoing page under the newly selected tab while its replacement prepares.

ui-shared-home-device.log and ui-shared-todo-device.log pass full theme/rotation/
state-retention workflows. Inspected dark Home and ToDo captures. Final app-PID
log has no matching render/fatal/OOM errors; no ANR since boot reported. Tasks
cleaned and Home/light restored. No builds/probes remain running from this stage.
Stream animations and the rest of the incomplete parity checklist remain open.

Shared renderer implementation: each gpu::Context now retains one lazily created
SceneRenderer/resources pair per compatible device. Presenter ordinary draws and
its compositor clone that same engine; page image/glyph caches, composition pools
and retained outputs stay separate. A renderer mutex covers all Vello resource
mutation through queue submission, preserving upload ordering across page users.
The domain releases engines with its last owner; frame metrics now also expose
gpuRenderers. This avoids keeping separate pipelines/scratch for every cached tab
without rebuilding an engine on each tab entry.

Nine desktop GPU tests pass (ui-shared-renderer-gpu.log). Added interleaved-page
coverage compares shared and isolated GPU output for different pictures, text,
clips, opacity and backdrop filters, retaining old outputs across repeated page
clear/destruction/recreation and delaying readback until all draws are submitted.
Workspace all-targets check passes (ui-shared-renderer-workspace.log). Native
release build is running (ui-shared-renderer-native.log), not installed yet.
Current installed version remains2009; its APK/library were preserved as
ui-shared-renderer-baseline-2009.{apk,so}. Device memory, latency and transition
acceptance for shared renderer resources remain pending; earlier shared-device
results do not validate this new resource-sharing layer.

Shared renderer physical acceptance completed: native release13m06s, APK2010/
0.2.10 installed, matching built/packaged native SHA-256
9A4D4C96ACD18237DF2AA01283DC1907D719E899DA76510DD480BC0228BBB07A.
ui-renderer-timings.json confirms five surfaces with gpuDevices=1 and gpuRenderers=1.
The readiness-confirmed baseline2009 samples are in ui-renderer-baseline-*:
Home232098KiB PSS/113631KiB graphics; after tabs519193/287731; later idle427717/287731.
New samples: Home229672/113043; after tabs444859/264163; after30s idle393610/264163.
The observed graphics reduction is23568KiB (~23MiB); total PSS is sensitive to
native/Java reclamation and sampling time. This is one before/after run, not a
long-run memory guarantee or a proven fix for the original freeze.

Matched click-to-submitted-frame samples: baseline29, new31; p50=213/183ms,
p95=654/325ms. Worker p95=581/230ms. These exclude cover removal and superseded
rapid clicks; do not interpret them as input-to-visible latency or FPS.
ui-renderer-new-tabs.log and ui-renderer-video-tabs.log both pass47-switch probes.
Video ui-renderer-tabs.mp4 decodes378 frames, with no near-full black content
frames by the explicit threshold in its analysis JSON. Inspected contact sheet
and darkest frame: transient new API chrome over outgoing Home/ToDo remains
visible during switches. Chrome/content presentation synchronization is an open
visual defect, despite no detected black frame. Variable recording cadence does
not establish rendering frame pacing or absence of partial flashes.

ui-renderer-home-device.log and ui-renderer-todo-device.log pass theme, rotation,
input/state retention and task workflows. Inspected dark Home/ToDo captures.
Final app-PID log has no matching render/fatal/OOM errors; no ANR since boot.
Post-parity memory captured separately (different workload, not benchmark pair).
Test tasks removed, Home/light restored; no build/probe from this stage remains.
Stream animation and remaining parity checks remain incomplete.

Navbar/content synchronization fix: transition covers now sit above the live
navbar, while live tabs/capsule remain interactive above the cover. Captured
navbar pixels therefore change with the captured page, rather than exposing the
incoming title/background over outgoing content. Native chrome ownership and
runtime state updates are unchanged. Contract updated in ui-renderer.md.
Android SDK/APK build passes; version2011/0.2.11 installed with unchanged2010
native library. No Rust source changed for this fix.

Light-theme video comparison uses the specific blue-API-header vs Home-photo/
pink-ToDo-title signature, manually inspected in the before montage. Prior
ui-renderer-tabs.mp4:38 mismatching frames among378 decoded; new
ui-chrome-sync-tabs.mp4:0 among356. The classifier is showcase/light-specific,
not a universal chrome validator. New light and dark videos (356/489 decoded
frames) have no near-full black content by the documented threshold; both
contact sheets were inspected. ui-chrome-sync-{before,after}-analysis.log and
video .navbar.json/.analysis.json preserve the measurements and limitations.

Both47-switch probes pass, including rapid retargeting and IME dismissal.
ui-chrome-sync-navigation.log passes push/back, retained local/Logic/module
state, redirect, popup close/reentry and switchTab in light/dark. Final app log
has no matching render/fatal/OOM errors; no ANR since boot reported. Home/light
restored and verified through accessibility text. This fixes the demonstrated
navbar/content mismatch, not all transition pacing, original freeze causes,
Stream animations or remaining React parity requirements.

Animation preparation now has a ViewSession-owned presentation seam. Candidate
visuals are built transactionally with the display frame; acknowledgement checks
the current generation and surface epoch before starting inserted effects. Tests
cover failed builds, invalid timestamps, stale/replaced surfaces, page parking,
reset and ordinary frames superseding an animation candidate. Animation-only
preparation has zero View/layout/text work in the unchanged-layout fixture.
Visible-window timing separately preserves offscreen lifetimes without starting
unseen effects or restarting previously shown charts.

ui-animation-session-tests.log:21 focused tests pass. Workspace/all-target check
passes. Initial parallel regression:265 pass, one existing editor-budget test
times out; its isolated retry passes. Serial full regression passes266 unit and
24 integration tests (ui-animation-session-regression-serial.log). Original
failure log retained; this does not prove the parallel timeout cause resolved.
No APK changed: native presentation callbacks, transformed display geometry,
compiler lowering and virtual-list definition collection are not wired to this
seam yet. Stream visual animation parity remains incomplete.

Shared paint encoding now supports absolute positive axis scale/translation,
including glyph transforms and transform inheritance across composition spans.
Clip geometry uses the same matrix; uniform backdrop scale also scales blur.
Singular/reflected/rotated transforms and nonuniform backdrop scale fail rather
than silently render differently. This is an internal paint primitive: authored
node transforms, presented hit geometry and Stream animation classes remain off.

ui-animation-paint-cpu.log compares flat/composited transformed text, images,
vectors, rounded clips and opacity at three scales/scroll offsets with uniform
and nonuniform scale. New primitive tests check expected transformed pixels,
sibling transform reset, backdrop clip/blur and rejection paths. The expanded
GPU equivalence fixture passes27 combinations; all9 real desktop GPU tests pass
(ui-animation-paint-gpu-regression.log). Workspace/all-target check passes with
an expected unused-Transform warning until node lowering emits the primitive.
No Android APK or physical animation acceptance changed in this stage.
Full serial regression passes270 unit and24 integration tests in
ui-animation-paint-regression.log.

Retained subtree animation lowering is now connected to ViewSession's animated
preparation path. Center-origin transforms affect paint, bounds, content/control
clips and accessibility snapshots while retaining layout footprints. Culling can
recover formerly offscreen paints. Ancestor world clips remain exact; descendants
use inverse-mapped clips for culling. Plot selection inverse-maps points into the
original bar/pie geometry. Opacity overrides base alpha as an isolated group.
Transformed editors, scrolling containers and sliders explicitly reject; ordinary
scrolling ancestors remain supported. Compiler/platform scheduling still absent.

ui-animation-geometry-regression.log passes274 unit and24 integration tests.
Fixtures cover offscreen-to-visible translation, sibling stability, overlapped
opacity, scrolling clip preservation, thousands of bar/pie hit samples and rejected
unsupported interactive transforms. The earlier GPU pixel fixture now animates an
actual subtree under a static scroll clip; all9 GPU tests pass in
ui-animation-geometry-gpu.log. Workspace/all-target check passes. No new APK or
device animation recording; Tailwind lowering, virtual-list lifetime collection
and native presentation/deadline wiring remain necessary for Stream parity.

Build-time animation lowerer and Rust payload decoder now share blink/chart-in
fixtures. Tests compile the actual React showcase token CSS through official
Tailwind, lower its emitted shorthand/keyframes, and compare exact tracks/timing.
Rust decodes the same fixture and verifies blink edges and ordered center-origin
chart transforms. Source-order keyframe merging, identity endpoints, translation
units and unsupported/oversized payload rejection are covered. This lowerer is
still separate from production utility/wire emission, which continues to reject
animation classes; it is not yet an enabled authoring feature.

ui-animation-compiler-tests-final.log:5 focused tests pass. Full JS package suite
passes162 tests (ui-animation-js-regression.log);30 Rust animation tests pass
(ui-animation-wire-tests-retry.log). First Rust link attempt failed LNK1104;
terminal retry passed, original log retained. Workspace/all-target check passes.
No new APK. Remaining integration includes utility cascade/reset, appended View
wire field, virtual-list semantic identities, native visibility/presentation and
deadline wakeups, followed by physical Stream recording in both themes.

Tailwind rule lowering now calls the animation lowerer through an internal staged
option; production page builds keep it disabled and runtime explicitly rejects
animation styles. Real-build tests assert failed animation builds leave no output
module. Cascade tests cover animate-none, class-order independence, theme/media
variants, inline reset and nested-track mutation isolation. Rule configuration and
resolution now copy nested style values. Animation names survive lowering/native
decoding and definition equality, so switching names restarts identical tracks.

Full JS suite passes167 tests before the final animation-name addition; focused
final animation suite passes10 tests. Full Rust serial regression before that
addition passes276 unit and24 integration tests; final31 animation tests pass.
Logs: ui-animation-cascade-{js-final,rust-regression,identity-js,identity-tests}.
Workspace/all-target check recorded in ui-animation-cascade-workspace.log.
No new APK. View wire emission/collection, virtual-list lifetime retention and
native scheduling/presentation remain unfinished; authoring gates stay enabled.

Animation View transport now uses append-only field101. Runtime class payloads
serialize to JSON; null reset omits the field. Native decoding revalidates the
definition and page budget. Ordinary renderer/session paths refuse animation
records; animated preparation requires matching named definitions and strips
metadata before geometry reuse. The end-to-end session test uses the JS View
encoder and verifies initial/follow-up pixels, transform bounds and zero View or
layout passes on an animation-only frame. Collection is still supplied by the
caller; virtual-list lifetimes and native presentation scheduling remain open.

ui-animation-record-rust-regression.log passes280 unit and24 integration tests;
ui-animation-record-js-regression.log passes169 JS tests. Workspace/all-target
check passes in ui-animation-record-workspace.log. No new APK or device animation
acceptance is implied by these shared-core tests.

The JS transport now counts UTF-8 bytes, matching native32KiB definition and
256KiB page limits. Exact-limit and one-byte-over tests cover ASCII, two-byte,
three-byte and supplementary characters; eight maximum-size definitions fit and
nine reject. The full JS suite passes170 tests in ui-animation-utf8-js.log;
280 Rust unit and24 integration tests pass in ui-animation-utf8-rust.log.
Production animation authoring remains gated pending the complete native path.

Virtual-list wrapper keys now match their JS semantic row scope rather than the
numeric row index. Native descendant identities therefore survive insertion and
reordering. A shared-session regression presents a half-completed entrance after
insertion, evicts the row, returns after completion without restarting, and checks
that a replacement key gets a new identity. It still supplies the lifetime set
explicitly; automatic definition/lifetime collection remains unfinished.

The distant-scroll regression exposed zero-width leading/trailing layout spacers
being excluded from scroll overflow. Large offset requests repeatedly clamped to
the realized rows and exhausted the eight-pass window reconciliation limit. The
spacers now stretch across the content width; the test checks estimated full-list
extent and a2000dp jump as well as returning to the first window. Initial failure
and trace logs are retained as ui-animation-row-{identity,trace}.log.
Full regression passes281 Rust unit and24 integration tests in
ui-animation-row-rust-verified.log, and170 JS tests in ui-animation-row-js.log.
Workspace/all-target check passes in ui-animation-row-workspace.log.
No APK or physical-device acceptance yet; this reproducible window error does not
establish the cause of the user's intermittent freeze.

Android2012 (0.2.12) installs the row identity/spacer fixes. Native library SHA256:
398998CFEDB84AD2FDBFB4FC450A211FD76450F807941B4CFA2A75BF5D667B47.
Native release build passes in8m29s (ui-list-identity-native.log); APK assembly and
installation pass (ui-list-identity-{apk,install}.log). The preceding2011 APK and
library are preserved in target/ui-list-baseline-2011.{apk,so}.

The new real-input GpuStreamHistoryProbe exposed another mismatch on2011: after
eight long requests the first message was still visible instead of following new
content (ui-list-history-baseline-focused.log, history-baseline-long.png). React's
Stream effect scrolls on messages/chat.data changes. The GPU TSX now changes its
tail-anchor sequence on stream chunks and message-count updates; unchanged
content retains the user's scroll position. Release View build and both TypeScript
checks pass (ui-list-tail-{lxapp,typecheck}.log).

Pixel3/Android12 passes16 long request/reply cycles across light/dark, twice
recovering the first and last user message by scrolling in each theme, then clear
and return Home Light (ui-list-history-2012.log). Screenshots gpu-history-{first,last}
-{light,dark}.png were captured; endpoint assertions require the corresponding
message to intersect the viewport, not the entire long bubble to fit. No matching
UI_LIST/UI update failed/panic/OOM error in ui-list-history-2012-final-app.log;
Android reports no ANR since boot. Existing network update checks fail to connect
to their server, separately from rendering. Earlier probe attempts are retained:
they mishandled unfocused input/Back and are not app failure evidence.
This does not prove animation support, frame-rate smoothness or the intermittent
freeze's root cause; native animation scheduling/collection remains open.
The2012 five-tab47-switch regression also passes (ui-list-tabs-2012.log), including
rapid retargeting and switching after IME dismissal. PID14282 remains unchanged;
the final app log has no matching renderer failure and lastanr remains empty.

The shared list contract now offers optional `itemKeys`, a complete ordered row
inventory transported through append-only field102. It does not invoke offscreen
row factories. JS snapshots and validates keys, resolves unkeyed row roots in the
supplied semantic scope, and reindexes retained row-local state by key. Offscreen
removal releases state; eviction/reorder retains it. Failed candidates preserve
the previous locals/callbacks, and changed keys cannot reuse stale anchor seeds.
Rust independently validates inventory type/count/uniqueness, normalized key size,
page-wide256KiB UTF-8 budget and each realized wrapper's key/index correspondence.
Generated TS declarations and wire constants include the new optional property.

Full JS suite passes174 tests (ui-list-keys-js-final.log); a final sparse-array
validation change passes all13 list tests (ui-list-keys-sparse-final.log). Rust
serial regression passes283 unit and24 integration tests
(ui-list-keys-rust-regression.log), including actual View-to-native inventory
transport through the insertion/eviction animation session fixture. The initial
handcrafted wire tests omitted required list geometry; corrected fixtures pass,
with the original failure retained in ui-list-keys-rust-focused.log.
Workspace/all-target check passes (ui-list-keys-workspace.log).
No new APK: installed2012 does not support field102, and the showcase does not emit
it yet. Animation definition collection must consume this inventory before the
production animation gate can be removed; native scheduling also remains open.

`ViewSession::prepare_view_animated` now collects authored field101 definitions
from the final measured window and reconciles virtual lifetimes using field102.
Its inventory records nested row owners: outer/inner eviction preserves effects,
while key removal, removal of a realized nested list or removal/reset of a
realized node's animation drops them. Animated rows require complete inventories
on all owning lists. Intermediate measurement windows never reconcile lifetimes.
The collected inventory, timeline, visible set and frame ticket commit only after
pixel build and View commit succeed. Actual presentation still starts effects.

Shared tests verify real JS View transport, zero View/layout passes on an
animation-only frame, failed-pixel rollback preserving the preceding ack, insertion
without restart, eviction/return after completion, offscreen removal/reinsertion,
unseen overscan not consuming an entrance, nested eviction/removal, and inward
transform visibility with actual raster pixels. Visibility uses the union of
base/transformed viewport intersections, preserving wakeups for outward motion
that may return. `view_animation_wakeup` derives its visible set from collection.

Full Rust regression passes288 unit and24 integration tests in
ui-animation-collection-rust.log. Workspace/all-target check passes in
ui-animation-collection-workspace.log. No APK change: Android still uses its
ordinary frame path, and production animation authoring remains gated until
presentation acknowledgements, visibility and wakeup scheduling are integrated.
Final animation-focused regression passes after boxing the candidate frame
(ui-animation-collection-final-tests.log); clippy completes with existing warnings
and no new collection-module warning (ui-animation-collection-clippy-final.log).

Android CPU/GPU preparation now shares the collected animation path. JNI receives
the UI enqueue timestamp and surface epoch; only successful presentation sends
the matching generation acknowledgement. Hidden/destroyed surfaces park timelines
and cancel callbacks. A single cancellable scheduler subtracts acknowledgement
latency from wakeup delays and also owns failed-presentation retries.

Verification: 288 Rust unit and 24 integration tests pass in
ui-animation-native-clock-rust.log; Android arm64 GPU cargo check passes in
ui-animation-android-check-small.log. All 168 Android UI unit tests pass after the
final retry change (ui-animation-sdk-retry-final.log). The new workspace check
is incomplete: OpenSSL configuration cannot find Perl
(ui-animation-native-workspace.log). The first Android check ran out of disk;
the successful check used a separate C-drive target with debug info disabled.
No matching APK has been built or installed; production animation gates remain
closed, and installed version2012 cannot validate the new JNI contract.

Follow-up freeze inspection: the installed app remains PID14282, Android reports
no ANR since boot, and another 47-tab transition probe passes without restarting
the app (gpu-freeze-current-tabs.log). Its captured process log contains no
UI update failed, UI_LIST, panic or out-of-memory match
(gpu-freeze-current-app.log). This does not establish the cause or resolution of
the intermittent freeze, nor prove every transition visually seamless.

Stream now supplies the complete semantic message-key inventory to its virtual
list, including the pending response, and uses that same key for each row root.
This prepares offscreen chart lifetime tracking without enabling animation.
The first actual release build exposed a compiler omission: TS/runtime accepted
itemKeys but the TSX audit rejected it. The audit now admits it on lists only.
Typecheck, 13 list-runtime tests, 14 compiler tests and the full lxapp release
build pass (ui-stream-keys-runtime.log, ui-stream-keys-compiler.log,
ui-stream-keys-build-final.log). The initial build failure is preserved in
ui-stream-keys-build.log. This bundle emits field102 and must only be deployed
with the updated native library; it has not been sent to installed version2012.

Physical system-interruption coverage found a refresh defect in installed2012:
expand the notification shade during a held pull, release, then return. The
probe verifies System UI actually owns the active window; the page then reports
Refreshing instead of Idle (ui-refresh-system-interrupt-retry.log). The earlier
attempts stopped on an obsolete page-back test ID and an unrestored entry route;
they are probe failures, not renderer evidence.
GpuView now sends ACTION_CANCEL to an owned refresh gesture on window focus loss,
page pause and surface destruction, and releases parent interception. A unit test
covers all three interruptions and duplicate cancellation; all 169 Android UI
tests pass (ui-refresh-interruption-unit.log). The probe remains a failing device
regression until a matching new native/SDK APK can be installed. This defect does
not establish the cause of the reported intermittent freeze.

After disk cleanup, a matching native/SDK APK is installed on Pixel 3 as
versionCode2013/versionName0.2.13, process18752. Native release compilation passes
(ui-animation-integrated-native.log, 16m10s), full View release build passes
(ui-animation-integrated-view.log), and APK assembly passes
(ui-animation-integrated-apk.log). Native SHA-256:
C31932A51FD320DBC3AC4D0C54B21C644B0C886537745F45E6C6FDACF195B9B7.
Workspace/all-target check now passes in ui-animation-workspace-resumed.log
(32m33s), with installed Strawberry Perl added to the process PATH.

Installed2013 passes the refresh system-interruption regression in both themes,
including subsequent pulls, rotation and reentry (ui-refresh-system-2013-windows.log).
The probe must subscribe to FLAG_RETRIEVE_INTERACTIVE_WINDOWS when opening the
notification shade; without it, UiAutomation returned a null active-window root
after closing the shade despite the visible page showing Idle. Earlier failed
attempts are retained separately. The final probe closes the shade explicitly,
touches the inert header, waits for the app window, then checks status/count/offset.

Both theme runs pass 47 tab transitions each (ui-tabs-2013.log,
ui-tabs-dark-2013.log). Every decoded recording frame passes the black-content
smoke check: 532 light and 491 dark frames (ui-tabs-video-2013.json,
ui-tabs-video-dark-2013.json). Recordings and inspected one-second contact sheets
are gpu-tabs[-dark]-2013.mp4 and gpu-tabs[-dark]-2013-contact.png. This does not
prove absence of every partial flash or establish frame pacing.
Stream's complete key inventory passes 16 long conversations across both themes,
repeated recovery of first/last virtual windows and clear (ui-history-2013.log).
The app remains process18752 with no ANR recorded since boot; captured process
logs contain no UI update failed, Render failed, animation failure, UI_LIST,
panic or out-of-memory match (ui-integrated-2013-app-final.log).
Production animation authoring remains gated: this install verifies the integrated
ordinary-frame path and field102, not visible cursor/chart animation acceptance.

## Authoring contract and flex integration (2014–2015)

Pixel 3 / Android 12 / serial 914X1UMW2. Installation hashes and versions are in
`target/ui-authoring-install-evidence.json` (2014) and
`target/ui-flex-install-evidence.json` (2015). All 21 release modules retain the
compatibility manifest; 2015 requires field104. Neither install verifies other platforms.

2014 passes Home parity and physical input focus/blur inspection, ToDo keyboard,
Stream history/keyboard and 47 tab transitions per theme. Logs are
`target/ui-home-2014.log`, `ui-home-font-2014.log`, `ui-todo-keyboard-2014.log`,
`ui-stream-history-2014.log`, `ui-stream-keyboard-2014.log`, and
`ui-tabs[-dark]-2014.log`. The two recordings pass the full-frame black-content
check for 368/495 decoded frames; see `ui-tabs-video-2014.json` and
`ui-tabs-video-dark-2014.json`. Contact-sheet padding is not video content.
This check does not establish frame pacing or exclude partial flashing.

2015 passes responsive flex footer/wide layout, ToDo state/theme/rotation,
16 long Stream conversations with repeated first/last recovery, and hardware
Enter/Gboard Send/Shift+Enter in both themes. Logs are
`target/ui-todo-wide-2015.log`, `ui-todo-parity-2015.log`,
`ui-stream-history-2015.log` and `ui-stream-keyboard-2015.log`.
The Stream tests exercise typed action-reference resource hooks after bootstrap.
185 authoring JS tests, 291 native unit tests, showcase typecheck/release build,
and the final workspace/all-targets cargo check pass. Recovery UI changes made
after this install require separate SDK and device validation.

## Candidate and session recovery (2018)

`target/ui-session-recovery-install-evidence.json` identifies version2018 / 0.2.18
and its native/APK/bundle hashes. Home parity and 47 tab switches pass
(`ui-home-2018.log`, `ui-tabs-2018.log`); all 370 decoded video frames pass the
black-content smoke check (`ui-tabs-video-2018.json`). The inspected contact
sheet is `gpu-tabs-2018-contact.png`. Current process27459's captured log has no
render failure, timeout, compatibility, panic or fatal-exception match
(`ui-session-recovery-device-final.log`). These are scenario checks, not a frame
pacing benchmark. Home remains at its probe-generated scroll position.

The separate test APK passes real host reload, fresh Logic readiness and stale
generation rejection (`ui-session-recovery-instrumentation.log`). The standalone
fixture passes candidate rollback/retry and actual infinite-loop interruption,
explicit session reload and subsequent interaction (`ui-session-recovery-device.log`).
Inspected images are `gpu-recovery-{failed,restored,timeout,restarted}.png`.
Those deliberate faults are expected errors in the test host, not production
regressions. 175 Android UI unit tests and 14 shared GPU lifecycle tests pass;
the workspace/all-targets cargo check passes. Initial endpoint-creation failures
and other platform adapters remain outside this acceptance.

### Image semantics: Android 2019

Pixel 3 (914X1UMW2), version 0.2.19 / 2019, passes image alternative-text
projection and Home greeting/flush, language, theme, rotation, navigation,
local-input retention and network/version checks. Evidence is in
`target/ui-image-alt-device.log`, `ui-home-2019.log` and
`ui-image-alt-install-evidence.json`. Native SHA256:
`F67036C3965348F98352D5265259295F3D6DAC4A97E07E1668646671FCC86659`;
APK SHA256:
`2251A06293D044D014F01524EA0D471632B2955EAB6B24A9224759D57B8D3A1C`.
All 21 release modules declare wire105. The inspected clipboard screenshot is
`gpu-image-alt-clipboard.png`; the sample image still paints in its preview.

186 JS, 294 shared UI and 176 Android UI tests pass, alongside showcase type
checks and workspace/all-targets cargo check. Process28031's captured log has
no render failure, timeout, compatibility, panic or fatal-exception match
(`ui-image-alt-device-final.log`). This batch checks accessibility projection,
not screen-reader speech, font scaling, frame pacing or other platforms.

### System text scaling: Android 2022

Pixel 3 / Android12 / 914X1UMW2, version0.2.22 / 2022, passes system font
preference100%→200%→100%, Home draft retention and greeting, Home/ToDo growing
inputs, native tab-label height, large-font Stream history, and restoration of
default geometry (`target/ui-text-scale-device-final.log`). The system font
preference is restored to1.0. Inspected screenshots include
`gpu-text-scale-todo-2022.png` and `gpu-text-scale-stream-2022.png`.

Normal-size Home parity passes (`ui-home-2022.log`), as does image semantics
(`ui-image-alt-2022.log`). The five-tab probe passes47 transitions, rapid final
target and switching after IME dismissal (`ui-tabs-2022.log`). The50-second
recording `gpu-tabs-2022.mp4` decodes364 frames with no black-content hit
(`ui-tabs-video-2022.json`); `gpu-tabs-2022-contact.png` was inspected. Blank
tiles at the end of the contact sheet are padding, not video frames. Home's
top is clipped by the probe's retained scroll position. Process29821's log
has no render-failure, timeout, compatibility, panic or fatal-exception match
(`ui-text-scale-device-final-process.log`).

297 shared UI tests,188 Android UI/chrome tests, showcase type checks,
workspace/all-targets check and Windows browser-shell clippy pass. SDK34's
nonlinear scaling test is Robolectric evidence, not Android14 device evidence.
This acceptance does not establish screen-reader speech, frame pacing or
other platform adapters.

Install identity (`ui-text-scale-install-evidence.json`): native SHA256
`F117D8111EBCFD8139B37A50D4AC721512BF166CD74ED26CA0AB1AE324E142C0`;
APK SHA256 `B8D998234C7FE1A5273395B553D328E8581FC7EA771B280359D82ECC342E8212`;
bundle integrity SHA256
`824CF268A4EDDE986519964E87CD002DA027CD46A56E6BC10A67953E30F7FCB6`.

### Shared page contracts: Android 2023

Version 0.2.23 / 2023 uses the same native library as 2022 and adds shared
Home/ToDo data and action payload types, plus recursive readonly View data.
186 JS tests, View/Logic type checks and the release bundle build pass.
The added nested-array runtime assertions also pass in the 35-test runtime
suite (`ui-page-contract-runtime.log`).

Pixel 3 passes Home parity (`ui-home-2023.log`) and the complete ToDo workflow,
including individual/bulk toggles, filters, clearing, empty state, dark theme
and rotation (`ui-todo-2023.log`). Process 30290's captured log has no render
failure, timeout, compatibility, panic or fatal-exception match
(`ui-page-contract-device-final.log`). The font preference remains 1.0.
Font scaling and transition video evidence above belongs to 2022; it was not
re-recorded for this types/data-contract batch.

Install identity (`ui-page-contract-install-evidence.json`): APK SHA256
`9CB208400C7C5ADA2040E15B6FF9633B31EDE516ACCEC1183C02E5845C5194C6`;
bundle integrity SHA256
`754B6FAD1FCBFC54B7696F3B932489B65811BAC0D70412844B0D17E9276DD469`.

### Architecture review corrections: Android 2026

Version 0.2.26 / 2026 addresses all eight concrete findings in
`docs/internal/gpu-architecture-review-2026-10-24.md`: keyed factory admission,
resolved list geometry, omitted payload semantics, readonly local types,
inherited font weight, CI coverage, image lifecycle and member JSX compilation.
It also adds editor accessibility labels, isolates image-handler failures,
preserves layout reuse for callback-only changes, aborts canceled HTTP transports,
and permits bounded shared-cache leader takeover. Native navbar/tabbar ownership
is unchanged.

Pixel 3 / Android 12 passes `GpuProductContractTest` and
`GpuSessionRecoveryTest` (`target/gpu-review-instrumentation-2026.log`). The
contract test exercises real host HTTP socket cancellation and worker release,
GPU image error/retry, obsolete-image cancellation, keyed row state after reorder,
and asymmetric list padding/border end alignment after repeated renders.
This last test exposed a further measurement-restoration inset bug; version 2026
fixes it and adds a repeated-render Rust regression.

303 Rust UI tests pass in one final run (`gpu-review-ui-303-final.log`).
191 JS authoring/compiler tests, 34 showcase behavior tests, View/Logic type
checks and 30 transfer tests pass. The transfer suite initially exposed a Windows
test-server reset race; its refusal fixture now half-closes and drains instead
of discarding an in-flight response. Synchronized download cancellation passes
separately (`gpu-review-cancel-synchronized.log`). Android UI/chrome unit checks
pass (`gpu-review-android-final.log`). Compile and device evidence are distinct.

The matching-APK core suite passes all five scenarios in one run:
Home, ToDo, image accessibility semantics, 47 native-tab switches and system
font scaling (`target/gpu-core-2026-final/results.json`). It records device/build
identity and exit codes. Cold-launch probes wait for Home content and UI idle
before acting; they do not retry navigation clicks. System font scale returns
to 1.0. The final workspace/all-targets check also passes
(`gpu-review-workspace-2026.log`); Windows browser-shell clippy passes
(`gpu-review-windows-clippy-final.log`).

A separate 55-second recording reruns all 47 tab switches on 2026. All 380
decoded frames pass the black-content check (`gpu-review-tabs-2026-frames.json`);
sampled Home/API frames show no blank region or distorted text. This is not a
display-FPS or complete visual-diff measurement. Process 8284's captured log has
no render-failure, timeout, compatibility, panic or fatal-exception match
(`gpu-review-process-2026.log`). Font scale remains 1.0.

Install identity (`gpu-review-install-evidence-2026.json`): native SHA256
`804D329E6DD101EDF00BF959CBBE54CF565FD47FE1807970ED37FB087ECB6F5F`;
APK SHA256 `3D7863C9F3DEFC512BB126C600E80B1A879D62701F4C4AF6EF88A1FDDD51166D`;
bundle integrity SHA256
`3F77316908A9109E33565B67456BDF391EA6B27A491FF35F400918795FB9B59D`.

These corrections do not establish frame-pacing/memory/power budgets, TalkBack
speech, Android 14 physical-device text scaling, animation acceptance or working
iOS/Harmony GPU adapters. The review's staged acceptance plan remains applicable.
