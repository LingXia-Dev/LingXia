# Android device verification

`run-core-suite.ps1 -ApkPath <matching-release-apk> -Serial <device>` runs the
scenarios in `core-suite.json`, checks the installed APK hash, enforces deadlines
and writes per-scenario logs plus device/build identity in `results.json`.
The suite requires an empty ToDo list, changes clipboard contents, and exercises
appearance/language, rotation and font scale. It creates and cleans only its own
tasks. Run on a dedicated test device; keep the original system settings in the
result record. This is a functional gate, not a frame-pacing or TalkBack gate.

Before building the instrumentation APK, run
`node lingxia-sdk/android/tests/gpu/scripts/build-fixtures.mjs`.
`GpuProductContractTest` adds a test-only GPU surface in the installed host and
checks deterministic image failure/retry/cancellation, keyed row state through
reorder, and Tailwind per-edge padding/border end anchoring on the actual GPU.
It also cancels two real loopback HTTP requests through the host image loader,
requires both connections to close, and verifies a new request can complete.
Generated fixtures live under `target/gpu-device-fixtures`, outside lxapp bundles.

These SDK-owned Android device tools exercise the GPU renderer and the
showcase app fixture at `examples/lingxia-showcase-gpu`. They are not bundled
with the host or lxapp. Shared interaction semantics belong in Rust tests under
`crates/lingxia-ui`; Android adapter unit tests live in
`lingxia-sdk/android/lingxia/src/test`.

The product probes still depend on showcase page IDs and its package. Moving
the drivers here does not make those scenarios app-independent.

- `probes/`: Java UiAutomation probes compiled to a temporary jar and run through
  adb `app_process` on the connected device.
- `scripts/`: Node drivers, Python pixel checks and the local file fixture server.
- `fixtures/`: renderer injection fixtures.

Run the product probes against the installed independent GPU APK, starting at
Home. From the repository root:

```powershell
./lingxia-sdk/android/tests/gpu/run-probe.ps1 -Probe GpuStreamProbe -Serial <device> -AndroidSdk <sdk-directory>
node lingxia-sdk/android/tests/gpu/scripts/verify-host.mjs <adb-path> <device>
```

`run-probe.ps1 -CompileOnly` checks all probe sources without running a device
test. Build artifacts go under the repository's `target/`, outside `lxapp/`.
SDK tools also accept `ANDROID_SDK_ROOT`/`ANDROID_HOME`; direct adb scripts use
`ADB` or adb on PATH and require a device argument or `ANDROID_SERIAL`.
The Node/Python scripts also run from the repository root; relative sibling
imports remain inside `scripts/`.

`GpuRecoveryProbe` targets `com.lingxia.ui.smoke`, not showcase. Build the
standalone host with `fixtures/recovery` using `tools/lingxia-ui-android/run.ps1
-Project <absolute-fixture-path>`. It deliberately fails one candidate, verifies
the retained frame, clicks Retry and checks the committed count and retained draft.
It also runs an infinite loop, verifies the execution deadline and explicitly
reloads the interrupted session before checking interaction again.
Inspect `gpu-recovery-failed.png` and `gpu-recovery-restored.png` in
`/data/local/tmp/` as well; semantic assertions do not prove surface visibility.

`instrumentation/` is a separate test APK for the integrated showcase host.
Build `:app:assembleReleaseAndroidTest` from its Android project with
`-Plingxia.gpuDeviceTests=true -Plingxia.applicationIdSuffix=.dev`, after installing
the matching release host. Install `app-release-androidTest.apk` and run
`adb shell am instrument -w -e class com.lingxia.ui.tests.GpuSessionRecoveryTest
com.lingxia.example.gpu.dev.test/androidx.test.runner.AndroidJUnitRunner`.
This injects a session error through reflection in the test APK, clicks the
host-owned reload action, waits for fresh Logic readiness and rejects an old
generation's restart callback. It adds no fault-injection API to the product.

`GpuFocusStyleProbe` physically focuses Home and ToDo editors, checks the Home
ring and light/dark ToDo background pixels, then blurs with Tab and checks their
restoration. It saves focused screenshots and returns Home/light without editing
tasks. Run with the keyboard available and the current showcase starting at Home.

`GpuFlexBasisProbe` runs on the lab's `rendererFixture: 'flex'` View. It measures
content versus zero basis, percentage basis and column growth, then resizes and
checks updated geometry. It returns from lab without modifying persisted data.

For the current showcase, enter the `lab` page through dev tooling before running
`node lingxia-sdk/android/tests/gpu/scripts/verify-images.mjs <adb> <serial> --current-page`.
This preserves that page instead of relaunching Home. The image workload checks
visible pixels, shared-source uploads, eviction/reload, surface resume and idle;
its log assertions exclude images loaded before the workload starts.

`GpuChartProbe` selects each deterministic Stream sample (bar, line and pie),
checks colored pixels and accessible summaries, and toggles the TS legend.
It saves `gpu-independent-chart-{bar,line,pie}.png` under `/data/local/tmp/`.
Pull and inspect those screenshots as well: colored-pixel counts alone cannot
detect rendering seams or label collisions.

For `GpuMediaImageProbe`, first push `examples/lingxia-showcase-gpu/lxapp/public/mark.png` to
`/sdcard/Download/lingxia-gpu-media-fixture.png` and broadcast
`android.intent.action.MEDIA_SCANNER_SCAN_FILE` for its `file://` URI.
The probe selects the newest album image; run immediately after importing this
1024×1024 fixture, without creating other photos. It grants the media permission
if prompted, verifies 256×256 compression with a nonzero file size, opens the
native preview, then checks the retained result on return. Inspect
`target/gpu-independent-media-preview.png` after pulling the corresponding
`/data/local/tmp/` screenshot to verify image content.

For `GpuMediaVideoProbe`, record the GPU Home screen with
`adb shell screenrecord --time-limit 3 --size 640x1280 --bit-rate 2000000 /sdcard/Download/lingxia-gpu-video-fixture.mp4`,
then scan that file with the same media-scanner broadcast. Start at Home in light
theme with media permission granted. The newest video must be this fixture.
The probe checks metadata, a 160×320 thumbnail, nonempty compressed output,
both previews and restored dark status icons after returning to the light page.
It also cancels a compression and restarts it before checking successful output.
It does not prove playback motion, compression ratio or mid-task navigation.

Some historical renderer probes target the earlier smoke harness or require
entering the playground first. They are retained as test sources; their presence
does not establish current product coverage. See [the parity checklist](../../../../examples/lingxia-showcase-gpu/tests/PARITY.md).

For `GpuScanProbe`, push `fixtures/scanner-qr.png` to
`/sdcard/Download/lingxia-gpu-scanner-fixture.png` and broadcast
`android.intent.action.MEDIA_SCANNER_SCAN_FILE` with that file URI immediately
before the run. It selects the newest album image, decodes the fixed text
`LingXia GPU scanner verification`, checks scanner and nested-picker Back,
then verifies that ordinary page Back still works after the overlays close.
The fixture becomes the newest image; re-import the image-compression fixture
before running `GpuMediaImageProbe`. This verifies album decoding, not live
camera recognition of a physical code.

For `GpuClipboardImageProbe`, import `examples/lingxia-showcase-gpu/lxapp/public/mark.png` as the newest album
image (for example `/sdcard/Download/lingxia-gpu-clipboard-selected.png`, then
broadcast the media scanner). Start at Home. The probe verifies selected-image
clipboard roundtrip, centered square GPU preview bounds, cancel without losing
clipboard contents, rotation and clear. Inspect
`gpu-clipboard-selected-image.png` for the actual logo pixels.

Sharing to a local receiver (no network delivery): run
`build-share-receiver.ps1`, then `run-probe.ps1 -Probe GpuShareReceiverProbe`
from Home with the 1024x1024 logo as the newest album image. The receiver stores
only these test intents in its private `received.jsonl`; image bytes are read
and hashed through the granted content URI. Run
`node lingxia-sdk/android/tests/gpu/scripts/verify-share-receiver.mjs`
from the repository root to validate the last three records. Then uninstall
`com.lingxia.test.receiver`. Build artifacts stay under `target/gpu-share-receiver`.
The host uses the original showcase App Link domain for URL construction; OS
domain verification is a separate gate.

For `GpuShareFileProbe`, generate `BODY` from `scripts/file-fixture.py` into
`target/gpu-share-file-fixture.pdf`, push it to the phone Downloads directory
with the same filename, and broadcast the media scanner. Install the test
receiver with `build-share-receiver.ps1`. Run the probe from Home, then
`python lingxia-sdk/android/tests/gpu/scripts/verify-share-file.py`
from the repository root. The check compares every byte through size and SHA-256.
Uninstall `com.lingxia.test.receiver` afterwards.

Cloud provider verification uses `D:/lingxia-cloud-client` through
`lingxia dev -p android --release --framework react --with-provider cloud
--provider-path D:/lingxia-cloud-client`. Keep both provider options on later
rebuilds; the ordinary example build intentionally leaves the provider disabled.
The local mock configuration for this run is
`target/gpu-cloud/cloud-mockd.toml`: HTTP `127.0.0.1:39881`, MQTT QUIC
`192.168.37.80:39883`, LingXia id `lingxia-showcase-gpu`, tenants A/B. The
example's dev server and HTTP probes use USB loopback:
`adb reverse tcp:39881 tcp:39881`. The mock binds HTTP to `0.0.0.0` and
advertises `127.0.0.1`; MQTT advertises the machine's reachable LAN address.
This avoids the device Wi-Fi proxy for browser authorization without changing
the user's network settings.
The mock daemon is built from an isolated snapshot under `target/gpu-cloud-source`
so dependency resolution does not modify the provider checkout.

Run `GpuCloudNetworkProbe` first, then (once the provider APK is installed)
`GpuCloudProviderProbe` for login/functions and `GpuCloudMqttProbe` for messages
and broker-side unsubscribe checks. Login/functions and MQTT now pass on Pixel 3;
see `ui-cloud-provider-final-device.log` and `ui-cloud-mqtt-final-device.log`
under `target/`. The MQTT probe publishes only to this test daemon's
`lingxia/tenant/tenant-a/demo/mqtt` topic, with explicit UTF-8 metadata.
Compile with `run-probe.ps1 -Probe GpuCloudMqttProbe -CompileOnly`, push
`target/gpu-independent-probe.jar` to `/data/local/tmp/`, then run:

```powershell
adb shell "GPU_MQTT_CLIENT_ID=<client-id-from-mock-connect-log> CLASSPATH=/data/local/tmp/gpu-independent-probe.jar app_process /system/bin GpuCloudMqttProbe"
```

Start at Home with Tenant A logged in. The page subscribes automatically.
Broker checks require both client ID and topic; the mock returns a global count
when client ID is omitted. The probe checks presence before checking removal.

Run `GpuCloudIdentityProbe` from Home with Tenant A logged in and Tenant B absent.
It adds B, verifies B's user and server whoami, switches A/B, then logs B out and
checks automatic fallback to A. It leaves A active and returns Home.
Evidence: `target/ui-cloud-lifecycle-identity-device.log`.

Run `GpuHomeFontProbe` from Home while recording with `adb shell screenrecord`.
It types short and mixed-width text, hides the IME and refocuses, then restores
the original value. Inspect transition frames, not just settled screenshots:
glyph proportions and the input rectangle must remain stable during IME resize.
The probe reports CAPTURED because visual inspection is required.

`GpuTabTransitionProbe` starts at Home and switches all five native tabs 47 times,
including rapid taps, a final-target assertion and a switch after IME dismissal.
Pass `dark` for dark mode; the default selects light mode. Record the entire run
with `adb shell screenrecord --time-limit 50`, starting the probe after recording
begins. Wait for recording to finish before pulling the MP4. Run
`python lingxia-sdk/android/tests/gpu/verify-tab-video.py <video> --ffmpeg <ffmpeg>`
to check every decoded frame for a black content region. Also inspect transition
frames for partial black strips, image scaling and stale content, and check the
app process's `LingXia.UI` / `AndroidRuntime` error log. Run cold with the dev
server disconnected to cover the packaged app.

Record `GpuNavigationLayoutProbe` for 80 seconds to include cold push, nested
push/pop, redirect, popup teardown and programmatic switchTab in both themes.
Run the same black-frame helper and inspect the slide edges. Follow with
`GpuTabbarLayoutProbe` to ensure same-page tabbar show/hide also removes its
transition cover and leaves controls responsive.

`GpuClipboardLayoutProbe` starts from Home, checks Clipboard/Share control
geometry and multiline IME editing in both themes, then returns Home/light.
Run at normal density and a320logical-pixel viewport; inspect the sizing-image
screenshot for wrapping of the longer sample-image action. It does not write
the clipboard or open share receivers. Restore the original device density.

`GpuChannelProbe` checks mixed-size price/change baseline geometry in both
themes and subscription continuity across rotation. Before the first physical
disconnect tap it waits for history to reach its bounded height, so incoming
ticks cannot move the control and cancel the queued press. Inspect portrait
and landscape screenshots as well as the probe result.

`GpuChannelScrollProbe` starts from Home at a 320 logical-pixel portrait viewport.
It tests physical horizontal drags and trailing-symbol taps, accessible backward/
forward scrolling, and vertical drags on the same strip reaching the outer page,
in both themes. Restore device density after the run. Compile-only success is
not device evidence.

`GpuTodoParityProbe` requires an empty ToDo list and preserves existing tasks by
refusing to run otherwise. It checks filter bounds, one-row spacing and 44dp
touch height in both themes, alongside its task/keyboard/rotation workflow.
Run at default density and 320dp portrait width. Layout-only containers are not
accessibility nodes; use the visible controls and scroll viewport for geometry.

`GpuImageSemanticsProbe` checks Home's decorative images are absent from the
accessibility tree, then writes the sample PNG to the clipboard and reads it
back to verify the image description, role and lack of a click action. It
overwrites the device clipboard and returns Home. This verifies accessibility
projection, not spoken output from a screen reader.

`GpuTextScaleProbe` changes the system font preference to 100%, 200%, then 100%,
checks Home title/input and native tab-label geometry, draft retention,
ToDo input growth and large-font Stream history, and
restores the original preference in `finally`. Inspect the large-font screenshot
for clipping. Android 12 device evidence does not verify Android 14's nonlinear
mapping; its SDK34 Robolectric test is adapter evidence only.
