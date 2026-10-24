fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let module =
        std::fs::read_to_string(args.get(1).ok_or("usage: render module.ui.js output.png")?)?;
    let frame = lingxia_ui::ViewSession::new(&module)?.render(393, 760, 1.0)?;
    let path = args.get(2).ok_or("missing output.png")?;
    std::fs::write(path, frame.pixels.into_png()?)?;
    std::fs::write(
        format!("{path}.json"),
        serde_json::to_string_pretty(&frame.nodes)?,
    )?;
    Ok(())
}
