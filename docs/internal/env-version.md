# Host env and lxapp channel

> **Audience**: contributors changing host env, package-id suffixing, or
> publish. App authors start at
> [`docs/skill/app/project.md#environment`](../skill/app/project.md#environment).
> Update signing and `minRuntime` live in [update-pipeline.md](update-pipeline.md).

Two independent axes decide which server a host talks to and which lxapp
packages it runs:

| Axis | Values | Meaning |
| --- | --- | --- |
| Host **env** | `dev` \| `prod` | Build-time property of the host: server, app-link hosts, `.dev` suffix + badge, publish token, self-update feed. **No channel.** |
| Lxapp **channel** | `release` \| `draft` | Publish line of an lxapp package inside an env, plus that line's rules. |

Invariants:

- `prod` is a host env, `release` is an lxapp channel, `--release` is a
  compiler profile. The three never alias each other, and each axis accepts
  only the spellings above.
- The build env is immutable. A `prod` build may switch its *service* env at
  run time; package id, signing and entitlements stay on the build env.
- Installs and data are keyed by channel and service env, so packages and login
  state never cross between dev and prod services.

There is no host `preview` env. Testers publish the **draft** channel
(same-version overwrite via checksum) or use a `dev` env for a staging server.

## Build env

YAML only overrides server and app-link hosts. Switching env must not rewrite
any git-tracked file. A server URL is optional: an env without one builds an
app with no service server. Omitting App Links for an env leaves that build
without App Links.

| `lingxia.yaml` | `--env dev` | `--env prod` |
| --- | --- | --- |
| no server | `.dev`, no server | none, no server |
| `lingxiaServer: "X"` | `.dev`, server=X | none, server=X |
| `lingxiaServer: {dev:A, prod:B}` | `.dev`, server=A | none, server=B |
| `lingxiaServer: {dev:A}` | `.dev`, server=A | none, no server |
| `appLinks.hosts: [H]` | hosts=`[H]` | hosts=`[H]` |
| `appLinks.hosts: {dev:[D], prod:[R]}` | hosts=`[D]` | hosts=`[R]` |

`app.json` emits `env` (`dev` \| `prod`) as the build env; a missing `env` is
`prod`. It also emits `lingxiaServers` when the YAML configured one or both
URLs, so a prod build can switch its service env at run time. `lingxiaServer`
stays the build-selected default.

## Runtime service env

Owned by `lingxia-host-config` (core state) and `lingxia-logic` (JS surface).

A `prod` build can switch its service env between configured `dev` and `prod`
servers; a `dev` build cannot switch. Bootstrap reads
`app_state/service-env.json` before initializing services; a missing or invalid
override falls back to the build env.

`lx.host.serviceEnv.set(env)` (control app only) persists the target and
requests exit; the new server and in-app App Link hosts take effect on the next
launch, because providers are built once at startup and are never hot-swapped.
A save failure throws, leaves the running env unchanged and requests no exit.
If exit fails, the result carries `exitError` and its state still reports the
saved target as `nextLaunchEnv`, so the user can restart manually. Setting the
env already running is a no-op.

### Data keying

Install dirs, install/download records, userdata, usercache, temp,
`lx.storage`, plugin installs and cached registry answers are all keyed by
channel and service env, home lxapp included. A dev-service package (possibly
unsigned) therefore never runs under the prod service, and the two services
never share login state. Uninstall acts on the running env's records; the
orphan sweep keeps every env's referenced installs.

### Signatures

Lxapps and plugins from the dev service may be unsigned on every channel,
including on a prod build switched to dev and including the home lxapp.
Packages from the prod service and host self-updates keep the build env's
signature requirements.

### Env marks

Package id, host signing, the installed icon, self-update server and signed
App Link entitlement stay on the build env. A `dev` build gets the icon D mark.

A `prod` build using the `dev` service shows a touch-through `DEV` mark above
its own content: a narrow vertical tab centered on the left screen edge on
phones, or a chip at the bottom-end of each app window on desktop. It stays
above native sheets and guest lxapps without intercepting input.

A process whose `app.json` has `devWsUrl` uses the same mark to show the port
it dials. The phone tab shows only the port; the desktop chip shows the port,
and `DEV` plus the port when the service mark is also up. Runner hosts never
show this mark, even with `devWsUrl` configured. Runner identity is registered
before SDK initialization (or supplied by `LINGXIA_RUNNER`); hiding the mark
does not disable the dev connection.

## Channel

The default lxapp channel is `release` in every service env. Opening an lxapp
and App Links select another with `channel=`; draft opens and publishes need an
explicit selector. The client does **not** forbid opening `draft` on a prod
host.

## CLI

`--env` is not a channel alias. `lingxia build` / `dev` / `package` take
`--env` only (`build`/`dev` default to `dev`, `package` to `prod`) and do not
accept `--channel`. `lingxia publish` splits the two axes: `--env` selects the
upload server and token; `--channel` selects the lxapp/plugin line (default
`release`). Host-app publish reads `env` from the packaged `app.json`.
