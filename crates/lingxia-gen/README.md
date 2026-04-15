# lingxia-gen

`lingxia-gen` is a resource generation library for the LingXia workspace. It manages internationalization (i18n), icons, and other asset pipelines used by build tools.

## Features

-   **i18n Generation**: Compiles YAML translation files into platform-specific formats (Rust enum, Android strings.xml, iOS Localizable.strings, HarmonyOS string.json).
-   **TypeScript Generation**: Generates `error.ts` and `i18n.ts` metadata files for `packages/lingxia-types`.
-   **Platform Overrides**: Supports defining platform-specific strings in YAML.
-   **Schema Lint**: Validates locale and permission files against JSON Schemas.
-   **Strict Validation**: Fails generation when locale keys mismatch, permission keys mismatch, or `err_code_*` definitions are missing.
-   **Icon Generation**: Convert SVG source icons to native platform resources.

## Library Usage

### i18n

Ensure your `i18n` source YAML files are prepared.

Projects like `crates/lingxia-logic` and `tools/lingxia-cli` integrate `lingxia-gen` as a library and emit generated resources during build steps.

The `i18n` module exposes `I18nConfig` and `run(...)` for build tooling.

### icons

The `icons` module exposes SVG conversion helpers and `IconsConfig` / `run(...)` for build tooling.

`tools/lingxia-cli` provides hidden internal `gen` commands for release scripts that need a process entry point.

## Validation Rules

- Locale YAML files must have identical flattened key sets.
- Locale YAML files must pass `ui.schema.json`.
- `ui/*.yaml` must not include `error` / `err_code`; those belong to `error/*.yaml`.
- `permission/runtime/*.yaml` files are merged into locale maps and must pass `ui.schema.json`.
- `error/*.yaml` files are merged into locale maps and must pass `ui.schema.json`.
- `error/*.yaml` may only define top-level `error` and/or `err_code`.
- `permission/cli/*.yaml` files must pass `permission.schema.json` and have identical key sets.
- At least one `err_code_*` key must exist in merged locale maps.

## Input Format (i18n)

`--input` should point to an i18n root with this layout:

```text
i18n/
  ui/**/*.yaml
  permission/runtime/*.yaml
  permission/cli/*.yaml
  error/*.yaml
  schema/*.json
```

Locale maps are merged by locale filename from `ui/**/*.yaml`, `permission/runtime/*.yaml`, and `error/*.yaml`.

See `i18n/ui` directory for locale examples. Supports nested keys and platform-specific overrides:

```yaml
permission:
  camera_denied:
    default: "Camera denied"
    android: "Android specific message"
    apple: "iOS specific message"
    rust: "Desktop specific message"
```
