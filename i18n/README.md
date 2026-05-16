# i18n Resources

Single source of truth for every localizable string LingXia ships, across:
Rust (`lingxia-logic`), TypeScript (`lingxia-types`), Android, iOS / macOS,
Harmony, and the Apple CLI permission descriptors.

## Directory layout (scopes)

The directory a YAML file lives in **determines which targets receive it**.
Pick the directory that matches who actually reads the string at runtime.

| Directory | Emits to | When to use |
|---|---|---|
| `ui/` | Rust + TS + Android + iOS + Harmony | Default. Cross-platform UI strings — 95% of new keys go here. |
| `error/` | Same as `ui/` | `error.*` and `err_code_*` entries. Required to define at least one `err_code_*`. |
| `permission/runtime/` | Same as `ui/` | Runtime permission dialog texts (e.g. `permission.media_reason`). |
| `permission/cli/` | Apple `Info.plist` (CLI build step only) | `apple.info_plist.*` keys consumed during `lingxia build` for Apple targets. |
| `logic/` *(optional)* | Rust + TS only | Strings the logic crate / JS bridge surfaces but no native SDK reads. |
| `android/` *(optional)* | Android `strings.xml` only | Android-only SDK strings (e.g. `R.string.lx_update_install_*`). |
| `apple/` *(optional)* | Apple `Localizable.strings` only | iOS / macOS-only SDK strings. |
| `harmony/` *(optional)* | Harmony `string.json` only | Harmony-only SDK strings. |

A key may live in **exactly one** scope directory. Moving a key between
scopes is the normal way to change its audience.

`schema/` holds JSON Schema files used to validate the YAML files:

- `ui.schema.json` — cross-platform leaves (`ui/`, `error/`, `permission/runtime/`).
- `permission.schema.json` — Apple `Info.plist` keys (`permission/cli/`).
- `native.schema.json` — single-audience scopes (`logic/`, `android/`, `apple/`, `harmony/`).

## Regenerating

The bare command picks up every workspace default and writes all five
targets in place:

```bash
cargo run -p lingxia-cli -- gen i18n
```

Equivalent to running with these defaults:

| Flag | Default |
|---|---|
| `--input` | `i18n` |
| `--rust-out` | `crates/lingxia-logic/src/i18n_generated.rs` |
| `--ts-out` | `packages/lingxia-types/src/generated` |
| `--android-out` | `lingxia-sdk/android/lingxia/src/main/res` |
| `--ios-out` | `lingxia-sdk/apple/Sources/Resources` |
| `--harmony-out` | `lingxia-sdk/harmony/lingxia/src/main/resources` |

Skip individual targets with `--no-rust`, `--no-ts`, `--no-android`,
`--no-ios`, `--no-harmony`.

### Verify-only mode (`--check`)

Catch "I edited yaml but forgot to regenerate":

```bash
cargo run -p lingxia-cli -- gen i18n --check
```

Generates every enabled target into a temp dir, diffs against the
checked-in copy, prints any drift, and exits non-zero. Run this in
pre-commit / pre-push hooks. The verify-only mode writes nothing.

Android `strings.xml` files are generated/ignored, so `--check` validates
that Android generation succeeds but does not require those XML files to be
tracked.

## What the generator enforces

- Within each scope, every locale file defines the same key set.
- Across scopes, a flattened key appears in **at most one** scope directory.
- `ui/*.yaml` files do not declare `error` or `err_code` top-level sections.
- `error/*.yaml` files contain only `error` and/or `err_code` sections, and
  at least one `err_code_<N>` key is defined.
- `err_code_<N>` keys only appear under `error/`.
- `logic/`, `android/`, `apple/`, `harmony/` use plain string leaves
  (no `default / android / apple / harmony / rust` variant overrides).
- YAML files validate against their scope's JSON schema.
- Apple CLI permission YAML files validate against `permission.schema.json`
  and share an identical key set across locales.

## Adding a new language

1. For each scope that has yaml files, copy `en-US.yaml` to `{locale}.yaml`
   and translate every value.
2. Run `cargo run -p lingxia-cli -- gen i18n` to regenerate.
3. Commit yaml + regenerated outputs together.

## Adding / updating error codes

1. Add `err_code.<CODE>` entries to **each** `error/{locale}.yaml`.
2. Run `cargo run -p lingxia-cli -- gen i18n`.
