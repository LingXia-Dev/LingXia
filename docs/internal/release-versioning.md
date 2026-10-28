# Release versioning & npm tiers

The Rust workspace version is the base-library version shared by the native
runtime, SDKs, and embedded JS runtime assets. The CLI has an explicit package
version so it can ship hotfixes against an unchanged base.

## Components

| Family | Where | Version |
|---|---|---|
| Rust crates and `lxdev` | crates.io / GitHub Release | workspace version |
| SDK (Apple/Android/Harmony) | GitHub Release | workspace version |
| CLI and Runner | GitHub Release | CLI package version |
| npm packages | npm registry | two compatibility tiers below |

## npm compatibility

### Tier 1 — base runtime

`@lingxia/bridge`, `@lingxia/polyfills`, and `@lingxia/types` release at the
workspace version with the Rust crates and SDKs.

- The CLI embeds bridge/polyfills `dist/` output. Its build rejects package
  versions that differ from `package.metadata.lingxia.{bridge,polyfills}-version`.
- Types are not embedded. `lingxia new` uses `~M.m.0` to resolve the latest
  published patch on the base runtime's line.
- Bridge and native `lingxia-lxapp` must share the [wire contract](bridge-protocol.md):
  ordinary app documents use `LegacyV2`; BrowserControlDocument uses
  non-downgradable `RequiredV3`. Both modes release with the base runtime.

### Tier 2 — framework libraries

`@lingxia/page-runtime`, `@lingxia/elements`, `@lingxia/react`,
and `@lingxia/html` are bundled into the lxapp. Their major.minor must match
its base runtime; patches may differ.

Published internal `@lingxia/*` dependencies use `~M.m.P`, floored at the
published patch. Scaffolds use `~M.m.0`, allowing packages ahead of or behind
the base patch to resolve on the same minor line. The release scripts currently
version all npm packages together; there is no `npm:<package>` component.

## `minRuntime` is a build target

`minRuntime` gates on major.minor everywhere; patches never gate. The host
refuses a package whose line is above its own. The CLI builds only for lines in
`[BUILD_TARGET_FLOOR, CLI line]` (`lingxia_control_protocol::dev_session::compat`)
and refuses others with the CLI to use instead. `lingxia dev` and `lingxia build` use that window for a
standalone lxapp and for a host project: SDK pins must agree and sit in the
window, each embedded lxapp's `minRuntime` must not exceed the host, and
`@lingxia/*` packages sit on the lxapp's `minRuntime` line — neither older nor
newer, since a newer package may call APIs that line's host lacks. A newer CLI compiles an
older project inside the window without an environment variable.

A change to CLI-generated output that a host line inside the window cannot read
must either emit the old form for that `minRuntime` or raise
`BUILD_TARGET_FLOOR` in the same change and say so in the release notes. CLI-
generated output: dist `lxapp.json`, injected page HTML, `logic.js` wrapper and
registration calls, package layout. View entry scaffolds count too: they import
`@lingxia/*` subpaths, so a new import ties the build to that npm line.

## Release commands

[version.sh](../../scripts/release/version.sh) and the
[prepare-release workflow](../../.github/workflows/prepare-release.yml)
accept `all` or `cli`:

- `--component all X` sets the workspace, CLI/Runner, npm packages, and embedded
  component metadata to **X**. It does not automatically increment the CLI patch.
  npm publishing skips versions already present in the registry.
- `--component cli Y` sets only the CLI/Runner version and refreshes `Cargo.lock`.
  The workspace, SDKs, npm packages, and embedded component metadata stay unchanged.
  Use this for CLI hotfixes on the base runtime's major.minor line.

For example, a CLI hotfix `0.20.1` can still embed base assets `0.20.0`.
When publishing its CLI release, pass `0.20.1` from
`tools/lingxia-cli/Cargo.toml`, rather than the workspace version.

CLI metadata pins embedded or downloaded assets (`bridge`, `polyfills`,
`rust-crate`, `sdk`). App npm packages, browser-shell-webui, and terminal-settings
resolve via `~M.m.0`; Cargo dependencies use `~M.m.P`, floored at the base patch
so they cannot resolve older than the paired SDK. A new minor requires a new CLI;
`lingxia new` warns when GitHub has a newer release.

## Agent skill

The skill is embedded in the CLI and has no independent version or release.
`lingxia new`, `lingxia upgrade`, and `lingxia skill install` write it to
`~/.agents/skills/lingxia`, with a link under `~/.claude/skills` for Claude Code.
Other runs reconcile the copy when its content digest differs, provided a copy
or skills root exists, so a home without agent tooling is never written to;
`upgrade --cli-only` leaves the skill alone.

## `lingxia upgrade` mechanics

The project half compares major.minor only. Applying a newer line updates:

- `@lingxia/*` npm ranges, then runs `npm install`;
- each `lxapp.json` `minRuntime` to `M.m.0`, adding it when missing and never lowering it;
- managed LingXia requirements in local Cargo manifests, including support crates
  and `[workspace.dependencies]`, then runs targeted `cargo update -p ...`.
  Inline and expanded dependency tables are supported;
- Android's Gradle `lingxia.sdkVersion` and cached Maven SDK under
  `~/.lingxia/sdk/android-maven/<ver>/`;
- Apple's cached source SDK under `~/.lingxia/sdk/apple/<ver>/` and generated
  `Package.swift` path references. The SDK's `unsafeFlags` require a local package;
  custom manifests are left alone;
- Harmony's cached HAR under `~/.lingxia/sdk/harmony/<ver>/`.

Framework workspace checkouts use source paths and are not re-fetched.
Skipping the project half non-interactively exits non-zero. On Windows,
CLI self-replacement and the project half are deferred until exit; the command
returns 10 even with `--yes`. The new binary synchronizes its own skill copy.

## CLI and Runner release assets

The `lingxia-cli-v*` release carries CLI binaries and the development Runner
used by `lingxia dev` for standalone lxapps.

- `install.sh` / `install.ps1` install `lingxia-*` and `lxdev-*`.
- CLI release targets are macOS arm64/x64 and Windows x64. Windows 11 Arm
  uses x64 emulation; installers and self-update select the x64 assets there.
  Existing native Arm CLIs from 0.28.1 or earlier must be reinstalled with the
  updated installer when a newer release no longer has Arm64 assets.
- Runner assets are fetched into `~/.lingxia/runner/<version>` as needed.
- The Windows Runner zip contains `lingxia-runner.exe` and `VERSION`.
  `lingxia dev` generates temporary host assets and passes them via `--asset-dir`.
  Product app distributions are described in [distribution](../skill/cli/distribution.md#windows).
