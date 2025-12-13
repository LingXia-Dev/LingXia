use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct IconsConfig {
    /// Path to the SVG source directory
    #[arg(short, long, default_value = "lingxia-sdk/resources/icons/svg")]
    pub input: PathBuf,

    /// Path to output iOS resources (PDF files)
    #[arg(long)]
    pub ios_out: Option<PathBuf>,

    /// Path to output Android resources (Vector Drawable XML)
    #[arg(long)]
    pub android_out: Option<PathBuf>,

    /// Path to output HarmonyOS resources (SVG copy)
    #[arg(long)]
    pub harmony_out: Option<PathBuf>,
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
        }
    }

    println!("\nSync Complete!");
    println!("  iOS (PDF):     {}", stats.ios);
    println!("  Android (XML): {}", stats.android);
    println!("  Harmony (SVG): {}", stats.harmony);

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
    errors: usize,
}

/// Convert SVG to PDF for iOS
fn convert_to_pdf(svg_content: &str, output_path: &Path) -> Result<()> {
    use svg2pdf::{ConversionOptions, PageOptions};

    let mut options = svg2pdf::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();

    let tree =
        svg2pdf::usvg::Tree::from_str(svg_content, &options).context("Failed to parse SVG")?;

    let pdf = svg2pdf::to_pdf(&tree, ConversionOptions::default(), PageOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to convert SVG to PDF: {:?}", e))?;

    fs::write(output_path, pdf).context("Failed to write PDF")?;
    Ok(())
}

/// Convert SVG to Android Vector Drawable (XML)
fn convert_to_android_vector(svg_content: &str, output_path: &Path) -> Result<()> {
    let xml = svg_to_vector_drawable(svg_content)?;
    fs::write(output_path, xml).context("Failed to write XML")?;
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
        if let Some(stroke) = path_info.stroke {
            xml.push_str(&format!("        android:strokeColor=\"{}\"\n", stroke));
        }
        if let Some(stroke_width) = path_info.stroke_width {
            xml.push_str(&format!(
                "        android:strokeWidth=\"{}\"\n",
                stroke_width
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
    stroke: Option<String>,
    stroke_width: Option<String>,
}

fn collect_paths(node: &roxmltree::Node, paths: &mut Vec<PathInfo>) {
    if node.tag_name().name() != "path" {
        // continue
    } else if let Some(d) = node.attribute("d") {
        let fill = node.attribute("fill").map(normalize_color);
        let stroke = node.attribute("stroke").map(normalize_color);
        let stroke_width = node.attribute("stroke-width").map(ToString::to_string);

        paths.push(PathInfo {
            data: d.to_string(),
            fill,
            stroke,
            stroke_width,
        });
    }

    for child in node.children() {
        if child.is_element() {
            collect_paths(&child, paths);
        }
    }
}

fn normalize_color(color: &str) -> String {
    if color == "none" {
        return "#00000000".to_string();
    }
    if color.starts_with('#') {
        return color.to_uppercase();
    }
    // Handle named colors
    match color.to_lowercase().as_str() {
        "white" => "#FFFFFF".to_string(),
        "black" => "#000000".to_string(),
        "red" => "#FF0000".to_string(),
        "green" => "#00FF00".to_string(),
        "blue" => "#0000FF".to_string(),
        _ => color.to_string(),
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
