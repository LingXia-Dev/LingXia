# LingXia Runner — macOS

The macOS Runner app. `lingxia dev` in an lxapp project launches the installed
Runner; it does not rebuild the app per run.

## Local install

Install a locally built Runner where `lingxia dev` looks for it
(`~/.lingxia/runner/<version>/`):

```bash
tools/lingxia-runner/macos/install-local-runner.sh
```

Re-run it after changing Runner code — `lingxia dev` alone will keep launching
the previously installed version.

## Building

Runner is a standalone Swift Package (no `lingxia.yaml`). Its SwiftPM build
plugin prepares the native resources: it builds the Rust static library from
`macos/native` (crate `lingxia-runner-lib`) and copies
`packages/lingxia-bridge/dist/bridge-runtime.es2020.js` to
`Sources/Resources/bridge-runtime.js` — the build fails if that dist file is
missing, and the copied file must never be edited by hand.

```bash
cd tools/lingxia-runner/macos
swift build --disable-sandbox
```

Or build/package through the CLI (produces `target/lingxia/macos/LingXia
Runner.app` and a versioned zip under `dist/macos/`):

```bash
cd tools/lingxia-runner/macos
cargo run --manifest-path ../../lingxia-cli/Cargo.toml -- package --platform macos
```

## Release

```bash
scripts/release/runner.sh --macos-arch arm64     # one arch per run
scripts/release/runner.sh --macos-arch x86_64
scripts/release/runner.sh --publish --macos-arch arm64   # upload to the release tag
```

Output lands in `dist/runner-release/` as an arch-specific `LingXia Runner-*.app`
and `lingxia-runner-<version>-macos-<arch>.zip`. `--tag` overrides the upload
tag, `--out` the output directory.

## Notes

- The package name and app product stay `LingXia Runner` — installed-runner
  lookup, release packaging, and user-facing docs depend on it.
- Runner does not embed a home lxapp at build time.
- `lingxia build`/`package` prepare `bridge-runtime.js` only for host projects
  with `lingxia.yaml`; Runner prepares its own through the SwiftPM plugin.
