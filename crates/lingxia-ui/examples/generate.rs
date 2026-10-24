fn main() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/lingxia-ui/src");
    std::fs::write(
        root.join("schema.js"),
        lingxia_ui::schema::javascript_schema(),
    )
    .unwrap();
    std::fs::write(
        root.join("generated/style.d.ts"),
        lingxia_ui::schema::typescript_schema(),
    )
    .unwrap();
}
