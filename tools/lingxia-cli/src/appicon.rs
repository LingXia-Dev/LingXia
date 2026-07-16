//! App Icon Generator
//!
//! Converts a source app icon (PNG) to platform-specific formats and sizes.
//! Supports Android, iOS, macOS, and HarmonyOS.
//!
//! Layered icons (Android adaptive, HarmonyOS) are derived automatically when
//! the source has a flat background: the background layer takes the detected
//! corner color and the foreground layer reuses the full source image, so the
//! two blend seamlessly under any launcher mask — no chroma-keying, no quality
//! loss. Sources that cannot be classified fail loudly instead of degrading.

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

// ---------------------------------------------------------------------------
// Source analysis: classify the icon's backdrop and measure its content
// ---------------------------------------------------------------------------

/// Corner pixels must agree within this tolerance to count as a flat backdrop.
const CORNER_TOLERANCE: u8 = 12;
/// Pixels within this tolerance of the backdrop color count as background.
const CONTENT_TOLERANCE: u8 = 32;
/// Pixels below this alpha count as background.
const ALPHA_THRESHOLD: u8 = 16;

/// Android adaptive icon safe zone: a 66dp-diameter circle on the 108dp canvas.
pub const ANDROID_SAFE_RADIUS_FRAC: f32 = 33.0 / 108.0;
/// HarmonyOS layered icon safe zone: circle inscribed in the central 75% box.
pub const HARMONY_SAFE_RADIUS_FRAC: f32 = 0.375;

/// How the source icon's backdrop was classified by [`analyze_icon`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconBackdrop {
    /// All four corners share one opaque color — a full-bleed flat background.
    Flat([u8; 4]),
    /// All four corners are transparent — the source is glyph artwork.
    Transparent,
}

/// Deterministic measurement of a source icon: backdrop classification plus
/// the pixel extent of its content.
#[derive(Clone, Debug)]
pub struct IconAnalysis {
    pub backdrop: IconBackdrop,
    /// Content bounding box (min_x, min_y, max_x, max_y), inclusive.
    pub bbox: (u32, u32, u32, u32),
    /// Center of the content bounding box, in source pixels.
    pub center: (f32, f32),
    /// Maximum distance of any content pixel from `center`, in source pixels.
    /// This is the true radial extent, tighter than the bbox diagonal for
    /// artwork with empty corners.
    pub radius: f32,
}

/// Classify the backdrop and measure the content of a source icon.
///
/// Returns `None` when the backdrop cannot be classified (corners disagree —
/// gradients, photos) or when no content pixels are found. This is a
/// deterministic rule, not a heuristic: callers must treat `None` as "ask the
/// user", never as "guess".
pub fn analyze_icon(img: &DynamicImage) -> Option<IconAnalysis> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 4 || h < 4 {
        return None;
    }

    let corners = [
        rgba.get_pixel(0, 0).0,
        rgba.get_pixel(w - 1, 0).0,
        rgba.get_pixel(0, h - 1).0,
        rgba.get_pixel(w - 1, h - 1).0,
    ];
    let backdrop = if corners.iter().all(|c| c[3] < ALPHA_THRESHOLD) {
        IconBackdrop::Transparent
    } else if corners.iter().all(|c| c[3] >= ALPHA_THRESHOLD)
        && corners[1..]
            .iter()
            .all(|c| color_close(*c, corners[0], CORNER_TOLERANCE))
    {
        IconBackdrop::Flat(corners[0])
    } else {
        return None;
    };

    let is_content = |p: &[u8; 4]| -> bool {
        match backdrop {
            IconBackdrop::Transparent => p[3] >= ALPHA_THRESHOLD,
            IconBackdrop::Flat(bg) => {
                p[3] >= ALPHA_THRESHOLD && !color_close(*p, bg, CONTENT_TOLERANCE)
            }
        }
    };

    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
    let mut found = false;
    for (x, y, px) in rgba.enumerate_pixels() {
        if is_content(&px.0) {
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if !found {
        return None;
    }

    let center = ((min_x + max_x) as f32 / 2.0, (min_y + max_y) as f32 / 2.0);
    let mut radius_sq = 0.0f32;
    for (x, y, px) in rgba.enumerate_pixels() {
        if is_content(&px.0) {
            let dx = x as f32 - center.0;
            let dy = y as f32 - center.1;
            radius_sq = radius_sq.max(dx * dx + dy * dy);
        }
    }

    Some(IconAnalysis {
        backdrop,
        bbox: (min_x, min_y, max_x, max_y),
        center,
        radius: radius_sq.sqrt().max(1.0),
    })
}

fn color_close(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a[0].abs_diff(b[0]) <= tol && a[1].abs_diff(b[1]) <= tol && a[2].abs_diff(b[2]) <= tol
}

fn hex_rgb(c: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Render `src` onto a transparent square canvas, scaled and positioned so its
/// content circle (per `analysis`) fits within `safe_radius_frac` of the
/// canvas. Sizing uses the content's true radial extent, so it survives every
/// launcher mask shape (circle included) without the bounding-box-fit corner
/// clipping.
///
/// For flat-background sources the full source is drawn — its own background
/// merges invisibly with the matching solid background layer, which is what
/// makes the derivation lossless.
pub fn render_layer_canvas(
    src: &DynamicImage,
    analysis: &IconAnalysis,
    canvas_size: u32,
    safe_radius_frac: f32,
) -> image::RgbaImage {
    let safe_r = canvas_size as f32 * safe_radius_frac;
    // Resampling spreads hard edges over a few pixels; keep that skirt inside
    // the safe circle too.
    const RESAMPLE_GUARD: f32 = 0.985;
    let scale = safe_r * RESAMPLE_GUARD / analysis.radius;
    let (sw, sh) = src.dimensions();
    let tw = ((sw as f32 * scale).round() as u32).max(1);
    let th = ((sh as f32 * scale).round() as u32).max(1);
    let resized = src.resize_exact(tw, th, FilterType::Lanczos3).to_rgba8();
    let ox = (canvas_size as f32 / 2.0 - analysis.center.0 * scale).round() as i64;
    let oy = (canvas_size as f32 / 2.0 - analysis.center.1 * scale).round() as i64;
    let mut canvas = image::RgbaImage::new(canvas_size, canvas_size);
    imageops::overlay(&mut canvas, &resized, ox, oy);
    canvas
}

/// Resolved inputs for a layered (foreground + solid background) icon.
pub struct LayerPlan<'a> {
    /// Image drawn on the transparent foreground layer.
    pub fg_src: &'a DynamicImage,
    /// Measurement of `fg_src`; `None` falls back to the legacy 61% box fit.
    pub fg_analysis: Option<IconAnalysis>,
    /// Normalized color for the solid background layer.
    pub background: String,
    /// Human-readable record of every decision, for the console and previews.
    pub notes: Vec<String>,
}

impl LayerPlan<'_> {
    /// Render the transparent foreground layer at `canvas_size`.
    pub fn render_foreground(&self, canvas_size: u32, safe_radius_frac: f32) -> image::RgbaImage {
        match &self.fg_analysis {
            Some(a) => render_layer_canvas(self.fg_src, a, canvas_size, safe_radius_frac),
            None => create_adaptive_foreground(self.fg_src, canvas_size).to_rgba8(),
        }
    }
}

/// Decide the foreground image and background color for layered icons.
///
/// Resolution ladder:
/// 1. An explicit `foreground` image wins; its own analysis drives the safe
///    scaling. The background comes from `background_color`, or from the flat
///    backdrop of `source` when detectable.
/// 2. Otherwise a `source` with a flat backdrop derives both layers by itself:
///    foreground = full source (safe-scaled), background = corner color.
/// 3. A transparent-backdrop `source` is treated as glyph artwork and requires
///    an explicit `background_color`.
/// 4. Anything else (gradients, photos) is an error asking for `--foreground`
///    — never a silent fallback.
pub fn resolve_layer_plan<'a>(
    source: &'a DynamicImage,
    foreground: Option<&'a DynamicImage>,
    background_color: Option<&str>,
) -> Result<LayerPlan<'a>> {
    let explicit_bg = background_color.map(normalize_android_color).transpose()?;
    let mut notes = Vec::new();

    let describe = |a: &IconAnalysis, notes: &mut Vec<String>| {
        let (x0, y0, x1, y1) = a.bbox;
        notes.push(format!(
            "Content {}x{} at ({}, {}), max radius {:.0}px from center",
            x1 - x0 + 1,
            y1 - y0 + 1,
            x0,
            y0,
            a.radius
        ));
    };

    if let Some(fg) = foreground {
        let fg_analysis = analyze_icon(fg);
        match &fg_analysis {
            Some(a) => {
                notes.push("Foreground: explicit artwork, scaled so its content circle fits the safe zone on every mask shape".to_string());
                describe(a, &mut notes);
            }
            None => notes.push(
                "Warning: could not measure the foreground artwork; using the legacy 61% box fit"
                    .to_string(),
            ),
        }
        let background = match (&explicit_bg, analyze_icon(source)) {
            (Some(b), _) => {
                notes.push(format!("Background: {b} (explicit)"));
                b.clone()
            }
            (
                None,
                Some(IconAnalysis {
                    backdrop: IconBackdrop::Flat(bg),
                    ..
                }),
            ) => {
                let hex = hex_rgb(bg);
                notes.push(format!(
                    "Background: {hex} (detected from the source's flat backdrop)"
                ));
                hex
            }
            (None, _) => anyhow::bail!(
                "Cannot determine the background layer color: the source has no flat backdrop to sample. Pass --background-color."
            ),
        };
        return Ok(LayerPlan {
            fg_src: fg,
            fg_analysis,
            background,
            notes,
        });
    }

    match analyze_icon(source) {
        Some(a) => match a.backdrop {
            IconBackdrop::Flat(bg) => {
                let detected = hex_rgb(bg);
                notes.push(format!(
                    "Flat backdrop detected: {detected} (all four corners agree)"
                ));
                describe(&a, &mut notes);
                notes.push(
                    "Foreground: full source reused — its backdrop merges seamlessly with the background layer".to_string(),
                );
                let background = match explicit_bg {
                    Some(b) => {
                        if b != detected {
                            notes.push(format!(
                                "Note: explicit background {b} overrides detected {detected}; the foreground plate will show its own backdrop color"
                            ));
                        }
                        b
                    }
                    None => detected,
                };
                Ok(LayerPlan {
                    fg_src: source,
                    fg_analysis: Some(a),
                    background,
                    notes,
                })
            }
            IconBackdrop::Transparent => {
                let background = explicit_bg.ok_or_else(|| {
                    anyhow::anyhow!(
                        "The source is transparent glyph artwork, so the background layer color cannot be detected. Pass --background-color."
                    )
                })?;
                notes.push(
                    "Transparent backdrop detected: source treated as glyph artwork".to_string(),
                );
                describe(&a, &mut notes);
                notes.push(format!("Background: {background} (explicit)"));
                Ok(LayerPlan {
                    fg_src: source,
                    fg_analysis: Some(a),
                    background,
                    notes,
                })
            }
        },
        None => anyhow::bail!(
            "The source backdrop is not a flat color (corners disagree — gradient or photographic art?), so the adaptive foreground cannot be derived safely.\n  Pass --foreground <transparent-glyph.png> (plus --background-color), or use a source with a flat background."
        ),
    }
}

fn warn_source_shape(width: u32, height: u32) {
    if width != height {
        eprintln!(
            "Warning: Source icon is not square ({}x{}). Icons may be distorted.",
            width, height
        );
    }
    if width < 1024 {
        eprintln!("Warning: Source icon is smaller than 1024x1024. Quality may be affected.");
    }
}

/// Generate Android app icons from a source image
///
/// # Arguments
/// * `source_icon` - Path to source icon (PNG, recommended 1024x1024)
/// * `res_dir` - Path to Android app/src/main/res directory
/// * `background_color` - Background color for adaptive icons; `None`
///   auto-detects from the source's flat backdrop (see [`resolve_layer_plan`])
/// * `include_legacy` - If true, generate legacy icons for minSdk < 26
/// * `foreground_icon` - Optional transparent artwork used for the adaptive
///   foreground instead of the derived one
pub fn generate_android_icons(
    source_icon: &Path,
    res_dir: &Path,
    background_color: Option<&str>,
    include_legacy: bool,
    foreground_icon: Option<&Path>,
) -> Result<()> {
    if !source_icon.exists() {
        anyhow::bail!("Source icon not found: {:?}", source_icon);
    }

    let img = image::open(source_icon).context("Failed to open source image")?;
    let foreground_img = match foreground_icon {
        Some(path) => {
            if !path.exists() {
                anyhow::bail!("Foreground icon not found: {:?}", path);
            }
            Some(image::open(path).context("Failed to open foreground image")?)
        }
        None => None,
    };
    let (width, height) = img.dimensions();
    warn_source_shape(width, height);

    let plan = resolve_layer_plan(&img, foreground_img.as_ref(), background_color)?;
    for note in &plan.notes {
        println!("  {note}");
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
        let foreground = plan.render_foreground(adaptive_size, ANDROID_SAFE_RADIUS_FRAC);
        DynamicImage::ImageRgba8(foreground).save_with_format(
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
        plan.background
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
    warn_source_shape(width, height);

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
    warn_source_shape(width, height);

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

/// Generate HarmonyOS app icons from a source image.
///
/// Creates foreground.png, background.png, and layered_image.json in both
/// `AppScope/resources/base/media/` and `entry/src/main/resources/base/media/`.
///
/// HarmonyOS uses a layered icon system similar to Android adaptive icons:
/// - foreground.png: 1024x1024 with the content safe-scaled for the mask
/// - background.png: 1024x1024 solid color
/// - layered_image.json: references foreground and background
///
/// # Arguments
/// * `source_icon` - Path to source icon (PNG, recommended 1024x1024)
/// * `harmony_dir` - Path to the HarmonyOS project directory (containing AppScope/)
/// * `background_color` - Background color hex; `None` auto-detects from the
///   source's flat backdrop (see [`resolve_layer_plan`])
/// * `foreground_icon` - Optional transparent artwork for the layered-icon
///   foreground (same semantics as the Android adaptive foreground)
pub fn generate_harmony_icons(
    source_icon: &Path,
    harmony_dir: &Path,
    background_color: Option<&str>,
    foreground_icon: Option<&Path>,
) -> Result<()> {
    if !source_icon.exists() {
        anyhow::bail!("Source icon not found: {:?}", source_icon);
    }

    let img = image::open(source_icon).context("Failed to open source image")?;
    let foreground_src = match foreground_icon {
        Some(path) => {
            if !path.exists() {
                anyhow::bail!("Foreground icon not found: {:?}", path);
            }
            Some(image::open(path).context("Failed to open foreground image")?)
        }
        None => None,
    };
    let (width, height) = img.dimensions();
    warn_source_shape(width, height);

    let plan = resolve_layer_plan(&img, foreground_src.as_ref(), background_color)?;
    for note in &plan.notes {
        println!("  {note}");
    }

    println!(
        "Generating HarmonyOS icons from {}x{} source...",
        width, height
    );

    // Parse background color
    let bg_rgba = parse_hex_color(&plan.background)?;

    // Target directories
    let media_dirs = [
        harmony_dir.join("AppScope/resources/base/media"),
        harmony_dir.join("entry/src/main/resources/base/media"),
    ];

    let canvas_size: u32 = 1024;

    // Create foreground: content safe-scaled on a transparent canvas
    let foreground = plan.render_foreground(canvas_size, HARMONY_SAFE_RADIUS_FRAC);

    // Create background: solid color
    let mut background = image::RgbaImage::new(canvas_size, canvas_size);
    for pixel in background.pixels_mut() {
        *pixel = image::Rgba(bg_rgba);
    }

    // layered_image.json content
    let layered_image_json = r#"{
  "layered-image":
  {
    "background" : "$media:background",
    "foreground" : "$media:foreground"
  }
}"#;

    // Create startIcon: foreground composited onto background (HarmonyOS start window)
    let start_icon_size: u32 = 256;
    let mut start_icon = background.clone();
    imageops::overlay(&mut start_icon, &foreground, 0, 0);
    let start_icon_img = DynamicImage::ImageRgba8(start_icon).resize_exact(
        start_icon_size,
        start_icon_size,
        FilterType::Lanczos3,
    );

    let mut count = 0;
    for media_dir in &media_dirs {
        fs::create_dir_all(media_dir)?;

        DynamicImage::ImageRgba8(foreground.clone())
            .save_with_format(media_dir.join("foreground.png"), ImageFormat::Png)?;
        count += 1;

        DynamicImage::ImageRgba8(background.clone())
            .save_with_format(media_dir.join("background.png"), ImageFormat::Png)?;
        count += 1;

        fs::write(media_dir.join("layered_image.json"), layered_image_json)?;
        count += 1;

        // Also generate startIcon.png for the HarmonyOS start window
        start_icon_img.save_with_format(media_dir.join("startIcon.png"), ImageFormat::Png)?;
        count += 1;
    }

    println!(
        "  Generated {} HarmonyOS icon files in {} directories",
        count,
        media_dirs.len()
    );

    Ok(())
}

/// Parse a hex color string into [R, G, B, A] bytes.
pub fn parse_hex_color(color: &str) -> Result<[u8; 4]> {
    let hex = color.trim().trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16)?;
            let g = u8::from_str_radix(&hex[2..4], 16)?;
            let b = u8::from_str_radix(&hex[4..6], 16)?;
            Ok([r, g, b, 255])
        }
        8 => {
            let a = u8::from_str_radix(&hex[0..2], 16)?;
            let r = u8::from_str_radix(&hex[2..4], 16)?;
            let g = u8::from_str_radix(&hex[4..6], 16)?;
            let b = u8::from_str_radix(&hex[6..8], 16)?;
            Ok([r, g, b, a])
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)?;
            Ok([r, g, b, 255])
        }
        4 => {
            let a = u8::from_str_radix(&hex[0..1].repeat(2), 16)?;
            let r = u8::from_str_radix(&hex[1..2].repeat(2), 16)?;
            let g = u8::from_str_radix(&hex[2..3].repeat(2), 16)?;
            let b = u8::from_str_radix(&hex[3..4].repeat(2), 16)?;
            Ok([r, g, b, a])
        }
        _ => anyhow::bail!(
            "Invalid color format: '{}'. Use #RGB, #ARGB, #RRGGBB, or #AARRGGBB.",
            color
        ),
    }
}

/// Android mipmap densities: (folder_suffix, icon_size, adaptive_size)
const ANDROID_DENSITIES: &[(&str, u32, u32)] = &[
    ("mdpi", 48, 108),
    ("hdpi", 72, 162),
    ("xhdpi", 96, 216),
    ("xxhdpi", 144, 324),
    ("xxxhdpi", 192, 432),
];

/// Legacy fallback: icon scaled into the 66/108 safe-zone *box* and centered.
/// Only used when the artwork cannot be measured; the box fit can clip corners
/// under circular masks, which is why measured sources use
/// [`render_layer_canvas`] instead.
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

#[cfg(test)]
mod smart_icon_tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    const BG: [u8; 4] = [12, 74, 60, 255];
    const FG: [u8; 4] = [231, 226, 216, 255];

    fn flat_source() -> DynamicImage {
        // 100x100 flat background with a 40x40 glyph at (30,40)..(69,79)
        let mut img = RgbaImage::from_pixel(100, 100, Rgba(BG));
        for y in 40..80 {
            for x in 30..70 {
                img.put_pixel(x, y, Rgba(FG));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn analyze_detects_flat_backdrop_and_content() {
        let a = analyze_icon(&flat_source()).expect("flat source should analyze");
        assert_eq!(a.backdrop, IconBackdrop::Flat(BG));
        assert_eq!(a.bbox, (30, 40, 69, 79));
        assert_eq!(a.center, (49.5, 59.5));
        // Farthest content pixel is a bbox corner: sqrt(19.5^2 + 19.5^2)
        let expected = (19.5f32 * 19.5 * 2.0).sqrt();
        assert!((a.radius - expected).abs() < 0.75, "radius {}", a.radius);
    }

    #[test]
    fn analyze_detects_transparent_backdrop() {
        let mut img = RgbaImage::new(64, 64);
        for y in 20..44 {
            for x in 20..44 {
                img.put_pixel(x, y, Rgba(FG));
            }
        }
        let a = analyze_icon(&DynamicImage::ImageRgba8(img)).expect("should analyze");
        assert_eq!(a.backdrop, IconBackdrop::Transparent);
    }

    #[test]
    fn analyze_rejects_gradient_backdrop() {
        let mut img = RgbaImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                img.put_pixel(x, y, Rgba([(y * 4) as u8, 80, 90, 255]));
            }
        }
        assert!(analyze_icon(&DynamicImage::ImageRgba8(img)).is_none());
    }

    #[test]
    fn layer_plan_derives_background_from_flat_source() {
        let src = flat_source();
        let plan = resolve_layer_plan(&src, None, None).expect("plan");
        assert_eq!(plan.background, "#0C4A3C");
        assert!(plan.fg_analysis.is_some());
    }

    #[test]
    fn layer_plan_requires_background_for_transparent_source() {
        let img = {
            let mut i = RgbaImage::new(64, 64);
            for y in 20..44 {
                for x in 20..44 {
                    i.put_pixel(x, y, Rgba(FG));
                }
            }
            DynamicImage::ImageRgba8(i)
        };
        assert!(resolve_layer_plan(&img, None, None).is_err());
        let plan = resolve_layer_plan(&img, None, Some("#123456")).expect("plan");
        assert_eq!(plan.background, "#123456");
    }

    #[test]
    fn layer_plan_rejects_gradient_source() {
        let mut img = RgbaImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                img.put_pixel(x, y, Rgba([(x * 3) as u8, (y * 3) as u8, 128, 255]));
            }
        }
        let img = DynamicImage::ImageRgba8(img);
        assert!(resolve_layer_plan(&img, None, None).is_err());
        // Even an explicit background must not silently produce a foreground.
        assert!(resolve_layer_plan(&img, None, Some("#FFFFFF")).is_err());
    }

    #[test]
    fn rendered_layer_keeps_content_inside_safe_circle() {
        // Realistic direction: large master downscaled onto the adaptive canvas.
        let mut img = RgbaImage::from_pixel(1000, 1000, Rgba(BG));
        for y in 400..800 {
            for x in 300..700 {
                img.put_pixel(x, y, Rgba(FG));
            }
        }
        let src = DynamicImage::ImageRgba8(img);
        let a = analyze_icon(&src).unwrap();
        let canvas = render_layer_canvas(&src, &a, 432, ANDROID_SAFE_RADIUS_FRAC);
        let safe_r = 432.0 * ANDROID_SAFE_RADIUS_FRAC;
        for (x, y, px) in canvas.enumerate_pixels() {
            if px.0[3] >= ALPHA_THRESHOLD && !color_close(px.0, BG, CONTENT_TOLERANCE) {
                let dx = x as f32 - 216.0;
                let dy = y as f32 - 216.0;
                let d = (dx * dx + dy * dy).sqrt();
                assert!(
                    d <= safe_r + 1.0,
                    "content pixel at ({x},{y}) is {d:.1}px from center, beyond safe {safe_r:.1}"
                );
            }
        }
        // Center of the glyph must land at the canvas center
        assert_eq!(canvas.get_pixel(216, 216).0, FG);
    }

    #[test]
    fn parse_hex_color_supports_short_argb() {
        assert_eq!(parse_hex_color("#8FAB").unwrap(), [0xFF, 0xAA, 0xBB, 0x88]);
        assert_eq!(
            parse_hex_color("#0C4A3C").unwrap(),
            [0x0C, 0x4A, 0x3C, 0xFF]
        );
    }
}
