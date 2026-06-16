# LingXia Runner

LingXia Runner is the macOS app used by `lingxia dev` to run standalone lxapps.

The Windows dev runner is intentionally separate: its executable crate lives at
`tools/lingxia-runner/windows` and depends on `crates/lingxia-windows-sdk`. Keep
this directory focused on the macOS Swift app and its Rust static library.

## Development

Run this from an lxapp project:

```bash
lingxia dev
```

The CLI keeps the installed Runner version aligned with the CLI version, then
launches Runner with the lxapp path supplied at runtime.

## Building Runner

Runner is a standalone Swift Package. It does not need `lingxia.yaml`.

The SwiftPM build plugin prepares the required native resources:

- builds the Rust static library from `tools/lingxia-runner/macos/runner-lib`
- copies `packages/lingxia-bridge/dist/bridge-runtime.es2020.js` to
  `Sources/Resources/bridge-runtime.js`

`packages/lingxia-bridge/dist/bridge-runtime.es2020.js` is required. If it is
missing, the Runner build fails.

Build Runner directly with SwiftPM:

```bash
cd tools/lingxia-runner/macos
swift build --disable-sandbox
```

You can also build/package it through the LingXia CLI:

```bash
cd tools/lingxia-runner/macos
cargo run --manifest-path ../../lingxia-cli/Cargo.toml -- package --platform macos
```

That produces:

- `tools/lingxia-runner/macos/.lingxia/LingXia Runner.app`
- `tools/lingxia-runner/macos/dist/macos/LingXia Runner-<version>-macos.zip`

## Release

Use the release script:

```bash
scripts/release/runner.sh
```

Build a specific release architecture:

```bash
scripts/release/runner.sh --macos-arch arm64
scripts/release/runner.sh --macos-arch x86_64
```

Each run produces one arch-specific app bundle and zip. Across both macOS
architectures, release output looks like:

- `dist/runner-release/LingXia Runner-arm64.app`
- `dist/runner-release/LingXia Runner-x64.app`
- `dist/runner-release/lingxia-runner-<version>-macos-arm64.zip`
- `dist/runner-release/lingxia-runner-<version>-macos-x64.zip`

Upload to the workspace release tag:

```bash
scripts/release/runner.sh --publish --macos-arch arm64
scripts/release/runner.sh --publish --macos-arch x86_64
```

Override the upload tag or output directory if needed:

```bash
scripts/release/runner.sh --tag lingxia-cli-v0.4.3
scripts/release/runner.sh --out /tmp/runner-release
```

## Notes

- This Runner package is macOS-only.
- The package name and app product stay `LingXia Runner` for compatibility
  with installed-runner lookup, release packaging, and user-facing docs.
- Windows runner code belongs in `tools/lingxia-runner/windows`.
- Runner does not embed a home lxapp at build time.
- `lingxia build` / `lingxia package` prepare `bridge-runtime.js` only for host
  projects with `lingxia.yaml`; Runner prepares its own bridge runtime through
  its SwiftPM build plugin.
- `Sources/Resources/bridge-runtime.js` is generated during build preparation
  and should not be edited by hand.
