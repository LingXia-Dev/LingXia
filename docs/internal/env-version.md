# Host env and lxapp channel

> **Audience**: contributors working on `tools/lingxia-cli`, `crates/lingxia-*`,
> or platform builders. If you're **building an app on LingXia**, the
> user-facing surface is documented in the skill at
> [`docs/skill/app/project.md#environment`](../skill/app/project.md#environment)
> — start there. This doc covers the two axes, build-time injection,
> resolution, publish, and per-platform plumbing that the skill omits.

## Two axes

These used to share the same three names. They do not.

| Axis | Values | Meaning |
| --- | --- | --- |
| Host **env** | `dev` \| `prod` | Build-time property of the host: `lingxiaServer`, `appLinks.hosts`, `packageIdSuffix` + badge, publish token, and which server the host self-updates from. **No channel.** |
| Lxapp **channel** | `release` \| `preview` \| `draft` | Publish line of an lxapp package inside an env, plus that line's rules. Fingermark directories include this value. |

`prod` is the host env. `release` is an lxapp channel. `--release` is a
compiler profile. Those three must stay distinct.

There is no host `preview` env. Testing uses the **prod** env's **preview**
channel, the way TestFlight talks to the production backend. A staging
server is a `dev` env.

No compatibility aliases: `developer`/`preview`/`release` are not env names,
`envVersion` is not a field, `developer`/`develop` are not channel spellings.

## Mental model

Env is a **build-time property** with built-in defaults. Every host build is
`dev` or `prod`. YAML only carries optional overrides.

| Env | Built-in `packageIdSuffix` | Launcher icon | Default `lingxia build/dev` | Default `lingxia package` |
| --- | --- | --- | --- | --- |
| `dev` | `.dev` | red `D` badge | ✓ | |
| `prod` | (none) | unmodified | | ✓ |

Different envs of the same app install **side by side** because their
bundle/package ids differ. No git-tracked file changes when you switch envs.

Default lxapp channel is derived from env and can be overridden per open:

| Host env | Default channel |
| --- | --- |
| `dev` | `draft` |
| `prod` | `release` |

The client does **not** forbid opening `draft` on a prod host. Registry
authorization is per channel.

Channel rules (already true of the registry / update path):

- `release` and `preview`: version only goes up.
- `draft`: same version may be overwritten; the client uses sha256 to
  decide whether to update.

## Schema in `lingxia.yaml`

```yaml
app:
  lingxiaServer: https://api.myapp.com
  # lingxiaServer:
  #   dev: http://192.168.1.10:8080
  #   prod: https://api.myapp.com

  # packageIdSuffix:
  #   dev: .internal
  #   prod: ""

appLinks:
  hosts: [app.example.com]
  # hosts:
  #   dev: [app-dev.example.com]
  #   prod: [app.example.com]
```

| Field | Type | Notes |
| --- | --- | --- |
| `app.lingxiaServer` | `string` \| `{dev?, prod?}` | Omit entirely for server-less apps. |
| `app.packageIdSuffix` | `{dev?, prod?}` | Absent → built-in default, `""` → opt out, `"<x>"` → use that. |
| `appLinks.hosts` | `[string]` \| `{dev?, prod?}` | Omit an env to give that build no App Links. |

`deny_unknown_fields` means typos (and leftover `developer`/`preview`/`release`
keys) surface as parse errors.

## CLI

`lingxia build`, `lingxia dev`, and `lingxia package` accept:

```
--env <dev|prod>
```

`--env` is not a channel alias. `--channel` is not accepted on these commands.

| Command | Default env |
| --- | --- |
| `lingxia build` | `dev` |
| `lingxia dev` | `dev` |
| `lingxia package` | `prod` |

`--release` (compiler profile) is independent of `--env`. A shippable host
build is `lingxia build --env prod --release` or `lingxia package`.

`lingxia publish` splits the two axes:

| Flag | Selects |
| --- | --- |
| `--env dev\|prod` | Upload server and publish token |
| `--channel release\|preview\|draft` | Lxapp/lxplugin package line |

Defaults: `--env` omitted → `dev`; `--channel` omitted → derived from env
(`dev` → `draft`, `prod` → `release`). Host-app publish does not take
`--channel`; env is read from the packaged `app.json`.

Wallet tokens are keyed by `(canonical server URL, env)`.

```
lingxia auth login lingxia --env prod --token …
lingxia publish --env prod --channel preview   # TestFlight-like
```

## Resolution

`LingXiaConfig::resolve_env` in `tools/lingxia-cli/src/config.rs`:

| `lingxia.yaml` | `--env dev` | `--env prod` |
| --- | --- | --- |
| no fields | `.dev`, server="" | none, server="" |
| `lingxiaServer: "X"` | `.dev`, server=X | none, server=X |
| `lingxiaServer: {dev:A, prod:B}` | `.dev`, server=A | none, server=B |
| `packageIdSuffix: {dev: ""}` | **none** (opt-out) | none |
| `packageIdSuffix: {dev: ".d"}` | `.d` | none |
| `appLinks.hosts: [H]` | hosts=`[H]` | hosts=`[H]` |
| `appLinks.hosts: {dev:[D], prod:[R]}` | hosts=`[D]` | hosts=`[R]` |

## Runtime

`app.json` emits `env` (`dev` \| `prod`). Missing `env` is `prod`.

```json
{
  "env": "dev",
  "lingxiaServer": "http://192.168.1.10:8080"
}
```

JS: `lx.app.env` — `'dev' | 'prod'`, type `HostAppEnv`.
Rust: `lingxia::app::env()` returns `AppEnv`.

Opening an lxapp takes `channel` (not `envVersion`). App Links use
`channel=` (not `envVersion=`). Omit it to use `default_channel()` from the
host env.

Host self-update does not send a channel. Lxapp update still does. The signed
host-update manifest uses an empty channel; lxapp/plugin manifests bind their
requested channel. Publishing requires a key in `prod`, including `draft`.
Runtime verification follows the host env: `prod` requires signatures and skips
checks without trusted keys; `dev` can query and accept unsigned updates.

## File map

| Concern | File |
| --- | --- |
| YAML schema + validation + resolution | `tools/lingxia-cli/src/config.rs` |
| App Link hosts per env | `tools/lingxia-cli/src/config.rs::AppLinkHosts` |
| `--env` CLI flag | `tools/lingxia-cli/src/main.rs::BuildOptions` |
| Resolve env per invocation | `tools/lingxia-cli/src/commands/build.rs::resolve_build_env` |
| `app.json` emission | `tools/lingxia-cli/src/assets/json.rs::build_app_json_from_config` |
| Android suffix + Gradle properties | `tools/lingxia-cli/src/platform/android.rs::build_gradle` |
| Android launcher-icon badge overlay | `tools/lingxia-cli/src/platform/android.rs::prepare_launcher_icon_overlay` |
| iOS/macOS bundle-id suffix | `tools/lingxia-cli/src/platform/{ios,macos}.rs` |
| iOS/macOS icon badge overlay | `tools/lingxia-cli/src/platform/apple/env_icon.rs` |
| Harmony staging mirror | `tools/lingxia-cli/src/platform/harmony/build.rs::prepare_harmony_staging` |
| Publish reads package `env` | `tools/lingxia-cli/src/commands/publish.rs::read_app_package_metadata` |
| Runtime `AppEnv` | `crates/lingxia-app-context/src/lib.rs` |
| Default lxapp channel | `crates/lingxia-update/src/lib.rs::default_channel` |
| `lx.app.env` JS binding | `crates/lingxia-logic/src/app.rs` |
| TS type metadata | `crates/lingxia-logic/src/public_types.rs::HostAppEnv` |
| App Link `channel=` | `crates/lingxia-service/src/applink.rs` |
