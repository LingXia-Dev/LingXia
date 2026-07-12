use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct IconsConfig {
    /// Path to the SVG source directory
    #[arg(short, long, default_value = "design/icons/svg")]
    pub input: PathBuf,

    /// Path to output iOS/macOS resources (PDF files). Defaults to the SDK's
    /// bundled icons dir so `gen icons` produces the Apple assets by default,
    /// mirroring `gen i18n`. Generated, gitignored — only the SVG is tracked.
    #[arg(long, default_value = "lingxia-sdk/apple/Sources/Resources/icons")]
    pub ios_out: Option<PathBuf>,

    /// Path to output Android resources (Vector Drawable XML)
    #[arg(long)]
    pub android_out: Option<PathBuf>,

    /// Path to output HarmonyOS resources (SVG copy)
    #[arg(long)]
    pub harmony_out: Option<PathBuf>,

    /// Path to output Windows resources (PNG files)
    #[arg(long)]
    pub windows_out: Option<PathBuf>,

    /// Rendered Windows PNG icon size in pixels
    #[arg(long, default_value_t = 64)]
    pub windows_png_size: u32,
}

pub fn run(config: IconsConfig) -> Result<()> {
    println!("Syncing icons from: {:?}", config.input);

    if !config.input.exists() {
        anyhow::bail!("Source directory not found: {:?}", config.input);
    }

    let mut stats = Stats::default();

    for entry in fs::read_dir(&config.input)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "svg") {
            let file_name = path.file_stem().context("No file stem")?.to_string_lossy();
            let svg_content = fs::read_to_string(&path)?;

            println!("Processing {}...", path.display());

            // iOS (PDF)
            if let Some(ref ios_dir) = config.ios_out {
                fs::create_dir_all(ios_dir)?;
                let dest = ios_dir.join(format!("{}.pdf", file_name));
                match convert_to_pdf(&svg_content, &dest) {
                    Ok(_) => stats.ios += 1,
                    Err(e) => {
                        eprintln!("  [iOS] Failed: {}", e);
                        stats.errors += 1;
                    }
                }
            }

            // Android (Vector Drawable XML)
            if let Some(ref android_dir) = config.android_out {
                fs::create_dir_all(android_dir)?;
                let dest = android_dir.join(format!("{}.xml", file_name));
                match convert_to_android_vector(&svg_content, &dest) {
                    Ok(_) => stats.android += 1,
                    Err(e) => {
                        eprintln!("  [Android] Failed: {}", e);
                        stats.errors += 1;
                    }
                }
            }

            // HarmonyOS (SVG copy)
            if let Some(ref harmony_dir) = config.harmony_out {
                fs::create_dir_all(harmony_dir)?;
                let dest = harmony_dir.join(format!("{}.svg", file_name));
                match fs::copy(&path, &dest) {
                    Ok(_) => stats.harmony += 1,
                    Err(e) => {
                        eprintln!("  [Harmony] Failed: {}", e);
                        stats.errors += 1;
                    }
                }
            }

            // Windows (PNG)
            if let Some(ref windows_dir) = config.windows_out {
                fs::create_dir_all(windows_dir)?;
                let dest = windows_dir.join(format!("{}.png", file_name));
                match convert_to_png(&svg_content, &dest, config.windows_png_size) {
                    Ok(_) => stats.windows += 1,
                    Err(e) => {
                        eprintln!("  [Windows] Failed: {}", e);
                        stats.errors += 1;
                    }
                }
            }
        }
    }

    println!("\nSync Complete!");
    println!("  iOS (PDF):     {}", stats.ios);
    println!("  Android (XML): {}", stats.android);
    println!("  Harmony (SVG): {}", stats.harmony);
    println!("  Windows (PNG): {}", stats.windows);

    if stats.errors > 0 {
        anyhow::bail!("Encountered {} errors during sync", stats.errors);
    }

    Ok(())
}

#[derive(Default)]
struct Stats {
    ios: usize,
    android: usize,
    harmony: usize,
    windows: usize,
    errors: usize,
}

pub fn svg_to_pdf_bytes(svg_content: &str) -> Result<Vec<u8>> {
    use svg2pdf::{ConversionOptions, PageOptions};

    let tree = parse_svg_tree(svg_content)?;
    let pdf = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to convert SVG to PDF: {:?}", e))?;
    Ok(pdf)
}

pub fn svg_to_png_bytes(svg_content: &str, target_size: u32) -> Result<Vec<u8>> {
    let tree = parse_png_svg_tree(svg_content)?;
    let source_size = tree.size();
    let max_side = source_size.width().max(source_size.height());
    anyhow::ensure!(max_side > 0.0, "SVG has an empty viewport");

    let scale = target_size as f32 / max_side;
    let offset_x = (target_size as f32 - source_size.width() * scale) / 2.0;
    let offset_y = (target_size as f32 - source_size.height() * scale) / 2.0;
    let mut pixmap = tiny_skia::Pixmap::new(target_size, target_size)
        .ok_or_else(|| anyhow::anyhow!("Failed to allocate icon pixmap"))?;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap
        .encode_png()
        .context("Failed to encode rendered SVG as PNG")
}

/// Standard sizes packed into a Windows `.exe` icon: 16/32 for the title bar and
/// small taskbar cell, 48 for the large taskbar/alt-tab cell, 256 for the
/// Explorer "large/extra-large icons" view and high-DPI downscaling.
pub const WINDOWS_ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

/// Pack an SVG into a multi-size Windows ICO for embedding as an `.exe` resource
/// (via the resource compiler in `lingxia-windows-build`). The `ico` crate stores
/// the 256px entry as PNG and the rest as BMP, which every resource compiler
/// (rc.exe / llvm-rc / windres) accepts.
pub fn svg_to_ico_bytes(svg_content: &str, sizes: &[u32]) -> Result<Vec<u8>> {
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in sizes {
        let png = svg_to_png_bytes(svg_content, size)?;
        let image = ico::IconImage::read_png(png.as_slice())
            .with_context(|| format!("Failed to decode {size}px PNG for ICO entry"))?;
        let entry = ico::IconDirEntry::encode(&image)
            .with_context(|| format!("Failed to encode {size}px ICO entry"))?;
        dir.add_entry(entry);
    }
    let mut buf = Vec::new();
    dir.write(&mut buf).context("Failed to write ICO")?;
    Ok(buf)
}

/// Pack a source app-icon PNG (e.g. a 1024px `AppIcon.png`) into a multi-size
/// Windows ICO, first cropping uniform launcher padding so the small taskbar /
/// Explorer cell is filled — matching the runtime `app_icon` look on Windows.
pub fn png_to_ico_bytes(png: &[u8], sizes: &[u32]) -> Result<Vec<u8>> {
    let source = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .context("Failed to decode app icon PNG")?
        .into_rgba8();
    let source = tighten_icon(source);
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in sizes {
        let resized =
            image::imageops::resize(&source, size, size, image::imageops::FilterType::Lanczos3);
        let entry = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        dir.add_entry(
            ico::IconDirEntry::encode(&entry)
                .with_context(|| format!("Failed to encode {size}px ICO entry"))?,
        );
    }
    let mut buf = Vec::new();
    dir.write(&mut buf).context("Failed to write ICO")?;
    Ok(buf)
}

/// Crop uniform launcher padding so the glyph fills the small icon cell. A
/// mobile launcher icon centers its glyph in a wide safe-area margin, which
/// reads as a tiny logo lost in padding at 16–48px. When the border is one flat
/// color (the four corners agree), crop to the glyph plus a small square margin;
/// full-bleed / photographic icons are returned unchanged. Mirrors the runtime
/// `lingxia-windows-sdk::app_icon` tightening so the embedded `.exe` icon and
/// the running window icon match.
fn tighten_icon(img: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img;
    }
    let bg = img.get_pixel(0, 0).0;
    let corners = [
        img.get_pixel(w - 1, 0).0,
        img.get_pixel(0, h - 1).0,
        img.get_pixel(w - 1, h - 1).0,
    ];
    if corners.iter().any(|c| !color_close(*c, bg, 12)) {
        return img;
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
    let mut found = false;
    for (x, y, pixel) in img.enumerate_pixels() {
        let p = pixel.0;
        if p[3] < 16 || color_close(p, bg, 32) {
            continue;
        }
        found = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if !found {
        return img;
    }
    let content = (max_x - min_x + 1).max(max_y - min_y + 1);
    let side = content + content / 4; // glyph + ~12% margin each side
    let cx = (min_x + max_x) / 2;
    let cy = (min_y + max_y) / 2;
    let half = (side / 2) as i64;
    let mut out = image::RgbaImage::from_pixel(side, side, image::Rgba(bg));
    for oy in 0..side {
        for ox in 0..side {
            let sx = cx as i64 - half + ox as i64;
            let sy = cy as i64 - half + oy as i64;
            if sx >= 0 && sy >= 0 && (sx as u32) < w && (sy as u32) < h {
                out.put_pixel(ox, oy, *img.get_pixel(sx as u32, sy as u32));
            }
        }
    }
    out
}

fn color_close(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a[0].abs_diff(b[0]) <= tol && a[1].abs_diff(b[1]) <= tol && a[2].abs_diff(b[2]) <= tol
}

pub fn svg_size(svg_content: &str) -> Result<(f32, f32)> {
    let tree = parse_svg_tree(svg_content)?;
    let size = tree.size();
    Ok((size.width(), size.height()))
}

fn parse_svg_tree(svg_content: &str) -> Result<svg2pdf::usvg::Tree> {
    let mut options = svg2pdf::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    svg2pdf::usvg::Tree::from_str(svg_content, &options).context("Failed to parse SVG")
}

fn parse_png_svg_tree(svg_content: &str) -> Result<usvg::Tree> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    usvg::Tree::from_str(svg_content, &options).context("Failed to parse SVG")
}

/// Convert SVG to PDF for iOS
fn convert_to_pdf(svg_content: &str, output_path: &Path) -> Result<()> {
    fs::write(output_path, svg_to_pdf_bytes(svg_content)?).context("Failed to write PDF")?;
    Ok(())
}

/// Convert SVG to Android Vector Drawable (XML)
fn convert_to_android_vector(svg_content: &str, output_path: &Path) -> Result<()> {
    let xml = svg_to_vector_drawable(svg_content)?;
    fs::write(output_path, xml).context("Failed to write XML")?;
    Ok(())
}

/// Convert SVG to PNG for Windows
fn convert_to_png(svg_content: &str, output_path: &Path, target_size: u32) -> Result<()> {
    fs::write(output_path, svg_to_png_bytes(svg_content, target_size)?)
        .context("Failed to write PNG")?;
    Ok(())
}

/// Parse SVG and convert to Android Vector Drawable format
fn svg_to_vector_drawable(svg_content: &str) -> Result<String> {
    use roxmltree::Document;

    let doc = Document::parse(svg_content).context("Failed to parse SVG XML")?;
    let svg_node = doc.root_element();

    // Extract viewBox or width/height
    let view_box = svg_node.attribute("viewBox");
    let (width, height, vp_width, vp_height) = if let Some(vb) = view_box {
        let parts: Vec<f64> = vb
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() >= 4 {
            (parts[2], parts[3], parts[2], parts[3])
        } else {
            (24.0, 24.0, 24.0, 24.0)
        }
    } else {
        let w = parse_dimension(svg_node.attribute("width")).unwrap_or(24.0);
        let h = parse_dimension(svg_node.attribute("height")).unwrap_or(24.0);
        (w, h, w, h)
    };

    let mut paths = Vec::new();
    collect_paths(&svg_node, &mut paths);

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str(&format!(
        "<vector xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    android:width=\"{}dp\"\n    android:height=\"{}dp\"\n    android:viewportWidth=\"{}\"\n    android:viewportHeight=\"{}\">\n",
        width, height, vp_width, vp_height
    ));

    for path_info in paths {
        xml.push_str("    <path\n");
        if let Some(fill) = path_info.fill {
            xml.push_str(&format!("        android:fillColor=\"{}\"\n", fill));
        }
        if path_info.fill_rule.as_deref() == Some("evenodd") {
            xml.push_str("        android:fillType=\"evenOdd\"\n");
        }
        if let Some(stroke) = path_info.stroke {
            xml.push_str(&format!("        android:strokeColor=\"{}\"\n", stroke));
        }
        if let Some(stroke_width) = path_info.stroke_width {
            xml.push_str(&format!(
                "        android:strokeWidth=\"{}\"\n",
                stroke_width
            ));
        }
        if let Some(ref line_cap) = path_info.stroke_line_cap {
            xml.push_str(&format!("        android:strokeLineCap=\"{}\"\n", line_cap));
        }
        if let Some(ref line_join) = path_info.stroke_line_join {
            xml.push_str(&format!(
                "        android:strokeLineJoin=\"{}\"\n",
                line_join
            ));
        }
        xml.push_str(&format!(
            "        android:pathData=\"{}\" />\n",
            path_info.data
        ));
    }

    xml.push_str("</vector>\n");
    Ok(xml)
}

struct PathInfo {
    data: String,
    fill: Option<String>,
    fill_rule: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<String>,
    stroke_line_cap: Option<String>,
    stroke_line_join: Option<String>,
}

/// Elements that should be skipped during path collection (definitions, masks, etc.)
const SKIP_ELEMENTS: &[&str] = &["defs", "mask", "clipPath", "symbol", "pattern", "filter"];

/// Inherited style attributes from parent elements
#[derive(Default, Clone)]
struct InheritedStyle {
    stroke: Option<String>,
    stroke_width: Option<String>,
    stroke_line_cap: Option<String>,
    stroke_line_join: Option<String>,
    fill: Option<String>,
}

fn collect_paths(node: &roxmltree::Node, paths: &mut Vec<PathInfo>) {
    collect_paths_with_style(node, paths, &InheritedStyle::default());
}

fn collect_paths_with_style(
    node: &roxmltree::Node,
    paths: &mut Vec<PathInfo>,
    inherited: &InheritedStyle,
) {
    let tag = node.tag_name().name();

    // Skip definition elements and their children
    if SKIP_ELEMENTS.contains(&tag) {
        return;
    }

    // Build inherited style for children (merge current node's attributes)
    let mut child_style = inherited.clone();
    if let Some(s) = node.attribute("stroke") {
        child_style.stroke = Some(s.to_string());
    }
    if let Some(s) = node.attribute("stroke-width") {
        child_style.stroke_width = Some(s.to_string());
    }
    if let Some(s) = node.attribute("stroke-linecap") {
        child_style.stroke_line_cap = Some(s.to_string());
    }
    if let Some(s) = node.attribute("stroke-linejoin") {
        child_style.stroke_line_join = Some(s.to_string());
    }
    if let Some(s) = node.attribute("fill") {
        child_style.fill = Some(s.to_string());
    }

    // Skip elements with mask attribute (they reference masks we can't render)
    if node.attribute("mask").is_some() {
        // Still process children but skip this element's own rendering
    }

    match tag {
        "path" => {
            if let Some(d) = node.attribute("d") {
                paths.push(extract_path_info_with_inherited(
                    node,
                    d.to_string(),
                    &child_style,
                ));
            }
        }
        "rect" => {
            if let Some(path_data) = rect_to_path(node) {
                paths.push(extract_path_info_with_inherited(
                    node,
                    path_data,
                    &child_style,
                ));
            }
        }
        "circle" => {
            if let Some(path_data) = circle_to_path(node) {
                paths.push(extract_path_info_with_inherited(
                    node,
                    path_data,
                    &child_style,
                ));
            }
        }
        "ellipse" => {
            if let Some(path_data) = ellipse_to_path(node) {
                paths.push(extract_path_info_with_inherited(
                    node,
                    path_data,
                    &child_style,
                ));
            }
        }
        "polygon" => {
            if let Some(path_data) = polygon_to_path(node) {
                paths.push(extract_path_info_with_inherited(
                    node,
                    path_data,
                    &child_style,
                ));
            }
        }
        "polyline" => {
            if let Some(path_data) = polyline_to_path(node) {
                paths.push(extract_path_info_with_inherited(
                    node,
                    path_data,
                    &child_style,
                ));
            }
        }
        "line" => {
            if let Some(path_data) = line_to_path(node) {
                paths.push(extract_path_info_with_inherited(
                    node,
                    path_data,
                    &child_style,
                ));
            }
        }
        _ => {}
    }

    for child in node.children() {
        if child.is_element() {
            collect_paths_with_style(&child, paths, &child_style);
        }
    }
}

fn extract_path_info_with_inherited(
    node: &roxmltree::Node,
    data: String,
    inherited: &InheritedStyle,
) -> PathInfo {
    // Element's own attributes override inherited ones
    let fill = node
        .attribute("fill")
        .map(|s| s.to_string())
        .or_else(|| inherited.fill.clone());
    let fill_opacity = node
        .attribute("fill-opacity")
        .and_then(|s| s.parse::<f64>().ok());
    let stroke = node
        .attribute("stroke")
        .map(|s| s.to_string())
        .or_else(|| inherited.stroke.clone());
    let stroke_width = node
        .attribute("stroke-width")
        .map(|s| s.to_string())
        .or_else(|| inherited.stroke_width.clone());
    let stroke_line_cap = node
        .attribute("stroke-linecap")
        .map(|s| s.to_string())
        .or_else(|| inherited.stroke_line_cap.clone());
    let stroke_line_join = node
        .attribute("stroke-linejoin")
        .map(|s| s.to_string())
        .or_else(|| inherited.stroke_line_join.clone());

    PathInfo {
        data,
        fill: fill.map(|s| normalize_color_with_opacity(&s, fill_opacity)),
        fill_rule: node.attribute("fill-rule").map(|s| s.to_string()),
        stroke: stroke.map(|s| normalize_color(&s)),
        stroke_width,
        stroke_line_cap,
        stroke_line_join,
    }
}

/// Convert <rect> to path data
fn rect_to_path(node: &roxmltree::Node) -> Option<String> {
    let x: f64 = node
        .attribute("x")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let y: f64 = node
        .attribute("y")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let w: f64 = node.attribute("width").and_then(|s| s.parse().ok())?;
    let h: f64 = node.attribute("height").and_then(|s| s.parse().ok())?;
    let rx: f64 = node
        .attribute("rx")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let ry: f64 = node
        .attribute("ry")
        .and_then(|s| s.parse().ok())
        .unwrap_or(rx);

    if rx > 0.0 || ry > 0.0 {
        // Rounded rectangle
        let rx = rx.min(w / 2.0);
        let ry = ry.min(h / 2.0);
        Some(format!(
            "M{} {} H{} A{} {} 0 0 1 {} {} V{} A{} {} 0 0 1 {} {} H{} A{} {} 0 0 1 {} {} V{} A{} {} 0 0 1 {} {} Z",
            x + rx,
            y,
            x + w - rx,
            rx,
            ry,
            x + w,
            y + ry,
            y + h - ry,
            rx,
            ry,
            x + w - rx,
            y + h,
            x + rx,
            rx,
            ry,
            x,
            y + h - ry,
            y + ry,
            rx,
            ry,
            x + rx,
            y
        ))
    } else {
        // Simple rectangle
        Some(format!("M{} {} H{} V{} H{} Z", x, y, x + w, y + h, x))
    }
}

/// Convert <circle> to path data
fn circle_to_path(node: &roxmltree::Node) -> Option<String> {
    let cx: f64 = node
        .attribute("cx")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let cy: f64 = node
        .attribute("cy")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let r: f64 = node.attribute("r").and_then(|s| s.parse().ok())?;

    // Circle as two arcs
    Some(format!(
        "M{} {} A{} {} 0 1 0 {} {} A{} {} 0 1 0 {} {} Z",
        cx - r,
        cy,
        r,
        r,
        cx + r,
        cy,
        r,
        r,
        cx - r,
        cy
    ))
}

/// Convert <ellipse> to path data
fn ellipse_to_path(node: &roxmltree::Node) -> Option<String> {
    let cx: f64 = node
        .attribute("cx")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let cy: f64 = node
        .attribute("cy")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let rx: f64 = node.attribute("rx").and_then(|s| s.parse().ok())?;
    let ry: f64 = node.attribute("ry").and_then(|s| s.parse().ok())?;

    Some(format!(
        "M{} {} A{} {} 0 1 0 {} {} A{} {} 0 1 0 {} {} Z",
        cx - rx,
        cy,
        rx,
        ry,
        cx + rx,
        cy,
        rx,
        ry,
        cx - rx,
        cy
    ))
}

/// Convert <polygon> to path data
fn polygon_to_path(node: &roxmltree::Node) -> Option<String> {
    let points = node.attribute("points")?;
    let coords = parse_points(points)?;
    if coords.is_empty() {
        return None;
    }
    let mut path = format!("M{},{}", coords[0].0, coords[0].1);
    for (x, y) in &coords[1..] {
        path.push_str(&format!(" L{},{}", x, y));
    }
    path.push_str(" Z");
    Some(path)
}

/// Convert <polyline> to path data
fn polyline_to_path(node: &roxmltree::Node) -> Option<String> {
    let points = node.attribute("points")?;
    let coords = parse_points(points)?;
    if coords.is_empty() {
        return None;
    }
    let mut path = format!("M{},{}", coords[0].0, coords[0].1);
    for (x, y) in &coords[1..] {
        path.push_str(&format!(" L{},{}", x, y));
    }
    Some(path)
}

/// Parse SVG points attribute (handles both "x,y x,y" and "x y x y" formats)
fn parse_points(points: &str) -> Option<Vec<(f64, f64)>> {
    let mut result = Vec::new();
    // First try comma-separated pairs: "x1,y1 x2,y2"
    if points.contains(',') {
        for pair in points.split_whitespace() {
            let mut parts = pair.split(',');
            let x: f64 = parts.next()?.parse().ok()?;
            let y: f64 = parts.next()?.parse().ok()?;
            result.push((x, y));
        }
    } else {
        // Space-separated: "x1 y1 x2 y2"
        let nums: Vec<f64> = points
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        for chunk in nums.chunks(2) {
            if chunk.len() == 2 {
                result.push((chunk[0], chunk[1]));
            }
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Convert <line> to path data
fn line_to_path(node: &roxmltree::Node) -> Option<String> {
    let x1: f64 = node
        .attribute("x1")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let y1: f64 = node
        .attribute("y1")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let x2: f64 = node
        .attribute("x2")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let y2: f64 = node
        .attribute("y2")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    Some(format!("M{} {} L{} {}", x1, y1, x2, y2))
}

fn normalize_color(color: &str) -> String {
    normalize_color_with_opacity(color, None)
}

fn normalize_color_with_opacity(color: &str, opacity: Option<f64>) -> String {
    if color == "none" {
        return "#00000000".to_string();
    }

    // Get base color in #RRGGBB format
    let base_color = if color.starts_with('#') {
        color.to_uppercase()
    } else {
        // Handle named colors
        match color.to_lowercase().as_str() {
            "white" => "#FFFFFF".to_string(),
            "black" => "#000000".to_string(),
            "red" => "#FF0000".to_string(),
            "green" => "#00FF00".to_string(),
            "blue" => "#0000FF".to_string(),
            _ => return color.to_string(),
        }
    };

    // If no opacity or fully opaque, return as-is
    let opacity = match opacity {
        Some(o) if o < 1.0 => o,
        _ => return base_color,
    };

    // Convert opacity to alpha hex and prepend to color
    // Android uses #AARRGGBB format
    let alpha = (opacity * 255.0).round() as u8;
    if base_color.len() == 7 {
        // #RRGGBB -> #AARRGGBB
        format!("#{:02X}{}", alpha, &base_color[1..])
    } else {
        base_color
    }
}

fn parse_dimension(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| {
        v.trim_end_matches("px")
            .trim_end_matches("pt")
            .trim_end_matches("dp")
            .parse()
            .ok()
    })
}

#[cfg(test)]
mod ico_tests {
    use super::*;

    #[test]
    fn svg_to_ico_packs_all_sizes() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" fill="#15181D"/><circle cx="32" cy="32" r="16" fill="#1FDDA4"/></svg>"##;
        let ico = svg_to_ico_bytes(svg, WINDOWS_ICO_SIZES).unwrap();
        // ICONDIR header: reserved=0, type=1 (icon), count = number of sizes.
        assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        assert_eq!(count, WINDOWS_ICO_SIZES.len());
    }
}
