# LingXia Runner

LingXia Runner is the macOS simulator app for lxapp development.

When `lingxia dev` runs inside an lxapp, the CLI automatically ensures the installed Runner matches the current CLI version and then launches it.

It is built from this package, but the supported entrypoints are:

- Local release/debug preparation via `lingxia package` / `lingxia dev`
- Release script: `scripts/release/runner.sh`

## Raw Build Output

`cargo run --manifest-path ../../tools/lingxia-cli/Cargo.toml -- package --platform macos` produces:

- `tools/lingxia-runner/.lingxia/LingXia Runner.app`
- `tools/lingxia-runner/dist/macos/LingXia Runner-<version>-macos.zip`

These are the direct `lingxia build` artifacts. Runner does not require a `lingxia.config.json` file for this path.

## Development

Use `lingxia dev` for local lxapp development against Runner. Use `lingxia package --platform macos`
to prepare a runnable app bundle with current resources.

Build a specific release arch with `lingxia package`:

```bash
cd tools/lingxia-runner
cargo run --manifest-path ../../tools/lingxia-cli/Cargo.toml -- package --platform macos --macos-arch arm64
cargo run --manifest-path ../../tools/lingxia-cli/Cargo.toml -- package --platform macos --macos-arch x86_64
```

Notes:

- `arm64` builds Apple Silicon artifacts.
- `x86_64` builds Intel macOS artifacts.
- The raw output paths are the same for both, so build one arch at a time unless you copy the artifacts out between builds.

The build plugin prepares:

- the Rust static library from `crates/lingxia-devtool`

Bridge runtime injection is handled by `lingxia build` / `lingxia package`, not by the SwiftPM plugin.

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

That script runs `lingxia package`, then normalizes the raw outputs into arch-specific release assets. Across the two macOS builds, the release output looks like:

- `dist/runner-release/LingXia Runner-arm64.app`
- `dist/runner-release/LingXia Runner-x64.app`
- `dist/runner-release/lingxia-runner-<version>-macos-arm64.zip`
- `dist/runner-release/lingxia-runner-<version>-macos-x64.zip`

Each run produces one arch-specific app bundle and zip. The zip is the published Runner release artifact.

Upload to the GitHub release for the current workspace version:

```bash
scripts/release/runner.sh --publish --macos-arch arm64
scripts/release/runner.sh --publish --macos-arch x86_64
```

By default it uploads to the workspace release tag:

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
- `scripts/release/runner.sh --publish` publishes both Runner macOS architectures.
- `lingxia.config.json` is optional here. Runner is a standalone Swift Package app, and the lxapp path is supplied at runtime by `lingxia dev`.
- `lingxia dev` is the supported way to open Runner for an lxapp. The CLI keeps the installed Runner version aligned with the CLI version.
- `swift build --disable-sandbox` only builds the Swift package. It does not prepare the embedded bridge runtime or other host assets by itself.
- No embedded home lxapp is required at build time. If there is no lxapp in the package, `lingxia build` skips that step.
- `Sources/Resources/` is generated during build preparation and should not be edited by hand.
