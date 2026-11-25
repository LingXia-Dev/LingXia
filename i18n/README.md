# i18n Resources

Single source of truth for every localizable string LingXia ships, across:
Rust (`lingxia-logic`), Android, iOS / macOS,
Harmony, and the Apple CLI permission descriptors.

LingXia translates these; an app overrides keys or adds a language in its own
`i18n/<tag>.yaml` (see `docs/internal/i18n.md` for why). Permission prompts and
`Info.plist` texts are never shipped from unreviewed machine translation. Not yet:
a `description` per key and plurals (ICU plural to Android `plurals` and Apple
`.stringsdict`); values take plain arguments only.

## Directory layout (scopes)

The directory a YAML file lives in **determines which targets receive it**.
Pick the directory that matches who actually reads the string at runtime.

| Directory | Emits to | When to use |
|---|---|---|
| `shared/` | Rust + TS + Android + iOS + Harmony | Default. Strings shared across every platform — 95% of new keys go here. |
| `error/` | Same as `shared/` | `error.*` and `err_code_*` entries. Required to define at least one `err_code_*`. |
| `permission/runtime/` | Same as `shared/` | Runtime permission dialog texts (e.g. `permission.media_reason`). |
| `permission/cli/` | Apple bundle files the CLI writes (build step only) | `apple.info_plist.*` permission texts and `apple.settings_bundle.*` iOS Settings titles, consumed during `lingxia build` for Apple targets. |
| `logic/` *(optional)* | Rust + TS only | Strings the logic crate / JS bridge surfaces but no native SDK reads. |
| `desktop/` *(optional)* | Rust + TS + Apple | Desktop-shell strings (sidebar, browser chrome, terminal) for the Windows SDK and the Apple SDK (`Localizable.strings`). Kept out of the Android and Harmony resource bundles. Do not move a desktop chrome key to `apple/` just because Swift reads it. |
| `android/` *(optional)* | Android `strings.xml` only | Android-only SDK strings (e.g. `R.string.lx_update_install_*`). |
| `apple/` *(optional)* | Apple `Localizable.strings` only | iOS / macOS-only SDK strings. |
| `harmony/` *(optional)* | Harmony `string.json` only | Harmony-only SDK strings. |

A key may live in **exactly one** scope directory. Moving a key between
scopes is the normal way to change its audience.

`schema/` holds JSON Schema files used to validate the YAML files:

- `shared.schema.json` — cross-platform leaves (`shared/`, `error/`, `permission/runtime/`).
- `permission.schema.json` — Apple CLI build-time keys (`permission/cli/`).
- `native.schema.json` — single-audience scopes (`logic/`, `desktop/`, `android/`, `apple/`, `harmony/`).

## Arguments

A value is an ICU message with plain arguments: `"Showing {filter}"`. The generator
writes them as each platform reads them — `%1$s` for Android and Harmony, `%1$@` for
Apple, numbered in the `en-US` order — and stops when a translation drops, adds or
renames one. Pass them positionally (`getString(id, filter)`, `L10n.string(key, filter)`).

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
| `--android-out` | `lingxia-sdk/android/lingxia/src/main/res` |
| `--ios-out` | `lingxia-sdk/apple/Sources/Resources` |
| `--harmony-out` | `lingxia-sdk/harmony/lingxia/src/main/resources` |

Skip individual targets with `--no-rust`, `--no-android`,
`--no-ios`, `--no-harmony`.

### Verify-only mode (`--check`)

Catch "I edited yaml but forgot to regenerate":

```bash
cargo run -p lingxia-cli -- gen i18n --check
```

Generates every enabled target into a temp dir, diffs against the
checked-in copy, prints any drift, and exits non-zero. Run this in
pre-commit / pre-push hooks. The verify-only mode writes nothing. This is
also the CI gate.

Only the Rust output (`i18n_generated.rs`) is committed, so
`--check` byte-compares it. The native resource bundles (Android
`strings.xml`, Apple `Localizable.strings`, Harmony `string.json`) are
gitignored and regenerated at build time, so `--check` validates that they
generate but does not require them to be tracked.

The generated Rust items carry `#[rustfmt::skip]`, so `cargo fmt` leaves the
file alone and plain `gen i18n` output is byte-stable — no separate
formatting step, no `rustfmt` binary needed at generation time, and no
spurious `--check` drift across toolchains.

## What the generator enforces

- Within each scope, every locale file defines the same key set.
- Across scopes, a flattened key appears in **at most one** scope directory.
- `shared/*.yaml` files do not declare `error` or `err_code` top-level sections.
- `error/*.yaml` files contain only `error` and/or `err_code` sections, and
  at least one `err_code_<N>` key is defined.
- `err_code_<N>` keys only appear under `error/`.
- `logic/`, `desktop/`, `android/`, `apple/`, `harmony/` use plain string
  leaves (no `default / android / apple / harmony / rust` variant overrides).
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
