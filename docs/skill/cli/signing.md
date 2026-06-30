# App signing

Each platform signs apps differently. This page covers what the LingXia CLI
needs to produce a distributable, signed build on each one.

| Platform | Model | What you provide |
|---|---|---|
| macOS | Developer ID + notarization | App Store Connect API key + Developer ID Application certificate |
| iOS | Provisioning profile + distribution certificate | via your Apple Developer account / Xcode |
| Android | Self-managed keystore | a release keystore (`keytool`) |
| Windows | Self-signed (or your own) MSIX | `--self-signed`, or a real code-signing cert |
| Harmony | Local keystore | nothing — the CLI generates one |

Across platforms the CLI follows the same principle: **sign with the configured
credentials when present, otherwise fall back to a safe default** (ad-hoc on
Apple, the debug keystore on Android) so a build always succeeds — but only a
properly signed build is distributable.

---

## Apple

### macOS (Developer ID + notarization)

macOS builds intended for distribution need two independent credentials:

| Credential | Used for | Stored by |
|---|---|---|
| App Store Connect API key | `notarytool submit` notarization | `lingxia auth apple login` |
| Developer ID Application certificate + private key | `codesign` signing | Keychain Access or `lingxia auth apple import-developer-id` |

Create the notary credential:

```bash
lingxia auth apple login --mode key \
  --key-id <KEY_ID> --issuer-id <ISSUER_ID> \
  --private-key-path AuthKey_XXXX.p8 --team-id <TEAM_ID>
```

Create the Developer ID credential:

```bash
lingxia auth apple import-developer-id ~/Desktop/DeveloperID.p12
```

On a local Mac, importing the `.p12` is optional if the same Developer ID
Application identity is already in your login keychain. The `.p12` flow is for
when you want LingXia to use a specific exported certificate instead of relying
on keychain discovery.

> The notary account and the signing certificate **must belong to the same
> team** — a Developer ID cert from one team will not notarize under another.

#### Get a Developer ID `.p12` with Xcode

1. Open Xcode → **Settings → Accounts**.
2. Select your Apple ID and Team, then **Manage Certificates...**.
3. Click **+** and create **Developer ID Application**.
4. Open **Keychain Access**, select the `login` keychain → **My Certificates**.
5. Find `Developer ID Application: ...`, expand it, and confirm a private key
   appears under the certificate.
6. Select both the certificate and the private key, right-click, and export as
   **Personal Information Exchange (.p12)** (use a plain alphanumeric password).
7. Import it with `lingxia auth apple import-developer-id`.

If **Developer ID Application** is missing, check the Team is an Apple Developer
Program team with permission to create Developer ID certificates. If Keychain
Access cannot export `.p12`, the private key is missing — recreate the
certificate on this Mac, or export it from the Mac that created it.

#### CI

Provide the two credential stores as secrets and restore them before building
(decode base64 into `~/.lingxia/apple/`):

| Secret | Source |
|---|---|
| `APPLE_CREDENTIALS_JSON_BASE64` | `base64 -i ~/.lingxia/apple/credentials.json` |
| `APPLE_DEVELOPER_ID_JSON_BASE64` | `base64 -i ~/.lingxia/apple/developer-id.json` |

The secret names above are **arbitrary** — they're just whatever you name them in
your CI provider, restored to disk before the build. They are **not** env vars
the CLI reads. The actual env overrides the build honors are
`LINGXIA_APPLE_NOTARY_KEY` / `_KEY_ID` / `_ISSUER_ID` (notarization) and
`LINGXIA_APPLE_DEVELOPER_ID_P12` / `_P12_PASSWORD` / `_IDENTITY` (signing); set
those instead if you'd rather not write files. Either way, without resolvable
credentials the build leaves the app ad-hoc signed and still succeeds.

#### Verify a built macOS app

```bash
codesign --verify --deep --strict --verbose=2 "MyApp.app"
spctl --assess --type execute --verbose=4 "MyApp.app"
xcrun stapler validate "MyApp.app"
```

### iOS

iOS distribution signing uses a **provisioning profile** plus a **distribution
certificate** from your Apple Developer account, applied at build time. Store
the account credential with `lingxia auth apple login`; manage profiles and the
distribution certificate through your Apple Developer account / Xcode.

---

## Android

Android signing is **self-managed**: you generate your own keystore with
`keytool` — there is no certificate authority and no notarization.

The generated `app/build.gradle.kts` reads four values from
**`android/keystore.properties`** (local) first, then **environment variables**
of the same name (CI). When all four resolve, `release` signs with the keystore;
otherwise it **falls back to the debug keystore** (a debug-signed APK installs
for testing but **cannot** go to Google Play or app stores).

| Value | Meaning |
|---|---|
| `RELEASE_STORE_FILE` | path to the keystore, relative to `android/` |
| `RELEASE_STORE_PASSWORD` | keystore password |
| `RELEASE_KEY_ALIAS` | key alias inside the keystore |
| `RELEASE_KEY_PASSWORD` | password for that key |

Create a keystore (keep it for the life of the app — updates must use the same
key):

```bash
keytool -genkeypair -v \
  -keystore release.jks -storetype PKCS12 \
  -alias upload -keyalg RSA -keysize 2048 -validity 10000
```

Configure it in `android/keystore.properties` (git-ignored):

```properties
RELEASE_STORE_FILE=release.jks
RELEASE_STORE_PASSWORD=your-store-password
RELEASE_KEY_ALIAS=upload
RELEASE_KEY_PASSWORD=your-key-password
```

Then build a signed release APK:

```bash
lingxia build --platform android --release
```

For **CI**, set the four `RELEASE_*` names as environment variables (decode a
base64 keystore to a file and point `RELEASE_STORE_FILE` at it).

**Distribution.** Sideload and Chinese app stores (Xiaomi, Huawei, OPPO, vivo,
Tencent MyApp, …) take the **APK signed with your own key** — what the default
`--dist sideload` produces. **Google Play** uses Play App Signing: you upload an
**AAB** signed with an *upload key* (this same keystore works) and Google
re-signs with the app signing key it holds. `lingxia build` / `lingxia package`
emit the AAB with `--dist play` (vs the APK-producing `--dist sideload`); see
`lingxia build --help` for specifics.

#### Verify a built APK

```bash
apksigner verify --print-certs app/build/outputs/apk/release/app-release.apk
keytool -printcert -jarfile app-release.apk
```

---

## Windows

`lingxia build --platform windows --msix --self-signed` packages an MSIX and
signs it with a generated/reused self-signed certificate (and trusts it
locally) — enough to install and test. Store distribution requires a real
code-signing certificate; without `--self-signed` the MSIX is left unsigned.

## Harmony

The CLI builds a local PKCS#12 keystore automatically, so Harmony builds sign
without extra setup for local install and testing. Publishing to AppGallery
requires Huawei's own signing material applied through the Harmony tooling.
