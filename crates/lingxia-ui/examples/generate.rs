fn main() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/lingxia-ui/src");
    std::fs::write(
        root.join("schema.js"),
        lingxia_ui::protocol::javascript_schema(),
    )
    .unwrap();
    std::fs::write(
        root.join("generated/style.d.ts"),
        lingxia_ui::protocol::typescript_schema(),
    )
    .unwrap();
}
