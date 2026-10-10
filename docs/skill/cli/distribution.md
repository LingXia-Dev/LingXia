# Distribution

Publishing, signing, OS stores, and developer credentials.
Flags: `lingxia <cmd> --help`.

Packages land in `dist/<platform>/`; names include the project, version, and
`-dev` for dev builds, plus platform-specific format/architecture suffixes.
Rebuilding replaces matching artifacts.

## `lingxia publish`

Uploads a package to the LingXia server (OS stores are `lingxia store`).

- An lxapp or lxplugin publish builds the current project, then reads id and
  version from `lxapp.json` or `lxplugin.json`.
- A host publish does not build. It uploads the package `lingxia package`
  already wrote: Android `.apk`, macOS `*-macos.zip`, Windows `*-windows.zip`.
  `lingxiaId`, `productVersion`, and `env` come from that package's `app.json`.
  `lingxia.yaml` finds the project and the artifact; it does not set the
  published version, and an invalid `productVersion` there does not block
  publish. No `--channel`.
- `--env dev|prod` picks server and token for an lxapp or lxplugin (default
  `dev`). `--channel release|draft` picks the lxapp line (default `release`).
- Token: `--token`, `LINGXIA_PUBLISH_TOKEN`, or the wallet
  (`lingxia auth login lingxia --env prod --token …`, keyed by server + env).

Draft lxapp publishes (`--channel draft`) print an HTTPS `/lxapp/open` URL
on the publish server origin. Scan it with `lx.scanCode()` in a host using that
server. Interactive terminals show a QR code (PNG on supported image terminals,
text otherwise); redirected output keeps only the URL. HTTP servers cannot
produce this HTTPS scan trigger. The link selects the draft channel, not a
fixed package revision; opening preserves the normal update lifecycle.

### Update signing keys

`prod` publishes need `--update-signing-key` (or
`LINGXIA_UPDATE_SIGNING_KEY`), draft channel included; `dev` may be
unsigned. A prod host switched to the dev service does not fetch
lxapps/plugins from that service; prod-service packages and host
updates still require signatures. The CLI signs; never hand-build `signed` /
`signatures`.

The value is one base64url (no `=`) 32-byte Ed25519 seed. Prefer the
environment variable so the seed stays out of shell history. Put 1 or 2
matching public keys under
[`update.trustedPublicKeys`](../app/project.md#update); store updates need none.

```bash
node --input-type=module -e '
import { generateKeyPairSync } from "node:crypto";
const { privateKey } = generateKeyPairSync("ed25519");
const { d: seed, x: pub } = privateKey.export({ format: "jwk" });
console.log("seed:", seed);
console.log("public:", pub);
'
```

Paste `public:` into `update.trustedPublicKeys`. Pass `seed:` as
`LINGXIA_UPDATE_SIGNING_KEY`.

```bash
LINGXIA_UPDATE_SIGNING_KEY=<seed> lingxia publish --env prod
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

`lingxia auth` stores credentials under `~/.lingxia/`; environment variables
override them. Development signing (Apple ad-hoc, Android debug keystore)
does not satisfy store distribution requirements.

| Platform | Model | What you provide |
|---|---|---|
| macOS | Developer ID + notarization | App Store Connect API key + Developer ID Application certificate |
| iOS | Provisioning profile + distribution certificate | via your Apple Developer account / Xcode |
| Android | Self-managed keystore | a release keystore (`keytool`) |
| Windows | Authenticode, or self-signed MSIX | a code-signing cert, or `--self-signed` |
| Harmony | AGC certificate + provisioning profile | AGC Connect API client credentials |

### macOS

Direct distribution needs two credentials from the same team:

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

- Formats: `nsis` (default Setup EXE), `portable`, `zip` (portable ZIP), `msix`.
  Use `lingxia package --format nsis,msix` to build release formats together.
  Direct formats also produce `*-windows.zip` with update metadata and the
  installers built in that run, so build every direct format you ship
  together. Publish that archive unchanged for direct updates. MSIX uses the
  Store/App Installer.
- The icon is the committed `windows/AppIcon.ico`; after changing the app icon
  run `lingxia icon <AppIcon.png> --platform windows`.
- Authenticode: a certificate in the current user's store and
  `LINGXIA_WINDOWS_CERT_SHA1` (its thumbprint). `LINGXIA_WINDOWS_REQUIRE_SIGNING=1`
  makes missing credentials fatal; `LINGXIA_SIGNTOOL` and
  `LINGXIA_WINDOWS_TIMESTAMP_URL` are optional. MSIX `windows.publisher` must
  match the certificate subject.
- Localized MSIX names need Windows SDK `makepri.exe` (beside `makeappx.exe`,
  or `LINGXIA_MAKEPRI`); without it the MSIX shows `productName` in every language.
- For local MSIX installation, use `--format msix --self-signed`. Store uploads
  can be unsigned; Microsoft signs distribution packages.
- The first install downloads WebView2 when it is missing.

### Harmony

The CLI resolves AGC credentials and manages the signing key, certificate, and
profile.

## `lingxia auth`

Signing, publishing, and store credentials:

- `lingxia auth login|logout apple|harmony|googleplay|xiaomi|oppo|honor|msstore`
  — add, refresh, or remove credentials (Apple modes `key`, `password`,
  `developer-id`)
- `lingxia auth login|logout lingxia` — a LingXia server publish token
- `lingxia auth status [--json]` — per-project diagnosis and the wallet
- `lingxia auth forget --platform <channel>` — re-resolve this checkout's
  credential selection
- `lingxia auth runner [set <LINGXIA_ID> --dev <URL> [--prod <URL>] | clear]`
  — the cloud identity the Runner signs in with

## `lingxia store`

`submit` uploads; `status` queries. Build/package first: artifacts come from
`dist/<platform>/` (Harmony `.app`, iOS App Store–signed `.ipa`). Store records
and default tracks live in the platform blocks of `lingxia.yaml`; credentials
come from `lingxia auth` or complete provider-specific env groups.

Readable artifact identities are checked against `lingxia.yaml` before login;
unsupported formats print a note. The CLI never submits for review or invites
testers. Processing completion is not review approval.

### Microsoft Store

- Partner Center: create the app and complete an initial submission with age
  ratings. Associate an Entra application and grant it the Manager role.
- `lingxia auth login msstore`: Tenant ID, Client ID, Client Secret. Login saves
  credentials; submit/status verify access. CI uses `LINGXIA_MSSTORE_TENANT`,
  `LINGXIA_MSSTORE_CLIENT_ID`, and `LINGXIA_MSSTORE_CLIENT_SECRET` together.
- Set the [Windows identities](../app/project.md#macos-and-windows) from
  Partner Center. The CLI uploads a ZIP containing the package. Existing pending
  submissions block uploads; finish or remove them in Partner Center.
- Windows has no `--wait` / `--json` support yet.

```bash
lingxia package --platform windows --env prod --format msix
lingxia store submit --platform windows
```

### Apple

The same upload appears in App Store and TestFlight after processing;
omit `--track`. Configure export compliance and tester groups in App Store
Connect. External testing may need beta review.

```bash
lingxia store submit --platform ios --wait --json
```

### Harmony

AppTest uploads to the test area, waits for package parsing, then
creates and binds an invitation-test draft. `--release-notes` supplies its
description (1–50 characters); `--test-version-id <id>` reuses an existing
draft. Configure testers and submit test review in AppGallery Connect:

```bash
lingxia store submit --platform harmony --track apptest --wait --json
```

JSON includes `submission_id` and optional `test_version_id`. Omit `--track`
(or use `production`) to upload to the production draft.

## `lingxia ds`

Read-only developer-service queries: `lingxia ds apple` (teams, certificates,
bundle ids, devices, profiles) and `lingxia ds harmony`. Needs the matching
`lingxia auth` credentials.
