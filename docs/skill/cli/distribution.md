# Distribution — publish, signing, stores, accounts

The low-frequency half of the `lingxia` CLI: getting a built app out the door.
Publish to the LingXia server, platform signing setup, OS app-store submission,
and the developer-account plumbing behind them. Daily commands (build, dev,
install, …) live in [`lingxia.md`](./lingxia.md).

## `lingxia publish`

Publish a package to the **LingXia server** (not an OS app store — that's
`store`). Auto-detects what it's publishing from the project marker file
(`lxapp.json` → lxapp, `lingxia.yaml` → host app) and reads the id/version from
it. An lxapp publish packages the current project first. `--env` (`dev` |
`prod`) selects the upload server and token; `--channel`
(`release` | `draft`) selects the lxapp line. Omitting
`--env` defaults to `dev`, which implies channel `draft`. `--env prod`
implies channel `release` unless `--channel` overrides it. Only host-app
publish accepts a prebuilt package path; it does not take `--channel` (env
is read from the packaged `app.json`). An lxapp or plugin belongs to one
product: pass `--lingxia-id`, or run from a host project whose `lingxia.yaml`
has `app.lingxiaId`. Authenticates with a bearer token:
the `--token` flag, `LINGXIA_PUBLISH_TOKEN`, or the LingXia credential wallet.

See `lingxia publish --help` for the flags.

**Update signatures:**

The CLI signs the update envelope. Do not hand-build `signed` / `signatures`
when publishing an app — pass a key file and let `lingxia publish` do it.

Follows `--env`: `dev` may be unsigned; `prod` requires
`--update-signing-key-file` (or `LINGXIA_UPDATE_SIGNING_KEY_FILE`), including
`--channel draft`.

The key file is one line: base64url (no `=`) of a 32-byte Ed25519 seed, mode
`0600` or `0400`. The matching public key is that seed's 32-byte verify key
in the same encoding. Put 1 or 2 of those under host `update.trustedPublicKeys`.
`update.channel` / `update.platforms` choose `direct` (self-install) vs
`store` per platform — see [host `update`](../app/project.md#update). Direct
prod updates require trusted keys; store updates use only a version and optional
release notes and do not need keys. `lingxia publish` uploads packages, while
`lingxia store` handles OS store submission.

```bash
umask 077
node --input-type=module -e '
import { generateKeyPairSync } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
const { privateKey } = generateKeyPairSync("ed25519");
const { d: seed, x: pub } = privateKey.export({ format: "jwk" });
const path = join(homedir(), ".lingxia", "update.key");
mkdirSync(join(homedir(), ".lingxia"), { recursive: true });
writeFileSync(path, seed + "\n", { mode: 0o600 });
console.log("public:", pub);
'
```

Paste the printed `public:` value into `update.trustedPublicKeys`, then:

```bash
lingxia publish --env prod --update-signing-key-file ~/.lingxia/update.key
```

The envelope is one compact JSON manifest plus 1–2 Ed25519 signatures **over
those JSON bytes** (not over the base64). `signatures` is an array so a key
rotation can carry old and new; verify is OR against the embedded public keys.
The CLI emits one signature today — do not add a second unless you are
rotating. Host packages sign `channel: ""`.

Verification follows the **host build's** `env`, never the requested channel.
A `prod` build requires a trusted signature even for a `draft` lxapp opened
with `lx.navigateToApp({ channel: 'draft' })`.

**Publish tokens (wallet):**

Store the token once with `lingxia auth login lingxia --env prod --token …`;
it is keyed by the canonical server URL + env, so the project's server and
`--env` pick the right token automatically. CI sets `LINGXIA_PUBLISH_TOKEN`
instead.

**Machine-wide server default (`~/.lingxia/cli/config.toml`):**

Set a per-user server default so lxapp projects (which have no `lingxia.yaml`)
need not pass `--lingxia-server` on every publish. The flag (and project
`app.lingxiaServer`) take precedence. The value follows the same shape as
`app.lingxiaServer` in `lingxia.yaml`: a scalar applies to every env, an
env-keyed map is explicit per env with no fallback for envs it omits. The file
is CLI-managed — `lingxia auth login lingxia --server …` writes it, hand
comments are lost.

```toml
[publish.lingxiaServer]
dev = "http://localhost:8080"
prod = "https://prod.example.com"
```

## App signing

Each platform signs differently; this is what the CLI needs to produce a
distributable build. One principle everywhere: **sign with the configured
credentials when present, otherwise fall back to a safe default** (ad-hoc on
Apple, the debug keystore on Android) so a build always succeeds — but only a
properly signed build is distributable. Credentials are stored by
`lingxia auth` under `~/.lingxia/` (mode `0600`); environment variables
override the stored files, which is the CI path.

| Platform | Model | What you provide |
|---|---|---|
| macOS | Developer ID + notarization | App Store Connect API key + Developer ID Application certificate |
| iOS | Provisioning profile + distribution certificate | via your Apple Developer account / Xcode |
| Android | Self-managed keystore | a release keystore (`keytool`) |
| Windows | Self-signed (or your own) MSIX | `--self-signed`, or a real code-signing cert |
| Harmony | AGC certificate + provisioning profile | AGC Connect API client credentials; the CLI manages signing material |

### macOS (Developer ID + notarization)

Two independent credentials, which **must belong to the same team**:

| Credential | Used for | Stored by |
|---|---|---|
| App Store Connect API key | `notarytool submit` notarization | `lingxia auth login apple --mode key` |
| Developer ID Application certificate + key | `codesign` signing | login keychain, or `lingxia auth login apple --mode developer-id` |

```bash
lingxia auth login apple --mode key \
  --key-id <KEY_ID> --issuer-id <ISSUER_ID> \
  --private-key-path AuthKey_XXXX.p8 --team-id <TEAM_ID>

lingxia auth login apple --mode developer-id --p12 DeveloperID.p12   # optional
# locally — keychain discovery finds an existing Developer ID identity by itself
```

To export a `.p12`: Xcode → Settings → Accounts → Manage Certificates → **+** →
Developer ID Application; then in Keychain Access select the certificate *and*
its private key and export as `.p12`. No private key under the certificate →
it was created on another Mac; recreate or export it there.

**CI:** either restore the two wallet files
(`~/.lingxia/credentials/apple/<team>/asc.json` and `…/developer-id.json`)
from secrets before building, or set the env groups
`LINGXIA_APPLE_KEY_PATH` / `_KEY_ID` / `_ISSUER_ID` (each group must be
complete) and `LINGXIA_APPLE_DEVELOPER_ID_P12` / `_P12_PASSWORD` /
`_IDENTITY`. Without resolvable credentials the build ad-hoc signs and still
succeeds.

**Verify:** `codesign --verify --deep --strict "MyApp.app"`,
`spctl --assess --type execute "MyApp.app"`, `xcrun stapler validate "MyApp.app"`.

### iOS

Distribution signing uses a **provisioning profile** plus a **distribution
certificate** from your Apple Developer account, applied at build time. Store
the account credential with `lingxia auth login apple`; manage profiles and the
certificate through your Apple Developer account / Xcode.

App Store builds reuse their distribution signing material across CI jobs;
embedded extensions need matching profiles and capabilities.

### Android

Self-managed: generate a keystore once and keep it for the life of the app
(updates must use the same key):

```bash
keytool -genkeypair -v -keystore release.jks -storetype PKCS12 \
  -alias upload -keyalg RSA -keysize 2048 -validity 10000
```

The generated Gradle build reads `RELEASE_STORE_FILE` / `RELEASE_STORE_PASSWORD`
/ `RELEASE_KEY_ALIAS` / `RELEASE_KEY_PASSWORD` from
**`android/keystore.properties`** (git-ignored, local) first, then **env vars
of the same names** (CI). All four present → release-signed; otherwise the
build falls back to the debug keystore (installs for testing, not
store-distributable). `RELEASE_STORE_FILE` is relative to `android/`.

**Distribution formats:** sideload and Chinese app stores take the APK signed
with your key (`--dist sideload`, the default); **Google Play** takes an
**AAB** signed with this same keystore as the *upload key* (`--dist play`) and
re-signs with the app signing key it holds.

**Verify:** `apksigner verify --print-certs <apk>`.

### Windows / Harmony

`lingxia build --platform windows --msix --self-signed` signs an MSIX with a
generated self-signed cert (trusted locally) — enough to install and test;
store distribution needs a real code-signing certificate. Harmony builds resolve
AGC credentials and manage the signing key, certificate, and profile; release
builds request release signing material.

## `lingxia auth`

The credential wallet behind signing and developer services. Log in once per
provider; commands pick the right credential automatically from the project:

- `lingxia auth login apple|harmony` — add or refresh credentials (Apple modes:
  `key`, `password`, `developer-id`)
- `lingxia auth login/logout lingxia` — add or remove a LingXia Server publish
  token; `lingxia publish` consumes it
- `lingxia auth logout apple|harmony` — remove them
- `lingxia auth status [--json]` — per-project diagnosis plus the wallet view
- `lingxia auth forget --platform <channel>` — drop this checkout's automatic
  credential selection so the next command re-resolves

The concrete flows are in [App signing](#app-signing) above; see
`lingxia auth login <provider> --help` for flags.

## `lingxia store`

Upload a built installable to an **OS app store**. Talks to stores only — never
the LingXia server (that's `publish`) and never builds (run `build`/`package`
first; `submit` consumes the staged `dist/<platform>/` and fails clearly if it's
missing). `lingxia package` writes store artifacts: Harmony `.app` (not a raw
HAP), iOS App Store–signed `.ipa`. The CLI does not submit for review; do that
in the store console.
The artifact's real bundle/package identity is checked against the
platform block in `lingxia.yaml` before any credential or network use, so a
dev-suffixed or wrong-app artifact fails immediately. Credentials come from
the wallet (`lingxia auth login googleplay|xiaomi|oppo|honor|msstore`, Apple
and Harmony reuse their `auth login` credentials); each provider's
`LINGXIA_<PROVIDER>_*` env group overrides the wallet for CI — complete groups
only, a partial group is an error. Store-record settings (numeric app ids,
default track) live under the platform blocks in `lingxia.yaml`.

Store-specific release notes and channels remain managed by the store flow.

### CI

Persist signing keys/certificates separately from API credentials. Transfer the
packaged artifact with its matching project config between jobs.

For Apple/Harmony, gate subsequent steps on processing completion. After a
timeout, resume querying the same submission instead of uploading again.
Processing completion does not mean review approval.

## `lingxia ds`

Query **developer services** read-only. `lingxia ds apple` lists Apple Developer
resources (teams, certificates, bundle identifiers, registered devices,
provisioning profiles); `lingxia ds harmony` covers Harmony developer services.
Requires the matching `lingxia auth` credentials.

See `lingxia ds apple --help` / `lingxia ds harmony --help`.
