//! Launch-screen (splash) asset generation.
//!
//! Turns the `splash:` section of `lingxia.yaml` into per-platform launch
//! assets at build time. The launch is two frames that must be one picture:
//! the OS launch frame, composed from build-time resources before the process
//! exists, then the app's first frame, which the SDK fills with the same art.
//! Anything chosen at runtime can only disagree with a frame already on
//! screen — that disagreement is what a "white flash" or a mid-launch swap
//! actually is.
//!
//! - Android: a res overlay (staged outside the source tree) with the art
//!   drawable and a splash theme applied to the launcher activity via the
//!   `lxSplashTheme` manifest placeholder. Android is the one platform whose
//!   OS frame cannot carry the art — the 12+ splash offers a colour and an
//!   icon slot and nothing else — so its frame is the ground, and the art
//!   arrives on the app's first frame.
//! - iOS: a launch storyboard naming the art as a plain bundle resource —
//!   the only launch mechanism that can fill the screen, and so the only one
//!   that can agree with the layer the SDK draws over it. `UILaunchScreen`'s
//!   `UIImageName` has no content mode and centres the image at its natural
//!   point size instead. The storyboards ship precompiled in the CLI, so a
//!   build needs no Interface Builder.
//! - HarmonyOS: start-window colour, the art as the start window's own
//!   background image, and a blanked icon slot, synced into the committed
//!   entry module.
//!
//! One face, drawn in every appearance — see the `splash:` reference for why.
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
/// Whether the launch ground is light, so the theme can ask for dark system
/// bar icons over it. A bool resource rather than a style item so the night
/// bucket can flip it without redeclaring the style.
pub const ANDROID_LIGHT_BARS_RES: &str = "lingxia_splash_light_bars";

/// The ground in the asset catalog, for the SDK overlay's fallback lookup.
/// One universal entry: the fixed face must resolve identically in every
/// appearance.
pub const APPLE_COLOR_ASSET: &str = "LingXiaSplashBackground";
/// The launch storyboard in the app, named by `UILaunchStoryboardName`.
pub const APPLE_LAUNCH_STORYBOARD: &str = "LingXiaLaunchScreen";

/// Harmony resource names — the start window points at the color and mark;
/// the SDK overlay loads the cover media by name.
const HARMONY_COLOR_RES: &str = "lingxia_splash_background";
const HARMONY_MARK_RES: &str = "lingxia_splash_mark";
/// A fully transparent icon. `startWindowIcon` is a required manifest field,
/// so blanking the slot — the way Android's splash theme blanks its own when
/// art is configured — has to be a real resource rather than an omission.
const HARMONY_BLANK_RES: &str = "lingxia_splash_blank";
const HARMONY_IMAGE_RES: &str = "lingxia_splash";

/// The launch face, resolved against the project root: images loaded, colors
/// normalized to `#RRGGBB`. The ground, and the two images drawn on it.
pub struct ResolvedSplash {
    image: Option<DynamicImage>,
    mark: Option<DynamicImage>,
    background: String,
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

        Ok(Self {
            image: open(&config.image, "image")?,
            mark: open(&config.mark, "mark")?,
            background: normalize_hex_rgb(&config.background)
                .with_context(|| "Invalid splash.background".to_string())?,
        })
    }

    /// The ground every launch frame is painted with.
    pub fn background(&self) -> &str {
        &self.background
    }

    pub fn has_image(&self) -> bool {
        self.image.is_some()
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
///
/// One bucket, so the frame is the same picture in every appearance.
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
    <color name="{ANDROID_COLOR_RES}">{background}</color>
    <bool name="{ANDROID_LIGHT_BARS_RES}">{light_bars}</bool>
    <style name="{ANDROID_SPLASH_THEME}" parent="Theme.AppCompat.DayNight.NoActionBar">
        <item name="android:windowBackground">@color/{ANDROID_COLOR_RES}</item>{bars}
    </style>
</resources>
"#,
        background = splash.background,
        light_bars = is_light_ground(&splash.background)?,
        bars = android_bar_items(),
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
    let animated_icon = if splash.has_image() {
        "\n        <item name=\"android:windowSplashScreenAnimatedIcon\">@android:color/transparent</item>"
    } else {
        ""
    };
    // Themes are appearance-agnostic: both reference the same resource names,
    // and the qualified buckets staged above decide what those resolve to. A
    // `values-night` copy of the style would be a second place to keep in
    // sync, for no behavioural difference.
    let v31 = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <style name="{ANDROID_SPLASH_THEME}" parent="Theme.AppCompat.DayNight.NoActionBar">
        <item name="android:windowBackground">@color/{ANDROID_COLOR_RES}</item>
        <item name="android:windowSplashScreenBackground">@color/{ANDROID_COLOR_RES}</item>{animated_icon}{}
    </style>
</resources>
"#,
        android_bar_items()
    );
    let v31_path = res_dir.join("values-v31/lingxia_splash.xml");
    fs::create_dir_all(v31_path.parent().unwrap())?;
    fs::write(&v31_path, v31)?;

    Ok(())
}

/// Style items that keep the system bars part of the launch face.
///
/// The OS splash paints the whole screen — bars included — with the ground,
/// but its exit lands on the bootstrap activity, whose bars otherwise come
/// from AppCompat defaults: a near-black strip top and bottom until the home
/// activity's edge-to-edge chrome takes over. Painting them the ground makes
/// the launch one unbroken surface, and the bool resource derives the icon
/// lightness from that fixed ground without a second style declaration.
fn android_bar_items() -> String {
    format!(
        r#"
        <item name="android:statusBarColor">@color/{ANDROID_COLOR_RES}</item>
        <item name="android:navigationBarColor">@color/{ANDROID_COLOR_RES}</item>
        <item name="android:windowLightStatusBar">@bool/{ANDROID_LIGHT_BARS_RES}</item>
        <item name="android:windowLightNavigationBar">@bool/{ANDROID_LIGHT_BARS_RES}</item>"#
    )
}

/// Whether `#RRGGBB` is a light ground, i.e. wants dark system-bar icons.
fn is_light_ground(hex_rgb: &str) -> Result<bool> {
    let [r, g, b, _] = crate::appicon::parse_hex_color(hex_rgb)?;
    let luminance = 0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b);
    Ok(luminance > 127.5)
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
/// builds), and the overlay must not go missing when it does. The launch
/// storyboard names the same files, so the OS frame and the overlay draw one
/// picture from one source.
pub const APPLE_BUNDLE_IMAGE: &str = "LingXiaSplash.png";
pub const APPLE_BUNDLE_MARK: &str = "LingXiaSplashMark.png";
/// The solid brand ground the launch storyboard stretches under the face.
pub const APPLE_BUNDLE_GROUND: &str = "LingXiaSplashGround.png";
/// The mark again, named `@3x` so the storyboard draws it at the point size
/// the overlay gives it; the overlay's own copy stays where it looks.
pub const APPLE_BUNDLE_LAUNCH_MARK: &str = "LingXiaLaunchMark@3x.png";

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

/// The launch storyboards, compiled once by `assets/apple/launch/compile.sh`.
/// They name only the bundle images above, so one compiled copy serves every
/// app and a build never needs Interface Builder.
static APPLE_LAUNCH_STORYBOARDS: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/assets/apple/launch/compiled");

/// Which compiled storyboard draws this splash: the cover filling the
/// screen, else the centred mark, else the ground alone.
fn apple_launch_variant(splash: &ResolvedSplash) -> &'static str {
    match (&splash.image, &splash.mark) {
        (Some(_), _) => "cover",
        (None, Some(_)) => "mark",
        (None, None) => "ground",
    }
}

/// Install the OS launch face into a built `.app`, and point Info.plist at it.
///
/// A storyboard, not the `UILaunchScreen` dictionary. That dictionary has no
/// content mode: `UIImageName` draws the image at its natural *point* size,
/// centred — a 1024x2048 cover lands 2.3x oversized on a phone, and the
/// overlay's aspect-filled copy then snaps it back. Two pictures, one launch.
/// A storyboard pins the art to the edges and fills, which is what the
/// overlay does and what HarmonyOS's start window does with
/// `startWindowBackgroundImageFit: Cover`.
pub fn install_apple_launch_screen(app_bundle: &Path, splash: &ResolvedSplash) -> Result<()> {
    // Solid, so any size stretches to the same picture.
    const GROUND_PX: u32 = 16;
    let [r, g, b, _] = crate::appicon::parse_hex_color(splash.background())?;
    let ground = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        GROUND_PX,
        GROUND_PX,
        image::Rgb([r, g, b]),
    ));
    save_png(&ground, &app_bundle.join(APPLE_BUNDLE_GROUND))?;
    let variant = apple_launch_variant(splash);
    if variant == "mark"
        && let Some(mark) = &splash.mark
    {
        save_png(mark, &app_bundle.join(APPLE_BUNDLE_LAUNCH_MARK))?;
    }

    let compiled = APPLE_LAUNCH_STORYBOARDS
        .get_dir(format!("{variant}.storyboardc"))
        .with_context(|| format!("the CLI embeds no `{variant}` launch storyboard"))?;
    let dest = app_bundle.join(format!("{APPLE_LAUNCH_STORYBOARD}.storyboardc"));
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    write_embedded_dir(compiled, compiled.path(), &dest)?;

    let path = app_bundle.join("Info.plist");
    let mut info: plist::Dictionary =
        plist::from_file(&path).context("Failed to read Info.plist for the launch face")?;
    info.remove("UILaunchScreen");
    info.insert(
        "UILaunchStoryboardName".into(),
        APPLE_LAUNCH_STORYBOARD.into(),
    );
    plist::to_file_xml(&path, &info).context("Failed to write Info.plist for the launch face")
}

/// Write an embedded directory's files under `dest`, relative to `root`.
fn write_embedded_dir(dir: &include_dir::Dir<'_>, root: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for file in dir.files() {
        let target = dest.join(file.path().strip_prefix(root)?);
        fs::write(&target, file.contents())
            .with_context(|| format!("Failed to write {}", target.display()))?;
    }
    for sub in dir.dirs() {
        write_embedded_dir(sub, root, &dest.join(sub.path().strip_prefix(root)?))?;
    }
    Ok(())
}

/// Only the ground colour: the launch face itself is a bundle resource the
/// storyboard names, so `actool` failing costs the app its icon and nothing
/// else. The colour stays in the catalog for the SDK overlay's fallback
/// lookup on hosts whose Info.plist predates the raw value.
fn inject_apple_splash_assets(xcassets_dir: &Path, splash: &ResolvedSplash) -> Result<()> {
    let colorset_dir = xcassets_dir.join(format!("{APPLE_COLOR_ASSET}.colorset"));
    fs::create_dir_all(&colorset_dir)?;
    let colorset_contents = json!({
        "colors": [{
            "idiom": "universal",
            "color": apple_color_components(splash.background())?,
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

    changed |= sync_harmony_face(splash, &resources_dir)?;
    // A `dark` bucket would be a second face; clear whatever a previous build
    // left there so the qualifier falls back to `base` in every appearance.
    changed |= clear_harmony_dark_face(&resources_dir)?;

    // The OS frame carries the launch art itself. It can, because the art is
    // build-time: the frame the OS composes before the process exists and the
    // frame the SDK draws over it are the same picture. That is what removes
    // the handoff — and with it the flat brand frame that reads as a white
    // flash whenever the ground is light.
    let profile = harmony_start_window_profile(splash.has_image());
    let profile_json = format!("{}\n", serde_json::to_string_pretty(&profile)?);
    changed |= write_if_changed(
        &resources_dir.join("base/profile/start_window.json"),
        profile_json.as_bytes(),
    )?;

    // With art, the icon slot is blanked — the same rule Android's system
    // splash follows: the art is the app's real first face, and a mark drawn
    // over it is a second one, which is what an icon flickering before the
    // launch screen actually is.
    changed |= sync_harmony_blank_icon(&resources_dir, splash.has_image())?;
    changed |= sync_harmony_start_window(harmony_dir, splash.has_image(), splash.has_mark())?;
    Ok(changed)
}

/// The 1x1 transparent PNG the blanked icon slot points at.
fn sync_harmony_blank_icon(resources_dir: &Path, has_image: bool) -> Result<bool> {
    let path = resources_dir.join(format!("base/media/{HARMONY_BLANK_RES}.png"));
    if !has_image {
        if path.exists() {
            fs::remove_file(&path)?;
            return Ok(true);
        }
        return Ok(false);
    }
    let blank = DynamicImage::new_rgba8(1, 1);
    write_if_changed(&path, &png_bytes(&blank)?)
}

fn harmony_start_window_profile(has_image: bool) -> Value {
    let mut profile = serde_json::Map::from_iter([
        (
            "startWindowBackgroundColor".to_string(),
            json!(format!("$color:{HARMONY_COLOR_RES}")),
        ),
        (
            "startWindowColorModeType".to_string(),
            json!("FOLLOW_SYSTEM"),
        ),
    ]);
    if has_image {
        profile.insert(
            "startWindowBackgroundImage".to_string(),
            json!(format!("$media:{HARMONY_IMAGE_RES}")),
        );
        // The same fit the SDK's own layer uses, so the two frames crop the
        // art identically and the handoff has nothing to move.
        profile.insert("startWindowBackgroundImageFit".to_string(), json!("Cover"));
    }
    Value::Object(profile)
}

/// Write the `base` bucket: the cover the overlay reads, the start window's
/// mark, and the ground colour element.
fn sync_harmony_face(splash: &ResolvedSplash, resources_dir: &Path) -> Result<bool> {
    let mut changed = false;

    // The overlay reads the cover by name as raw bytes and decodes it
    // itself, so density qualifiers never touch it.
    let image_path = resources_dir.join(format!("base/media/{HARMONY_IMAGE_RES}.png"));
    match &splash.image {
        Some(image) => changed |= write_if_changed(&image_path, &png_bytes(&fit_splash(image))?)?,
        None => changed |= remove_if_present(&image_path)?,
    }

    // The mark ships at its authored pixels: the start window draws icons
    // unscaled, which is exactly what keeps it sharp where a full-bleed
    // image cannot be.
    let mark_path = resources_dir.join(format!("base/media/{HARMONY_MARK_RES}.png"));
    match &splash.mark {
        Some(mark) => changed |= write_if_changed(&mark_path, &png_bytes(mark)?)?,
        None => changed |= remove_if_present(&mark_path)?,
    }

    changed |= upsert_harmony_color(
        &resources_dir.join("base/element/color.json"),
        HARMONY_COLOR_RES,
        &splash.background,
    )?;
    Ok(changed)
}

/// Remove whatever a build that still staged a dark face left behind.
fn clear_harmony_dark_face(resources_dir: &Path) -> Result<bool> {
    let mut changed = false;
    for res in [HARMONY_IMAGE_RES, HARMONY_MARK_RES] {
        changed |= remove_if_present(&resources_dir.join(format!("dark/media/{res}.png")))?;
    }
    changed |= remove_harmony_color(
        &resources_dir.join("dark/element/color.json"),
        HARMONY_COLOR_RES,
    )?;
    Ok(changed)
}

fn remove_if_present(path: &Path) -> Result<bool> {
    if path.exists() {
        fs::remove_file(path)?;
        return Ok(true);
    }
    Ok(false)
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

/// Drop one `{name, value}` entry from a Harmony color.json, leaving the rest
/// of the file — and the file itself — alone.
fn remove_harmony_color(path: &Path, name: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let mut root: Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    let Some(colors) = root.get_mut("color").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    let before = colors.len();
    colors.retain(|entry| entry.get("name").and_then(Value::as_str) != Some(name));
    if colors.len() == before {
        return Ok(false);
    }
    let serialized = format!("{}\n", serde_json::to_string_pretty(&root)?);
    write_if_changed(path, serialized.as_bytes())
}

/// Point the entry ability's start window at the generated profile, keeping
/// the pre-profile fallback fields (color, icon) in agreement with it.
fn sync_harmony_start_window(harmony_dir: &Path, has_image: bool, has_mark: bool) -> Result<bool> {
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
    // Three shapes, one rule: whatever the OS frame draws must be the app's
    // real first face and nothing else. With art there is no icon at all —
    // an icon over the art is a second face, seen as a flicker before the
    // launch screen. Without art the mark is the face. With neither, the icon
    // belongs to the project.
    let wanted_icon = match (has_image, has_mark) {
        (true, _) => Some(format!("$media:{HARMONY_BLANK_RES}")),
        (false, true) => Some(format!("$media:{HARMONY_MARK_RES}")),
        (false, false) => current_icon.clone(),
    };
    if ability_obj
        .get("startWindowBackground")
        .and_then(Value::as_str)
        == Some(background_ref.as_str())
        && ability_obj.get("startWindow").and_then(Value::as_str) == Some(profile_ref)
        && wanted_icon == current_icon
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

    fn art() -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(4, 4))
    }

    fn splash(with_cover: bool) -> ResolvedSplash {
        ResolvedSplash {
            image: with_cover.then(art),
            mark: None,
            background: "#130CA2".to_string(),
        }
    }

    fn splash_with_mark() -> ResolvedSplash {
        ResolvedSplash {
            image: Some(art()),
            mark: Some(art()),
            background: "#F4F2ED".to_string(),
        }
    }

    fn staged_v31(with_cover: bool) -> String {
        let dir = tempfile::tempdir().unwrap();
        stage_android_res(&splash(with_cover), dir.path()).unwrap();
        fs::read_to_string(dir.path().join("values-v31/lingxia_splash.xml")).unwrap()
    }

    /// Both launch frames must be the same picture, so the OS frame carries
    /// the configured art with the fit the SDK's own layer uses.
    #[test]
    fn harmony_start_window_carries_the_launch_art() {
        let profile = harmony_start_window_profile(true);
        assert_eq!(
            profile["startWindowBackgroundImage"].as_str(),
            Some("$media:lingxia_splash")
        );
        assert_eq!(
            profile["startWindowBackgroundImageFit"].as_str(),
            Some("Cover")
        );
    }

    /// A placeholder-only launch has no art to carry, and the ground plus the
    /// mark are the whole face.
    #[test]
    fn harmony_start_window_without_art_is_ground_only() {
        let profile = harmony_start_window_profile(false);
        assert!(profile.get("startWindowBackgroundImage").is_none());
        assert_eq!(
            profile["startWindowBackgroundColor"].as_str(),
            Some("$color:lingxia_splash_background")
        );
    }

    /// A cover is the app's real first face, so the system splash must not
    /// show a second one ahead of it.
    /// The OS frame must draw what the overlay draws: the cover when there is
    /// one, else the centred mark, else the ground alone.
    #[test]
    fn launch_storyboard_follows_the_configured_face() {
        assert_eq!(apple_launch_variant(&splash(true)), "cover");
        assert_eq!(apple_launch_variant(&splash_with_mark()), "cover");
        let mut mark_only = splash_with_mark();
        mark_only.image = None;
        assert_eq!(apple_launch_variant(&mark_only), "mark");
        assert_eq!(apple_launch_variant(&splash(false)), "ground");
        for variant in ["cover", "mark", "ground"] {
            assert!(
                APPLE_LAUNCH_STORYBOARDS
                    .get_dir(format!("{variant}.storyboardc"))
                    .is_some(),
                "{variant} not compiled: run assets/apple/launch/compile.sh"
            );
        }
    }

    /// No Interface Builder: the compiled storyboard is copied in, with the
    /// images it names.
    #[test]
    fn launch_screen_installs_the_precompiled_storyboard() {
        let dir = tempfile::tempdir().unwrap();
        plist::to_file_xml(dir.path().join("Info.plist"), &plist::Dictionary::new()).unwrap();
        let mut mark_only = splash_with_mark();
        mark_only.image = None;

        install_apple_launch_screen(dir.path(), &mark_only).unwrap();

        let storyboard = dir
            .path()
            .join(format!("{APPLE_LAUNCH_STORYBOARD}.storyboardc"));
        assert!(fs::read_dir(&storyboard).unwrap().next().is_some());
        assert!(dir.path().join(APPLE_BUNDLE_GROUND).is_file());
        assert!(dir.path().join(APPLE_BUNDLE_LAUNCH_MARK).is_file());
        let info: plist::Dictionary = plist::from_file(dir.path().join("Info.plist")).unwrap();
        assert_eq!(
            info["UILaunchStoryboardName"].as_string(),
            Some(APPLE_LAUNCH_STORYBOARD)
        );
        assert!(!info.contains_key("UILaunchScreen"));
    }

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

    /// The launch frame owns the system bars: both are painted the ground and
    /// the icon lightness follows it, so the OS splash's exit never reveals a
    /// black strip the home activity has to clean up.
    #[test]
    fn system_bars_carry_the_ground() {
        let dir = tempfile::tempdir().unwrap();
        stage_android_res(&splash_with_mark(), dir.path()).unwrap();

        for file in ["values/lingxia_splash.xml", "values-v31/lingxia_splash.xml"] {
            let xml = fs::read_to_string(dir.path().join(file)).unwrap();
            assert!(
                xml.contains(&format!(
                    "<item name=\"android:statusBarColor\">@color/{ANDROID_COLOR_RES}</item>"
                )),
                "{file} must colour the status bar"
            );
            assert!(
                xml.contains(&format!(
                    "<item name=\"android:windowLightNavigationBar\">@bool/{ANDROID_LIGHT_BARS_RES}</item>"
                )),
                "{file} must key icon lightness off the bool"
            );
        }
        // A light ground wants dark icons over it.
        let values = fs::read_to_string(dir.path().join("values/lingxia_splash.xml")).unwrap();
        assert!(values.contains(&format!(
            "<bool name=\"{ANDROID_LIGHT_BARS_RES}\">true</bool>"
        )));
    }

    /// One bucket: a night copy would be a second face, and for the style a
    /// duplicate declaration rather than an override, which AGP treats as a
    /// build error.
    #[test]
    fn android_stages_one_bucket_only() {
        let dir = tempfile::tempdir().unwrap();
        stage_android_res(&splash_with_mark(), dir.path()).unwrap();

        assert!(!dir.path().join("values-night").exists());
        assert!(!dir.path().join("drawable-night-nodpi").exists());
    }
}
