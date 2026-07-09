# Distribution & accounts — `publish`, `auth`, `store`, `ds`

The low-frequency half of the `lingxia` CLI: getting a built app out the door
and the developer-account plumbing behind it. Daily commands (build, dev,
install, …) live in [`lingxia.md`](./lingxia.md); signing setup lives in
[App signing](./signing.md).

## `lingxia publish`

Publish a package to the **LingXia server** (not an OS app store — that's
`store`). Auto-detects what it's publishing from the project marker file
(`lxapp.json` → lxapp, `lingxia.yaml` → host app) and reads the id/version from
it. An lxapp publish packages the current project first and defaults to the
`developer` env when `--env` is omitted; only host-app publish accepts a
prebuilt package path. Authenticates with a bearer token: the `--token` flag,
or `[publish] token` in `~/.lingxia/cli/config.toml`.

See `lingxia publish --help` for the flags.

**Machine-wide publish defaults (`~/.lingxia/cli/config.toml`):**

Set per-user defaults so lxapp projects (which have no `lingxia.yaml`) need not
pass `--token` / `--lingxia-server` on every publish. The flags (and, for the
server, project `app.lingxiaServer`) take precedence. Publish keys sit under
`[publish]` (the file holds more areas than publishing), and each value follows
the same shape as `app.lingxiaServer` in `lingxia.yaml`: a scalar applies to
every env, an env-keyed map is explicit per env with no fallback for envs it
omits. The env comes from the package's `--env`/`--channel` (`developer` when
omitted for lxapp publish; host-app publish reads the package's `app.json
envVersion`). The file is CLI-managed — `lingxia publish login` writes it, hand
comments are lost.

```toml
[publish.token]                    # map: explicit per env — each env is a
developer = "lx_dev_token"         # distinct backend with its own credentials
release = "lx_prod_token"

[publish.lingxiaServer]
developer = "http://localhost:8080"
release = "https://prod.example.com"
```

A scalar form covers the single-backend case: `token = "lx_tok"` under
`[publish]` applies to every env.

## `lingxia auth`

Store developer-account credentials so builds can sign and notarize without
interactive prompts. Credentials live under `~/.lingxia/` (mode `0600`), and
environment variables override the stored files (handy in CI). Two providers:

- **`lingxia auth apple`** — `login` stores App Store Connect API (or Apple ID)
  credentials used for **notarization**; `import-developer-id <p12>` stores a
  **Developer ID Application** certificate used for **code-signing**; plus
  `logout` / `status`. On a local Mac, importing the `.p12` is optional when the
  Developer ID identity is already in your login keychain. To create a `.p12`,
  see [App signing → Apple](./signing.md#get-a-developer-id-p12-with-xcode).
- **`lingxia auth harmony`** — `login` / `logout` / `status` for Harmony
  developer credentials.

See `lingxia auth apple --help` / `lingxia auth harmony --help` for the flags,
and [App signing](./signing.md) for how signing/notarization resolves at build
time.

## `lingxia store`

Submit a built installable to an **OS app store**. Talks to stores only — never
the LingXia server (that's `publish`) and never builds (run `build`/`package`
first; `submit` consumes the staged `dist/<platform>/` and fails clearly if it's
missing). Each platform has a `login` / `logout` / `submit` / `status` flow;
store identity lives in `lingxia.yaml` and credentials in
`~/.lingxia/store/credentials.toml`, with **env vars overriding the file** for
CI.

Run `lingxia store --help` for the current set of supported stores and per-action
flags (`--draft`, release notes, track, etc.).

## `lingxia ds`

Query **developer services** read-only. `lingxia ds apple` lists Apple Developer
resources (teams, certificates, bundle identifiers, registered devices,
provisioning profiles); `lingxia ds harmony` covers Harmony developer services.
Requires the matching `lingxia auth` credentials.

See `lingxia ds apple --help` / `lingxia ds harmony --help`.
