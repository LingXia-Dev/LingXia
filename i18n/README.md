# i18n Resources

This directory contains internationalization (i18n) resource files for LingXia.

## Directory Layout

- `ui/` - UI text locales (non-error)
- `permission/cli/` - Permission usage texts for CLI-managed Apple metadata
- `permission/runtime/` - Runtime permission UI text fragments
- `error/` - Error text locales (`error.*`, `err_code.*`)
- `schema/` - JSON Schemas for validation

## Files

- `ui/en-US.yaml` - Core UI texts (English)
- `ui/zh-CN.yaml` - Core UI texts (Simplified Chinese)
- `permission/runtime/en-US.yaml` - Runtime permission UI text fragment (English)
- `permission/runtime/zh-CN.yaml` - Runtime permission UI text fragment (Simplified Chinese)
- `permission/cli/en-US.yaml` - Permission usage texts for CLI-managed Apple metadata (English)
- `permission/cli/zh-CN.yaml` - Permission usage texts for CLI-managed Apple metadata (Simplified Chinese)
- `error/en-US.yaml` - Error texts and error code texts (English)
- `error/zh-CN.yaml` - Error texts and error code texts (Simplified Chinese)
- `schema/ui.schema.json` - Schema for locale documents (`ui/en-US.yaml`, `ui/zh-CN.yaml`)
- `schema/permission.schema.json` - Schema for permission locale documents

## When To Regenerate

You must regenerate `lingxia-logic/src/i18n_generated.rs` when any of the following changes:

- Any locale source YAML under `i18n/ui/`, `i18n/error/`, or `i18n/permission/runtime/`
- Schema files under `i18n/schema/`
- i18n generator logic in `tools/lingxia-gen/src/i18n.rs`

Use this command:

```bash
cargo run -p lingxia-gen -- i18n --input i18n --rust-out lingxia-logic/src/i18n_generated.rs --ts-out packages/lingxia-types/src/generated
```

This regenerates both Rust (`lingxia-logic/src/i18n_generated.rs`) and TypeScript (`packages/lingxia-types/src/generated/*`) artifacts.

Generation is strict:
- Locale key sets must be identical.
- Locale documents must pass `schema/ui.schema.json`.
- `ui/*.yaml` must not contain `error` or `err_code` sections.
- `permission/runtime/*.yaml` documents are treated as locale fragments and must pass `schema/ui.schema.json`.
- `error/*.yaml` documents are treated as locale fragments and must pass `schema/ui.schema.json`.
- `error/*.yaml` must contain only `error` and/or `err_code`.
- `permission/cli/*.yaml` documents must pass `schema/permission.schema.json`.
- `permission/cli` locale key sets must be identical.
- At least one `err_code_*` key must exist in locale fragments.

## Adding a New Language

1. Create a new file `ui/{locale}.yaml`
2. Copy the structure from `ui/en-US.yaml`
3. Translate all values
4. Run the `lingxia-gen i18n` command above to validate generation
5. Regenerate `lingxia-logic/src/i18n_generated.rs` and commit both source + generated files

## Adding/Updating Error Codes

1. Add `err_code.<CODE>` entries in each `error/{locale}.yaml` file.
2. Run the `lingxia-gen i18n` command above to regenerate artifacts and validate.
