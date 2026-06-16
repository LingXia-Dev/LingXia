# lingxia-showcase

The kitchen-sink LingXia example: one host app (Android / iOS / macOS /
HarmonyOS) embedding the `lingxia-showcase` lxapp, which demos pages, the
`lx.*` API surface, native components, media, file transfer, and the terminal.

> Unlike a real product, the home pages ship React **and** Vue
> implementations side by side — that's why every build needs an explicit
> `--framework`.

## Run

```bash
# From this directory; pick one platform and one of react | vue
lingxia dev --platform macos   --framework react
lingxia dev --platform android --framework vue
lingxia dev --platform ios     --framework react
lingxia dev --platform harmony --framework vue
```

Any platform works with either framework — the pairings above are just to
show both views exist.

> **Tip:** add `--release` if disk space is tight — debug Rust artifacts are
> several times larger than release ones, and skipping the debug profile
> keeps `target/` much smaller.

`lingxia doctor` checks platform toolchains. Once a dev session is live, drive
it with `lxdev` (tabs, eval, screenshots, logs) — see the skill's
`cli/lxdev.md`.

## Credits

- **Big Buck Bunny** (video demo page) — © Blender Foundation,
  [CC-BY 3.0](https://creativecommons.org/licenses/by/3.0/),
  <https://peach.blender.org>. Streamed from Blender's official mirror;
  poster frame via Wikimedia Commons.
- Tab bar icons are original LingXia artwork (SVG masters in
  `./design/icons/`); the app icon master lives in the repo-root `design/`.
  Both are licensed with the repo (MIT).
