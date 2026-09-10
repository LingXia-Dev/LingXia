# LingXia AppLinks

LingXia AppLinks are verified HTTPS URLs that open the host app. The host app
declares which **domains** it accepts — that is the only gate. Every path on a
configured host is delivered to the home lxapp's Logic as `scene: 8003`, with
the original URL attached, and Logic decides what it means.

Your own product URLs work as-is. A password reset link, a console deep link, a
marketing landing page: if its host is configured, it reaches Logic unchanged —
no `/lxapp/` prefix, no native rewriting.

Inbound links reach Logic from:

- OS App Links or Universal Links
- browser handoff into the app
- push notification links

QR and barcode scans are the exception: `scanCode` auto-opens **only** the
`/lxapp/` namespace. A product URL that merely shares a configured host is
returned to the caller as the scan result and nothing else, so pointing the
scanner at a poster cannot take over the app.

## Product URLs

Any HTTPS URL on a configured host:

```text
https://app.example.com/app/auth/reset-password?code=…&email=…
```

reaches the home lxapp as:

```json
{
  "url": "https://app.example.com/app/auth/reset-password?code=…&email=…",
  "query": { "code": "…", "email": "…" },
  "scene": 8003
}
```

`url` is the link exactly as the OS delivered it, fragment included. `query` is
the same query as an object, for convenience. Nothing is consumed or rewritten:
a `path` or `appId` parameter of your own stays in `query`, an odd percent
escape is passed through rather than rejected, and the link always opens the
**home** lxapp.

```ts
App({
  onLaunch: routeFromAppLink, // cold
  onShow: routeFromAppLink,   // warm; cold onShow has no 8003
});

function routeFromAppLink(options?: { scene?: number; url?: string }) {
  if (options?.scene !== 8003 || !options.url) return;
  const { pathname, searchParams } = new URL(options.url);
  // Route from an allowlist. `url` is attacker-reachable: it can come from a
  // scanned code, a push payload, or any email.
  if (pathname === '/app/auth/reset-password') {
    void lx.navigateTo({ page: 'reset', query: { code: searchParams.get('code') ?? '' } });
  }
}
```

`scene === 8003` is delivered once per tap: cold → `onLaunch`, warm → `onShow`.
Put the same handler on both.

## The `/lxapp/` namespace

Product URLs always open home. To target a **specific** lxapp, page, or release
channel from a link, use the reserved `/lxapp/` namespace:

```text
https://<host>/lxapp/open?appId=<appId>&path=<pagePath>&envVersion=<release|preview|developer>&<pageQuery>
```

Examples:

```text
https://app.example.com/lxapp/open?appId=shop
https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&id=42
https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&envVersion=preview&id=42
```

Only in this namespace are routing parameters consumed, and only here is a
malformed URL rejected. Do not mint product links under `/lxapp/`.

### Routing Parameters

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
        "paths": ["*"]
      }
    ]
  }
}
```

`paths` is an OS-level filter and Apple applies it before the app is ever
started: a path outside `paths` never reaches the SDK. Narrow it only to paths
you genuinely want to open the app — `["/lxapp/*", "/app/*"]`, say. Android
intent filters and Harmony skills match on host alone and need no equivalent.

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

When the SDK receives a link:

1. Accepts only `https://`.
2. Checks the host against the hosts baked into this build's `app.json`. This is
   the only gate — a non-matching host is ignored, everything else is delivered.
3. `/lxapp/*` only: parses the routing query, resolves `envVersion`, and ensures
   the requested lxapp release is installed and compatible. A malformed
   `/lxapp/*` URL is rejected here.
4. Opens the target with `scene: 8003` and the original `url`. Any other path
   opens home at its initial page.
5. `scene === 8003` once per tap: cold `onLaunch`, warm `onShow`. Put the same
   handler on both.

`url` and `query` are untrusted input — they can originate from a scanned code,
a push payload, or any email. Route from an allowlist of paths you recognize;
never feed a query value straight into `navigateTo` as a page name.

### Taking links over natively

The host app can handle inbound links itself instead — for a link that must land
on a native screen outside every lxapp. There is no API for this: simply do not
hand the link to the SDK.

| Platform | How |
|---|---|
| iOS | Drop `Lingxia.handleAppLink(url:)` from `.onOpenURL` |
| macOS | Drop it from `application(_:continue:)` |
| Android | `Lingxia.quickStart(this, deliverAppLinks = false)` |
| HarmonyOS | Override `deliverAppLinks()` in your ability to return `false` |

Handling part of the URL natively and delivering the rest is the same move: do
your work, then call the SDK entry point for the links you want in Logic.

## Testing

Warm path (`lingxia dev` / Runner):

```bash
lxdev app applink "https://app.example.com/app/auth/reset-password?code=abc"
```

Use the real product URL — the SDK sees exactly what the OS would deliver.
Product host must match `appLinks.hosts`. Runner has no hosts (no
`lingxia.yaml`); any URL is accepted, so host rejection can only be verified in
a product build. Does not simulate cold start.

Android:

```bash
adb shell am start -a android.intent.action.VIEW \
  -d "https://app.example.com/app/auth/reset-password?code=abc" \
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
- Apple `paths` in the AASA covers every product path that should open the app.
- Logic handles `scene === 8003` in both `onLaunch` and `onShow`, routing from an allowlist.
- `/lxapp/open` is reserved for links that target a specific lxapp, page, or channel.
