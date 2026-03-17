# lingxia-gen

`lingxia-gen` is a unified resource generation tool for the LingXia project. It manages internationalization (i18n), icons, and other asset pipelines.

## Features

-   **i18n Generation**: Compiles YAML translation files into platform-specific formats (Rust enum, Android strings.xml, iOS Localizable.strings, HarmonyOS string.json).
-   **TypeScript Generation**: Generates `error.ts` and `i18n.ts` metadata files for `packages/lingxia-types`.
-   **Platform Overrides**: Supports defining platform-specific strings in YAML.
-   **Schema Lint**: Validates locale and permission files against JSON Schemas.
-   **Strict Validation**: Fails generation when locale keys mismatch, permission keys mismatch, or `err_code_*` definitions are missing.
-   **(Future) Icon Generation**: Sync SVG icons to native platforms.

## Usage

### i18n

Ensure your `i18n` source YAML files are prepared.

#### Automated Build (Rust)
Projects like `lingxia-logic` integrate `lingxia-gen` via `build.rs` as a library and emit generated Rust into `OUT_DIR`.

#### Manual Execution (CLI)
Run from the project root:

```bash
 cargo run -p lingxia-gen -- i18n \
  --input i18n \
  --rust-out /tmp/i18n_generated.rs \
  --ts-out packages/lingxia-types/src/generated \
  --android-out lingxia-sdk/android/lingxia/src/main/res \
  --ios-out lingxia-sdk/apple/Sources/Resources \
  --harmony-out lingxia-sdk/harmony/lingxia/src/main/resources
```

### Subcommands

*   `i18n`: Generate internationalization resources.

### Arguments (i18n)

-   `-i, --input <PATH>`: Source directory (Default: `i18n`)
-   `--rust-out <PATH>`: Output path for Rust code (Optional)
-   `--ts-out <PATH>`: Output directory for TypeScript generated files (`error.ts`, `i18n.ts`) (Optional)
-   `--android-out <PATH>`: Output path for Android resources (Optional)
-   `--ios-out <PATH>`: Output path for iOS resources (Optional)
-   `--harmony-out <PATH>`: Output path for HarmonyOS resources (Optional)
-   `--schema-dir <PATH>`: JSON Schema directory (Optional, defaults to `<input>/schema`)

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
