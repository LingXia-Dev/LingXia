# Release versioning & npm tiers

LingXia ships several artifact families. The rust workspace version is the
**base-library version** (think WeChat mini-program base lib): the native
runtime, SDK, CLI, and the JS runtime assets embedded in the app all share it.

## Components

| Family | Where | Version |
|---|---|---|
| rust crates | crates.io | workspace version |
| SDK (apple/android/harmony) | GitHub Release | workspace version |
| CLI (`lingxia`) | GitHub Release | **own version line** (major.minor mirrors the workspace; patch independent) |
| npm packages | npm registry | **tiered — see below** |

## npm tiers

Not all npm packages may drift from the workspace. They split into three tiers:

### Tier 1 — base runtime (locked to the workspace version)
`@lingxia/bridge`, `@lingxia/polyfills`, `@lingxia/types`

- `bridge` and `polyfills` are **embedded into the CLI as app runtime assets**
  (`tools/lingxia-cli/build.rs` `include_bytes!`s their `dist/` output and
  **panics if the package.json version ≠ the CLI's pinned
  `package.metadata.lingxia.{bridge,polyfills}-version`**).
- `@lingxia/types` is on the same line but is **not** embedded. `lingxia new`
  writes the minor-floor tilde (`~M.m.0`) so `npm install` takes the latest
  published patch after a release, without a per-package pin in the CLI.
- `@lingxia/bridge` (JS) 必须与 native `lingxia-lxapp` 使用同一 wire contract：普通 app
  document 使用 `LegacyV2`，BrowserControlDocument 使用不可降级的 `RequiredV3`。它是 runtime
  的 JS 半边，因此两种 mode 的 codec/bootstrap 都必须随 base runtime 同步发布。
- **Release only via `--component all`**, at the workspace version, together with
  the rust crates / SDK / CLI. `scripts/release/version.sh` rejects
  `--component npm:bridge|polyfills|types`.

### Tier 2 — framework libraries (major.minor tracks the workspace)
`@lingxia/page-runtime`, `@lingxia/elements`, `@lingxia/react`, `@lingxia/vue`,
`@lingxia/html`

- Imported by an lxapp and bundled into the lxapp's own dist. They speak the
  bridge protocol (via `@lingxia/bridge`), so their **major.minor must match the
  base runtime**; patch may drift.
- Internal `@lingxia/*` deps are tilde ranges pinned at the patch being published
  (`~0.x.y`, written by `version.sh`): a later patch on the same minor still
  satisfies them, an older one must not. Patch-release a single package with
  `--component npm:<package>`; move major.minor with `--component all`.
- `--component all` versions and republishes **every** npm package at the
  workspace version; `npm.sh` skips only what the registry already has at that
  exact version. A framework package may still run *ahead* of the base from a
  standalone `--component npm:<package>` patch.
- Scaffolds written by `lingxia new` use a **minor-floor tilde** (`~M.m.0`,
  `versions::minor_tilde_range`) instead of the CLI's baked patch: a fresh
  `npm install` resolves the newest patch on the line either way, and the lower
  floor keeps scaffolding working when a package lags the base.

### The agent skill (no tier — it is not a package)
The skill describes what the CLI can do, so it is compiled into the CLI and
written out by the CLI itself — there is no install command. It has no version
of its own and no release train: an installed copy always came from the binary
that wrote it, and every run rewrites it when its content digest differs from
the embedded one, so a development build's edits land as soon as it runs.

Giving it a version of its own would reintroduce the only failure it can have:
a skill describing calls the runtime it is paired with does not provide.

## CLI version line

The CLI embeds the base runtime (bridge/polyfills) as assets, so a base release
must re-release the CLI. But the CLI also ships its own fixes, which must not
require a base bump and must never be regressed by one. So the CLI keeps its
**own version line**:

- **major.minor mirrors the workspace** — CLI `0.9.x` means "the CLI for base
  runtime 0.9". Metadata only names what the binary embeds or downloads by
  exact asset name (`bridge`, `polyfills`, `rong`, `rust-crate`, `sdk`). It
  does **not** pin `@lingxia/react|vue|html|types`, browser-shell-webui, or
  terminal-settings. Those resolve to `~M.m.0` at `lingxia new` or first fetch,
  so a published patch is picked up without rebuilding the CLI. Scaffolded
  crate deps float the same way but floor at the base patch (`~M.m.P`): the
  crate workspace publishes in lockstep, and the crates must never resolve
  older than the SDK zip the same metadata names. A new **minor** still needs a
  new CLI — `lingxia new` warns when GitHub has a newer one.
- **patch is independent.** `--component all X` advances the CLI to
  `X.major.X.minor.(currentCliPatch+1)` on the same minor, or `X.major.X.minor.0`
  on a new minor — it reads the current CLI version and rolls forward, never
  back. `--component cli Y` sets the CLI explicitly for a standalone hotfix: it
  touches the CLI package version, the Runner that tracks it, and `Cargo.lock`,
  and nothing else. `lxdev` stays on the workspace line, because it reports
  version skew against npm packages that move with the workspace.
- When publishing a base release, pass the CLI's **own** version (from
  `tools/lingxia-cli/Cargo.toml`) to `component=cli`, not the workspace version.

Example: workspace `0.9.0`, CLI already `0.9.1` from a hotfix. `--component all
0.9.0` → workspace/base npm stay `0.9.0`, CLI rolls to `0.9.2`, CLI metadata →
`0.9.0`. No collision, no regression.

## CLI and Runner release assets

The `lingxia-cli-v*` GitHub Release carries both user-installed CLI binaries
and the developer Runner used by `lingxia dev` for standalone lxapps.

- CLI assets (`lingxia-*`, `lxdev-*`) are installed by `install.sh` /
  `install.ps1`.
- Runner assets are fetched lazily by the CLI into
  `~/.lingxia/runner/<version>`. They are not user-facing app distributions.
- The Windows Runner zip intentionally contains only `lingxia-runner.exe` and
  `VERSION`. `lingxia dev` generates temporary host assets from the installed
  CLI and the current lxapp, then launches the runner with `--asset-dir`.
- A normal Windows host app is different: distribution must be either an MSIX
  or a portable bundle with the `.exe` next to its `assets/` directory. A bare
  host-app `.exe` is not a runnable distribution.

## Suggested CI release grouping

1. **Base runtime** (one version = workspace version): rust crates + SDK + CLI +
   Tier-1 npm (`bridge`, `polyfills`, `types`) — published together.
2. **Framework npm train**: Tier-2 packages, major.minor pinned to the base
   runtime, patch may ship on its own via `--component npm:<package>`.
The prepare-release workflow exposes `component=all | cli | npm:<framework>`
accordingly; base-runtime npm has no standalone option on purpose.
