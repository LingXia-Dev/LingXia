// i18n_generated.rs is pre-generated and checked into src/
// Run `lingxia-gen i18n` to regenerate if i18n/*.yaml changes
fn main() {
    // Rerun if pre-generated file changes
    println!("cargo:rerun-if-changed=src/i18n_generated.rs");
}
