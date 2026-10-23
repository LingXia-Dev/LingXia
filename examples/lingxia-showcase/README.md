# lingxia-showcase

The kitchen-sink LingXia example: one host app (Android / iOS / macOS /
HarmonyOS / Windows) embedding the `lingxia-showcase` lxapp, which demos pages, the
`lx.*` API surface, native components, media, file transfer, and the terminal.

The showcase pages are React. A project is one view framework, React or HTML,
taken from its page files or the `framework` field in `lxapp.json`.

## Run

```bash
# From this directory; pick a platform
lingxia dev --platform macos
lingxia dev --platform windows
lingxia dev --platform android
lingxia dev --platform ios
lingxia dev --platform harmony
```

> **Tip:** add `--release` if disk space is tight — debug Rust artifacts are
> several times larger than release ones, and skipping the debug profile
> keeps `target/` much smaller.

`lingxia doctor` checks platform toolchains. Once a dev session is live, drive
it with `lxdev` (tabs, eval, screenshots, logs) — see the skill's
`cli/lxdev.md`.

## Build

```bash
lingxia build --platform windows --release
```

When working inside the monorepo without an installed `lingxia` CLI, run the
same build through Cargo:

```bash
cargo run -p lingxia-cli -- build --platform windows --release
```

The Windows executable is written to `../../target/release/LingXiaDemo.exe`.

## Credits

- **Big Buck Bunny** (video demo page) — © Blender Foundation,
  [CC-BY 3.0](https://creativecommons.org/licenses/by/3.0/),
  <https://peach.blender.org>. Streamed from Blender's official mirror;
  poster frame via Wikimedia Commons.
- Tab bar icons are original LingXia artwork (SVG masters in
  `./design/icons/`); the app icon master lives in the repo-root `design/`.
  Both are licensed with the repo (MIT).
