# LxApp Project Structure

This document explains the structure and responsibilities of LingXia host app and lxapp projects.

For quick onboarding, start with [Getting Started](./getting-started.md).
For command syntax, see [CLI Command Reference](./cli.md).

---

## Host App vs LxApp

- Host App: native shell app + runtime container.
- LxApp: lightweight application package running inside host runtime.
- A host app usually embeds one home lxapp (`app.homeLxAppID`) and can open other lxapps later.

---

## Host App Project Layout

Typical host project:

```text
my-app/
├── lingxia.config.json
├── android/             # optional, if android selected
├── ios/                 # optional, if ios selected
├── macos/               # optional, if macos selected
├── harmony/             # optional, if harmony selected
└── homelxapp/           # embedded home lxapp source
```

Key files:

- `lingxia.config.json`: Build-time project config consumed by CLI, including `app` metadata (`projectName`, `productName`, `productVersion`, `platforms`, `homeLxAppID`) and optional platform sections/panels.

Generated at build time:

- Runtime `app.json` is generated from `lingxia.config.json` + built home lxapp metadata.
- `homeLxAppVersion` is generated, not manually configured in `lingxia.config.json`.

---

## LxApp Project Layout

Typical lxapp project:

```text
my-lxapp/
├── lxapp.json
├── lxapp.config.ts
├── package.json
├── pages/
│   └── home/
│       ├── index.tsx
│       ├── index.ts
│       └── index.json
├── public/
└── shared/
```

Key files:

- `lxapp.json`: Runtime metadata (`appId`, `appName`, `version`, `pages`).
- `lxapp.config.ts`: Build config for path aliases, source dirs, and related build behavior.
- `pages/<name>/index.tsx`: View layer (UI rendering in WebView).
- `pages/<name>/index.ts`: Logic layer (page lifecycle and business operations).
- `pages/<name>/index.json`: Page-level config (navigation/title/style and related options).

---

## Build Outputs

- `lingxia build` (lxapp project) builds page assets and runtime artifacts into `dist/`.
- `lingxia build --release --package` (lxapp/lxplugin) produces package archive for publish.
- Host build consumes home lxapp assets and copies them into native platform resources.

---

## Common Pitfalls

- Editing generated runtime `app.json` directly instead of `lingxia.config.json`.
- Confusing `projectName` (technical) with `productName` (display name).
- Setting `homeLxAppVersion` in `lingxia.config.json` (it is generated).
- Mixing view logic and page logic in only one file; keep `index.tsx` and `index.ts` roles clear.
