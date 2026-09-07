# LingXia AppLinks

LingXia AppLinks are verified HTTPS URLs that open the host app and route to a
specific lxapp page. The host app declares which domains it accepts; the URL
path and query define the action to run.

The same URL rule is used everywhere the SDK receives a link:

- OS App Links or Universal Links
- browser handoff into the app
- push notification links
- QR code and barcode scan results

## URL Structure

A LingXia AppLink is an HTTPS URL split into `scheme`, `host`, `path`, and
`query`:

```text
https://<host>/lxapp/open?appId=<appId>&path=<pagePath>&envVersion=<release|preview|developer>&<pageQuery>
```

| Field | Value | Description |
|---|---|---|
| `scheme` | `https` | Required. Platform verified links are HTTPS URLs. |
| `host` | A host from `lingxia.yaml` `appLinks.hosts` for this build's env | The OS uses this host to verify and deliver the link to the app. |
| `path` | `/lxapp/open` | `lxapp` is the LingXia namespace. `open` is the action that opens a target lxapp. |
| `query` | `appId`, `path`, `envVersion`, plus page params | Routing params select the target. Other params are forwarded to the page. |

Examples:

```text
https://app.example.com/lxapp/open?appId=shop
https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&id=42
https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&envVersion=preview&id=42
```

The explicit `open` action keeps the `/lxapp` namespace extensible for future
actions.

## Routing Parameters

| Parameter | Required | Description |
|---|---:|---|
| `appId` | No | Target lxapp. Omitted → home. |
| `path` | No | Target page path. Omitted → current/initial page; Logic routes from `query`. |
| `envVersion` | No | Target release channel. Matches `navigateToApp`. Omitted, the host build's own channel (its `envVersion`) is used — a developer build opens the target from the developer channel, not release. |

All query keys and values should be URL encoded. Routing parameters are consumed
by the SDK and are not forwarded to the page. Other query parameters are
forwarded to the target page.

Release channel mapping:

| Link value | Runtime release type |
|---|---|
| `envVersion=release` | `release` |
| `envVersion=preview` | `preview` |
| `envVersion=developer` | `developer` |

`develop` is accepted as the pre-0.13 spelling of `developer`. No other aliases;
invalid `envVersion` values are rejected.

Example:

```text
https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&envVersion=preview&id=42
```

opens:

```text
appId: shop
release: preview
path: pages/detail/index.html
page query: id=42
scene: 8003
```

Home routes from query. Omit `appId`/`path`; put the page name in query:

```text
https://app.example.com/lxapp/open?page=order&id=42
```

```ts
function routeFromAppLink(options?: { scene?: number; query?: Record<string, string> }) {
  if (options?.scene !== 8003) return;
  const page = options.query?.page;
  if (page) void lx.navigateTo({ page, query: { id: options.query?.id } });
}

App({
  onLaunch: routeFromAppLink, // cold
  onShow: routeFromAppLink,   // warm; cold onShow has no 8003
});
```

`scene === 8003` is delivered once per tap: cold → `onLaunch`, warm → `onShow`.

## Host Configuration

Hosts only, not routing. A list applies to every env; a map (same shape as
`app.lingxiaServer`) is per env. Omit an env → that build has no App Links.
Empty for the active env → ignored.

```yaml
appLinks:
  hosts:
    - app.example.com
# hosts:
#   developer: [app-dev.example.com]
#   preview: [app-preview.example.com]
#   release: [app.example.com]
```

`lingxia build --env` writes that env's hosts into `app.json` and platform
association files. Share URLs use the first host of the running build.
`lingxia new -t native-app` leaves this off. Envs use different package ids
(`.dev` / `.preview`); each host's `.well-known` file should list the matching
id.

## Well-Known Verification Files

Every configured host must serve the platform verification files needed by the
platforms you ship. For `app.example.com`, serve these from the same HTTPS host:

```text
https://app.example.com/.well-known/apple-app-site-association
https://app.example.com/.well-known/assetlinks.json
https://app.example.com/.well-known/applinking.json
```

Only serve the files required by your target platforms.

### Apple

Entitlement:

```xml
<key>com.apple.developer.associated-domains</key>
<array>
    <string>applinks:app.example.com</string>
</array>
```

Verification URL:

```text
https://app.example.com/.well-known/apple-app-site-association
```

Example response:

```json
{
  "applinks": {
    "apps": [],
    "details": [
      {
        "appID": "TEAM_ID.com.example.app",
        "paths": ["/lxapp/*"]
      }
    ]
  }
}
```

Requirements:

- No `.json` suffix in the URL.
- No redirects.
- Public HTTPS.
- `appID` is Apple Team ID plus bundle id.

### Android

Intent filter:

```xml
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="https" android:host="app.example.com" />
</intent-filter>
```

Verification URL:

```text
https://app.example.com/.well-known/assetlinks.json
```

Example response:

```json
[
  {
    "relation": ["delegate_permission/common.handle_all_urls"],
    "target": {
      "namespace": "android_app",
      "package_name": "com.example.app",
      "sha256_cert_fingerprints": [
        "SHA256_CERT_FINGERPRINT"
      ]
    }
  }
]
```

Include every signing certificate used by debug, internal, and release builds.

### HarmonyOS

Browsable skill:

```json5
{
  "entities": ["entity.system.browsable"],
  "actions": ["ohos.want.action.viewData"],
  "uris": [
    {
      "scheme": "https",
      "host": "app.example.com"
    }
  ]
}
```

Verification URL:

```text
https://app.example.com/.well-known/applinking.json
```

Example response:

```json
{
  "applinking": {
    "apps": [
      {
        "appIdentifier": "HARMONY_APP_ID"
      }
    ]
  }
}
```

## Runtime Behavior

When the SDK receives a link, it routes the URL through the shared AppLink
handler.

The handler:

1. Accepts only `https://`.
2. Checks the host against the hosts baked into this build's `app.json`.
3. Parses `/lxapp/open` and its query parameters.
4. Resolves `envVersion`.
5. Ensures the requested lxapp release is installed and compatible.
6. Opens the target (`scene: 8003`). No `appId` → home. No `path` → current/initial page.
7. `scene === 8003` once per tap: cold `onLaunch`, warm `onShow`. Put the same `scene === 8003` handler on both.

Unknown paths are ignored. Bad encoding is rejected. Missing `appId` opens home.

When `scanCode` sees a supported AppLink, the SDK forwards it to the shared
handler and closes the scanner. The scan result is still returned to the caller,
so existing `scanCode` callers do not hang.

## Testing

Warm path (`lingxia dev` / Runner):

```bash
lxdev app applink "https://app.example.com/lxapp/open?page=order&id=42"
```

Product host must match `appLinks.hosts`. Runner has no hosts (no `lingxia.yaml`); any AppLink URL is accepted. Does not simulate cold start.

Android:

```bash
adb shell am start -a android.intent.action.VIEW \
  -d "https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&envVersion=preview&id=42" \
  com.example.app

adb shell pm get-app-links com.example.app
```

iOS/macOS:

```bash
curl https://app.example.com/.well-known/apple-app-site-association
curl https://app-site-association.cdn-apple.com/a/v1/app.example.com
```

HarmonyOS:

```bash
hdc shell aa start -A ohos.want.action.viewData \
  -U "https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&envVersion=developer&id=42"
```

## Checklist

- `lingxia.yaml` has every production host under `appLinks.hosts` (a list, or a per-env map matching `app.lingxiaServer`).
- Each host serves the required `.well-known` verification files.
- Apple entitlements use `applinks:<host>`.
- Android manifest has verified HTTPS intent filters for each host.
- Harmony module skill has HTTPS URI entries for each host.
- AppLink URLs use `/lxapp/open`; `appId` optional (defaults to home).
- Page parameters are URL encoded and do not rely on `envVersion` being forwarded.
