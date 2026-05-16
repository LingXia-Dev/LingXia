# `android/` scope

Strings here are emitted **only** to
`lingxia-sdk/android/lingxia/src/main/res/values*/strings.xml`. They do
NOT appear in:

- Apple `Localizable.strings`
- Harmony `string.json`
- Rust `I18nKey` enum (in `lingxia-logic`)
- TypeScript `I18N_KEYS` list (in `lingxia-types`)

Use this scope for Kotlin SDK code that reads `R.string.lx_*` directly
and has no cross-platform logic-layer counterpart (e.g. `UpdateManager`'s
PackageInstaller status messages, install-confirm notification text).

**Leaf format:** plain string only. The `default / android / apple /
harmony / rust` variant syntax is not allowed here — single-audience
strings have no per-platform variants.

**Schema:** `../schema/native.schema.json`
