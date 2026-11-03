# Android GPU smoke host

A standalone Android app (`com.lingxia.view.smoke`) that runs one Logic-free GPU
lxapp through the SDK's `GpuView`, without the lxapp runtime or a Logic
service. Use it to look at renderer behavior on a device in isolation; anything
that needs Logic, navigation or host chrome runs in the SDK host instead.

From the repository root:

```sh
npm --prefix packages install
node lingxia-sdk/android/tests/gpu/smoke/run.mjs --device <serial>
node lingxia-sdk/android/tests/gpu/smoke/run.mjs --project lingxia-sdk/android/tests/gpu/fixtures/renderer --page flex
```

`run.mjs` builds the project with `packages/lingxia-view/build.mjs`, cross-builds
the core for `aarch64-linux-android`, assembles the host and launches it.
`--page` opens one page of a multi-page fixture (default: the first page).
It needs Rust's Android target, JDK 17, Android SDK 35 and an NDK; set
`ANDROID_HOME` or pass `--sdk`.

Off-device checks for the same pages:

```sh
node packages/lingxia-view/build.mjs examples/hello-view
cargo test -p lingxia-view
npm test --prefix packages --workspace @lingxia/view
```

The host supplies assets, system insets, lifecycle and an evidence snapshot;
rendering, input and accessibility come from the SDK component. After a frame
is presented, nodes carry their `testId` as the accessibility resource id, so
the drivers here locate controls without coordinates:

- `verify.mjs <adb> <serial>` — keyboard input, rotation retention, landscape fit.
- `verify-pages.mjs <adb> <serial>` — two pages on one executor, switched surfaces.
- `verify-feedback.mjs <adb> <serial> <package> <testId>` — press/release pixels
  (Python with Pillow).
