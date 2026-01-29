# i18n Resources

This directory contains internationalization (i18n) resource files for LingXia.

## Files

- `en-US.yaml` - English (US)
- `zh-CN.yaml` - Simplified Chinese

## After Modifying

After modifying any `.yaml` file in this directory, you **must** regenerate the Rust code:

```bash
cargo run -p lingxia-gen -- i18n --rust-out lingxia-logic/src/i18n_generated.rs
```

**Do not forget to commit the generated file along with your yaml changes.**

## Adding a New Language

1. Create a new file `{locale}.yaml` (e.g., `ja-JP.yaml`)
2. Copy the structure from `en-US.yaml`
3. Translate all values
4. Run `cargo run -p lingxia-gen -- i18n`
5. Commit both the yaml file and the regenerated Rust code
