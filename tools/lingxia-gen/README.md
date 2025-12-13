# lingxia-gen

`lingxia-gen` is a unified resource generation tool for the LingXia project. It manages internationalization (i18n), icons, and other asset pipelines.

## Features

-   **i18n Generation**: Compiles YAML translation files into platform-specific formats (Rust enum, Android strings.xml, iOS Localizable.strings, HarmonyOS string.json).
-   **Platform Overrides**: Supports defining platform-specific strings in YAML.
-   **(Future) Icon Generation**: Sync SVG icons to native platforms.

## Usage

### i18n

Ensure your `i18n` source YAML files are prepared.

#### Automated Build (Rust)
Projects like `lingxia-logic` integrate `lingxia-gen` via `build.rs` as a library.

#### Manual Execution (CLI)
Run from the project root:

```bash
 cargo run -p lingxia-gen -- i18n \
  --input i18n \
  --rust-out lingxia-logic/src/i18n_generated.rs \
  --android-out lingxia-sdk/android/lingxia/src/main/res \
  --ios-out lingxia-sdk/apple/Sources/Resources \
  --harmony-out lingxia-sdk/harmony/lingxia/src/main/resources
```

### Subcommands

*   `i18n`: Generate internationalization resources.

### Arguments (i18n)

-   `-i, --input <PATH>`: Source directory (Default: `i18n`)
-   `--rust-out <PATH>`: Output path for Rust code (Optional)
-   `--android-out <PATH>`: Output path for Android resources (Optional)
-   `--ios-out <PATH>`: Output path for iOS resources (Optional)
-   `--harmony-out <PATH>`: Output path for HarmonyOS resources (Optional)

## Input Format (i18n)

See `i18n` directory for examples. Supports nested keys and platform-specific overrides:

```yaml
permission:
  camera_denied:
    default: "Camera denied"
    android: "Android specific message"
    apple: "iOS specific message"
    rust: "Desktop specific message"
```

