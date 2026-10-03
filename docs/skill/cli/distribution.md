# Distribution

Getting a built app out: publish to the LingXia server, platform signing, OS
app stores, and developer accounts. Flags: `lingxia <cmd> --help`.

Packages land in `dist/<platform>/` as `<projectName>-<productVersion>`, with
`-dev` for a `--env dev` build (`my-app-1.2.0-dev.apk`); a rebuild replaces
the previous one.

## `lingxia publish`

Uploads a package to the LingXia server (OS stores are `lingxia store`).

- Detects the project from `lxapp.json` (lxapp; packaged first) or
  `lingxia.yaml` (host app) and reads id and version from it.
- `--env dev|prod` picks server and token (default `dev`). `--channel
  release|draft` picks the lxapp line (`dev` → `draft`, `prod` → `release`).
- A host publish takes a prebuilt package path and no `--channel`.
- Token: `--token`, `LINGXIA_PUBLISH_TOKEN`, or the wallet
  (`lingxia auth login lingxia --env prod --token …`, keyed by server + env).

### Update signing keys

`prod` publishes need `--update-signing-key-file` (or
`LINGXIA_UPDATE_SIGNING_KEY_FILE`), draft channel included; `dev` may be
unsigned. A prod host switched to the dev service accepts unsigned draft
lxapps/plugins; prod-service packages, release packages, and host updates
still require signatures. The CLI signs; never hand-build `signed` /
`signatures`.

The key file is one line: base64url (no `=`) of a 32-byte Ed25519 seed, mode
`0600` or `0400`. Put 1 or 2 matching public keys under
[`update.trustedPublicKeys`](../app/project.md#update); store updates need none.

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

### Server default

Lxapp projects have no `lingxia.yaml`; set a per-user server with
`lingxia auth login lingxia --server …`, which writes
`~/.lingxia/cli/config.toml` (same shape as `app.lingxiaServer`). The
`--lingxia-server` flag and `app.lingxiaServer` win.

```toml
[publish.lingxiaServer]
dev = "http://localhost:8080"
prod = "https://prod.example.com"
```

## App signing

Configured credentials sign; without them the build still succeeds with a safe
default (ad-hoc on Apple, the debug keystore on Android) that is not
distributable. `lingxia auth` stores credentials under `~/.lingxia/`;
environment variables override them.

| Platform | Model | What you provide |
|---|---|---|
| macOS | Developer ID + notarization | App Store Connect API key + Developer ID Application certificate |
| iOS | Provisioning profile + distribution certificate | via your Apple Developer account / Xcode |
| Android | Self-managed keystore | a release keystore (`keytool`) |
| Windows | Authenticode, or self-signed MSIX | a code-signing cert, or `--self-signed` |
| Harmony | AGC certificate + provisioning profile | AGC Connect API client credentials |

### macOS

Two credentials from the same team:

| Credential | Used for | Stored by |
|---|---|---|
| App Store Connect API key | notarization | `lingxia auth login apple --mode key` |
| Developer ID Application certificate + key | code signing | login keychain, or `lingxia auth login apple --mode developer-id` |

```bash
lingxia auth login apple --mode key \
  --key-id <KEY_ID> --issuer-id <ISSUER_ID> \
  --private-key-path AuthKey_XXXX.p8 --team-id <TEAM_ID>

lingxia auth login apple --mode developer-id --p12 DeveloperID.p12   # optional
```

A `.p12` must include the certificate's private key (export both from Keychain
Access). Env overrides, each group complete: `LINGXIA_APPLE_KEY_PATH` /
`_KEY_ID` / `_ISSUER_ID`, and `LINGXIA_APPLE_DEVELOPER_ID_P12` /
`_P12_PASSWORD` / `_IDENTITY`.

### iOS

A provisioning profile plus a distribution certificate from your Apple
Developer account; store the account with `lingxia auth login apple`. Embedded
extensions need matching profiles and capabilities.

### Android

Generate a keystore once and keep it for the app's life:

```bash
keytool -genkeypair -v -keystore release.jks -storetype PKCS12 \
  -alias upload -keyalg RSA -keysize 2048 -validity 10000
```

The Gradle build reads `RELEASE_STORE_FILE` (relative to `android/`) /
`RELEASE_STORE_PASSWORD` / `RELEASE_KEY_ALIAS` / `RELEASE_KEY_PASSWORD` from
`android/keystore.properties` (git-ignored), then from env vars of the same
names. All four → release-signed; otherwise debug-signed.

`--dist sideload` (default) builds an APK for sideloading and Chinese stores;
`--dist play` builds an AAB for Google Play, with this keystore as upload key.

### Windows

- Formats: NSIS Setup (default), `--format portable`, `--format msix`. Build
  every direct format you ship together (`--format nsis,portable,zip`) and
  publish `*-windows.zip` for direct updates. MSIX updates through the Store or
  App Installer.
- The icon is the committed `windows/AppIcon.ico`; after changing the app icon
  run `lingxia icon <AppIcon.png> --platform windows`.
- Authenticode: a certificate in the current user's store and
  `LINGXIA_WINDOWS_CERT_SHA1` (its thumbprint). `LINGXIA_WINDOWS_REQUIRE_SIGNING=1`
  makes missing credentials fatal; `LINGXIA_SIGNTOOL` and
  `LINGXIA_WINDOWS_TIMESTAMP_URL` are optional. MSIX `windows.publisher` must
  match the certificate subject.
- Unsigned MSIX cannot be installed normally; use `--msix --self-signed` for
  local tests.
- The first install downloads WebView2 when it is missing.

### Harmony

The CLI resolves AGC credentials and manages the signing key, certificate, and
profile.

## `lingxia auth`

The credential wallet behind signing and developer services:

- `lingxia auth login apple|harmony` — add or refresh (Apple modes `key`,
  `password`, `developer-id`)
- `lingxia auth login|logout lingxia` — a LingXia server publish token
- `lingxia auth logout apple|harmony`
- `lingxia auth status [--json]` — per-project diagnosis and the wallet
- `lingxia auth forget --platform <channel>` — re-resolve this checkout's
  credential selection
- `lingxia auth runner [set <LINGXIA_ID> --dev <URL> [--prod <URL>] | clear]`
  — the cloud identity the Runner signs in with

## `lingxia store`

Uploads a built installable to an OS app store; it never builds and never
submits for review.

- Run `lingxia package` first; `submit` reads `dist/<platform>/` (Harmony
  `.app`, iOS App Store–signed `.ipa`).
- The artifact's identity is checked against `lingxia.yaml` first, so a
  dev-suffixed or wrong artifact fails immediately.
- Credentials: `lingxia auth login googleplay|xiaomi|oppo|honor|msstore`;
  Apple and Harmony reuse theirs. `LINGXIA_<PROVIDER>_*` env groups override
  (complete groups only).
- Store records (numeric app ids, default track) live in the platform blocks
  of `lingxia.yaml`.
- Processing completion is not review approval.

## `lingxia ds`

Read-only developer-service queries: `lingxia ds apple` (teams, certificates,
bundle ids, devices, profiles) and `lingxia ds harmony`. Needs the matching
`lingxia auth` credentials.
