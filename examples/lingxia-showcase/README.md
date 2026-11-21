# lingxia-showcase

The kitchen-sink LingXia example: one host app (Android / iOS / macOS /
HarmonyOS / Windows) embedding the `lingxia-showcase` lxapp, which demos pages, the
`lx.*` API surface, View elements, media, file transfer, and the terminal.

Every page is a GPU View: a page's View is `index.tsx` beside its Logic in
`index.ts`, and renders natively. Shared View code lives in `lxapp/shared/ui`
(the design system) and `lxapp/shared` (charts, types). WebView pages live in
`../lingxia-showcase-web`.

## Run

```bash
cd lxapp && npm install && cd ..
# From this directory; pick a platform
lingxia dev --platform macos
lingxia dev --platform windows
lingxia dev --platform android
lingxia dev --platform ios
lingxia dev --platform harmony --release
```

`lingxia doctor` checks platform toolchains. Once a dev session is live, drive
it with `lxdev` (eval, screenshots, logs, `lxdev test --preset <platform>`) — see
the skill's `cli/lxdev.md` and `lxapp/tests/README.md`.

Checks that need no device:

```sh
cd lxapp && npm run typecheck
node --experimental-strip-types --test ../tests/*.test.mjs
```

Device probes and renderer fixtures live with the Android SDK tests in
`lingxia-sdk/android/tests/gpu`; they are not part of the app bundle.

## Build

```bash
lingxia build --platform windows --release
```

The Windows executable is written to `../../target/release/LingXiaDemo.exe`.

## Credits

- **Big Buck Bunny** (video demo page) — © Blender Foundation,
  [CC-BY 3.0](https://creativecommons.org/licenses/by/3.0/),
  <https://peach.blender.org>. Streamed from Blender's official mirror.
- The app icon master lives in the repo-root `design/`, licensed with the repo (MIT).
