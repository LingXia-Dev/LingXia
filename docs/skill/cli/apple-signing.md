# Apple signing and notarization

This page covers the Apple credentials needed for macOS Developer ID signing and
notarization when building a LingXia app.

## What the CLI needs

macOS builds intended for distribution need two independent credentials:

| Credential | Used for | Stored by |
|---|---|---|
| App Store Connect API key | `notarytool submit` notarization | `lingxia auth apple login` |
| Developer ID Application certificate + private key | `codesign` signing | Keychain Access or `lingxia auth apple import-developer-id` |

Create the notary credential with:

```bash
lingxia auth apple login --mode key \
  --key-id <KEY_ID> --issuer-id <ISSUER_ID> \
  --private-key-path AuthKey_XXXX.p8 --team-id <TEAM_ID>
```

Create the Developer ID credential with:

```bash
lingxia auth apple import-developer-id ~/Desktop/DeveloperID.p12
```

On a local Mac, importing the `.p12` into LingXia is optional if the same
Developer ID Application identity is already available in your login keychain.
The `.p12` flow is useful when you want LingXia to use a specific exported
certificate instead of relying on keychain discovery.

## Get a Developer ID `.p12` with Xcode

Xcode can create the Developer ID Application certificate without opening a
project:

1. Open Xcode.
2. Go to **Xcode > Settings > Accounts**.
3. Select your Apple ID and Team, then click **Manage Certificates...**.
4. Click **+** and create **Developer ID Application**.
5. Open **Keychain Access**.
6. Select the `login` keychain and **My Certificates**.
7. Find `Developer ID Application: ...`.
8. Expand it and confirm a private key appears under the certificate.
9. Select both the certificate and private key.
10. Right-click and export them as **Personal Information Exchange (.p12)**.
11. Import the exported file with `lingxia auth apple import-developer-id`.

If Xcode does not show **Developer ID Application**, check that the selected
Team is an Apple Developer Program team and that your account has permission to
create Developer ID certificates.

If Keychain Access cannot export `.p12`, the private key is missing. Create the
certificate again from Xcode on this Mac, or export it from the Mac that created
the certificate request.

## Verify a built app

After building or packaging a macOS app:

```bash
codesign --verify --deep --strict --verbose=2 "MyApp.app"
spctl --assess --type execute --verbose=4 "MyApp.app"
xcrun stapler validate "MyApp.app"
```
