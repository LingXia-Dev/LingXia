# Android SDK modules

| Maven artifact | Minimum API | Includes |
| --- | --- | --- |
| `lingxia-core` | 21 | Runtime, WebView bridge, Media3 playback, media preview/picking and processing; no camera or barcode engine |
| `lingxia-camera` | 23 | Photo/video capture with CameraX; depends on core |
| `lingxia-scanner` | 23 | Camera and album barcode scanning with CameraX and ML Kit; depends on core, not the capture module |
| `lingxia` | 24 | Complete SDK: core, camera and scanner with current AppCompat/Material/WebKit dependencies |

Use core for playback hosts such as Muke. Playback is retained in core in this
split; it is not yet a separately removable AAR. Core selects API 21-compatible
UI/WebView dependencies. Optional modules and the complete SDK can advance
without raising the baseline for playback hosts. Android's WebView engine is
installed on the device; selecting the AndroidX adapter does not select the
engine version.

Generated hosts default to core. In `android/app/build.gradle.kts`, add only the
capabilities the host needs, with the same SDK version:

```kotlin
implementation("io.github.lingxia-dev:lingxia-core:$lingxiaSdkVersion")
// Optional; set the host minSdk to at least 23 when adding these.
implementation("io.github.lingxia-dev:lingxia-camera:$lingxiaSdkVersion")
implementation("io.github.lingxia-dev:lingxia-scanner:$lingxiaSdkVersion")
```

Existing hosts that keep the `lingxia` coordinate consume the complete SDK and
must declare API 24 or higher. To retain API 21/22, select core and omit camera
and scanner. Do not use `overrideLibrary` or Gradle dependency exclusions to
bypass a module's platform requirements. Keep the host's own direct UI
dependencies compatible with its minimum API too.

The optional AARs provide implementations through Java service descriptors.
Core discovers them lazily; SDK initialization does not load or warm up the
camera. Missing modules reject native callbacks with
`MODULE_NOT_INSTALLED:camera` or `MODULE_NOT_INSTALLED:scanner`. An album picker
hides its capture entry when the camera module is absent. Hosts do not register
providers manually. The SDK consumer rules preserve providers in minified hosts.

The module support API lives in `com.lingxia.app.media.modules`. It delegates
callbacks, logging, insets and album selection back into core; JNI entry points
remain in core and retain their existing names. This SPI does not grant any
additional host or guest permissions.

Build every Maven publication together (a facade AAR alone is insufficient):

```sh
./gradlew publishAllPublicationsToLocalExampleRepository -Pversion=<version>
```

`scripts/release/sdk.sh --platform android` packages all four publications in
the existing Maven SDK zip. Generate icons and translations first when building
a clean checkout directly; see `.github/actions/bootstrap-android-sdk`.

Verification:

```sh
./gradlew :compatibility:assembleCoreDebug :compatibility:assembleFullDebug :lingxia-core:testDebugUnitTest
./gradlew :compatibility:connectedCoreDebugAndroidTest :compatibility:connectedFullDebugAndroidTest
# Also exercise service discovery after R8 shrinking:
./gradlew :compatibility:connectedCoreDebugAndroidTest :compatibility:connectedFullDebugAndroidTest -Pcompatibility.minify=true
```

The compatibility host uses no Rust native library. Its two dependency graphs
exercise manifest merging at API 21 and API 24; device tests check optional
provider discovery and play a generated PCM WAV through Media3 without
initializing the LingXia runtime. Product native migration and Android 5.1
product journeys still require their own device tests.
