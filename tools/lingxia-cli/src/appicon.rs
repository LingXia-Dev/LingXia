//! App Icon Generator
//!
//! Converts a source app icon (PNG) to platform-specific formats and sizes.
//! Supports Android, iOS, and HarmonyOS (future).

use anyhow::{Context, Result};
use image::imageops::{self, FilterType};
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::fs;
use std::path::Path;

/// Normalize and validate an Android color hex string.
///
/// Accepts `#RGB`, `#ARGB`, `#RRGGBB`, `#AARRGGBB` (with or without the leading `#`).
pub fn normalize_android_color(color: &str) -> Result<String> {
    let trimmed = color.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Background color cannot be empty");
    }

    let with_hash = if let Some(rest) = trimmed.strip_prefix('#') {
        format!("#{rest}")
    } else {
        format!("#{trimmed}")
    };

    let hex = &with_hash[1..];
    match hex.len() {
        3 | 4 | 6 | 8 => {}
        _ => anyhow::bail!(
            "Invalid Android color '{with_hash}'. Use #RGB, #ARGB, #RRGGBB, or #AARRGGBB."
        ),
    }

    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Invalid Android color '{with_hash}'. Only 0-9 and A-F are allowed.");
    }

    Ok(with_hash.to_ascii_uppercase())
}

/// Generate Android app icons from a source image
///
/// # Arguments
/// * `source_icon` - Path to source icon (PNG, recommended 1024x1024)
/// * `res_dir` - Path to Android app/src/main/res directory
/// * `background_color` - Background color for adaptive icons (hex, e.g., "#FFFFFF")
/// * `include_legacy` - If true, generate legacy icons for minSdk < 26
pub fn generate_android_icons(
    source_icon: &Path,
    res_dir: &Path,
    background_color: &str,
    include_legacy: bool,
) -> Result<()> {
    let background_color = normalize_android_color(background_color)?;

    if !source_icon.exists() {
        anyhow::bail!("Source icon not found: {:?}", source_icon);
    }

    let img = image::open(source_icon).context("Failed to open source image")?;
    let (width, height) = img.dimensions();

    if width != height {
        eprintln!(
            "Warning: Source icon is not square ({}x{}). Icons may be distorted.",
            width, height
        );
    }

    if width < 1024 {
        eprintln!("Warning: Source icon is smaller than 1024x1024. Quality may be affected.");
    }

    println!(
        "Generating Android icons from {}x{} source{}...",
        width,
        height,
        if include_legacy {
            " (with legacy support)"
        } else {
            ""
        }
    );

    let mut count = 0;

    // Generate mipmap icons for each density
    for &(density, icon_size, adaptive_size) in ANDROID_DENSITIES {
        let mipmap_dir = res_dir.join(format!("mipmap-{}", density));
        fs::create_dir_all(&mipmap_dir)?;

        // Legacy icons (only if --legacy flag is set)
        if include_legacy {
            let resized = img.resize_exact(icon_size, icon_size, FilterType::Lanczos3);
            resized.save_with_format(mipmap_dir.join("ic_launcher.webp"), ImageFormat::WebP)?;
            count += 1;

            resized
                .save_with_format(mipmap_dir.join("ic_launcher_round.webp"), ImageFormat::WebP)?;
            count += 1;
        }

        // Foreground for adaptive icon (always generated)
        let foreground = create_adaptive_foreground(&img, adaptive_size);
        foreground.save_with_format(
            mipmap_dir.join("ic_launcher_foreground.webp"),
            ImageFormat::WebP,
        )?;
        count += 1;
    }

    // Generate adaptive icon XML files
    let mipmap_anydpi = res_dir.join("mipmap-anydpi-v26");
    fs::create_dir_all(&mipmap_anydpi)?;

    let adaptive_icon_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
</adaptive-icon>
"#;

    fs::write(mipmap_anydpi.join("ic_launcher.xml"), adaptive_icon_xml)?;
    fs::write(
        mipmap_anydpi.join("ic_launcher_round.xml"),
        adaptive_icon_xml,
    )?;
    count += 2;

    // Generate background color resource
    let values_dir = res_dir.join("values");
    fs::create_dir_all(&values_dir)?;

    let colors_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="ic_launcher_background">{}</color>
</resources>
"#,
        background_color
    );

    fs::write(values_dir.join("ic_launcher_background.xml"), colors_xml)?;
    count += 1;

    println!("  Generated {} Android icon files", count);

    Ok(())
}

/// Generate iOS app icons from a source image.
///
/// Creates an Assets.xcassets/AppIcon.appiconset with essential icon sizes
/// and the Contents.json descriptor. If you need an Assets.car, compile the
/// asset catalog during the build process.
///
/// # Arguments
/// * `source_icon` - Path to source icon (PNG, recommended 1024x1024)
/// * `resources_dir` - Path to target resources directory
pub fn generate_ios_icons(source_icon: &Path, resources_dir: &Path) -> Result<()> {
    use std::collections::HashMap;

    if !source_icon.exists() {
        anyhow::bail!("Source icon not found: {:?}", source_icon);
    }

    let img = image::open(source_icon).context("Failed to open source image")?;
    let (width, height) = img.dimensions();

    if width != height {
        eprintln!(
            "Warning: Source icon is not square ({}x{}). Icons may be distorted.",
            width, height
        );
    }

    if width < 1024 {
        eprintln!("Warning: Source icon is smaller than 1024x1024. Quality may be affected.");
    }

    println!("Generating iOS icons from {}x{} source...", width, height);

    // Create Assets.xcassets/AppIcon.appiconset directory
    let appiconset_dir = resources_dir.join("Assets.xcassets/AppIcon.appiconset");
    fs::create_dir_all(&appiconset_dir)?;

    // Essential iOS icon sizes (size string, scale string, pixel size, idiom, filename)
    // These cover iPhone home screen, iPad, and App Store
    let icon_specs: &[(&str, &str, u32, &str, &str)] = &[
        // iPhone home screen (60pt @2x and @3x)
        ("60x60", "2x", 120, "iphone", "Icon-60@2x.png"),
        ("60x60", "3x", 180, "iphone", "Icon-60@3x.png"),
        // iPad home screen (76pt @2x)
        ("76x76", "2x", 152, "ipad", "Icon-76@2x~ipad.png"),
        // iPad Pro (83.5pt @2x)
        ("83.5x83.5", "2x", 167, "ipad", "Icon-83.5@2x~ipad.png"),
        // App Store (1024pt @1x)
        ("1024x1024", "1x", 1024, "ios-marketing", "Icon-1024.png"),
    ];

    let mut images: Vec<HashMap<String, serde_json::Value>> = Vec::new();

    for (size_str, scale_str, pixel_size, idiom, filename) in icon_specs {
        let resized = img.resize_exact(*pixel_size, *pixel_size, FilterType::Lanczos3);
        resized.save_with_format(appiconset_dir.join(filename), ImageFormat::Png)?;

        // Build Contents.json entry
        let mut entry: HashMap<String, serde_json::Value> = HashMap::new();
        entry.insert("filename".into(), filename.to_string().into());
        entry.insert("idiom".into(), idiom.to_string().into());
        entry.insert("size".into(), size_str.to_string().into());
        entry.insert("scale".into(), scale_str.to_string().into());
        images.push(entry);
    }

    // Generate Contents.json
    let contents = serde_json::json!({
        "images": images,
        "info": {
            "author": "xcode",
            "version": 1
        }
    });

    fs::write(
        appiconset_dir.join("Contents.json"),
        serde_json::to_string_pretty(&contents)?,
    )?;

    println!(
        "  Generated {} iOS icon files in {}",
        icon_specs.len(),
        appiconset_dir.display()
    );

    Ok(())
}

/// Generate macOS app icons from a source image.
///
/// Creates an Assets.xcassets/AppIcon.appiconset with essential icon sizes
/// for macOS. Uses the same asset catalog location as iOS.
///
/// # Arguments
/// * `source_icon` - Path to source icon (PNG, recommended 1024x1024)
/// * `resources_dir` - Path to target resources directory
pub fn generate_macos_icons(source_icon: &Path, resources_dir: &Path) -> Result<()> {
    use std::collections::HashMap;

    if !source_icon.exists() {
        anyhow::bail!("Source icon not found: {:?}", source_icon);
    }

    let img = image::open(source_icon).context("Failed to open source image")?;
    let (width, height) = img.dimensions();

    if width != height {
        eprintln!(
            "Warning: Source icon is not square ({}x{}). Icons may be distorted.",
            width, height
        );
    }

    if width < 1024 {
        eprintln!("Warning: Source icon is smaller than 1024x1024. Quality may be affected.");
    }

    println!("Generating macOS icons from {}x{} source...", width, height);

    // Create Assets.xcassets/AppIcon.appiconset directory
    let appiconset_dir = resources_dir.join("Assets.xcassets/AppIcon.appiconset");
    fs::create_dir_all(&appiconset_dir)?;

    // Essential macOS icon sizes (point size, scale, filename)
    let icon_specs: &[(u32, u32, &str)] = &[
        (16, 1, "icon_16x16.png"),
        (16, 2, "icon_16x16@2x.png"),
        (32, 1, "icon_32x32.png"),
        (32, 2, "icon_32x32@2x.png"),
        (128, 1, "icon_128x128.png"),
        (128, 2, "icon_128x128@2x.png"),
        (256, 1, "icon_256x256.png"),
        (256, 2, "icon_256x256@2x.png"),
        (512, 1, "icon_512x512.png"),
        (512, 2, "icon_512x512@2x.png"),
    ];

    // Normalize visual weight across different source icon styles.
    // If source artwork already contains transparent padding, we avoid shrinking too much.
    let source_visual_ratio = estimate_nontransparent_bounds_ratio(&img);
    const TARGET_DOCK_VISUAL_RATIO: f32 = 0.73;
    let content_scale = (TARGET_DOCK_VISUAL_RATIO / source_visual_ratio).clamp(0.60, 0.92);

    let mut images: Vec<HashMap<String, serde_json::Value>> = Vec::new();

    for (size_pt, scale, filename) in icon_specs {
        let pixel_size = size_pt * scale;

        // Keep extra transparent padding so icon visual size matches other Dock icons.
        let padded = create_macos_icon_with_padding(&img, pixel_size, content_scale);
        padded.save_with_format(appiconset_dir.join(filename), ImageFormat::Png)?;

        // Build Contents.json entry
        let mut entry: HashMap<String, serde_json::Value> = HashMap::new();
        entry.insert("filename".into(), filename.to_string().into());
        entry.insert("idiom".into(), "mac".into());
        entry.insert("size".into(), format!("{}x{}", size_pt, size_pt).into());
        entry.insert("scale".into(), format!("{}x", scale).into());
        images.push(entry);
    }

    // Generate Contents.json
    let contents = serde_json::json!({
        "images": images,
        "info": {
            "author": "xcode",
            "version": 1
        }
    });

    fs::write(
        appiconset_dir.join("Contents.json"),
        serde_json::to_string_pretty(&contents)?,
    )?;

    println!(
        "  Generated {} macOS icon files in {}",
        icon_specs.len(),
        appiconset_dir.display()
    );

    Ok(())
}

/// Estimate source icon bounds ratio based on non-transparent pixels.
fn estimate_nontransparent_bounds_ratio(img: &DynamicImage) -> f32 {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    // Ignore near-transparent antialiasing noise when estimating bounds.
    const ALPHA_THRESHOLD: u8 = 12;
    for y in 0..h {
        for x in 0..w {
            let a = rgba.get_pixel(x, y).0[3];
            if a <= ALPHA_THRESHOLD {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if !found {
        return 1.0;
    }

    let bw = (max_x - min_x + 1) as f32 / w as f32;
    let bh = (max_y - min_y + 1) as f32 / h as f32;
    bw.max(bh).clamp(0.01, 1.0)
}

/// Add transparent padding to a macOS icon so it doesn't appear oversized in Dock.
fn create_macos_icon_with_padding(
    img: &DynamicImage,
    size: u32,
    content_scale: f32,
) -> DynamicImage {
    let icon_size = (size as f32 * content_scale).round().max(1.0) as u32;
    let offset = (size - icon_size) / 2;
    let mut resized = img
        .resize_exact(icon_size, icon_size, FilterType::Lanczos3)
        .to_rgba8();
    apply_rounded_corner_mask(&mut resized, icon_size as f32 * 0.22);

    let mut canvas = image::RgbaImage::new(size, size);
    imageops::overlay(&mut canvas, &resized, offset as i64, offset as i64);

    DynamicImage::ImageRgba8(canvas)
}

/// Apply a rounded-corner alpha mask in place.
fn apply_rounded_corner_mask(img: &mut image::RgbaImage, radius: f32) {
    let (w, h) = img.dimensions();
    let r = radius.clamp(1.0, (w.min(h) as f32) * 0.5);
    let left = r;
    let top = r;
    let right = (w as f32) - r;
    let bottom = (h as f32) - r;

    for y in 0..h {
        for x in 0..w {
            let xf = x as f32 + 0.5;
            let yf = y as f32 + 0.5;

            let cx = if xf < left {
                left
            } else if xf > right {
                right
            } else {
                xf
            };
            let cy = if yf < top {
                top
            } else if yf > bottom {
                bottom
            } else {
                yf
            };

            let dx = xf - cx;
            let dy = yf - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= r - 1.0 {
                continue;
            }

            let px = img.get_pixel_mut(x, y);
            if dist >= r {
                px.0[3] = 0;
                continue;
            }

            let edge_alpha = ((r - dist) * 255.0).clamp(0.0, 255.0) as u8;
            let current_alpha = px.0[3];
            px.0[3] = ((current_alpha as u16 * edge_alpha as u16) / 255) as u8;
        }
    }
}

/// Generate HarmonyOS app icons from a source image (future implementation)
#[allow(dead_code)]
pub fn generate_harmony_icons(
    _source_icon: &Path,
    _harmony_dir: &Path,
    _background_color: &str,
) -> Result<()> {
    anyhow::bail!("HarmonyOS icon generation not yet implemented");
}

/// Android mipmap densities: (folder_suffix, icon_size, adaptive_size)
const ANDROID_DENSITIES: &[(&str, u32, u32)] = &[
    ("mdpi", 48, 108),
    ("hdpi", 72, 162),
    ("xhdpi", 96, 216),
    ("xxhdpi", 144, 324),
    ("xxxhdpi", 192, 432),
];

/// Create foreground image for adaptive icon (icon centered in 108dp equivalent canvas)
fn create_adaptive_foreground(img: &DynamicImage, canvas_size: u32) -> DynamicImage {
    // The icon should occupy the inner 66/108 = ~61% of the canvas (safe zone is 66dp in 108dp)
    let icon_size = (canvas_size as f32 * 66.0 / 108.0).round() as u32;
    let offset = (canvas_size - icon_size) / 2;

    let resized = img
        .resize_exact(icon_size, icon_size, FilterType::Lanczos3)
        .to_rgba8();

    // Create transparent canvas
    let mut canvas = image::RgbaImage::new(canvas_size, canvas_size);

    imageops::overlay(&mut canvas, &resized, offset as i64, offset as i64);

    DynamicImage::ImageRgba8(canvas)
}
