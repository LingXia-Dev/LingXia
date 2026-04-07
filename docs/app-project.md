# App Project

A LingXia app is a native shell (Host App) that embeds one home lxapp and can open other lxapps at runtime.

For lxapp page development, see [LxApp Development Guide](./lxapp-guide.md).
For quick onboarding, see [Getting Started](./getting-started.md).
For CLI commands, see [CLI Command Reference](./cli.md).

---

## Create a Host App

```bash
lingxia new my-app -t native-app -p android,ios --package-id com.example.myapp -y
```

This scaffolds a host app project with platform directories and an embedded home lxapp (`lingxia-showcase/`). The `-t native-app` flag creates a host app — not a standalone lxapp.

To create a standalone lxapp instead, use `-t lxapp` (see [LxApp Development Guide](./lxapp-guide.md)).

---

## Project Layout

```text
my-app/
├── lingxia.config.json          # build-time project config
├── android/                     # optional
├── ios/                         # optional
├── macos/                       # optional
├── harmony/                     # optional
└── lingxia-showcase/             # embedded home lxapp source (see LxApp Guide)
```

- `lingxia.config.json` — the single source of truth for host build metadata. CLI reads it and generates the runtime `app.json`.
- `lingxia-showcase/` — an lxapp project embedded in the host. See [LxApp Development Guide](./lxapp-guide.md) for how to write pages inside it.
- Platform directories (`android/`, `ios/`, etc.) — native project scaffolding, selected by `app.platforms`.

---

## `lingxia.config.json` Reference

### Minimal Example

```json
{
  "app": {
    "projectName": "myapp",
    "productName": "My App",
    "productVersion": "1.0.0",
    "platforms": ["android"],
    "homeLxAppID": "lingxia-showcase"
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

### Root Sections

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

### `app` Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `projectName` | string | Yes (native build) | Technical project identifier, used by native build tooling |
| `productName` | string | Yes | User-facing app name |
| `productVersion` | string | Yes | Host app version |
| `apiServer` | string | No | Optional API server base URL |
| `lingxiaId` | string | No | Logical app publishing ID; required when `lingxia publish --target app` |
| `platforms` | string[] | Yes | Enabled platforms (e.g. `android`, `ios`, `macos`, `harmony`) |
| `homeLxAppID` | string | No | Home lxapp appId to open by default when the host boots |
| `cacheMaxAgeDays` | number | No | Cache TTL days; `0` disables age-based cleanup |
| `cacheMaxSizeMB` | number | No | Cache size limit per lxapp cache dir; `0` disables size-based cleanup |

### Platform Sections

| Section | Key fields |
|---|---|
| `android` | `packageId`, `minSdk`, `targetSdk`, `compileSdk`, `ndkVersion`, `apiLevel` |
| `ios` | `bundleId`, `deploymentTarget`, `swiftVersion`, `targetName` |
| `macos` | `bundleId`, `deploymentTarget`, `executableName`, `targetName` |
| `harmony` | `bundleName`, `compatibleSdkVersion`, `targetSdkVersion` |

### `splash` Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `path` | string | Yes | Path to the splash PNG image, relative to the project root. Non-PNG files are rejected by CLI. |
| `timeout` | number | Yes | Minimum splash display duration in milliseconds. The splash hides only after content is ready and this timeout has elapsed. |

### Notes

- `homeLxAppVersion` is not configured here. CLI derives it from home lxapp build metadata and writes it into runtime `app.json`.
- Missing cache fields are allowed. Runtime defaults are applied from `app.json` parsing.
- When `splash` is configured, CLI requires a PNG source image and copies it into each native host as `splash.png`.
- CLI also writes `splash.timeout` into runtime `app.json` as `splashTimeout`.
- `panels` is passed through by CLI and validated by runtime schema (e.g. unique panel id, one panel per position).

---

## Build

- `lingxia build` in the host project builds home lxapp assets and native platform resources.
- Runtime `app.json` is generated from `lingxia.config.json` + built home lxapp metadata.
- Do not edit `app.json` directly — it is regenerated on every build.

---

## Common Pitfalls

- Editing generated runtime `app.json` directly instead of `lingxia.config.json`.
- Confusing `projectName` (technical) with `productName` (display name).
- Setting `homeLxAppVersion` in `lingxia.config.json` (it is generated).
