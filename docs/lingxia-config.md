# `lingxia.config.json` Reference

This file defines host-project build metadata for LingXia CLI.

It is used by CLI build/run/publish flows, and parts of it are transformed into runtime `app.json`.

## Minimal Example

```json
{
  "app": {
    "projectName": "myapp",
    "productName": "My App",
    "productVersion": "1.0.0",
    "platforms": ["android"],
    "homeLxAppID": "homelxapp"
  },
  "splash": {
    "path": "path/to/splash.png",
    "timeout": 1500
  },
  "android": {
    "packageId": "com.example.myapp"
  }
}
```

## Root Sections

| Section | Type | Required | Description |
|---|---|---|---|
| `app` | object | Yes (host build) | Core host metadata |
| `android` | object | No | Android platform config |
| `ios` | object | No | iOS platform config |
| `macos` | object | No | macOS platform config |
| `harmony` | object | No | Harmony platform config |
| `resources` | object | No | Resource/runtime overrides |
| `splash` | object | No | Host splash-screen asset and minimum display duration |
| `panels` | object | No | Panel configuration (passed through to runtime `app.json`) |

## `app` Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `projectName` | string | Yes (native build) | Technical project identifier, used by native build tooling |
| `productName` | string | Yes | User-facing app name |
| `productVersion` | string | Yes | Host app version |
| `apiServer` | string | No | Cloud API base URL |
| `lingxiaId` | string | No | Logical app/device-cloud ID; required when `lingxia publish --target app` |
| `platforms` | string[] | Yes | Enabled platforms (for example `android`, `ios`, `macos`, `harmony`) |
| `homeLxAppID` | string | Yes | Home lxapp appId |
| `cacheMaxAgeDays` | number | No | Cache TTL days; `0` disables age-based cleanup |
| `cacheMaxSizeMB` | number | No | Cache size limit per lxapp cache dir; `0` disables size-based cleanup |

## Platform Sections

| Section | Key fields |
|---|---|
| `android` | `packageId`, `minSdk`, `targetSdk`, `compileSdk`, `ndkVersion`, `apiLevel` |
| `ios` | `bundleId`, `deploymentTarget`, `swiftVersion`, `targetName` |
| `macos` | `bundleId`, `deploymentTarget`, `executableName`, `targetName` |
| `harmony` | `bundleName`, `compatibleSdkVersion`, `targetSdkVersion` |

## `splash` Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `path` | string | Yes | Path to the splash PNG image, relative to the project root. Non-PNG files are rejected by CLI. |
| `timeout` | number | Yes | Minimum splash display duration in milliseconds. The splash hides only after content is ready and this timeout has elapsed. |

## Notes

- `homeLxAppVersion` is not configured here. CLI derives it from home lxapp build metadata and writes it into runtime `app.json`.
- Missing cache fields are allowed. Runtime defaults are applied from `app.json` parsing.
- When `splash` is configured, CLI requires a PNG source image and copies it into each native host as `splash.png`.
- CLI also writes `splash.timeout` into runtime `app.json` as `splashTimeout`.
- `panels` is passed through by CLI and validated by runtime schema (for example unique panel id, one panel per position).
