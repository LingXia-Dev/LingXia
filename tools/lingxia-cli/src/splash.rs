//! Launch-screen (splash) asset generation.
//!
//! Turns the `splash:` section of `lingxia.yaml` into per-platform launch
//! assets at build time. The splash is a full-screen image (aspect-fill) over
//! a background color; the OS static launch frame shows the color (plus the
//! system icon where the OS mandates it), and the runtime overlay brings the
//! full-screen image until the home page first renders.
//!
//! - Android: a res overlay (staged outside the source tree) with a splash
//!   theme applied to the launcher activity via the `lxSplashTheme` manifest
//!   placeholder.
//! - iOS: `LingXiaSplash` / `LingXiaSplashBackground` asset-catalog entries in
//!   a staged copy of `Assets.xcassets`; `UILaunchScreen` shows the color.
//! - HarmonyOS: start-window color and splash media synced into the committed
//!   entry module.
//!
//! The resource names are looked up at runtime by the SDK splash overlay, so
//! generation here is the single source of truth for them.

use anyhow::{Context, Result, anyhow};
use image::DynamicImage;
use image::imageops::FilterType;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::SplashConfig;

/// Max pixel dimension shipped for the splash image.
const SPLASH_MAX_PX: u32 = 2048;

/// Android resource names — must match the SDK's runtime lookups.
pub const ANDROID_IMAGE_RES: &str = "lingxia_splash_image";
pub const ANDROID_COLOR_RES: &str = "lingxia_splash_background";
pub const ANDROID_SPLASH_THEME: &str = "Theme.LingXia.Splash";

/// Apple asset-catalog names — must match the SDK's runtime lookups and the
/// generated `UILaunchScreen` dictionary.
pub const APPLE_IMAGE_ASSET: &str = "LingXiaSplash";
pub const APPLE_COLOR_ASSET: &str = "LingXiaSplashBackground";

/// Harmony resource names — module.json5 `startWindowBackground` points at
/// the color; the SDK overlay loads the media by name.
const HARMONY_MEDIA_RES: &str = "lingxia_splash";
const HARMONY_COLOR_RES: &str = "lingxia_splash_background";

/// Splash config resolved against the project root: images loaded, colors
/// normalized to `#RRGGBB`.
pub struct ResolvedSplash {
    light_image: DynamicImage,
    dark_image: Option<DynamicImage>,
    pub light_background: String,
    pub dark_background: Option<String>,
}

impl ResolvedSplash {
    pub fn resolve(project_root: &Path, config: &SplashConfig) -> Result<Self> {
        let light_path = project_root.join(&config.image);
        let light_image = image::open(&light_path)
            .with_context(|| format!("Failed to open splash image {}", light_path.display()))?;
        let light_background = normalize_hex_rgb(&config.background)
            .with_context(|| "Invalid splash.background".to_string())?;

        let (dark_image, dark_background) = match &config.dark {
            Some(dark) => {
                let dark_image = match &dark.image {
                    Some(rel) => {
                        let path = project_root.join(rel);
                        Some(image::open(&path).with_context(|| {
                            format!("Failed to open dark splash image {}", path.display())
                        })?)
                    }
                    None => None,
                };
                let background = normalize_hex_rgb(&dark.background)
                    .with_context(|| "Invalid splash.dark.background".to_string())?;
                (dark_image, Some(background))
            }
            None => (None, None),
        };

        Ok(Self {
            light_image,
            dark_image,
            light_background,
            dark_background,
        })
    }
}

/// Normalize `#RGB` / `#RRGGBB` to uppercase `#RRGGBB`. Alpha is rejected —
/// launch-window backgrounds are opaque on every platform.
pub fn normalize_hex_rgb(color: &str) -> Result<String> {
    let hex = color.trim().trim_start_matches('#');
    let expanded = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => hex.to_string(),
        _ => {
            return Err(anyhow!(
                "Invalid splash color '{color}'. Use #RGB or #RRGGBB (no alpha)."
            ));
        }
    };
    if !expanded.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "Invalid splash color '{color}'. Only 0-9 and A-F are allowed."
        ));
    }
    Ok(format!("#{}", expanded.to_ascii_uppercase()))
}

/// Downscale to [`SPLASH_MAX_PX`]; never upscale — the runtime aspect-fills.
fn fit_splash(img: &DynamicImage) -> DynamicImage {
    if img.width().max(img.height()) > SPLASH_MAX_PX {
        img.resize(SPLASH_MAX_PX, SPLASH_MAX_PX, FilterType::Lanczos3)
    } else {
        img.clone()
    }
}

fn save_png(img: &DynamicImage, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    img.save_with_format(dest, image::ImageFormat::Png)
        .with_context(|| format!("Failed to write {}", dest.display()))
}

fn png_bytes(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .context("Failed to encode splash PNG")?;
    Ok(buf.into_inner())
}

/// Write only when content differs, so committed files stay clean on no-op
/// syncs. Returns whether the file changed.
fn write_if_changed(dest: &Path, content: &[u8]) -> Result<bool> {
    if let Ok(existing) = fs::read(dest)
        && existing == content
    {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, content).with_context(|| format!("Failed to write {}", dest.display()))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Android: res overlay staged outside the source tree
// ---------------------------------------------------------------------------

/// Stage the Android splash resources into `res_dir` (an overlay directory
/// merged by Gradle). The launcher activity picks the theme up through the
/// `lxSplashTheme` manifest placeholder. The static window is color-only —
/// an OS window background can't aspect-fill a bitmap without distortion —
/// and the runtime overlay brings the full-screen image.
pub fn stage_android_res(splash: &ResolvedSplash, res_dir: &Path) -> Result<()> {
    save_png(
        &fit_splash(&splash.light_image),
        &res_dir.join(format!("drawable-nodpi/{ANDROID_IMAGE_RES}.png")),
    )?;
    if let Some(dark) = &splash.dark_image {
        save_png(
            &fit_splash(dark),
            &res_dir.join(format!("drawable-night-nodpi/{ANDROID_IMAGE_RES}.png")),
        )?;
    }

    let values = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="{ANDROID_COLOR_RES}">{}</color>
    <style name="{ANDROID_SPLASH_THEME}" parent="Theme.AppCompat.DayNight.NoActionBar">
        <item name="android:windowBackground">@color/{ANDROID_COLOR_RES}</item>
    </style>
</resources>
"#,
        splash.light_background
    );
    let values_path = res_dir.join("values/lingxia_splash.xml");
    fs::create_dir_all(values_path.parent().unwrap())?;
    fs::write(&values_path, values)?;

    if let Some(dark_background) = &splash.dark_background {
        let night = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="{ANDROID_COLOR_RES}">{dark_background}</color>
</resources>
"#
        );
        let night_path = res_dir.join("values-night/lingxia_splash.xml");
        fs::create_dir_all(night_path.parent().unwrap())?;
        fs::write(&night_path, night)?;
    }

    // API 31+: the system splash window takes over the launch frame; keep its
    // background in sync. The icon stays the launcher icon — Android 12 does
    // not allow removing or replacing it with a full-screen image.
    let v31 = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <style name="{ANDROID_SPLASH_THEME}" parent="Theme.AppCompat.DayNight.NoActionBar">
        <item name="android:windowBackground">@color/{ANDROID_COLOR_RES}</item>
        <item name="android:windowSplashScreenBackground">@color/{ANDROID_COLOR_RES}</item>
    </style>
</resources>
"#
    );
    let v31_path = res_dir.join("values-v31/lingxia_splash.xml");
    fs::create_dir_all(v31_path.parent().unwrap())?;
    fs::write(&v31_path, v31)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Apple: asset-catalog entries in a staged catalog copy
// ---------------------------------------------------------------------------

/// Ensure a staged `Assets.xcassets` copy carrying the splash image set and
/// color set, and return the resources dir to hand to `actool`.
///
/// `staged_resources` is the env-icon staging dir when that overlay already
/// ran — the splash entries are injected there in place. Otherwise the source
/// catalog is copied to `staging_base/overlay/splash/Resources` first.
pub fn stage_apple_splash_resources(
    staging_base: &Path,
    source_resources_dir: &Path,
    staged_resources: Option<PathBuf>,
    splash: &ResolvedSplash,
) -> Result<PathBuf> {
    let resources_dir = match staged_resources {
        Some(dir) => dir,
        None => {
            let source_xcassets = source_resources_dir.join("Assets.xcassets");
            if !source_xcassets.exists() {
                return Err(anyhow!(
                    "Assets.xcassets not found at {} — required for splash generation",
                    source_xcassets.display()
                ));
            }
            let staging_root = staging_base.join("overlay").join("splash");
            let staging_resources = staging_root.join("Resources");
            if staging_root.exists() {
                fs::remove_dir_all(&staging_root)
                    .with_context(|| format!("Failed to clean {}", staging_root.display()))?;
            }
            copy_dir_recursive(&source_xcassets, &staging_resources.join("Assets.xcassets"))?;
            staging_resources
        }
    };

    inject_apple_splash_assets(&resources_dir.join("Assets.xcassets"), splash)?;
    Ok(resources_dir)
}

/// Loose splash image names in the app bundle. The runtime overlay prefers
/// these over the asset catalog: `actool` is an external tool that can fail
/// (it needs an installed simulator runtime even for device builds), and the
/// overlay must not go missing when it does.
pub const APPLE_BUNDLE_IMAGE: &str = "LingXiaSplash.png";
pub const APPLE_BUNDLE_IMAGE_DARK: &str = "LingXiaSplash~dark.png";

/// Copy the splash images into a built `.app` as plain bundle resources.
pub fn install_apple_bundle_images(app_bundle: &Path, splash: &ResolvedSplash) -> Result<()> {
    save_png(
        &fit_splash(&splash.light_image),
        &app_bundle.join(APPLE_BUNDLE_IMAGE),
    )?;
    if let Some(dark) = &splash.dark_image {
        save_png(&fit_splash(dark), &app_bundle.join(APPLE_BUNDLE_IMAGE_DARK))?;
    }
    Ok(())
}

fn inject_apple_splash_assets(xcassets_dir: &Path, splash: &ResolvedSplash) -> Result<()> {
    // Image set: one single-scale universal image (the overlay aspect-fills
    // it), plus a dark-appearance variant when configured.
    let imageset_dir = xcassets_dir.join(format!("{APPLE_IMAGE_ASSET}.imageset"));
    fs::create_dir_all(&imageset_dir)?;

    save_png(
        &fit_splash(&splash.light_image),
        &imageset_dir.join("splash.png"),
    )?;
    let mut images = vec![json!({
        "idiom": "universal",
        "filename": "splash.png",
    })];
    if let Some(dark) = &splash.dark_image {
        save_png(&fit_splash(dark), &imageset_dir.join("splash-dark.png"))?;
        images.push(json!({
            "idiom": "universal",
            "appearances": [{"appearance": "luminosity", "value": "dark"}],
            "filename": "splash-dark.png",
        }));
    }
    let imageset_contents = json!({
        "images": images,
        "info": {"author": "lingxia", "version": 1},
    });
    fs::write(
        imageset_dir.join("Contents.json"),
        serde_json::to_string_pretty(&imageset_contents)?,
    )?;

    // Color set: light color, plus a dark-appearance color when configured.
    let colorset_dir = xcassets_dir.join(format!("{APPLE_COLOR_ASSET}.colorset"));
    fs::create_dir_all(&colorset_dir)?;
    let mut colors = vec![json!({
        "idiom": "universal",
        "color": apple_color_components(&splash.light_background)?,
    })];
    if let Some(dark_background) = &splash.dark_background {
        colors.push(json!({
            "idiom": "universal",
            "appearances": [{"appearance": "luminosity", "value": "dark"}],
            "color": apple_color_components(dark_background)?,
        }));
    }
    let colorset_contents = json!({
        "colors": colors,
        "info": {"author": "lingxia", "version": 1},
    });
    fs::write(
        colorset_dir.join("Contents.json"),
        serde_json::to_string_pretty(&colorset_contents)?,
    )?;

    Ok(())
}

fn apple_color_components(hex_rgb: &str) -> Result<Value> {
    let [r, g, b, _] = crate::appicon::parse_hex_color(hex_rgb)?;
    Ok(json!({
        "color-space": "srgb",
        "components": {
            "red": format!("0x{r:02X}"),
            "green": format!("0x{g:02X}"),
            "blue": format!("0x{b:02X}"),
            "alpha": "1.000",
        },
    }))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("Failed to create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("Failed to read {}", src.display()))? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest).with_context(|| {
                format!("Failed to copy {} -> {}", path.display(), dest.display())
            })?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HarmonyOS: start-window resources synced into the committed entry module
// ---------------------------------------------------------------------------

/// Harmony renders the start window from committed module resources, so this
/// syncs them in place (same model as the managed AppLinks/ACL syncs):
/// splash media, color elements, and `startWindowBackground` in module.json5.
/// `startWindowIcon` is left to the project — the OS start window cannot
/// render a full-screen image; the SDK overlay brings it.
/// Returns whether anything changed.
pub fn sync_harmony_splash(splash: &ResolvedSplash, harmony_dir: &Path) -> Result<bool> {
    let resources_dir = harmony_dir.join("entry/src/main/resources");
    let mut changed = false;

    changed |= write_if_changed(
        &resources_dir.join(format!("base/media/{HARMONY_MEDIA_RES}.png")),
        &png_bytes(&fit_splash(&splash.light_image))?,
    )?;
    if let Some(dark) = &splash.dark_image {
        changed |= write_if_changed(
            &resources_dir.join(format!("dark/media/{HARMONY_MEDIA_RES}.png")),
            &png_bytes(&fit_splash(dark))?,
        )?;
    }

    changed |= upsert_harmony_color(
        &resources_dir.join("base/element/color.json"),
        HARMONY_COLOR_RES,
        &splash.light_background,
    )?;
    if let Some(dark_background) = &splash.dark_background {
        changed |= upsert_harmony_color(
            &resources_dir.join("dark/element/color.json"),
            HARMONY_COLOR_RES,
            dark_background,
        )?;
    }

    changed |= sync_harmony_start_window(harmony_dir)?;
    Ok(changed)
}

/// Insert or update one `{name, value}` entry in a Harmony color.json.
fn upsert_harmony_color(path: &Path, name: &str, value: &str) -> Result<bool> {
    let mut root: Value = if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?
    } else {
        json!({"color": []})
    };
    let colors = root
        .get_mut("color")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("Invalid {}: missing `color` array", path.display()))?;

    if let Some(entry) = colors
        .iter_mut()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(name))
    {
        if entry.get("value").and_then(Value::as_str) == Some(value) {
            return Ok(false);
        }
        entry["value"] = json!(value);
    } else {
        colors.push(json!({"name": name, "value": value}));
    }

    let serialized = format!("{}\n", serde_json::to_string_pretty(&root)?);
    write_if_changed(path, serialized.as_bytes())
}

/// Point the entry ability's start-window background at the splash color.
fn sync_harmony_start_window(harmony_dir: &Path) -> Result<bool> {
    let module_path = harmony_dir.join("entry/src/main/module.json5");
    let content = fs::read_to_string(&module_path)
        .with_context(|| format!("Failed to read {}", module_path.display()))?;
    let mut root: Value = json5::from_str(&content)
        .with_context(|| format!("Failed to parse {}", module_path.display()))?;

    let main_element = root
        .get("module")
        .and_then(|module| module.get("mainElement"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let abilities = root
        .get_mut("module")
        .and_then(|module| module.get_mut("abilities"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("Invalid module.json5: `module.abilities` must be an array"))?;
    let ability = abilities
        .iter_mut()
        .find(|ability| {
            main_element.is_none()
                || ability.get("name").and_then(Value::as_str) == main_element.as_deref()
        })
        .ok_or_else(|| anyhow!("Invalid module.json5: no main ability found"))?;
    let ability_obj = ability
        .as_object_mut()
        .ok_or_else(|| anyhow!("Invalid module.json5: ability must be an object"))?;

    let background_ref = format!("$color:{HARMONY_COLOR_RES}");
    if ability_obj
        .get("startWindowBackground")
        .and_then(Value::as_str)
        == Some(background_ref.as_str())
    {
        return Ok(false);
    }
    ability_obj.insert("startWindowBackground".to_string(), json!(background_ref));

    let updated =
        serde_json::to_string_pretty(&root).context("Failed to serialize module.json5")?;
    fs::write(&module_path, format!("{updated}\n"))
        .with_context(|| format!("Failed to write {}", module_path.display()))?;
    Ok(true)
}
