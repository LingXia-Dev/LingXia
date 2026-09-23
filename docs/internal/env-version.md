# Host env and lxapp channel

> **Audience**: contributors changing host env, package-id suffixing, or
> publish. App authors start at
> [`docs/skill/app/project.md#environment`](../skill/app/project.md#environment).
> Update signing and `minRuntime` live in [update-pipeline.md](update-pipeline.md).

These used to share the same three names. They do not.

| Axis | Values | Meaning |
| --- | --- | --- |
| Host **env** | `dev` \| `prod` | Build-time property of the host: server, app-link hosts, `.dev` suffix + badge, publish token, self-update feed. **No channel.** |
| Lxapp **channel** | `release` \| `draft` | Publish line of an lxapp package inside an env, plus that line's rules. |

`prod` is the host env. `release` is an lxapp channel. `--release` is a
compiler profile. Those three must stay distinct.

There is no host `preview` env. Testers publish the **draft** channel
(same-version overwrite via checksum) or use a `dev` env for a staging
server.

No compatibility aliases: `developer`/`preview`/`release` are not env names,
`envVersion` is not a field, `developer`/`develop`/`preview` are not channel
spellings.

YAML only overrides server and app-link hosts. Switching env must not
rewrite any git-tracked file. A build requires a server URL for its env;
omitting App Links for an env leaves that build without App Links.

| `lingxia.yaml` | `--env dev` | `--env prod` |
| --- | --- | --- |
| no server | build error | build error |
| `lingxiaServer: "X"` | `.dev`, server=X | none, server=X |
| `lingxiaServer: {dev:A, prod:B}` | `.dev`, server=A | none, server=B |
| `lingxiaServer: {dev:A}` | `.dev`, server=A | build error |
| `appLinks.hosts: [H]` | hosts=`[H]` | hosts=`[H]` |
| `appLinks.hosts: {dev:[D], prod:[R]}` | hosts=`[D]` | hosts=`[R]` |

`app.json` emits `env` (`dev` \| `prod`) as the **immutable build** env.
Missing `env` is `prod`. It also emits `lingxiaServers` when the YAML
configured one or both URLs, so a prod build can switch the **service** env
at runtime. `lingxiaServer` stays the build-selected default. Opening an
lxapp and App Links take `channel=` (not `envVersion=`). Omit it to use the
host env's default (`dev` → `draft`, `prod` → `release`). The client does
**not** forbid opening `draft` on a prod host.

## Runtime service environment

The build env is immutable. A `prod` build can switch its service env between
configured `dev` and `prod` servers; a `dev` build cannot switch. Bootstrap
reads `app_state/service-env.json` before initializing services. A missing or
invalid override falls back to the build env. `lx.host.toggleServiceEnv()`
persists the other env and requests exit; the new server and in-app App Link
hosts take effect on the next launch. Save failure leaves the running env
unchanged and does not request exit. If exit fails, `nextLaunchEnv` still
reports the saved target so the user can restart manually. Only the control
app may call this API.

Package id, signing, the installed icon, self-update server, and signed App
Link entitlement stay on the build env. A `dev` build gets the icon D mark.
A `prod` build using the `dev` service shows a touch-through `DEV` mark above
its own content: a narrow vertical tab centered on the left screen edge on
phones, or a chip at the bottom-end of each app window on desktop. It must
remain above native sheets and guest lxapps without intercepting input.

`--env` is not a channel alias. `--channel` is not accepted on
`lingxia build` / `dev` / `package`. `lingxia publish` splits the two axes:
`--env` selects the upload server and token; `--channel` selects the
lxapp/plugin line. Host-app publish reads `env` from the packaged `app.json`.
