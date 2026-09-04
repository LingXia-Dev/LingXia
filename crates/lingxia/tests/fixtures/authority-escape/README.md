# Authority escape compile-fail fixture

Run `cargo check --manifest-path Cargo.toml --all-features`. Success is a
security regression: a downstream crate must not be able to mint native or
authenticated caller authority, enter the platform bootstrap, pre-install a
resource resolver, or invoke Settings without an initialized runtime handle.
