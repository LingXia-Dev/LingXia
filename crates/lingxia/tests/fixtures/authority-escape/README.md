# Authority escape compile-fail fixture

CI runs this fixture through `scripts/ci/authority-escape-gate.sh`. The gate
requires `cargo check --all-features` to fail and verifies the expected Rust
privacy/not-found diagnostic for every probe. A successful compile or a
missing diagnostic is a security regression: a downstream crate must not be
able to mint native or authenticated caller authority, enter the platform
bootstrap, pre-install a resource resolver, invoke Settings without an
initialized runtime handle, or use a former safe process-authority installer.
It also proves that downstream extensions cannot restore page-script injection.
