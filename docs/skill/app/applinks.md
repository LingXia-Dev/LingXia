# App links

Verified HTTPS URLs that open the host app. The host declares which domains it
accepts; that is the only gate. Only `https://` is accepted, and every path on a
configured host reaches the home lxapp's Logic as `scene: 8003` with the
original URL. Logic decides what it means.

Links arrive from OS App Links / Universal Links, browser handoff, push links,
and local notifications posted with
`lx.host.notification.show({ target: { kind: 'appLink', url } })`. QR scans are
the exception: `scanCode` auto-opens only the `/lxapp/` namespace; any other URL
is just returned as the scan result.

## Product URLs

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

`url` is the link exactly as delivered, fragment included; `query` is its query
as an object. Nothing is consumed or rewritten, and the link always opens home.

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
Never feed a query value straight into `navigateTo` as a page name.

## The `/lxapp/` namespace

To target a specific lxapp, page, or channel, use the reserved namespace:

```text
https://<host>/lxapp/open?appId=<appId>&path=<pagePath>&channel=<release|draft>&<pageQuery>
https://app.example.com/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&channel=draft&id=42
```

| Parameter | Required | Description |
|---|---:|---|
| `appId` | No | Target lxapp. Omitted → home. |
| `path` | No | Target page path. Omitted → current/initial page. |
| `channel` | No | `release` or `draft`, as in `navigateToApp`. Omitted → the host env's default (`dev` → `draft`, `prod` → `release`). |

URL-encode keys and values. Routing parameters are consumed; other parameters
go to the page as its query. Only in this namespace is a malformed URL rejected.
Do not mint product links under `/lxapp/`.

## `appLinks.hosts`

```yaml
appLinks:
  hosts:
    - app.example.com
# hosts:
#   dev: [app-dev.example.com]
#   prod: [app.example.com]
```

A list applies to every env; a map (same shape as `app.lingxiaServer`) is per
env. Omit an env → that build has no App Links. `lingxia build --env` writes
that env's hosts into `app.json` and the platform association files. The
scaffold leaves App Links off. Each env has its own package id (`.dev` suffix
on `dev`), so each host's `.well-known` file lists the matching id. The OS
association is fixed at install; a prod build that switches its service env
uses the new env's hosts for share URLs and in-app checks on the next launch.

Do not put a store URL in `appLinks.hosts`: those hosts open this app.

## Well-known files

Serve from each configured host, over public HTTPS, with no redirects, only the
files your platforms need:

```text
https://app.example.com/.well-known/apple-app-site-association
https://app.example.com/.well-known/assetlinks.json
https://app.example.com/.well-known/applinking.json
```

### Apple

```xml
<key>com.apple.developer.associated-domains</key>
<array>
    <string>applinks:app.example.com</string>
</array>
```

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

- No `.json` suffix on the URL; `appID` is Team ID plus bundle id.
- `paths` filters before the app starts: a path outside it never reaches the
  SDK. Cover every product path that should open the app.

### Android

```xml
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <data android:scheme="https" android:host="app.example.com" />
</intent-filter>
```

The launcher activity is `singleTop` and forwards `onNewIntent`:

```xml
<activity android:name=".MainActivity" android:exported="true" android:launchMode="singleTop">
```

```kotlin
override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    Lingxia.handleAppLink(intent)
}
```

Never `singleTask`: it destroys `LxAppActivity` on each launcher tap.

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

List every signing certificate (debug, internal, release).

### HarmonyOS

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

## Native takeover

To land a link on a native screen, do not hand it to the SDK:

| Platform | How |
|---|---|
| iOS | Drop `Lingxia.handleAppLink(url:)` from `.onOpenURL` |
| macOS | Drop it from `application(_:continue:)` |
| Android | `Lingxia.quickStart(this, deliverAppLinks = false)` |
| HarmonyOS | Override `deliverAppLinks()` in your ability to return `false` |

To split, handle what you need and call the SDK entry point for the rest.

## Testing

```bash
lxdev host applink "https://app.example.com/app/auth/reset-password?code=abc"
```

Warm path only, with the real product URL. The Runner has no configured hosts
and accepts any URL, so host rejection shows only in a product build. Verify
association files with each platform's own tools.
