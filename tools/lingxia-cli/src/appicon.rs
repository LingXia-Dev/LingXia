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
    // macOS uses square icons without idiom specification
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

    let mut images: Vec<HashMap<String, serde_json::Value>> = Vec::new();

    for (size_pt, scale, filename) in icon_specs {
        let pixel_size = size_pt * scale;
        let resized = img.resize_exact(pixel_size, pixel_size, FilterType::Lanczos3);
        resized.save_with_format(appiconset_dir.join(filename), ImageFormat::Png)?;

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
