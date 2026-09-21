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
rewrite any git-tracked file. Per-env maps omit an env to give that build
no server / no App Links.

| `lingxia.yaml` | `--env dev` | `--env prod` |
| --- | --- | --- |
| no fields | `.dev`, server="" | none, server="" |
| `lingxiaServer: "X"` | `.dev`, server=X | none, server=X |
| `lingxiaServer: {dev:A, prod:B}` | `.dev`, server=A | none, server=B |
| `appLinks.hosts: [H]` | hosts=`[H]` | hosts=`[H]` |
| `appLinks.hosts: {dev:[D], prod:[R]}` | hosts=`[D]` | hosts=`[R]` |

`app.json` emits `env` (`dev` \| `prod`). Missing `env` is `prod`. Opening an
lxapp and App Links take `channel=` (not `envVersion=`). Omit it to use the
host env's default (`dev` → `draft`, `prod` → `release`). The client does
**not** forbid opening `draft` on a prod host.

`--env` is not a channel alias. `--channel` is not accepted on
`lingxia build` / `dev` / `package`. `lingxia publish` splits the two axes:
`--env` selects the upload server and token; `--channel` selects the
lxapp/plugin line. Host-app publish reads `env` from the packaged `app.json`.
