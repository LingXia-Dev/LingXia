//! Launch-screen (splash) asset generation.
//!
//! Turns the `splash:` section of `lingxia.yaml` into per-platform launch
//! assets at build time. The launch is two OS-owned beats: the launch frame
//! (the placeholder — background color with a small centered image, the one
//! composition launch-frame compositors render sharp and on time), then the
//! app's first frame, which the SDK fills with the cover — full screen,
//! already opaque when the placeholder's exit reveals it. A full-bleed image
//! inside the launch frame itself is a lost cause on every OS, which is why
//! the cover rides the first app frame instead.
//!
//! - Android: a res overlay (staged outside the source tree) with the cover
//!   drawable and a splash theme applied to the launcher activity via the
//!   `lxSplashTheme` manifest placeholder. The frame is color-only — the
//!   icon slot keeps the real app icon, whose launcher-zoom morph the OS
//!   composes.
//! - iOS: `LingXiaSplashBackground` / `LingXiaSplashMark` asset-catalog
//!   entries in a staged copy of `Assets.xcassets`, referenced by the
//!   generated `UILaunchScreen`; the cover ships as a loose bundle PNG.
//! - HarmonyOS: start-window color and mark, plus the cover media, synced
//!   into the committed entry module.
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

/// Max pixel dimension shipped for the cover.
const SPLASH_MAX_PX: u32 = 2048;

/// Android resource names — must match the SDK's runtime lookups.
pub const ANDROID_IMAGE_RES: &str = "lingxia_splash_image";
pub const ANDROID_COLOR_RES: &str = "lingxia_splash_background";
pub const ANDROID_SPLASH_THEME: &str = "Theme.LingXia.Splash";

/// Apple asset-catalog names — must match the SDK's runtime lookups and the
/// generated `UILaunchScreen` dictionary.
pub const APPLE_COLOR_ASSET: &str = "LingXiaSplashBackground";
pub const APPLE_MARK_ASSET: &str = "LingXiaSplashMark";

/// Harmony resource names — the start window points at the color and mark;
/// the SDK overlay loads the cover media by name.
const HARMONY_COLOR_RES: &str = "lingxia_splash_background";
const HARMONY_MARK_RES: &str = "lingxia_splash_mark";
const HARMONY_IMAGE_RES: &str = "lingxia_splash";

/// Splash config resolved against the project root: images loaded, color
/// normalized to `#RRGGBB`.
pub struct ResolvedSplash {
    image: Option<DynamicImage>,
    mark: Option<DynamicImage>,
    pub background: String,
}

impl ResolvedSplash {
    pub fn resolve(project_root: &Path, config: &SplashConfig) -> Result<Self> {
        let open = |rel: &Option<String>, what: &str| -> Result<Option<DynamicImage>> {
            match rel {
                Some(rel) => {
                    let path = project_root.join(rel);
                    Ok(Some(image::open(&path).with_context(|| {
                        format!("Failed to open splash {what} {}", path.display())
                    })?))
                }
                None => Ok(None),
            }
        };
        let image = open(&config.image, "image")?;
        let mark = open(&config.mark, "mark")?;
        let background = normalize_hex_rgb(&config.background)
            .with_context(|| "Invalid splash.background".to_string())?;

        Ok(Self {
            image,
            mark,
            background,
        })
    }

    pub fn has_mark(&self) -> bool {
        self.mark.is_some()
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
/// merged by Gradle): the cover drawable the overlay renders as the first
/// frame, and the color themes. The launcher activity picks the theme up
/// through the `lxSplashTheme` manifest placeholder. The configured mark is
/// deliberately not staged — on API 31+ the icon slot belongs to the real app
/// icon, whose launcher-zoom morph the OS composes.
pub fn stage_android_res(splash: &ResolvedSplash, res_dir: &Path) -> Result<()> {
    if let Some(image) = &splash.image {
        save_png(
            &fit_splash(image),
            &res_dir.join(format!("drawable-nodpi/{ANDROID_IMAGE_RES}.png")),
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
        splash.background
    );
    let values_path = res_dir.join("values/lingxia_splash.xml");
    fs::create_dir_all(values_path.parent().unwrap())?;
    fs::write(&values_path, values)?;

    // API 31+: the system splash window takes over the launch frame and cannot
    // render the cover, so what belongs in its icon slot depends on whether a
    // cover is coming.
    //
    // With a cover, the slot is blanked: the cover is the app's real first
    // face, and an icon here would be a second face shown before it. The beat
    // becomes a plain brand-color frame that reads as the cover's entrance.
    //
    // Without one (`image` omitted — the documented placeholder-only launch
    // that holds until the home page is ready) there is no second face to
    // avoid, and blanking the slot leaves the launch showing nothing at all
    // until the home page renders, which reads as a hang rather than a launch.
    // So the slot is left alone and the platform draws the launcher icon,
    // preserving its zoom morph out of the launcher.
    let animated_icon = if splash.image.is_some() {
        "\n        <item name=\"android:windowSplashScreenAnimatedIcon\">@android:color/transparent</item>"
    } else {
        ""
    };
    let v31 = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <style name="{ANDROID_SPLASH_THEME}" parent="Theme.AppCompat.DayNight.NoActionBar">
        <item name="android:windowBackground">@color/{ANDROID_COLOR_RES}</item>
        <item name="android:windowSplashScreenBackground">@color/{ANDROID_COLOR_RES}</item>{animated_icon}
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

/// Ensure a staged `Assets.xcassets` copy carrying the splash color set and
/// mark image set, and return the resources dir to hand to `actool`.
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

/// Loose splash image names in the app bundle. The runtime overlay reads
/// only these, never the asset catalog: `actool` is an external tool that
/// can fail (it needs an installed simulator runtime even for device
/// builds), and the overlay must not go missing when it does.
pub const APPLE_BUNDLE_IMAGE: &str = "LingXiaSplash.png";
pub const APPLE_BUNDLE_MARK: &str = "LingXiaSplashMark.png";

/// Copy the splash images into a built `.app` as plain bundle resources.
pub fn install_apple_bundle_images(app_bundle: &Path, splash: &ResolvedSplash) -> Result<()> {
    if let Some(image) = &splash.image {
        save_png(&fit_splash(image), &app_bundle.join(APPLE_BUNDLE_IMAGE))?;
    }
    if let Some(mark) = &splash.mark {
        save_png(mark, &app_bundle.join(APPLE_BUNDLE_MARK))?;
    }
    Ok(())
}

fn inject_apple_splash_assets(xcassets_dir: &Path, splash: &ResolvedSplash) -> Result<()> {
    // The mark ships as a single 3x entry: `UILaunchScreen` centers it at
    // point size, so 3x makes "authored pixels" mean physical pixels on
    // today's 3x phones — and one fixed point size everywhere else. The
    // overlay reproduces the same math from the loose bundle copy.
    if let Some(mark) = &splash.mark {
        let imageset_dir = xcassets_dir.join(format!("{APPLE_MARK_ASSET}.imageset"));
        fs::create_dir_all(&imageset_dir)?;

        save_png(mark, &imageset_dir.join("mark.png"))?;
        let imageset_contents = json!({
            "images": [{ "idiom": "universal", "filename": "mark.png", "scale": "3x" }],
            "info": {"author": "lingxia", "version": 1},
        });
        fs::write(
            imageset_dir.join("Contents.json"),
            serde_json::to_string_pretty(&imageset_contents)?,
        )?;
    }

    let colorset_dir = xcassets_dir.join(format!("{APPLE_COLOR_ASSET}.colorset"));
    fs::create_dir_all(&colorset_dir)?;
    let colorset_contents = json!({
        "colors": [{
            "idiom": "universal",
            "color": apple_color_components(&splash.background)?,
        }],
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
/// syncs them in place (same model as the managed AppLinks/ACL syncs): the
/// cover media, the mark, color elements, the start-window profile, and the
/// ability wiring in module.json5. `startWindowIcon` becomes the configured
/// mark; without one it is left to the project.
/// Returns whether anything changed.
pub fn sync_harmony_splash(splash: &ResolvedSplash, harmony_dir: &Path) -> Result<bool> {
    let resources_dir = harmony_dir.join("entry/src/main/resources");
    let mut changed = false;

    // The overlay reads the cover by name as raw bytes and decodes it
    // itself, so density qualifiers never touch it.
    let image_path = resources_dir.join(format!("base/media/{HARMONY_IMAGE_RES}.png"));
    if let Some(image) = &splash.image {
        changed |= write_if_changed(&image_path, &png_bytes(&fit_splash(image))?)?;
    } else if image_path.exists() {
        fs::remove_file(&image_path)?;
        changed = true;
    }

    // The mark ships at its authored pixels: the start window draws icons
    // unscaled, which is exactly what keeps it sharp where a full-bleed
    // image cannot be.
    let mark_path = resources_dir.join(format!("base/media/{HARMONY_MARK_RES}.png"));
    if let Some(mark) = &splash.mark {
        changed |= write_if_changed(&mark_path, &png_bytes(mark)?)?;
    } else if mark_path.exists() {
        fs::remove_file(&mark_path)?;
        changed = true;
    }

    changed |= upsert_harmony_color(
        &resources_dir.join("base/element/color.json"),
        HARMONY_COLOR_RES,
        &splash.background,
    )?;

    // Color only, never a full-bleed image: during the launch zoom the start
    // window is composited through a sub-thread cache that softens any asset,
    // and after it a cheap scaler is sharp only at exact screen pixels —
    // which a static resource cannot promise every device. Major apps all
    // ship color plus a small icon here; the icon channel draws 1:1 and stays
    // sharp even mid-animation.
    let profile = serde_json::Map::from_iter([(
        "startWindowBackgroundColor".to_string(),
        json!(format!("$color:{HARMONY_COLOR_RES}")),
    )]);
    let profile_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(profile))?
    );
    changed |= write_if_changed(
        &resources_dir.join("base/profile/start_window.json"),
        profile_json.as_bytes(),
    )?;

    changed |= sync_harmony_start_window(harmony_dir, splash.mark.is_some())?;
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

/// Point the entry ability's start window at the generated profile, keeping
/// the pre-profile fallback fields (color, icon) in agreement with it.
fn sync_harmony_start_window(harmony_dir: &Path, has_mark: bool) -> Result<bool> {
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
    let profile_ref = "$profile:start_window";
    let current_icon = ability_obj
        .get("startWindowIcon")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    // With a configured mark the icon is managed; without one it belongs to
    // the project.
    let wanted_icon = has_mark.then(|| format!("$media:{HARMONY_MARK_RES}"));
    if ability_obj
        .get("startWindowBackground")
        .and_then(Value::as_str)
        == Some(background_ref.as_str())
        && ability_obj.get("startWindow").and_then(Value::as_str) == Some(profile_ref)
        && (wanted_icon.is_none() || wanted_icon == current_icon)
    {
        return Ok(false);
    }
    ability_obj.insert("startWindowBackground".to_string(), json!(background_ref));
    ability_obj.insert("startWindow".to_string(), json!(profile_ref));
    if let Some(icon) = wanted_icon {
        ability_obj.insert("startWindowIcon".to_string(), json!(icon));
    }

    let updated =
        serde_json::to_string_pretty(&root).context("Failed to serialize module.json5")?;
    fs::write(&module_path, format!("{updated}\n"))
        .with_context(|| format!("Failed to write {}", module_path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};

    fn splash(with_cover: bool) -> ResolvedSplash {
        ResolvedSplash {
            image: with_cover.then(|| DynamicImage::ImageRgba8(RgbaImage::new(4, 4))),
            mark: None,
            background: "#130CA2".to_string(),
        }
    }

    fn staged_v31(with_cover: bool) -> String {
        let dir = tempfile::tempdir().unwrap();
        stage_android_res(&splash(with_cover), dir.path()).unwrap();
        fs::read_to_string(dir.path().join("values-v31/lingxia_splash.xml")).unwrap()
    }

    /// A cover is the app's real first face, so the system splash must not
    /// show a second one ahead of it.
    #[test]
    fn cover_blanks_the_api31_icon_slot() {
        assert!(staged_v31(true).contains(
            "<item name=\"android:windowSplashScreenAnimatedIcon\">@android:color/transparent</item>"
        ));
    }

    /// Without a cover there is no second face to avoid, and a blanked slot
    /// would leave the launch showing nothing at all until the home page
    /// renders. The slot is left to the platform so it draws the launcher
    /// icon and keeps the zoom morph.
    #[test]
    fn placeholder_only_launch_keeps_the_real_app_icon() {
        assert!(!staged_v31(false).contains("windowSplashScreenAnimatedIcon"));
    }

    /// Either way the beat is the configured brand colour, never a white flash.
    #[test]
    fn both_shapes_paint_the_configured_background() {
        for with_cover in [true, false] {
            let xml = staged_v31(with_cover);
            assert!(xml.contains(&format!(
                "<item name=\"android:windowSplashScreenBackground\">@color/{ANDROID_COLOR_RES}</item>"
            )));
        }
    }
}
