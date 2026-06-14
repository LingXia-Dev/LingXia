# Release versioning & npm tiers

LingXia ships several artifact families. The rust workspace version is the
**base-library version** (think WeChat mini-program base lib): the native
runtime, SDK, CLI, and the JS runtime assets embedded in the app all share it.

## Components

| Family | Where | Version |
|---|---|---|
| rust crates | crates.io | workspace version |
| SDK (apple/android/harmony) | GitHub Release | workspace version |
| CLI (`lingxia`) | GitHub Release | workspace version (may patch independently via `--component cli`) |
| npm packages | npm registry | **tiered — see below** |

## npm tiers

Not all npm packages may drift from the workspace. They split into three tiers:

### Tier 1 — base runtime (locked to the workspace version)
`@lingxia/bridge`, `@lingxia/polyfills`, `@lingxia/types`

- `bridge` and `polyfills` are **embedded into the CLI as app runtime assets**
  (`tools/lingxia-cli/build.rs` `include_bytes!`s their `dist/` output and
  **panics if the package.json version ≠ the CLI's pinned
  `package.metadata.lingxia.{bridge,polyfills}-version`**).
- `@lingxia/bridge` (JS) must speak the same bridge wire protocol (`v:2`) as the
  native `lingxia-lxapp` bridge — it is the JS half of the runtime.
- **Release only via `--component all`**, at the workspace version, together with
  the rust crates / SDK / CLI. `scripts/release/version.sh` rejects
  `--component npm:bridge|polyfills|types`.

### Tier 2 — framework libraries (major.minor tracks the workspace)
`@lingxia/page-runtime`, `@lingxia/elements`, `@lingxia/react`, `@lingxia/vue`,
`@lingxia/html`

- Imported by an lxapp and bundled into the lxapp's own dist. They speak the
  bridge protocol (via `@lingxia/bridge`), so their **major.minor must match the
  base runtime**; patch may drift.
- Internal `@lingxia/*` deps are caret ranges (`^0.x.y`). Patch-release a single
  one with `--component npm:<package>`; move major.minor with `--component all`.
- **Unchanged framework packages are not republished.** During `--component all`,
  `version.sh` skips bumping a framework/tool package whose source is identical
  to its last `lingxia-<pkg>-v*` release tag — it keeps its current version, so
  `npm.sh` sees it already published and skips it. Base-runtime packages always
  bump in lockstep. (No prior tag → bumped, the safe default.)
- Because a framework package may lag the base by a patch, scaffolds pin the
  framework dep to a **minor-floor caret** (`^M.m.0`, see
  `lxapp_scaffold.rs::framework_caret_range`) rather than the exact base version,
  so `npm install` resolves any patch within the minor.

### Tier 3 — standalone tools (independent)
`@lingxia/skill`

- Agent/CLI helper; not embedded, not protocol-bound. Version freely
  (`--component npm:skill`).

## Suggested CI release grouping

1. **Base runtime** (one version = workspace version): rust crates + SDK + CLI +
   Tier-1 npm (`bridge`, `polyfills`, `types`) — published together.
2. **Framework npm train**: Tier-2 packages, major.minor pinned to the base
   runtime, patch may ship on its own. `--component all` only re-versions the
   ones that actually changed since their last tag; the rest are skipped.
3. **Tools**: `skill`, independent.

The prepare-release workflow exposes `component=all | cli | npm:<framework|skill>`
accordingly; base-runtime npm has no standalone option on purpose.
