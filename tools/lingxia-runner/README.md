# LingXia Runner

LingXia Runner is the macOS simulator app for lxapp development.

When `lingxia dev` runs inside an lxapp, the CLI automatically ensures the installed Runner matches the current CLI version and then launches it.

It is built from this package, but the supported entrypoints are:

- Local debug build: `swift build --disable-sandbox`
- Local release bundle: `cargo run --manifest-path ../../tools/lingxia-cli/Cargo.toml -- build --platform macos --package --release`
- Release script: `scripts/release/runner.sh`

## Raw Build Output

`cargo run --manifest-path ../../tools/lingxia-cli/Cargo.toml -- build --platform macos --package --release` produces:

- `tools/lingxia-runner/.lingxia/LingXia Runner.app`
- `tools/lingxia-runner/dist/macos/LingXia Runner-<version>-macos.zip`

These are the direct `lingxia build` artifacts. Runner does not require a `lingxia.config.json` file for this path.

## Development

Build the Runner package directly:

```bash
cd tools/lingxia-runner
swift build --disable-sandbox
```

Build a specific release arch with `lingxia build`:

```bash
cd tools/lingxia-runner
cargo run --manifest-path ../../tools/lingxia-cli/Cargo.toml -- build --platform macos --package --release --macos-arch arm64
cargo run --manifest-path ../../tools/lingxia-cli/Cargo.toml -- build --platform macos --package --release --macos-arch x86_64
```

Notes:

- `arm64` builds Apple Silicon artifacts.
- `x86_64` builds Intel macOS artifacts.
- The raw output paths are the same for both, so build one arch at a time unless you copy the artifacts out between builds.

The build plugin prepares:

- the web runtime from `packages/lingxia-bridge`
- the Rust static library from `crates/lingxia-devtool`

## Release

Use the dedicated release script:

```bash
scripts/release/runner.sh
```

Or build a specific release arch:

```bash
scripts/release/runner.sh --macos-arch arm64
scripts/release/runner.sh --macos-arch x86_64
```

That script runs `lingxia build`, then normalizes the raw outputs into release assets:

- `dist/runner-release/LingXia Runner.app`
- `dist/runner-release/lingxia-runner-<version>-macos.zip`

The zip is the published Runner release artifact.

Upload to the GitHub release for the current workspace version:

```bash
scripts/release/runner.sh --publish
```

By default it uploads to tag:

```text
lingxia-cli-v<version>
```

Override the upload tag or output directory if needed:

```bash
scripts/release/runner.sh --tag lingxia-cli-v0.4.3
scripts/release/runner.sh --out /tmp/runner-release
```

## Notes

- Runner is macOS-only.
- `lingxia.config.json` is optional here. Runner is a standalone Swift Package app, and the lxapp path is supplied at runtime by `lingxia dev`.
- `lingxia dev` is the supported way to open Runner for an lxapp. The CLI keeps the installed Runner version aligned with the CLI version.
- No embedded home lxapp is required at build time. If there is no lxapp in the package, `lingxia build` skips that step.
- `Sources/Resources/` is generated during build preparation and should not be edited by hand.
