# Environment Version (`envVersion`)

> **Audience**: contributors working on `tools/lingxia-cli`, `crates/lingxia-*`, or platform builders. If you're **building an app on LingXia**, the user-facing surface (CLI flag, YAML schema, `lx.app.envVersion`) is documented in the skill at [`docs/skill/app/project.md#environment-versions`](../skill/app/project.md#environment-versions) — start there. This doc covers the build-time injection mechanics, resolution algorithm, validation rules, publish-flow contract, and per-platform plumbing that the skill deliberately omits.

## Purpose

This document describes how a host LingXia app declares per-environment
configuration, how the CLI resolves and injects it at build time, and how the
running app reads it through the JS and Rust runtime APIs.

The system supports three environments — `developer`, `preview`, `release` —
and lets the same source tree produce side-by-side installable variants
(distinct package IDs, distinct backend URLs, and non-release icon badges)
**without mutating any Git-tracked file**.

Scope includes:

- `tools/lingxia-cli` (config schema, `--env` flag, suffix injection)
- `crates/lingxia-app-context` (runtime `AppConfig.env_version`)
- `crates/lingxia-logic::app` (binds `lx.app.envVersion`)
- `crates/lingxia-logic/src/public_types.rs` (`HostAppEnvVersion` type metadata)
- Platform builders under `tools/lingxia-cli/src/platform/{android,apple,harmony}`

## Mental model

Env-version is a **build-time property** with built-in defaults, not an
opt-in feature. Every build is one of `{developer, preview, release}`. The
defaults already do the right thing:

| Env | Built-in `packageIdSuffix` |
| --- | --- |
| `developer` | `.dev` |
| `preview` | `.preview` |
| `release` | (none) |

YAML only carries optional **overrides**. An app with no cloud component
omits all server config; an app that wants the standard suffix names doesn't
touch `packageIdSuffix` at all.

## Schema in `lingxia.yaml`

Two optional fields on `app`, organized by purpose:

```yaml
app:
  projectName: my-app
  productName: My App
  productVersion: 1.0.0
  lingxiaId: com.example.myapp        # base package/bundle id

  # Optional. Single URL for every env:
  lingxiaServer: https://api.myapp.com
  # ...or a per-env map:
  # lingxiaServer:
  #   developer: http://192.168.1.10:8080
  #   preview: https://preview.api.myapp.com
  #   release: https://api.myapp.com

  # Optional. Override built-in suffix per env; "" opts out.
  # packageIdSuffix:
  #   developer: .internal
  #   preview: ".preview"
  #   release: ""
```

| Field | Type | Notes |
| --- | --- | --- |
| `app.lingxiaServer` | `string` \| `{developer?, preview?, release?}` | Omit entirely for server-less apps. |
| `app.packageIdSuffix` | `{developer?, preview?, release?}` | Each value: absent → built-in default, `""` → opt out, `"<x>"` → use that. |

`deny_unknown_fields` means typos (e.g. `enviroments:`) surface as parse
errors instead of being silently ignored.

## CLI

`lingxia build`, `lingxia dev`, and `lingxia package` all accept the same flag:

```
--env <developer|preview|release>       # `dev` is also accepted as `developer`
```

Default per command when `--env` is omitted:

| Command | Default env | Why |
| --- | --- | --- |
| `lingxia build` | `developer` | Day-to-day iteration on a real device. |
| `lingxia dev` | `developer` | Same as build. |
| `lingxia package` | `release` | Produces a shippable artifact. |
| `lingxia publish` | `developer` for lxapp/lxplugin | Matches dev by default; host-app publish uses the package's `app.json envVersion`. |

Build profile (`--release`) is independent of env selection. The CLI prints
a banner on every build so the active env is visible:

```
ℹ Build env: developer (default)
```

`(--env)` if the flag was passed explicitly, `(default)` otherwise.

## Resolution

Implemented by `LingXiaConfig::resolve_env` in
`tools/lingxia-cli/src/config.rs`. Single pure function — no strict/lax
branch, no "is env block configured" toggle.

```rust
pub fn resolve_env(&self, version: EnvVersion) -> Result<ResolvedEnv> {
    let app = self.app.as_ref().ok_or(...)?;

    let lingxia_server = app.lingxia_server.as_ref()
        .and_then(|cfg| cfg.for_env(version))     // Single → same URL always;
                                                  // PerEnv → entry or None
        .map(str::to_string)
        .unwrap_or_default();

    let configured = app.package_id_suffix.as_ref()
        .and_then(|over| over.for_env(version));
    let package_id_suffix = resolve_env_suffix(
        configured,
        version.default_package_id_suffix(),       // built-in fallback
    );

    Ok(ResolvedEnv { version, lingxia_server, package_id_suffix })
}
```

Behavior table (covered by unit tests in `config.rs`):

| `lingxia.yaml`                            | `--env developer`     | `--env release`       |
| ----------------------------------------- | --------------------- | --------------------- |
| no fields                                 | `.dev`, server=""     | none, server=""       |
| `lingxiaServer: "X"`                      | `.dev`, server=X      | none, server=X        |
| `lingxiaServer: {developer:A, release:B}` | `.dev`, server=A      | none, server=B        |
| `packageIdSuffix: {developer: ""}`        | **none** (opt-out)    | none                  |
| `packageIdSuffix: {developer: ".d"}`      | `.d`                  | none                  |

## Validation

Enforced by `LingXiaConfig::validate`:

| Rule | Error |
| --- | --- |
| Unknown key under `lingxiaServer` map or `packageIdSuffix` | YAML parse error from `deny_unknown_fields` |
| `lingxiaServer` (single form) is empty string | `app.lingxiaServer must not be empty` |
| `lingxiaServer` (per-env map) has all three entries unset | `app.lingxiaServer must configure at least one of developer, preview, or release` |
| `lingxiaServer.<env>` empty string | `app.lingxiaServer.<env> must not be empty` |
| `packageIdSuffix.<env>` non-empty but doesn't match `^\.[a-z0-9]+(\.[a-z0-9]+)*$` | `app.packageIdSuffix.<env> must start with '.' and use lowercase a-z 0-9 segments; use "" to opt out of the default` |

Note: missing config is never an error. `--env developer` on a yaml with no
fields produces `(.dev, server="")` and proceeds.

## Build-time injection

Every platform builder receives `BuildConfig.resolved_env` and applies
`ResolvedEnv::effective_package_id_suffix()` to identity outputs. **No
tracked source files are modified** — every effect lands in a Git-ignored
output directory or is passed via build-time properties.

### Android

`tools/lingxia-cli/src/platform/android.rs::build_gradle` invokes Gradle
with project properties:

```
-Plingxia.applicationIdSuffix=<.dev|.preview|...|empty>
-Plingxia.appName=<productName>
# When the env needs a launcher-icon badge (developer/preview):
-Plingxia.resOverlayDir=<target/lingxia/android/overlay/<env>/res>
-Plingxia.appIcon=@mipmap/ic_launcher_lingxia_env
-Plingxia.appRoundIcon=@mipmap/ic_launcher_lingxia_env_round
```

The template's `app/build.gradle.kts` consumes them via
`applicationIdSuffix`, `manifestPlaceholders["lxAppName"]`, and adds the
overlay dir to `sourceSets.main.res.srcDirs`. `AndroidManifest.xml`
references `${lxAppName}`, `${lxAppIcon}`, `${lxAppRoundIcon}`.

Non-release Android builds composite a red `D` or `P` adaptive-icon badge
into a uniquely-named drawable under `target/lingxia/android/overlay/<env>/res/` so AGP's
resource merger sees a new name (no duplicate-resource collision with the
project's own `ic_launcher.xml`).

### iOS

`tools/lingxia-cli/src/platform/ios.rs::create_app_bundle` writes the
suffixed bundle id directly into the generated `Info.plist`:

```
CFBundleIdentifier = <bundle_id> + packageIdSuffix
```

`CFBundleDisplayName` is not changed by env — only the bundle id changes.
The source `Info.plist` in the project tree is never edited.

For developer/preview env, `apple::env_icon::prepare_overlay_resources_dir`
stages a copy of `Assets.xcassets` under `target/lingxia/<platform>/overlay/<env>/Resources/`
whose `AppIcon.appiconset` PNGs are composited with a circular D/P badge
(hand-rolled 5x7 bitmap glyph + accent circle, no external deps). `actool`
runs against the staging dir instead of the source xcassets.

### macOS

Same as iOS: `tools/lingxia-cli/src/platform/macos.rs` applies the suffix
to the bundle id in the generated `.app/Contents/Info.plist`, and uses
`apple::env_icon` for the dock-icon badge.

### Harmony

Hvigor has no build-time injection point for `bundleName` — it reads
`AppScope/app.json5` directly. The Harmony builder
(`tools/lingxia-cli/src/platform/harmony/build.rs::prepare_harmony_staging`)
**mirrors the project into `target/lingxia/harmony/build/<env>/`** and operates
exclusively on the copy:

1. Recursive copy from source, excluding `.lingxia`, `oh_modules`, `build`.
2. Rewrite relative `oh-package.json5` `file:` deps in staging to source-tree
   absolute paths.
3. Rewrite `AppScope/app.json5::bundleName` in staging via a token-aware
   JSON5 scanner (`replace_json5_string_field_value`) that skips comments
   and string contents.
4. `ohpm install` and `hvigorw assembleHap` run inside staging.

`install`-time hap discovery (`auto_detect_hap` in `deploy.rs`) scans
`target/lingxia/harmony/build/*/` first, then legacy `<harmony>/.lingxia/build/*/`,
for the newest hap by mtime.

## `app.json` output

`tools/lingxia-cli/src/assets/json.rs::build_app_json_from_config` always
emits `envVersion`, the resolved server (omitted when empty), and the
suffixed `lingxiaId`:

```json
{
  "productName": "My App",
  "productVersion": "1.0.0",
  "lingxiaServer": "https://preview.api.myapp.com",
  "lingxiaId": "com.example.myapp.preview",
  "envVersion": "preview"
}
```

For pre-envVersion `app.json` artifacts, runtime parsing falls back to
`EnvVersion::Release`.

## Runtime API

### Rust

`crates/lingxia-app-context/src/lib.rs`:

```rust
pub enum EnvVersion {
    #[default]
    Release,
    Preview,
    Developer,
}

pub struct AppConfig {
    // ...
    pub env_version: EnvVersion,  // defaults to Release on missing field
}

pub fn env_version() -> EnvVersion;  // process-wide accessor
```

The enum's serde representation is lowercase (`"developer" | "preview" |
"release"`), wire-compatible with `lingxia_update::ReleaseType`.

### JavaScript

Surface generated from `crates/lingxia-logic/src/public_types.rs` and the
`HostAppApi` augmentation in `packages/lingxia-types/typegen/logic-prelude.ts`:

```ts
export type HostAppEnvVersion = 'developer' | 'preview' | 'release';

export interface HostAppApi {
  readonly envVersion: HostAppEnvVersion;
  // ...
}
```

Bound by `crates/lingxia-logic/src/app.rs::init` as a synchronous string
property on `lx.app`. Fixed at app boot; no IPC.

```ts
if (lx.app.envVersion === 'developer') {
  enableVerboseLogging();
}
```

> Not to be confused with `LxAppEnvVersion` in the navigator module
> (`develop | preview | release`), which encodes lxapp release channels
> for cross-app navigation URLs and uses the truncated `develop` form.

## Publish

`lingxia publish` reads the suffixed `lingxiaId` and `envVersion` straight
from the package's baked `app.json`
(`tools/lingxia-cli/src/commands/publish.rs::read_app_package_metadata`):

- Android `.apk` → `assets/app.json` (or `app/src/main/assets/app.json`)
- macOS `*-macos.zip` → `*.app/Contents/Resources/app.json`

The upload `id` field is set to the suffixed value from the package, not
the base id from `lingxia.yaml`. This is mandatory: the runtime queries
updates against the exact suffixed id, so server-side id must match. The
upload `channel` field carries the `envVersion`. For lxapp/lxplugin publish,
omitting `--env` defaults to `developer`; host-app publish always reads
`envVersion` from the package.

## File map

| Concern | File |
| --- | --- |
| YAML schema + validation + resolution | `tools/lingxia-cli/src/config.rs` |
| `--env` CLI flag | `tools/lingxia-cli/src/main.rs::BuildOptions` |
| Resolve env per invocation | `tools/lingxia-cli/src/commands/build.rs::resolve_build_env` |
| `app.json` emission | `tools/lingxia-cli/src/assets/json.rs::build_app_json_from_config` |
| Android suffix + Gradle properties | `tools/lingxia-cli/src/platform/android.rs::build_gradle` |
| Android launcher-icon badge overlay | `tools/lingxia-cli/src/platform/android.rs::prepare_launcher_icon_overlay` |
| iOS/macOS bundle-id suffix | `tools/lingxia-cli/src/platform/{ios,macos}.rs` |
| iOS/macOS icon badge overlay | `tools/lingxia-cli/src/platform/apple/env_icon.rs` |
| Harmony staging mirror | `tools/lingxia-cli/src/platform/harmony/build.rs::prepare_harmony_staging` |
| Harmony JSON5 bundleName rewrite | `tools/lingxia-cli/src/platform/harmony/build.rs::replace_json5_string_field_value` |
| Harmony install hap discovery | `tools/lingxia-cli/src/platform/harmony/deploy.rs::auto_detect_hap` |
| Publish reads package metadata | `tools/lingxia-cli/src/commands/publish.rs::read_app_package_metadata` |
| Runtime config struct | `crates/lingxia-app-context/src/lib.rs::AppConfig` |
| `lx.app.envVersion` JS binding | `crates/lingxia-logic/src/app.rs::init` |
| TS type metadata | `crates/lingxia-logic/src/public_types.rs::HostAppEnvVersion` |
