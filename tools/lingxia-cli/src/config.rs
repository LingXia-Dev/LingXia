use anyhow::{Context, Result, anyhow};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use serde_yaml_ng as yaml;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub const HOST_CONFIG_FILE: &str = "lingxia.yaml";
pub const LXAPP_BUILD_CONFIG_FILE: &str = "lxapp.config.ts";
const AUTHORING_PLATFORMS: &[&str] = &["macos", "windows", "ios", "android", "harmony"];
const DESKTOP_SURFACE_PLATFORMS: &[&str] = &["macos", "windows"];
const CONTENT_AGNOSTIC_MAIN_PLATFORMS: &[&str] = &["macos", "windows"];

/// Host project configuration (native app project)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LingXiaConfig {
    /// Host app settings used to generate `app.json` at build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<HostAppConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<AndroidConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ios: Option<IosConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macos: Option<MacosConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harmony: Option<HarmonyConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<WindowsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<FeaturesConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<CapabilitiesConfig>,
    /// Application-wide native UI colors emitted into `app.json`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<BrowserConfig>,
    /// Generated UI structure (`ui.json`). Built from `surfaces` at load time;
    /// never authored directly.
    #[serde(skip)]
    pub generated_ui: Option<Value>,
    /// Top-level `surfaces:` — the UI authoring format. Mapped into `ui` during
    /// `load`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surfaces: Option<Vec<SurfaceDecl>>,
    #[serde(rename = "appLinks", skip_serializing_if = "Option::is_none")]
    pub app_links: Option<AppLinksConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesConfig>,
    /// Launch screen: a static platform splash generated at build time plus a
    /// runtime overlay the SDK keeps up until the home page's first render.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub splash: Option<SplashConfig>,
    /// Directory of host assets (relative to the project root), packaged
    /// into every platform build and readable at runtime through
    /// `lingxia::assets`. Rides each platform's asset pipeline — store
    /// thinning, lazy loading — where bytes embedded in the native library
    /// cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SplashConfig {
    /// Background color, `#RRGGBB` — the ground of the OS launch placeholder,
    /// and what shows behind a hook-picked cover. The only required field: no
    /// code runs before the launch frame, so only build-time config can brand
    /// it. A dark color keeps the placeholder from ever reading as a white
    /// flash.
    ///
    /// Deliberately one color, not a light/dark pair: the launch frame and
    /// the first app frame must be the same color, and a pair would let them
    /// disagree.
    pub background: String,
    /// The launch cover (PNG), path relative to the project root. Rendered
    /// full-screen (aspect-fill) as the app's first frame on every cold
    /// start — the OS placeholder's exit reveals it, so the launch reads as
    /// "tap the icon, see the cover". Omit for a placeholder-only launch.
    /// The runtime hook can substitute a different file per launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// The brand mark (PNG) shown centered on the launch placeholder, at the
    /// pixel size it should occupy on screen, path relative to the project
    /// root. OS launch frames draw small images unscaled — the one form
    /// their compositors keep sharp, where any full-bleed bitmap goes soft.
    /// Used where the frame accepts a custom image (HarmonyOS
    /// `startWindowIcon`, iOS `UILaunchScreen`); Android 12+ keeps the real
    /// app icon, whose launcher-zoom morph must not be broken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<String>,
    /// Minimum time the launch face (placeholder or cover) stays up, in
    /// milliseconds (default 600). Keeps a fast first render from flashing
    /// it. The hard upper bound is a framework constant and deliberately not
    /// configurable — a splash that can be configured to never leave is a
    /// failure mode, not a feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_duration: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeaturesConfig {
    #[serde(default = "default_true")]
    pub app_service: bool,
    #[serde(default)]
    pub devtools: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            app_service: true,
            devtools: false,
        }
    }
}

// One shared definition with the runtime (which reads it back from app.json),
// so a capability can never exist on one side only.
pub use lingxia_app_context::{CapabilitiesConfig, ThemeConfig};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webui: Option<BrowserWebUiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserWebUiConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppLinksConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageConfig {
    #[serde(rename = "tempMaxSizeMB")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp_max_size_mb: Option<u64>,
    #[serde(rename = "cacheMaxSizeMB")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_max_size_mb: Option<u64>,
    #[serde(rename = "dataMaxSizeMB")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_max_size_mb: Option<u64>,
    #[serde(rename = "appStorageMaxSizeMB")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_storage_max_size_mb: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<ResourceBundleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceBundleConfig {
    #[serde(rename = "type", default)]
    pub bundle_type: ResourceBundleType,
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ResourceBundleType {
    #[default]
    Lxapp,
}

// ---------------------------------------------------------------------------
// Top-level `surfaces:` authoring format.
//
// This is INPUT schema only. `surfaces_to_ui` maps it into the internal
// generated `ui.json` structure (`launch`/`surfaces`/`activators`) consumed by
// native runtimes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceRole {
    Main,
    Aside,
    Float,
}

/// One `surfaces:` entry, keyed by content: exactly one of `lxapp` / `page` /
/// `url` / `native` names the content, and that key doubles as the surface's
/// identity — there is no separate `id` and no `render` discriminator.
///
/// Admission is target-aware. Desktop targets accept content-agnostic mains;
/// mobile targets keep the home-lxapp startup contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceDecl {
    /// Content key: an lxapp, by appId.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lxapp: Option<String>,
    /// Content key: a home-lxapp page, by page name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// Content key: a URL opened in the in-app browser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Content key: a host-registered native capability (e.g. `terminal`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<String>,
    pub role: SurfaceRole,
    /// At most one `role: main` may set `launch: true` (the initial surface).
    #[serde(default)]
    pub launch: bool,
    /// Aside docking edge, one of left|right|top|bottom. Optional: asides
    /// default to `right`; a native capability may carry its own default
    /// (terminal: `bottom`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<String>,
    /// Preferred aside size hint (points). The shell clamps it at admission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<SurfaceSize>,
    /// Inline tray/menubar entry. Required on a declared float.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tray: Option<SurfaceTray>,
    /// Availability filter. Omitted = follows `app.platforms`. When present,
    /// it must be a non-empty subset of `app.platforms` using canonical tokens:
    /// `macos`, `windows`, `ios`, `android`, `harmony`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platforms: Option<Vec<String>>,
}

/// A surface declaration's resolved content key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceContent<'a> {
    Lxapp(&'a str),
    Page(&'a str),
    Url(&'a str),
    Native(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSurfaceName {
    Terminal,
    Browser,
}

impl NativeSurfaceName {
    fn parse(value: &str) -> Result<Self> {
        match value.trim() {
            "terminal" => Ok(Self::Terminal),
            "browser" => Ok(Self::Browser),
            other => Err(anyhow!(
                "native surface '{other}' is not supported; expected terminal or browser"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Browser => "browser",
        }
    }
}

impl<'a> SurfaceContent<'a> {
    fn kind(self) -> &'static str {
        match self {
            SurfaceContent::Lxapp(_) => "lxapp",
            SurfaceContent::Page(_) => "page",
            SurfaceContent::Url(_) => "url",
            SurfaceContent::Native(_) => "native",
        }
    }

    fn name(self) -> &'a str {
        match self {
            SurfaceContent::Lxapp(name)
            | SurfaceContent::Page(name)
            | SurfaceContent::Url(name)
            | SurfaceContent::Native(name) => name,
        }
    }

    fn trimmed(self) -> Self {
        match self {
            SurfaceContent::Lxapp(name) => SurfaceContent::Lxapp(name.trim()),
            SurfaceContent::Page(name) => SurfaceContent::Page(name.trim()),
            SurfaceContent::Url(name) => SurfaceContent::Url(name.trim()),
            SurfaceContent::Native(name) => SurfaceContent::Native(name.trim()),
        }
    }

    fn native_name(self) -> Result<Option<NativeSurfaceName>> {
        match self {
            SurfaceContent::Native(value) => NativeSurfaceName::parse(value).map(Some),
            _ => Ok(None),
        }
    }
}

impl SurfaceDecl {
    /// Resolve the content key. Exactly one of `lxapp` / `page` / `url` /
    /// `native` must be set to a non-empty value.
    pub fn content(&self) -> Result<SurfaceContent<'_>> {
        let keys = [
            self.lxapp.as_deref().map(SurfaceContent::Lxapp),
            self.page.as_deref().map(SurfaceContent::Page),
            self.url.as_deref().map(SurfaceContent::Url),
            self.native.as_deref().map(SurfaceContent::Native),
        ];
        // An empty value is diagnosed before the exactly-one count so that
        // e.g. `lxapp: ""` + `url: ...` names the real problem.
        for key in keys.iter().flatten() {
            if key.name().trim().is_empty() {
                return Err(anyhow!(
                    "surfaces[]: the {} content key must not be empty",
                    key.kind()
                ));
            }
        }
        let mut set = keys.iter().flatten();
        match (set.next(), set.next()) {
            (Some(one), None) => Ok(one.trimmed()),
            (None, _) => Err(anyhow!(
                "surfaces[]: each entry must set exactly one content key (lxapp | page | url | native)"
            )),
            (Some(_), Some(_)) => Err(anyhow!(
                "surfaces[]: entry sets more than one content key; use exactly one of lxapp | page | url | native"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceTray {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<SurfaceTrayAction>,
    /// When true the app lives only in the menu bar (macOS) / system tray
    /// (Windows) with no dock / taskbar icon (tray-only). Default false keeps
    /// the dock / taskbar icon alongside the tray.
    #[serde(default)]
    pub exclusive: bool,
    /// Size of the popover this tray opens (its content area, in points). Applies
    /// when the surface is a tray-anchored popover (`role: float`). Omit for the
    /// default size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<SurfaceSize>,
}

/// Content-area size in points. On a tray popover it is the popover size.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceSize {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum SurfaceTrayAction {
    #[default]
    Toggle,
    Activate,
}

/// Map the `surfaces:` declaration into the internal `ui.json` structure
/// consumed by native runtimes.
///
/// Mapping:
/// - `lxapp` + `role: main` -> surface `role: main`, content
///   `{ kind: lxapp, appId }`. `launch: true` -> `launch.initialSurface`.
/// - `lxapp`/`url` + `role: aside` -> surface `role: aside`,
///   `attachTo: <launch surface>`, edge defaulting to `right`.
/// - `native: terminal` + `role: aside` -> the built-in terminal surface,
///   edge defaulting to `bottom`. `capabilities.terminal` does not invent a
///   terminal surface.
/// - `url` -> content `{ kind: url, url }` (requires the browser capability).
/// - `native` -> content `{ kind: native, name }`.
/// - `tray` -> a `menuBarItem` activator (closest existing kind).
///
/// There is no `sidebar:` entry field: persistent entries are declared at
/// runtime through the shell activator API, never in YAML.
#[cfg(test)]
fn surfaces_to_ui(
    surfaces: &[SurfaceDecl],
    terminal_enabled: bool,
    browser_enabled: bool,
) -> Result<Value> {
    let home_app_id = surfaces
        .iter()
        .find_map(|surface| {
            (surface.role == SurfaceRole::Main)
                .then(|| surface.content().ok())
                .flatten()
                .and_then(|content| match content {
                    SurfaceContent::Lxapp(app_id) => Some(app_id),
                    _ => None,
                })
        })
        .unwrap_or_default();
    surfaces_to_ui_for_target(
        surfaces,
        terminal_enabled,
        browser_enabled,
        "macos",
        home_app_id,
    )
}

fn surfaces_to_ui_for_target(
    surfaces: &[SurfaceDecl],
    terminal_enabled: bool,
    browser_enabled: bool,
    platform: &str,
    home_app_id: &str,
) -> Result<Value> {
    let desktop = DESKTOP_SURFACE_PLATFORMS.contains(&platform);
    let content_agnostic_main = CONTENT_AGNOSTIC_MAIN_PLATFORMS.contains(&platform);
    let mut resolved: Vec<(SurfaceContent<'_>, &SurfaceDecl)> = Vec::new();
    let mut seen_names = HashSet::new();
    for surface in surfaces {
        let content = surface.content()?;
        content.native_name()?;
        if !seen_names.insert(content.name().to_string()) {
            return Err(anyhow!(
                "surfaces: duplicate declaration for '{}' (surface ids share one namespace across content kinds)",
                content.name()
            ));
        }
        resolved.push((content, surface));
    }

    let tray_surfaces = surfaces
        .iter()
        .filter(|surface| surface.tray.is_some())
        .count();
    if tray_surfaces > 1 {
        return Err(anyhow!("surfaces: at most one surface may declare tray"));
    }

    for (content, surface) in &resolved {
        let name = content.name();
        if let SurfaceContent::Url(url) = *content {
            validate_declared_surface_url(url)?;
        }
        if surface.launch && surface.role != SurfaceRole::Main {
            return Err(anyhow!(
                "surface '{name}': launch: true is only valid on a main surface"
            ));
        }
        if surface.edge.is_some() && surface.role != SurfaceRole::Aside {
            return Err(anyhow!(
                "surface '{name}': edge is only valid on an aside surface"
            ));
        }
        if surface.size.is_some() && surface.role != SurfaceRole::Aside {
            return Err(anyhow!(
                "surface '{name}': size is only valid on an aside surface (a float popover's size lives under tray.size)"
            ));
        }
        if surface.role == SurfaceRole::Float && surface.tray.is_none() {
            return Err(anyhow!(
                "surface '{name}' uses role: float, which is only supported as a tray-anchored popover (add a tray:)"
            ));
        }
    }

    let effective = resolved
        .iter()
        .copied()
        .filter(|(_, surface)| surface_available_for_target(surface, platform))
        .collect::<Vec<_>>();
    if effective.is_empty() {
        return Err(anyhow!(
            "surfaces: no surface is available on target {platform}"
        ));
    }

    for (content, surface) in &effective {
        let name = content.name();
        match (*content, surface.role) {
            (SurfaceContent::Lxapp(_), _) => {}
            (SurfaceContent::Page(_), _) => {
                return Err(anyhow!(
                    "surface '{name}': declarative page surfaces are not supported on {platform}"
                ));
            }
            (SurfaceContent::Url(_), SurfaceRole::Main) if !content_agnostic_main => {
                return Err(anyhow!(
                    "surface '{name}': url main surfaces are currently supported only on macOS and Windows"
                ));
            }
            (SurfaceContent::Url(_), SurfaceRole::Aside) if platform == "macos" => {
                return Err(anyhow!(
                    "surface '{name}': declarative url asides are not supported on macOS; open the browser aside at runtime"
                ));
            }
            (SurfaceContent::Url(_), SurfaceRole::Main | SurfaceRole::Aside) => {
                if !browser_enabled {
                    return Err(anyhow!(
                        "surface '{name}': a url surface requires the browser capability"
                    ));
                }
            }
            (SurfaceContent::Url(_), SurfaceRole::Float) => {
                return Err(anyhow!(
                    "surface '{name}': a url surface only supports role: main or aside"
                ));
            }
            (SurfaceContent::Native(native), SurfaceRole::Main) => {
                if !content_agnostic_main {
                    return Err(anyhow!(
                        "surface '{name}': native main surfaces are currently supported only on macOS and Windows"
                    ));
                }
                match NativeSurfaceName::parse(native)? {
                    NativeSurfaceName::Terminal if !terminal_enabled => {
                        return Err(anyhow!(
                            "surface '{name}' uses the native terminal but capabilities.terminal is not enabled"
                        ));
                    }
                    NativeSurfaceName::Browser if !browser_enabled => {
                        return Err(anyhow!(
                            "surface '{name}' uses the native browser but capabilities.browser is not enabled"
                        ));
                    }
                    _ => {}
                }
            }
            (SurfaceContent::Native(native), SurfaceRole::Aside) => {
                if NativeSurfaceName::parse(native)? != NativeSurfaceName::Terminal {
                    return Err(anyhow!(
                        "surface '{name}': native browser only supports role: main"
                    ));
                }
                if !desktop {
                    return Err(anyhow!(
                        "surface '{name}': native terminal asides are supported only on macOS and Windows"
                    ));
                }
                if !terminal_enabled {
                    return Err(anyhow!(
                        "surface '{name}' uses the native terminal but capabilities.terminal is not enabled"
                    ));
                }
            }
            (SurfaceContent::Native(_), SurfaceRole::Float) => {
                return Err(anyhow!(
                    "surface '{name}': native surfaces do not support role: float"
                ));
            }
        }
    }

    let launch_mains: Vec<_> = effective
        .iter()
        .filter(|(_, surface)| surface.role == SurfaceRole::Main && surface.launch)
        .collect();
    if launch_mains.len() > 1 {
        return Err(anyhow!(
            "surfaces: at most one main surface may set launch: true on {platform}"
        ));
    }

    let mains = effective
        .iter()
        .copied()
        .filter(|(_, surface)| surface.role == SurfaceRole::Main)
        .collect::<Vec<_>>();
    let floats = effective
        .iter()
        .copied()
        .filter(|(_, surface)| surface.role == SurfaceRole::Float)
        .collect::<Vec<_>>();

    let (launch_content, open_on_launch) = if content_agnostic_main {
        if !mains.is_empty() {
            if !floats.is_empty() {
                return Err(anyhow!(
                    "surfaces: {platform} cannot combine main surfaces with a tray float root"
                ));
            }
            if mains.len() != 1 {
                return Err(anyhow!(
                    "surfaces: {platform} requires exactly one declared main surface"
                ));
            }
            let explicit = launch_mains.first().map(|(content, _)| *content);
            if explicit.is_none() && !effective.iter().any(|(_, surface)| surface.tray.is_some()) {
                return Err(anyhow!(
                    "surfaces: {platform} mains without launch: true require a tray activator"
                ));
            }
            (explicit.unwrap_or(mains[0].0), explicit.is_some())
        } else {
            if floats.len() != 1 || floats[0].1.tray.is_none() {
                return Err(anyhow!(
                    "surfaces: {platform} requires at least one main or one tray-anchored float"
                ));
            }
            if effective
                .iter()
                .any(|(_, surface)| surface.role == SurfaceRole::Aside)
            {
                return Err(anyhow!(
                    "surfaces: a pure tray float configuration cannot contain asides"
                ));
            }
            (floats[0].0, false)
        }
    } else if desktop {
        if !mains.is_empty() {
            if !floats.is_empty() {
                return Err(anyhow!(
                    "surfaces: {platform} cannot combine main surfaces with a tray float root"
                ));
            }
            if mains.len() != 1 {
                return Err(anyhow!(
                    "surfaces: {platform} currently requires exactly one home lxapp main"
                ));
            }
            let SurfaceContent::Lxapp(initial_app_id) = mains[0].0 else {
                return Err(anyhow!(
                    "surfaces: {platform} initial surface must be the home lxapp '{home_app_id}'"
                ));
            };
            if initial_app_id != home_app_id {
                return Err(anyhow!(
                    "surfaces: {platform} initial surface must be the home lxapp '{home_app_id}', got '{initial_app_id}'"
                ));
            }
            let explicit = launch_mains.first().map(|(content, _)| *content);
            if explicit.is_none() && !effective.iter().any(|(_, surface)| surface.tray.is_some()) {
                return Err(anyhow!(
                    "surfaces: {platform} mains without launch: true require a tray activator"
                ));
            }
            (mains[0].0, explicit.is_some())
        } else {
            if floats.len() != 1 || floats[0].1.tray.is_none() {
                return Err(anyhow!(
                    "surfaces: {platform} requires one home lxapp main or one tray-anchored float"
                ));
            }
            if effective
                .iter()
                .any(|(_, surface)| surface.role == SurfaceRole::Aside)
            {
                return Err(anyhow!(
                    "surfaces: a pure tray float configuration cannot contain asides"
                ));
            }
            (floats[0].0, false)
        }
    } else {
        if mains.len() != 1 {
            return Err(anyhow!(
                "surfaces: {platform} requires exactly one home lxapp main"
            ));
        }
        let SurfaceContent::Lxapp(initial_app_id) = mains[0].0 else {
            return Err(anyhow!(
                "surfaces: {platform} initial surface must be the home lxapp '{home_app_id}'"
            ));
        };
        if initial_app_id != home_app_id {
            return Err(anyhow!(
                "surfaces: {platform} initial surface must be the home lxapp '{home_app_id}', got '{initial_app_id}'"
            ));
        }
        if let Some((launch_content, _)) = launch_mains.first()
            && launch_content.name() != home_app_id
        {
            return Err(anyhow!(
                "surfaces: {platform} launch: true is only valid on the home lxapp '{home_app_id}'"
            ));
        }
        (mains[0].0, true)
    };
    let launch_id = launch_content.name().to_string();

    let mut out_surfaces: Vec<Value> = Vec::new();
    let mut out_activators: Vec<Value> = Vec::new();

    for (content, surface) in &effective {
        let name = content.name();
        let mut out = Map::new();
        out.insert("id".into(), json!(name));
        match surface.role {
            SurfaceRole::Float => {
                // Tray-anchored popover (float always carries tray, checked
                // above). Emit it anchored to the tray activator so the runtime
                // presents it as an auto-dismissing panel under the icon.
                out.insert("role".into(), json!("float"));
                out.insert("anchor".into(), json!("activator"));
                out.insert("content".into(), surface_content_json(*content)?);
                if let Some(size) = surface
                    .tray
                    .as_ref()
                    .and_then(|t| t.size.as_ref())
                    .and_then(size_to_json)
                {
                    out.insert("size".into(), size);
                }
            }
            SurfaceRole::Main => {
                out.insert("role".into(), json!("main"));
                out.insert("content".into(), surface_content_json(*content)?);
            }
            SurfaceRole::Aside => {
                let default_edge = match content {
                    SurfaceContent::Native(_) => "bottom",
                    _ => "right",
                };
                let edge = surface
                    .edge
                    .as_deref()
                    .map(str::trim)
                    .filter(|e| !e.is_empty())
                    .unwrap_or(default_edge);
                let edge = map_edge(edge, name)?;
                if matches!(content.native_name()?, Some(NativeSurfaceName::Terminal))
                    && edge != "bottom"
                    && edge != "top"
                {
                    return Err(anyhow!(
                        "terminal surface '{name}' must use edge 'top' or 'bottom'"
                    ));
                }
                out.insert("role".into(), json!("aside"));
                out.insert("attachTo".into(), json!(launch_id));
                out.insert("edge".into(), json!(edge));
                let mut size = surface.size.as_ref().and_then(size_to_json);
                if matches!(content.native_name()?, Some(NativeSurfaceName::Terminal)) {
                    // The terminal keeps its historical default height, also
                    // when a size hint sets only the width.
                    let obj = size
                        .get_or_insert_with(|| json!({}))
                        .as_object_mut()
                        .expect("size_to_json emits an object");
                    obj.entry("height").or_insert(json!(320));
                }
                if let Some(size) = size {
                    out.insert("size".into(), size);
                }
                out.insert("content".into(), surface_content_json(*content)?);
            }
        }
        out_surfaces.push(Value::Object(out));

        if let Some(tray) = &surface.tray {
            // The internal schema's closest existing kind is `menuBarItem`.
            // (There is no dedicated status/tray runtime kind today.)
            let mut activator = Map::new();
            activator.insert("id".into(), json!(format!("{name}Tray")));
            activator.insert("kind".into(), json!("menuBarItem"));
            if let Some(icon) = tray
                .icon
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                activator.insert("icon".into(), json!(icon));
            }
            if let Some(label) = tray
                .label
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                activator.insert("label".into(), json!(label));
            }
            let action_kind = match tray.action.unwrap_or_default() {
                SurfaceTrayAction::Toggle => "toggleSurface",
                SurfaceTrayAction::Activate => "openSurface",
            };
            activator.insert(
                "action".into(),
                json!({ "kind": action_kind, "surface": name }),
            );
            out_activators.push(Value::Object(activator));
        }
    }

    let mut launch = Map::new();
    launch.insert("initialSurface".into(), json!(launch_id));
    launch.insert("openOnLaunch".into(), json!(open_on_launch));
    // An exclusive tray hides the dock / taskbar icon. Drives LSUIElement +
    // .accessory on macOS and WS_EX_TOOLWINDOW on Windows.
    let hide_dock_icon = effective
        .iter()
        .any(|(_, surface)| surface.tray.as_ref().is_some_and(|tray| tray.exclusive));
    if hide_dock_icon {
        launch.insert("hideDockIcon".into(), json!(true));
    }

    Ok(json!({
        "launch": launch,
        "surfaces": out_surfaces,
        "activators": out_activators
    }))
}

fn surface_available_for_target(surface: &SurfaceDecl, platform: &str) -> bool {
    surface
        .platforms
        .as_ref()
        .is_none_or(|platforms| platforms.iter().any(|candidate| candidate == platform))
}

fn validate_declared_surface_url(url: &str) -> Result<()> {
    let (scheme, rest) = url
        .split_once(':')
        .ok_or_else(|| anyhow!("surface '{url}': url must be absolute"))?;
    let scheme = scheme.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "https" | "file") {
        return Err(anyhow!(
            "surface '{url}': url scheme must be https or a host-authorized file"
        ));
    }
    if !rest.starts_with("//") {
        return Err(anyhow!("surface '{url}': url must use {scheme}:// syntax"));
    }
    Ok(())
}

fn surface_content_json(content: SurfaceContent<'_>) -> Result<Value> {
    Ok(match content {
        SurfaceContent::Lxapp(app_id) => json!({ "kind": "lxapp", "appId": app_id }),
        SurfaceContent::Page(page) => json!({ "kind": "page", "page": page }),
        SurfaceContent::Url(url) => json!({ "kind": "url", "url": url }),
        SurfaceContent::Native(capability) => {
            let name = NativeSurfaceName::parse(capability)?.as_str();
            json!({ "kind": "native", "name": name })
        }
    })
}

fn size_to_json(size: &SurfaceSize) -> Option<Value> {
    let mut obj = Map::new();
    if let Some(width) = size.width {
        obj.insert("width".into(), json!(width));
    }
    if let Some(height) = size.height {
        obj.insert("height".into(), json!(height));
    }
    if obj.is_empty() {
        None
    } else {
        Some(Value::Object(obj))
    }
}

fn map_edge(edge: &str, id: &str) -> Result<&'static str> {
    Ok(match edge {
        "left" => "left",
        "right" => "right",
        "top" => "top",
        "bottom" => "bottom",
        other => {
            return Err(anyhow!(
                "aside surface '{id}' has invalid edge '{other}'; expected left|right|top|bottom"
            ));
        }
    })
}

fn is_authoring_platform(value: &str) -> bool {
    AUTHORING_PLATFORMS.contains(&value)
}

/// Capabilities that only mean something alongside another.
///
/// Deriving the feature independently turns a typo into a build that
/// contradicts what was declared — every browser command answering "feature
/// unavailable" in a product whose configuration says it drives the browser.
/// Better to refuse with a sentence naming the missing line.
fn validate_capability_dependencies(capabilities: Option<&CapabilitiesConfig>) -> Result<()> {
    let Some(capabilities) = capabilities else {
        return Ok(());
    };
    if capabilities.browser_use && !capabilities.browser {
        return Err(anyhow!(
            "capabilities.browserUse drives the in-app browser, which this product does not have; \
             add `browser: true` or remove `browserUse`"
        ));
    }
    Ok(())
}

fn validate_app_platforms(app: &HostAppConfig) -> Result<Vec<String>> {
    if app.platforms.is_empty() {
        return Err(anyhow!("app.platforms must include at least one platform"));
    }

    let mut seen = HashSet::new();
    let mut platforms = Vec::new();
    for (index, raw) in app.platforms.iter().enumerate() {
        let platform = raw.as_str();
        if !is_authoring_platform(platform) {
            return Err(anyhow!(
                "app.platforms[{index}] has unsupported platform '{raw}'; expected one of: {}",
                AUTHORING_PLATFORMS.join(", ")
            ));
        }
        if !seen.insert(platform.to_string()) {
            return Err(anyhow!(
                "app.platforms contains duplicate platform '{platform}'"
            ));
        }
        platforms.push(platform.to_string());
    }
    Ok(platforms)
}

fn validate_host_without_home_lxapp(
    config: &LingXiaConfig,
    app_platforms: &[String],
) -> Result<()> {
    if let Some(platform) = app_platforms
        .iter()
        .find(|platform| !DESKTOP_SURFACE_PLATFORMS.contains(&platform.as_str()))
    {
        return Err(anyhow!(
            "app.homeAppId is required for {platform}; hosts without a home lxapp are currently supported only on macOS and Windows"
        ));
    }
    if config.app_service_enabled() {
        return Err(anyhow!(
            "features.appService must be false when app.homeAppId is omitted"
        ));
    }
    let surfaces = config.surfaces.as_deref().ok_or_else(|| {
        anyhow!(
            "surfaces is required when app.homeAppId is omitted; declare native: terminal or native: browser as the main surface"
        )
    })?;
    for platform in app_platforms {
        let mains = surfaces
            .iter()
            .filter(|surface| {
                surface.role == SurfaceRole::Main && surface_available_for_target(surface, platform)
            })
            .collect::<Vec<_>>();
        if mains.len() != 1 {
            return Err(anyhow!(
                "surfaces: {platform} requires exactly one native main when app.homeAppId is omitted"
            ));
        }
        let SurfaceContent::Native(name) = mains[0].content()? else {
            return Err(anyhow!(
                "surfaces: {platform} main must be native: terminal or native: browser when app.homeAppId is omitted"
            ));
        };
        NativeSurfaceName::parse(name)?;
    }
    Ok(())
}

fn validate_surface_platforms(surfaces: &[SurfaceDecl], app_platforms: &[String]) -> Result<()> {
    let app_platform_set: HashSet<&str> = app_platforms.iter().map(String::as_str).collect();

    for surface in surfaces {
        let content = surface.content()?;
        let id = content.name();
        let Some(platforms) = surface.platforms.as_ref() else {
            validate_surface_intrinsic_platforms(surface, id, app_platforms)?;
            continue;
        };

        if platforms.is_empty() {
            return Err(anyhow!(
                "surface '{id}': platforms must not be empty; omit platforms to follow app.platforms"
            ));
        }

        let mut seen = HashSet::new();
        let mut effective = Vec::new();
        for (index, raw) in platforms.iter().enumerate() {
            let platform = raw.as_str();
            if !is_authoring_platform(platform) {
                return Err(anyhow!(
                    "surface '{id}': platforms[{index}] has unsupported platform '{raw}'; expected one of: {}",
                    AUTHORING_PLATFORMS.join(", ")
                ));
            }
            if !app_platform_set.contains(platform) {
                return Err(anyhow!(
                    "surface '{id}': platform '{platform}' is not listed in app.platforms"
                ));
            }
            if !seen.insert(platform.to_string()) {
                return Err(anyhow!(
                    "surface '{id}': platforms contains duplicate platform '{platform}'"
                ));
            }
            effective.push(platform.to_string());
        }
        validate_surface_intrinsic_platforms(surface, id, &effective)?;
    }
    Ok(())
}

fn validate_surface_intrinsic_platforms(
    surface: &SurfaceDecl,
    id: &str,
    effective_platforms: &[String],
) -> Result<()> {
    if matches!(
        surface.content()?.native_name()?,
        Some(NativeSurfaceName::Terminal | NativeSurfaceName::Browser)
    ) {
        for platform in effective_platforms {
            if !DESKTOP_SURFACE_PLATFORMS.contains(&platform.as_str()) {
                return Err(anyhow!(
                    "surface '{id}' is a native surface and only supports platforms: {}",
                    DESKTOP_SURFACE_PLATFORMS.join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// Host app settings (checked into git via `lingxia.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostAppConfig {
    /// Project name (technical identifier, used for native build paths, the
    /// Swift target name, the default package-id base, etc.).
    pub project_name: String,

    /// Directory name of the native Rust library crate, relative to the project
    /// root. `lingxia new` writes `native`. Optional for backward compatibility:
    /// when omitted, the legacy `<projectName>-lib` directory is assumed.
    #[serde(default)]
    #[serde(rename = "rustLibDir")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_lib_dir: Option<String>,

    /// Product name (user-facing display name)
    pub product_name: String,
    pub product_version: String,

    /// Optional cloud server. Single string applies to all envs; per-env map
    /// lets you point dev/preview/release at different backends. Apps with
    /// no cloud component simply omit this field.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lingxia_server: Option<LingxiaServer>,

    #[serde(default)]
    #[serde(rename = "lingxiaId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lingxia_id: Option<String>,

    /// Optional overrides for the built-in env-version package-id suffixes
    /// (`.dev` / `.preview` / none). Specify `""` to opt out of a default,
    /// e.g. `developer: ""` keeps the developer build using the base id.
    /// Almost no projects need this — the defaults match the common case.
    #[serde(default)]
    #[serde(rename = "packageIdSuffix")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_id_suffix: Option<PackageIdSuffixOverrides>,

    /// Platforms to build for this app (e.g. ["android"]).
    pub platforms: Vec<String>,

    /// Product control lxapp. Desktop hosts whose main surface is fully native
    /// may omit it; mobile hosts still require one.
    #[serde(default)]
    #[serde(rename = "homeAppId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_app_id: Option<String>,
}

/// Cloud server config. `Single("...")` applies the same URL to every env;
/// `PerEnv {...}` selects per-env URLs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LingxiaServer {
    Single(String),
    PerEnv(PerEnvServer),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerEnvServer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
}

impl LingxiaServer {
    /// Return the URL that applies to `version`, or `None` if not configured
    /// for that env. `Single` always returns the same value.
    pub fn for_env(&self, version: EnvVersion) -> Option<&str> {
        match self {
            LingxiaServer::Single(url) => Some(url.as_str()),
            LingxiaServer::PerEnv(per) => match version {
                EnvVersion::Developer => per.developer.as_deref(),
                EnvVersion::Preview => per.preview.as_deref(),
                EnvVersion::Release => per.release.as_deref(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageIdSuffixOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
}

impl PackageIdSuffixOverrides {
    pub fn for_env(&self, version: EnvVersion) -> Option<&str> {
        match version {
            EnvVersion::Developer => self.developer.as_deref(),
            EnvVersion::Preview => self.preview.as_deref(),
            EnvVersion::Release => self.release.as_deref(),
        }
    }
}

/// Canonical env-version enum. Wire-compatible with `lingxia_update::ReleaseType`
/// — both serialize as lowercase `"developer" | "preview" | "release"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EnvVersion {
    Developer,
    Preview,
    #[default]
    Release,
}

impl EnvVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Developer => "developer",
            Self::Preview => "preview",
            Self::Release => "release",
        }
    }

    /// Parse the user-facing CLI value. Case-sensitive on purpose — clap's
    /// `value_parser` already restricts inputs to the lowercase forms below,
    /// so accepting other cases here would silently widen the contract.
    pub fn parse_cli(value: &str) -> Result<Self> {
        match value.trim() {
            "developer" | "dev" => Ok(Self::Developer),
            "preview" => Ok(Self::Preview),
            "release" => Ok(Self::Release),
            other => Err(anyhow!(
                "unknown env version '{other}'; valid: developer (or dev), preview, release"
            )),
        }
    }

    /// Built-in default `packageIdSuffix` for this environment. Used when the
    /// override block doesn't specify one — most projects never need to. An
    /// explicit `packageIdSuffix: ""` in YAML opts out (no suffix at all).
    pub fn default_package_id_suffix(self) -> Option<&'static str> {
        match self {
            Self::Developer => Some(".dev"),
            Self::Preview => Some(".preview"),
            Self::Release => None,
        }
    }
}

impl std::fmt::Display for EnvVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Resolved per-build environment context, single source of truth threaded
/// through the build pipeline (asset generation + each platform builder).
#[derive(Debug, Clone)]
pub struct ResolvedEnv {
    pub version: EnvVersion,
    pub lingxia_server: String,
    /// `None` means "do not append a suffix". `Some` always means "append
    /// this exact string" — `effective_package_id_suffix()` already filters
    /// out empty strings.
    pub package_id_suffix: Option<String>,
}

impl ResolvedEnv {
    /// Suffix to apply to package/bundle IDs, or `None` when no suffix
    /// should be appended. Empty strings are treated as no-suffix.
    pub fn effective_package_id_suffix(&self) -> Option<&str> {
        self.package_id_suffix
            .as_deref()
            .filter(|suffix| !suffix.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidConfig {
    pub package_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndk_version: Option<String>,
    /// API level for NDK toolchain (e.g., 21 for android21-clang)
    /// If not specified, will be derived from minSdk, then targetSdk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_level: Option<u32>,
    /// Google Play Console identity for `lingxia store --platform googleplay`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google_play_store: Option<GooglePlayConfig>,
    /// Xiaomi GetApps identity for `lingxia store --platform xiaomi`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xiaomi_store: Option<XiaomiStoreConfig>,
    /// OPPO open-platform identity for `lingxia store --platform oppo`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oppo_store: Option<OppoStoreConfig>,
    /// Honor AppGallery identity for `lingxia store --platform honor`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub honor_store: Option<HonorStoreConfig>,
}

/// Google Play submission identity. Lives in `lingxia.yaml` under
/// `android.googlePlayStore`; credentials live in
/// `~/.lingxia/store/credentials.toml` (`[googleplay]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GooglePlayConfig {
    /// Play `applicationId` (the package name, e.g. `app.lingxia.example`).
    pub package_name: String,
    /// Default release track when `--track` is omitted (e.g. `internal`,
    /// `alpha`, `beta`, `production`). Defaults to `internal`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_track: Option<String>,
}

/// Xiaomi GetApps submission identity. Lives in `lingxia.yaml` under
/// `android.xiaomiStore`; credentials live in
/// `~/.lingxia/store/credentials.toml` (`[xiaomi]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct XiaomiStoreConfig {
    /// Application package name (e.g. `app.lingxia.example`).
    pub package_name: String,
}

/// OPPO software-store submission identity. Lives in `lingxia.yaml` under
/// `android.oppoStore`; credentials live in
/// `~/.lingxia/store/credentials.toml` (`[oppo]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OppoStoreConfig {
    /// Application package name (e.g. `app.lingxia.example`).
    pub package_name: String,
    /// OPPO numeric app id, if the open-platform API requires it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
}

/// Honor AppGallery submission identity. Lives in `lingxia.yaml` under
/// `android.honorStore`; credentials live in
/// `~/.lingxia/store/credentials.toml` (`[honor]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HonorStoreConfig {
    /// Honor Developer numeric app id.
    pub app_id: String,
    /// Application package name (e.g. `app.lingxia.example`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
}

impl AndroidConfig {
    /// Get the API level to use for NDK toolchain
    pub fn get_api_level(&self) -> u32 {
        // 1. Explicit API level takes priority
        if let Some(api) = self.api_level {
            return api;
        }

        // 2. Derive from minSdk (keeps native ABI compatible with oldest supported Android)
        if let Some(min) = self.min_sdk {
            return min;
        }

        // 3. Fallback to targetSdk
        if let Some(target) = self.target_sdk {
            return target;
        }

        // 4. Default to 33
        33
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IosConfig {
    pub bundle_id: String,
    /// Optional Apple Team constraint. When present it is hard: only
    /// credentials proven to belong to this team may be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_target: Option<String>, // e.g., "17.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swift_version: Option<String>,
    /// SwiftPM target name for resources lookup.
    /// If omitted, CLI will try app.projectName or infer from Sources/ when unambiguous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
    /// App Store Connect identity for `lingxia store`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<AppStoreConfig>,
}

/// App Store Connect submission identity. Lives in `lingxia.yaml` under
/// `ios.store` / `macos.store`; credentials live in
/// `~/.lingxia/store/credentials.toml` (`[appstore]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppStoreConfig {
    /// The app's bundle identifier (must match the App Store Connect record).
    pub bundle_id: String,
    /// The App Store Connect numeric app id (the "Apple ID" of the app).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacosConfig {
    /// Bundle identifier (e.g., "app.lingxia.example")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,

    /// Optional Apple Team constraint. When present it is hard: only
    /// credentials proven to belong to this team may be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,

    /// Deployment target (e.g., "14.0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_target: Option<String>,

    /// Executable product name (SwiftPM). If omitted, CLI will try a few
    /// reasonable defaults and fall back to "the only executable in bin dir".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_name: Option<String>,

    /// SwiftPM target name for resources lookup.
    /// If omitted, CLI will try app.projectName or infer from Sources/ when unambiguous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,

    /// App Store Connect identity for `lingxia store`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<AppStoreConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarmonyConfig {
    pub bundle_name: String,
    /// Minimum supported SDK version (e.g., "5.0.0(12)")
    /// Equivalent to iOS deploymentTarget / Android minSdk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compatible_sdk_version: Option<String>,
    /// Target SDK version (e.g., "6.0.1(21)")
    /// Equivalent to Android targetSdk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sdk_version: Option<String>,
    /// Huawei AppGallery Connect identity for `lingxia store`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<AppGalleryConfig>,
}

/// Huawei AppGallery Connect submission identity. Lives in `lingxia.yaml`
/// under `harmony.store`; credentials live in
/// `~/.lingxia/store/credentials.toml` (`[appgallery]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppGalleryConfig {
    /// AppGallery Connect app id.
    pub app_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsConfig {
    /// Windows host application identifier. Env suffixes are applied the same
    /// way as package/bundle identifiers on other platforms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// Cargo binary name produced by windows/Cargo.toml.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_name: Option<String>,
    /// MSIX package Identity `Publisher` (a distinguished name such as
    /// `CN=Contoso`). Must match the signing certificate's subject. Defaults to
    /// `CN=<productName>` when omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    /// Microsoft Store (Partner Center) identity for `lingxia store`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<MsStoreConfig>,
}

/// Microsoft Store (Partner Center) submission identity. Lives in
/// `lingxia.yaml` under `windows.store`; credentials live in
/// `~/.lingxia/store/credentials.toml` (`[msstore]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MsStoreConfig {
    /// Partner-Center-reserved Store ID (app id) for the application.
    pub app_id: String,
    /// Optional reserved package name (display only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
}

impl LingXiaConfig {
    /// Get the project name from config
    pub fn get_project_name(&self) -> Option<&str> {
        self.app.as_ref().map(|app| app.project_name.as_str())
    }

    /// Get the Rust library directory name.
    ///
    /// Prefers the explicit `app.rustLibDir` (written as `native` by
    /// `lingxia new`). Falls back to the legacy `<projectName>-lib` convention
    /// for projects scaffolded before the field existed, so existing projects
    /// keep building unchanged.
    pub fn get_rust_lib_name(&self) -> Option<String> {
        let configured = self
            .app
            .as_ref()
            .and_then(|app| app.rust_lib_dir.as_deref())
            .map(str::trim)
            .filter(|dir| !dir.is_empty());
        match configured {
            Some(dir) => Some(dir.to_string()),
            None => self.get_project_name().map(|name| format!("{}-lib", name)),
        }
    }

    pub fn app_service_enabled(&self) -> bool {
        self.features
            .as_ref()
            .map(|features| features.app_service)
            .unwrap_or(true)
    }

    /// The adaptive-layout host shell (window + sidebar chrome) and webview input
    /// are the desktop baseline — always present on macOS and Windows. The
    /// browser it can dock is a separate opt-in capability (`browser_enabled`).
    pub fn desktop_runtime_enabled(&self, platform: &str) -> bool {
        matches!(platform, "macos" | "windows")
    }

    /// The in-app browser capability — cross-platform and opt-in. Gates the
    /// browser runtime/feature and the bundling of its webui pages everywhere.
    pub fn browser_enabled(&self) -> bool {
        self.capabilities
            .as_ref()
            .map(|capabilities| capabilities.browser)
            .unwrap_or(false)
    }

    pub fn terminal_enabled(&self, platform: &str) -> bool {
        let terminal_requested = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.terminal)
            .unwrap_or(false);
        terminal_requested && matches!(platform, "macos" | "windows")
    }

    pub fn process_enabled(&self, platform: &str) -> bool {
        let process_requested = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.process)
            .unwrap_or(false);
        process_requested && matches!(platform, "macos" | "windows")
    }

    pub fn proxy_enabled(&self, platform: &str) -> bool {
        let proxy_requested = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.proxy)
            .unwrap_or(false);
        proxy_requested && self.browser_enabled() && matches!(platform, "macos" | "windows")
    }

    /// The local control socket, derived from whatever the product actually
    /// asked for. Desktop only — a phone has no command line to drive the
    /// product from, and no per-user IPC namespace to scope it to.
    pub fn control_enabled(&self, platform: &str) -> bool {
        let requested = self
            .capabilities
            .as_ref()
            .map(CapabilitiesConfig::needs_control_socket)
            .unwrap_or(false);
        requested && matches!(platform, "macos" | "windows")
    }

    /// Machine-wide automation. Rides the control socket, which its own
    /// presence turns on.
    pub fn computer_use_enabled(&self, platform: &str) -> bool {
        let requested = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.computer_use)
            .unwrap_or(false);
        requested && matches!(platform, "macos" | "windows")
    }

    pub fn media_capture_enabled(&self) -> bool {
        self.capabilities
            .as_ref()
            .map(|capabilities| capabilities.media_capture_enabled())
            .unwrap_or(false)
    }

    /// Driving the in-app browser. The handlers live behind their own devtool
    /// feature, so a product that declared `browserUse` and did not get it
    /// would answer every browser command with "feature unavailable" — a
    /// capability the user granted and the build silently dropped.
    pub fn browser_use_enabled(&self, platform: &str) -> bool {
        let requested = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.browser_use)
            .unwrap_or(false);
        requested && matches!(platform, "macos" | "windows")
    }

    pub fn devtools_enabled(&self) -> bool {
        self.features
            .as_ref()
            .map(|features| features.devtools)
            .unwrap_or(false)
    }

    pub fn native_features_for_platform(&self, platform: &str) -> Vec<String> {
        let mut features = Vec::new();
        if self.app_service_enabled() {
            features.push("standard".to_string());
        }
        if self.process_enabled(platform) {
            features.push("process".to_string());
        }
        // The browser capability brings its runtime and webui pages (newtab /
        // settings / downloads) on every platform it is enabled for.
        if self.browser_enabled() {
            features.push("browser-shell".to_string());
        }
        if self.terminal_enabled(platform) {
            features.push("terminal-runtime".to_string());
        }
        if self.desktop_runtime_enabled(platform) {
            features.push("webview-input".to_string());
        }
        if self.proxy_enabled(platform) {
            features.push("proxy".to_string());
        }
        if self.control_enabled(platform) {
            features.push("control".to_string());
        }
        if self.computer_use_enabled(platform) {
            features.push("computer-use".to_string());
        }
        if self.media_capture_enabled() {
            match platform {
                "macos" | "windows" => features.push("desktop-realtime-capture".to_string()),
                "android" => features.push("android-capture".to_string()),
                "ios" => features.push("apple-capture".to_string()),
                "harmony" => features.push("harmony-capture".to_string()),
                _ => {}
            }
        }
        if self.browser_use_enabled(platform) {
            features.push("browser-use".to_string());
        }
        if self.devtools_enabled() {
            features.push("devtools".to_string());
        }
        features
    }

    pub fn native_features_for_platform_with_extra(
        &self,
        platform: &str,
        extra_features: &[String],
    ) -> Vec<String> {
        let mut features = self.native_features_for_platform(platform);
        append_native_features(&mut features, extra_features);
        features
    }

    pub fn native_default_features_enabled(&self) -> bool {
        self.app_service_enabled()
    }

    /// Load config from `lingxia.yaml` in the given directory.
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(HOST_CONFIG_FILE);
        if !config_path.exists() {
            anyhow::bail!(
                "{} not found in {}. Run 'lingxia new' to create a new project.",
                HOST_CONFIG_FILE,
                project_root.display()
            );
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;

        let mut config: LingXiaConfig = yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", config_path.display()))?;
        config.theme = config.theme.take().and_then(ThemeConfig::normalized);
        config.apply_surfaces()?;
        config.validate()?;

        Ok(config)
    }

    #[allow(dead_code)]
    /// Save config to `lingxia.yaml` in the given directory.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let config_path = project_root.join(HOST_CONFIG_FILE);

        let content = yaml::to_string(self).context("Failed to serialize config")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write {}", HOST_CONFIG_FILE))?;

        Ok(())
    }

    /// Create a default Android config
    #[allow(dead_code)] // Used in tests
    pub fn new_android(project_name: &str, package_id: &str, home_app_id: &str) -> Self {
        Self {
            app: Some(HostAppConfig {
                project_name: project_name.to_string(),
                rust_lib_dir: None,
                product_name: project_name.to_string(),
                product_version: "0.0.1".to_string(),
                lingxia_server: Some(LingxiaServer::Single("https://api.example.com".to_string())),
                lingxia_id: None,
                package_id_suffix: None,
                platforms: vec!["android".to_string()],
                home_app_id: Some(home_app_id.to_string()),
            }),
            android: Some(AndroidConfig {
                package_id: package_id.to_string(),
                min_sdk: Some(28),
                target_sdk: Some(35),
                compile_sdk: Some(35),
                ndk_version: None, // Auto-detect
                api_level: None,   // Derive from minSdk/targetSdk
                google_play_store: None,
                xiaomi_store: None,
                oppo_store: None,
                honor_store: None,
            }),
            ios: None,
            macos: None,
            harmony: None,
            windows: None,
            features: Some(FeaturesConfig::default()),
            capabilities: Some(CapabilitiesConfig::default()),
            theme: None,
            browser: None,
            generated_ui: None,
            surfaces: None,
            app_links: None,
            storage: None,
            resources: Some(ResourcesConfig {
                bundles: vec![ResourceBundleConfig {
                    bundle_type: ResourceBundleType::Lxapp,
                    app_id: home_app_id.to_string(),
                    path: Some(home_app_id.to_string()),
                    package: None,
                    version: None,
                }],
            }),
            splash: None,
            assets: None,
        }
    }

    /// Map the top-level `surfaces:` block into the generated `ui` structure
    /// consumed by the runtime.
    fn apply_surfaces(&mut self) -> Result<()> {
        let Some(surfaces) = self.surfaces.as_ref() else {
            return Ok(());
        };
        if surfaces.is_empty() {
            return Err(anyhow!("surfaces: must contain at least one surface"));
        }
        let app = self
            .app
            .as_ref()
            .ok_or_else(|| anyhow!("surfaces requires app.platforms"))?;
        let app_platforms = validate_app_platforms(app)?;
        validate_surface_platforms(surfaces, &app_platforms)?;
        let terminal_enabled = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.terminal)
            .unwrap_or(false);
        let browser_enabled = self.browser_enabled();
        let home_app_id = app.home_app_id.as_deref().unwrap_or_default().trim();
        let first_platform = app_platforms
            .first()
            .ok_or_else(|| anyhow!("app.platforms must not be empty"))?;
        self.generated_ui = Some(surfaces_to_ui_for_target(
            surfaces,
            terminal_enabled,
            browser_enabled,
            first_platform,
            home_app_id,
        )?);
        Ok(())
    }

    pub(crate) fn resolved_ui_for_platform(&self, platform: &str) -> Result<Option<Value>> {
        let Some(surfaces) = self.surfaces.as_ref() else {
            return Ok(self.generated_ui.clone());
        };
        let app = self
            .app
            .as_ref()
            .ok_or_else(|| anyhow!("surfaces requires app.platforms"))?;
        surfaces_to_ui_for_target(
            surfaces,
            self.terminal_enabled(platform),
            self.browser_enabled(),
            platform,
            app.home_app_id.as_deref().unwrap_or_default().trim(),
        )
        .map(Some)
    }

    fn validate(&self) -> Result<()> {
        validate_capability_dependencies(self.capabilities.as_ref())?;
        if let Some(app) = &self.app {
            if app.project_name.trim().is_empty() {
                return Err(anyhow!("app.projectName must not be empty"));
            }
            if app.product_name.trim().is_empty() {
                return Err(anyhow!("app.productName must not be empty"));
            }
            Version::parse(app.product_version.trim()).map_err(|_| {
                anyhow!("app.productVersion must be a semantic version (major.minor.patch)")
            })?;
            let app_platforms = validate_app_platforms(app)?;
            let process_requested = self
                .capabilities
                .as_ref()
                .is_some_and(|capabilities| capabilities.process);
            if process_requested && !self.app_service_enabled() {
                return Err(anyhow!(
                    "capabilities.process requires features.appService: true"
                ));
            }
            if process_requested
                && !app_platforms
                    .iter()
                    .any(|platform| matches!(platform.as_str(), "macos" | "windows"))
            {
                return Err(anyhow!(
                    "capabilities.process is supported only by macOS and Windows hosts"
                ));
            }
            let home_app_id = app.home_app_id.as_deref().map(str::trim);
            if home_app_id.is_some_and(str::is_empty) {
                return Err(anyhow!("app.homeAppId must not be empty when set"));
            }
            if let Some(home_app_id) = home_app_id
                && is_home_forbidden_app_id(home_app_id)
            {
                return Err(anyhow!(
                    "app.homeAppId '{home_app_id}' is an SDK-reserved appId. Pick a different id \
                     for your home app (e.g. the project's reverse-domain identifier)."
                ));
            }
            if home_app_id.is_none() {
                validate_host_without_home_lxapp(self, &app_platforms)?;
            }
            if let Some(server) = app.lingxia_server.as_ref() {
                validate_lingxia_server(server)?;
            }
            if let Some(over) = app.package_id_suffix.as_ref() {
                validate_package_id_suffix_overrides(over)?;
            }
            for platform in &app_platforms {
                let Some(ui) = self.resolved_ui_for_platform(platform)? else {
                    if platform == "macos" {
                        return Err(anyhow!(
                            "surfaces is required for macOS host app projects; define top-level surfaces:"
                        ));
                    }
                    continue;
                };
                if platform == "macos" {
                    validate_macos_ui_config(
                        &ui,
                        self.terminal_enabled("macos"),
                        self.browser_enabled(),
                    )?;
                }
            }
        }
        if let Some(windows) = &self.windows {
            if windows
                .app_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(anyhow!("windows.appId must not be empty"));
            }
            if windows
                .executable_name
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err(anyhow!("windows.executableName must not be empty"));
            }
        }
        if let Some(app_links) = &self.app_links {
            for host in &app_links.hosts {
                validate_applink_host(host)?;
            }
        }
        if let Some(resources) = &self.resources {
            let mut app_ids = HashSet::new();
            for bundle in &resources.bundles {
                let app_id = bundle.app_id.trim();
                if app_id.is_empty() {
                    return Err(anyhow!("resources.bundles[].appId must not be empty"));
                }
                if is_resource_forbidden_app_id(app_id) {
                    return Err(anyhow!(
                        "resources.bundles[{app_id}] uses an SDK-reserved appId. \
                         Customize it with `browser.webui.path` (or `browser.webui.package`) \
                         instead of declaring `{app_id}` as a resource bundle."
                    ));
                }
                // Bundles land in the asset root as a directory named by
                // appId, where `hostassets/` is the `assets:` namespace.
                if app_id == "hostassets" {
                    return Err(anyhow!(
                        "resources.bundles appId 'hostassets' collides with the \
                         host-assets namespace (`assets:` in lingxia.yaml)"
                    ));
                }
                if !app_ids.insert(app_id.to_string()) {
                    return Err(anyhow!("resources.bundles appId must be unique: {app_id}"));
                }
                let has_path = bundle
                    .path
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty());
                let has_package = bundle
                    .package
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty());
                if has_path && has_package {
                    return Err(anyhow!(
                        "resources.bundles[{app_id}] must not set both path and package"
                    ));
                }
                if bundle
                    .version
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| value.is_empty())
                {
                    return Err(anyhow!(
                        "resources.bundles[{app_id}].version must not be empty"
                    ));
                }
            }
        }
        if let Some(webui) = self
            .browser
            .as_ref()
            .and_then(|browser| browser.webui.as_ref())
        {
            let has_path = webui
                .path
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            let has_package = webui
                .package
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            if has_path && has_package {
                return Err(anyhow!(
                    "browser.webui must use either path or package, not both"
                ));
            }
            if webui
                .version
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| value.is_empty())
            {
                return Err(anyhow!("browser.webui.version must not be empty"));
            }
        }
        if let Some(ui) = &self.generated_ui
            && !ui.is_object()
        {
            return Err(anyhow!("ui must be a JSON object"));
        }
        Ok(())
    }
}

pub fn append_native_features(features: &mut Vec<String>, extra_features: &[String]) {
    for feature in extra_features {
        let feature = feature.trim();
        if feature.is_empty() || features.iter().any(|existing| existing == feature) {
            continue;
        }
        features.push(feature.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacosUiSurfaceRole {
    Main,
    Aside,
    Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacosUiContentKind {
    Lxapp,
    Url,
    NativeTerminal,
    NativeBrowser,
}

#[derive(Debug, Clone)]
struct MacosUiSurface {
    role: MacosUiSurfaceRole,
    content_kind: MacosUiContentKind,
    attach_to: Option<String>,
    edge: Option<String>,
}

fn non_empty_str<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("{field} must be a non-empty string"))
}

fn optional_non_empty_str(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

/// App IDs whose assets are owned by SDK-internal hosts rather than resource bundles.
///
/// Source of truth for each entry (kept in sync manually to avoid pulling the
/// full browser runtime into the CLI build):
/// - `crate::host_assets::BROWSER_SHELL_WEBUI_APP_ID` mirrors `lingxia_browser::BUILTIN_BROWSER_APPID`.
const RESOURCE_FORBIDDEN_APP_IDS: &[&str] = &[
    crate::host_assets::BROWSER_SHELL_WEBUI_APP_ID,
    "app.lingxia.host-surface-owner",
];

fn is_resource_forbidden_app_id(app_id: &str) -> bool {
    RESOURCE_FORBIDDEN_APP_IDS.contains(&app_id)
}

fn is_home_forbidden_app_id(app_id: &str) -> bool {
    RESOURCE_FORBIDDEN_APP_IDS.contains(&app_id)
}

fn validate_applink_host(host: &str) -> Result<()> {
    let raw_host = host;
    let host = raw_host.trim();
    if host.is_empty() {
        return Err(anyhow!("appLinks.hosts entries must not be empty"));
    }
    if host.len() != raw_host.len() {
        return Err(anyhow!(
            "appLinks.hosts entries must not contain surrounding whitespace"
        ));
    }
    if host.len() > 253 {
        return Err(anyhow!(
            "appLinks.hosts entries must be DNS host names, got '{host}'"
        ));
    }
    let labels = host.split('.').collect::<Vec<_>>();
    if labels.len() < 2
        || labels.iter().any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(anyhow!(
            "appLinks.hosts entries must be DNS host names, got '{host}'"
        ));
    }

    Ok(())
}

fn validate_macos_ui_config(
    ui: &Value,
    terminal_enabled: bool,
    browser_enabled: bool,
) -> Result<()> {
    let ui_obj = ui
        .as_object()
        .ok_or_else(|| anyhow!("ui must be a JSON object"))?;
    let launch = ui_obj
        .get("launch")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("ui.launch must be an object"))?;
    let initial_surface = non_empty_str(launch.get("initialSurface"), "ui.launch.initialSurface")?;
    if let Some(open_on_launch) = launch.get("openOnLaunch")
        && open_on_launch.as_bool().is_none()
    {
        return Err(anyhow!("ui.launch.openOnLaunch must be a boolean"));
    }
    let surfaces = ui_obj
        .get("surfaces")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("ui.surfaces must be an array"))?;
    if surfaces.is_empty() {
        return Err(anyhow!("ui.surfaces must contain at least one surface"));
    }
    let activators = ui_obj
        .get("activators")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("ui.activators must be an array"));
    let activators = activators?;

    let mut all_surface_ids = HashSet::<String>::new();
    let mut surface_by_id = HashMap::<String, MacosUiSurface>::new();
    let mut skipped_surface_ids = HashSet::<String>::new();
    let mut seen_app_ids = HashSet::<String>::new();

    for (index, surface) in surfaces.iter().enumerate() {
        let obj = surface
            .as_object()
            .ok_or_else(|| anyhow!("ui.surfaces[{index}] must be an object"))?;
        let id = non_empty_str(obj.get("id"), &format!("ui.surfaces[{index}].id"))?;
        if !all_surface_ids.insert(id.to_string()) {
            return Err(anyhow!("duplicate ui surface id '{id}'"));
        }
        if !ui_surface_available_on_platform(obj, "macos", &format!("ui.surfaces[{index}]"))? {
            skipped_surface_ids.insert(id.to_string());
            continue;
        }

        let role = non_empty_str(obj.get("role"), &format!("ui.surfaces[{index}].role"))?;
        let role = match role {
            "main" => MacosUiSurfaceRole::Main,
            "aside" => MacosUiSurfaceRole::Aside,
            "float" => MacosUiSurfaceRole::Float,
            other => {
                return Err(anyhow!("ui surface '{id}' has unknown role '{other}'"));
            }
        };
        let content = obj
            .get("content")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("ui.surfaces[{index}].content must be an object"))?;
        let kind = non_empty_str(
            content.get("kind"),
            &format!("ui.surfaces[{index}].content.kind"),
        )?;
        let content_kind = match kind {
            "lxapp" => {
                let app_id = non_empty_str(
                    content.get("appId"),
                    &format!("ui.surfaces[{index}].content.appId"),
                )?;
                if !seen_app_ids.insert(app_id.to_string()) {
                    return Err(anyhow!(
                        "macOS app UI currently requires unique lxapp content.appId values; duplicate '{app_id}'"
                    ));
                }
                MacosUiContentKind::Lxapp
            }
            "page" => {
                non_empty_str(
                    content.get("page"),
                    &format!("ui.surfaces[{index}].content.page"),
                )?;
                return Err(anyhow!(
                    "ui surface '{id}' uses declarative page content, which is not admitted on macOS"
                ));
            }
            "url" => {
                non_empty_str(
                    content.get("url"),
                    &format!("ui.surfaces[{index}].content.url"),
                )?;
                if !browser_enabled {
                    return Err(anyhow!(
                        "ui surface '{id}' uses url content but capabilities.browser is not enabled"
                    ));
                }
                MacosUiContentKind::Url
            }
            "native" => {
                let name = non_empty_str(
                    content.get("name"),
                    &format!("ui.surfaces[{index}].content.name"),
                )?;
                if content.contains_key("backend") {
                    return Err(anyhow!(
                        "ui surface '{id}' must not set content.backend; native runtime is selected internally"
                    ));
                }
                match NativeSurfaceName::parse(name)? {
                    NativeSurfaceName::Terminal => {
                        if !terminal_enabled {
                            return Err(anyhow!(
                                "ui surface '{id}' uses terminal content but capabilities.terminal is not enabled"
                            ));
                        }
                        MacosUiContentKind::NativeTerminal
                    }
                    NativeSurfaceName::Browser => {
                        if !browser_enabled {
                            return Err(anyhow!(
                                "ui surface '{id}' uses browser content but capabilities.browser is not enabled"
                            ));
                        }
                        MacosUiContentKind::NativeBrowser
                    }
                }
            }
            _ => {
                return Err(anyhow!(
                    "ui surface '{id}' uses unsupported macOS content.kind '{kind}'"
                ));
            }
        };

        surface_by_id.insert(
            id.to_string(),
            MacosUiSurface {
                role,
                content_kind,
                attach_to: optional_non_empty_str(obj.get("attachTo")),
                edge: optional_non_empty_str(obj.get("edge")),
            },
        );
    }

    if surface_by_id.is_empty() {
        return Err(anyhow!(
            "ui.surfaces must contain at least one surface available on macOS"
        ));
    }

    let Some(initial) = surface_by_id.get(initial_surface) else {
        if skipped_surface_ids.contains(initial_surface) {
            return Err(anyhow!(
                "ui.launch.initialSurface '{initial_surface}' is not available on macOS"
            ));
        }
        return Err(anyhow!(
            "ui.launch.initialSurface references unknown surface '{initial_surface}'"
        ));
    };
    if !matches!(
        initial.role,
        MacosUiSurfaceRole::Main | MacosUiSurfaceRole::Float
    ) {
        return Err(anyhow!(
            "ui.launch.initialSurface must reference a supported macOS surface"
        ));
    }

    let main_ids = surface_by_id
        .iter()
        .filter_map(|(id, surface)| {
            (surface.role == MacosUiSurfaceRole::Main).then_some(id.as_str())
        })
        .collect::<Vec<_>>();
    let float_ids = surface_by_id
        .iter()
        .filter_map(|(id, surface)| {
            (surface.role == MacosUiSurfaceRole::Float).then_some(id.as_str())
        })
        .collect::<Vec<_>>();
    if main_ids.is_empty() && float_ids.len() != 1 {
        return Err(anyhow!(
            "macOS app UI requires at least one main or one tray float root"
        ));
    }
    if main_ids.len() > 1 {
        return Err(anyhow!(
            "macOS app UI requires exactly one declared main surface"
        ));
    }
    if !main_ids.is_empty() && !float_ids.is_empty() {
        return Err(anyhow!(
            "macOS app UI cannot combine main surfaces with a tray float root"
        ));
    }

    for (id, surface) in &surface_by_id {
        if surface.content_kind == MacosUiContentKind::NativeTerminal
            && surface.role == MacosUiSurfaceRole::Aside
        {
            let edge = surface
                .edge
                .as_deref()
                .ok_or_else(|| anyhow!("terminal ui surface '{id}' requires edge"))?;
            if edge != "bottom" && edge != "top" {
                return Err(anyhow!(
                    "terminal ui surface '{id}' must use edge 'top' or 'bottom'"
                ));
            }
        }
        if surface.content_kind == MacosUiContentKind::NativeBrowser
            && surface.role != MacosUiSurfaceRole::Main
        {
            return Err(anyhow!(
                "native browser ui surface '{id}' must use role 'main'"
            ));
        }
        if surface.content_kind == MacosUiContentKind::Url
            && surface.role != MacosUiSurfaceRole::Main
        {
            return Err(anyhow!(
                "url ui surface '{id}' must use role 'main' on macOS"
            ));
        }

        match surface.role {
            MacosUiSurfaceRole::Main | MacosUiSurfaceRole::Float => {
                if surface.attach_to.is_some() {
                    return Err(anyhow!("root ui surface '{id}' cannot set attachTo"));
                }
            }
            MacosUiSurfaceRole::Aside => {
                let parent_id = surface
                    .attach_to
                    .as_deref()
                    .ok_or_else(|| anyhow!("aside ui surface '{id}' requires attachTo"))?;
                let parent = surface_by_id.get(parent_id).ok_or_else(|| {
                    anyhow!("ui surface '{id}' attaches to unknown surface '{parent_id}'")
                })?;
                if parent.role != MacosUiSurfaceRole::Main {
                    return Err(anyhow!(
                        "macOS app UI currently does not support aside -> aside; surface '{id}' attaches to '{parent_id}'"
                    ));
                }
                if parent_id != initial_surface {
                    return Err(anyhow!(
                        "macOS app UI requires asides to attach to launch.initialSurface"
                    ));
                }
                let edge = surface
                    .edge
                    .as_deref()
                    .ok_or_else(|| anyhow!("aside ui surface '{id}' requires edge"))?;
                match edge {
                    "left" | "right" | "bottom" | "top" => {}
                    other => {
                        return Err(anyhow!(
                            "aside ui surface '{id}' has unknown edge '{other}'"
                        ));
                    }
                }
            }
        }
    }

    let mut seen_activator_ids = HashSet::<String>::new();
    for (index, activator) in activators.iter().enumerate() {
        let obj = activator
            .as_object()
            .ok_or_else(|| anyhow!("ui.activators[{index}] must be an object"))?;
        let id = non_empty_str(obj.get("id"), &format!("ui.activators[{index}].id"))?;
        let kind = non_empty_str(obj.get("kind"), &format!("ui.activators[{index}].kind"))?;
        let action = obj
            .get("action")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("ui.activators[{index}].action must be an object"))?;
        let action_kind = non_empty_str(
            action.get("kind"),
            &format!("ui.activators[{index}].action.kind"),
        )?;
        match action_kind {
            "toggleSurface" | "openSurface" => {}
            other => {
                return Err(anyhow!(
                    "ui activator '{id}' has unknown action.kind '{other}'"
                ));
            }
        }
        let action_surface = non_empty_str(
            action.get("surface"),
            &format!("ui.activators[{index}].action.surface"),
        )?;
        if skipped_surface_ids.contains(action_surface) {
            continue;
        }
        if !surface_by_id.contains_key(action_surface) {
            return Err(anyhow!(
                "ui activator '{id}' references unknown surface '{action_surface}'"
            ));
        }

        let mut keep_activator = true;
        match kind {
            "menuBarItem" | "appActivation" => {
                if obj.get("hostSurface").is_some() {
                    return Err(anyhow!(
                        "ui activator '{id}' with kind '{kind}' cannot set hostSurface"
                    ));
                }
            }
            "sidebarItem" | "toolbarItem" | "titlebarItem" => {
                let host_surface = non_empty_str(
                    obj.get("hostSurface"),
                    &format!("ui.activators[{index}].hostSurface"),
                )?;
                if skipped_surface_ids.contains(host_surface) {
                    keep_activator = false;
                } else if !surface_by_id.contains_key(host_surface) {
                    return Err(anyhow!(
                        "ui activator '{id}' references unknown hostSurface '{host_surface}'"
                    ));
                }
            }
            other => {
                return Err(anyhow!("ui activator '{id}' has unknown kind '{other}'"));
            }
        }
        if !keep_activator {
            continue;
        }
        if !seen_activator_ids.insert(id.to_string()) {
            return Err(anyhow!("duplicate ui activator id '{id}'"));
        }
    }

    Ok(())
}

fn ui_surface_available_on_platform(
    surface: &Map<String, Value>,
    platform: &str,
    context: &str,
) -> Result<bool> {
    let Some(platforms) = surface.get("platforms") else {
        return Ok(true);
    };
    let platforms = platforms
        .as_array()
        .ok_or_else(|| anyhow!("{context}.platforms must be an array"))?;
    if platforms.is_empty() {
        return Ok(true);
    }
    for (index, platform_value) in platforms.iter().enumerate() {
        let value = platform_value
            .as_str()
            .ok_or_else(|| anyhow!("{context}.platforms[{index}] must be a string"))?;
        if value.eq_ignore_ascii_case(platform) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn has_host_config(project_root: &Path) -> bool {
    project_root.join(HOST_CONFIG_FILE).exists()
}

fn validate_lingxia_server(cfg: &LingxiaServer) -> Result<()> {
    match cfg {
        LingxiaServer::Single(url) => {
            if url.trim().is_empty() {
                return Err(anyhow!("app.lingxiaServer must not be empty"));
            }
        }
        LingxiaServer::PerEnv(per) => {
            let entries = [
                ("developer", per.developer.as_deref()),
                ("preview", per.preview.as_deref()),
                ("release", per.release.as_deref()),
            ];
            if entries.iter().all(|(_, url)| url.is_none()) {
                return Err(anyhow!(
                    "app.lingxiaServer must configure at least one of developer, preview, or release"
                ));
            }
            for (name, url) in entries {
                if let Some(url) = url
                    && url.trim().is_empty()
                {
                    return Err(anyhow!("app.lingxiaServer.{name} must not be empty"));
                }
            }
        }
    }
    Ok(())
}

fn validate_package_id_suffix_overrides(over: &PackageIdSuffixOverrides) -> Result<()> {
    for (name, suffix) in [
        ("developer", over.developer.as_deref()),
        ("preview", over.preview.as_deref()),
        ("release", over.release.as_deref()),
    ] {
        let Some(suffix) = suffix else {
            continue;
        };
        // Empty string is the explicit "opt out of default suffix" form.
        if !suffix.is_empty() && !is_valid_package_id_suffix(suffix) {
            return Err(anyhow!(
                "app.packageIdSuffix.{name} must start with '.' \
                 and use lowercase a-z 0-9 segments (got '{suffix}'); \
                 use \"\" to opt out of the default"
            ));
        }
    }
    Ok(())
}

fn is_valid_package_id_suffix(suffix: &str) -> bool {
    // Pattern: ^\.[a-z0-9]+(\.[a-z0-9]+)*$
    if !suffix.starts_with('.') || suffix.len() < 2 {
        return false;
    }
    let body = &suffix[1..];
    body.split('.').all(|seg| {
        !seg.is_empty()
            && seg
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

impl LingXiaConfig {
    /// Resolve the active environment for this build.
    ///
    /// Model: env-version is a build-time property with built-in defaults
    /// (developer=".dev", preview=".preview", release=no suffix). Yaml only
    /// supplies optional overrides.
    ///
    /// - `lingxia_server`: `app.lingxiaServer` is queried; `Single` applies
    ///   everywhere, `PerEnv` selects by env. Empty string if not configured.
    /// - `package_id_suffix`: `app.packageIdSuffix.<env>` wins; an explicit
    ///   `""` opts out of the built-in default. Otherwise the env's built-in
    ///   default is used.
    pub fn resolve_env(&self, version: EnvVersion) -> Result<ResolvedEnv> {
        let app = self
            .app
            .as_ref()
            .ok_or_else(|| anyhow!("Missing app section in {}", HOST_CONFIG_FILE))?;

        let lingxia_server = app
            .lingxia_server
            .as_ref()
            .and_then(|cfg| cfg.for_env(version))
            .map(str::to_string)
            .unwrap_or_default();

        let configured_suffix = app
            .package_id_suffix
            .as_ref()
            .and_then(|over| over.for_env(version));
        let package_id_suffix =
            resolve_env_suffix(configured_suffix, version.default_package_id_suffix());

        Ok(ResolvedEnv {
            version,
            lingxia_server,
            package_id_suffix,
        })
    }
}

fn resolve_env_suffix(configured: Option<&str>, default: Option<&str>) -> Option<String> {
    match configured {
        None => default.map(str::to_string),
        Some("") => None,
        Some(value) => Some(value.to_string()),
    }
}

pub fn dir_matches_host_config(dir: &Path, requested_name: &str) -> bool {
    dir.join(requested_name).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_app_context::{ThemeColor, ThemeStyle};
    use tempfile::TempDir;

    fn load_config_yaml(source: &str) -> Result<LingXiaConfig> {
        let mut config: LingXiaConfig = yaml::from_str(source)?;
        config.theme = config.theme.take().and_then(ThemeConfig::normalized);
        config.apply_surfaces()?;
        config.validate()?;
        Ok(config)
    }

    #[test]
    fn test_android_api_level_derivation() {
        let config = AndroidConfig {
            package_id: "com.example.app".to_string(),
            min_sdk: Some(28),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: None,
            google_play_store: None,
            xiaomi_store: None,
            oppo_store: None,
            honor_store: None,
        };
        assert_eq!(config.get_api_level(), 28);

        let config_explicit = AndroidConfig {
            package_id: "com.example.app".to_string(),
            min_sdk: Some(28),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: Some(33),
            google_play_store: None,
            xiaomi_store: None,
            oppo_store: None,
            honor_store: None,
        };
        assert_eq!(config_explicit.get_api_level(), 33);
    }

    #[test]
    fn test_config_serialization() {
        let config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let yaml = yaml::to_string(&config).unwrap();
        println!("{}", yaml);

        let parsed: LingXiaConfig = yaml::from_str(&yaml).unwrap();
        let app = parsed.app.unwrap();
        assert_eq!(app.product_name, "my-app");
        assert_eq!(app.home_app_id.as_deref(), Some("my-app"));
        assert_eq!(parsed.android.unwrap().package_id, "com.example.myapp");
        let resources = parsed.resources.unwrap();
        assert_eq!(resources.bundles[0].app_id, "my-app");
        assert_eq!(resources.bundles[0].path.as_deref(), Some("my-app"));
    }

    #[test]
    fn rejects_sdk_reserved_app_id_in_resources_bundles() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config
            .resources
            .as_mut()
            .unwrap()
            .bundles
            .push(ResourceBundleConfig {
                bundle_type: ResourceBundleType::Lxapp,
                app_id: "app.lingxia.browser".to_string(),
                path: Some("./my-browser-shell-webui".to_string()),
                package: None,
                version: None,
            });

        let err = config
            .validate()
            .expect_err("validate must reject reserved appId");
        let msg = err.to_string();
        assert!(
            msg.contains("app.lingxia.browser") && msg.contains("browser.webui"),
            "error must point at the new customization API; got: {msg}"
        );
    }

    #[test]
    fn rejects_sdk_reserved_app_id_as_home_app_id() {
        let mut config =
            LingXiaConfig::new_android("my-app", "com.example.myapp", "app.lingxia.browser");
        // Drop the resources.bundles entry that new_android wrote pointing at the
        // reserved appId so the homeAppId check is the one that fires (not the
        // resources.bundles check).
        config.resources.as_mut().unwrap().bundles.clear();

        let err = config
            .validate()
            .expect_err("validate must reject reserved homeAppId");
        let msg = err.to_string();
        assert!(
            msg.contains("homeAppId") && msg.contains("app.lingxia.browser"),
            "error must mention homeAppId and the reserved id; got: {msg}"
        );
    }

    #[test]
    fn rejects_legacy_app_environments_config() {
        let yaml = r#"
app:
  projectName: my-app
  productName: My App
  productVersion: 0.0.1
  platforms:
    - android
  homeAppId: my-app
  environments:
    developer:
      lingxiaServer: http://localhost:8080
android:
  packageId: com.example.myapp
"#;

        let err = yaml::from_str::<LingXiaConfig>(yaml).unwrap_err();
        assert!(
            err.to_string().contains("environments"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn desktop_runtime_is_baseline_on_desktop_only() {
        let config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");

        // The layout host + webview input are baseline on desktop, with no
        // opt-in flag required. The browser is NOT baseline — without the
        // `browser` capability there is no browser-shell anywhere.
        assert!(config.desktop_runtime_enabled("macos"));
        assert!(config.desktop_runtime_enabled("windows"));
        assert!(!config.desktop_runtime_enabled("android"));
        assert!(!config.desktop_runtime_enabled("ios"));
        assert!(!config.desktop_runtime_enabled("harmony"));

        assert_eq!(
            config.native_features_for_platform("macos"),
            vec!["standard".to_string(), "webview-input".to_string()]
        );
        assert_eq!(
            config.native_features_for_platform("harmony"),
            vec!["standard".to_string()]
        );
    }

    #[test]
    fn browser_capability_enables_runtime_cross_platform() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");

        // Off by default — no browser runtime on any platform.
        assert!(!config.browser_enabled());
        assert!(
            !config
                .native_features_for_platform("ios")
                .contains(&"browser-shell".to_string())
        );
        assert!(
            !config
                .native_features_for_platform("macos")
                .contains(&"browser-shell".to_string())
        );

        // Opt in: the browser runtime ships everywhere, mobile included.
        config.capabilities.as_mut().unwrap().browser = true;
        assert!(config.browser_enabled());
        for platform in ["ios", "android", "macos", "windows", "harmony"] {
            assert!(
                config
                    .native_features_for_platform(platform)
                    .contains(&"browser-shell".to_string()),
                "browser runtime missing on {platform}"
            );
        }
    }

    #[test]
    fn proxy_capability_adds_proxy_feature_on_desktop() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        // Proxy serves the browser, so it requires the browser capability.
        config.capabilities.as_mut().unwrap().browser = true;
        config.capabilities.as_mut().unwrap().proxy = true;

        assert!(config.proxy_enabled("macos"));
        assert!(!config.proxy_enabled("android"));
        assert_eq!(
            config.native_features_for_platform("macos"),
            vec![
                "standard".to_string(),
                "browser-shell".to_string(),
                "webview-input".to_string(),
                "proxy".to_string(),
            ]
        );
    }

    #[test]
    fn proxy_requires_browser_capability() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.capabilities.as_mut().unwrap().proxy = true;
        // Browser off: proxy has nothing to serve, so it stays disabled.
        assert!(!config.proxy_enabled("macos"));
    }

    #[test]
    fn terminal_capability_enables_macos_and_windows_runtime() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.capabilities.as_mut().unwrap().terminal = true;

        assert!(config.desktop_runtime_enabled("macos"));
        assert!(config.terminal_enabled("windows"));
        assert!(!config.control_enabled("macos"));
        assert!(!config.desktop_runtime_enabled("android"));
        assert_eq!(
            config.native_features_for_platform("macos"),
            vec![
                "standard".to_string(),
                "terminal-runtime".to_string(),
                "webview-input".to_string(),
            ]
        );
        assert_eq!(
            config.native_features_for_platform("windows"),
            vec![
                "standard".to_string(),
                "terminal-runtime".to_string(),
                "webview-input".to_string(),
            ]
        );
    }

    #[test]
    fn process_capability_enables_desktop_runtime_feature() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.capabilities.as_mut().unwrap().process = true;

        assert!(config.process_enabled("macos"));
        assert!(config.process_enabled("windows"));
        assert!(!config.process_enabled("android"));
        assert_eq!(
            config.native_features_for_platform("macos"),
            vec![
                "standard".to_string(),
                "process".to_string(),
                "webview-input".to_string(),
            ]
        );
    }

    #[test]
    fn process_capability_requires_app_service_and_desktop_host() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.capabilities.as_mut().unwrap().process = true;

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("supported only by macOS and Windows"), "{err}");

        config.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
        config.features.as_mut().unwrap().app_service = false;
        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("requires features.appService: true"), "{err}");
    }

    #[test]
    fn resolve_env_applies_builtin_suffix_with_single_server() {
        // Default template: single top-level server, no overrides. Each env
        // still resolves to its built-in suffix.
        let config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");

        let dev = config.resolve_env(EnvVersion::Developer).unwrap();
        assert_eq!(dev.version, EnvVersion::Developer);
        assert_eq!(dev.lingxia_server, "https://api.example.com");
        assert_eq!(dev.effective_package_id_suffix(), Some(".dev"));

        let release = config.resolve_env(EnvVersion::Release).unwrap();
        assert_eq!(release.lingxia_server, "https://api.example.com");
        assert_eq!(release.effective_package_id_suffix(), None);
    }

    #[test]
    fn resolve_env_per_env_server_routes_by_version() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.lingxia_server = Some(LingxiaServer::PerEnv(PerEnvServer {
            developer: Some("http://localhost:8080".to_string()),
            preview: None,
            release: Some("https://prod.example.com".to_string()),
        }));

        let dev = config.resolve_env(EnvVersion::Developer).unwrap();
        assert_eq!(dev.lingxia_server, "http://localhost:8080");

        let preview = config.resolve_env(EnvVersion::Preview).unwrap();
        assert_eq!(preview.lingxia_server, ""); // not configured for preview

        let release = config.resolve_env(EnvVersion::Release).unwrap();
        assert_eq!(release.lingxia_server, "https://prod.example.com");
    }

    #[test]
    fn resolve_env_suffix_override_opts_out_with_empty_string() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.package_id_suffix = Some(PackageIdSuffixOverrides {
            developer: Some(String::new()),
            ..Default::default()
        });

        let dev = config.resolve_env(EnvVersion::Developer).unwrap();
        assert_eq!(dev.effective_package_id_suffix(), None);
    }

    #[test]
    fn resolve_env_no_server_config_is_empty_string() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.app.as_mut().unwrap().lingxia_server = None;

        let env = config.resolve_env(EnvVersion::Release).unwrap();
        assert_eq!(env.lingxia_server, "");
    }

    #[test]
    fn save_and_load_yaml() {
        let temp = TempDir::new().unwrap();
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.theme = Some(ThemeConfig {
            light: Some(ThemeStyle {
                accent_color: Some(ThemeColor::parse("#a1b2c3").unwrap()),
                ..ThemeStyle::default()
            }),
            dark: None,
        });

        config.save(temp.path()).unwrap();

        let loaded = LingXiaConfig::load(temp.path()).unwrap();
        assert_eq!(loaded.app.as_ref().unwrap().project_name, "my-app");
        assert_eq!(
            loaded
                .theme
                .as_ref()
                .and_then(|theme| theme.light.as_ref())
                .and_then(|style| style.accent_color)
                .map(ThemeColor::rgb),
            Some(0xA1B2C3)
        );
        assert!(temp.path().join(HOST_CONFIG_FILE).exists());
    }

    #[test]
    fn browser_use_requires_browser_without_surfaces() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let capabilities = config.capabilities.as_mut().unwrap();
        capabilities.browser_use = true;
        capabilities.browser = false;
        assert!(config.surfaces.is_none());

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("capabilities.browserUse drives the in-app browser"));
    }

    #[test]
    fn theme_yaml_rejects_alpha_and_unknown_tokens() {
        for theme in [
            "  light:\n    accentColor: '#80A1B2C3'",
            "  light:\n    sidebarBackgroundColor: '#A1B2C3'",
        ] {
            let yaml = format!(
                "app:\n  projectName: demo\n  productName: Demo\n  productVersion: 0.1.0\n  platforms: [android]\n  homeAppId: home\ntheme:\n{theme}\n"
            );
            assert!(yaml::from_str::<LingXiaConfig>(&yaml).is_err(), "{theme}");
        }
    }

    #[test]
    fn macos_host_requires_surfaces() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("surfaces is required for macOS host app projects"));
    }

    #[test]
    fn rejects_root_ui_schema() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos]
  homeAppId: demo-home
ui:
  launch:
    initialSurface: main
  surfaces: []
  activators: []
"#;

        let err = yaml::from_str::<LingXiaConfig>(yaml)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown field `ui`"));
    }

    #[test]
    fn rejects_removed_terminal_edge_capability() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos]
  homeAppId: demo-home
capabilities:
  terminal: true
  terminalEdge: bottom
"#;

        let err = yaml::from_str::<LingXiaConfig>(yaml)
            .unwrap_err()
            .to_string();
        assert!(err.contains("terminalEdge"));
    }

    #[test]
    fn rejects_unknown_app_platform_token() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [mac]
  homeAppId: home
surfaces:
  - lxapp: home
    role: main
    launch: true
"#;

        let err = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(err.contains("app.platforms[0]"), "{err}");
        assert!(err.contains("mac"), "{err}");
    }

    #[test]
    fn rejects_unknown_surface_platform_token() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos, android]
  homeAppId: home
surfaces:
  - lxapp: home
    role: main
    launch: true
    platforms: [andriod]
"#;

        let err = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(err.contains("surface 'home'"), "{err}");
        assert!(err.contains("unsupported platform 'andriod'"), "{err}");
    }

    #[test]
    fn rejects_empty_surface_platforms() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos]
  homeAppId: home
surfaces:
  - lxapp: home
    role: main
    launch: true
    platforms: []
"#;

        let err = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(err.contains("platforms must not be empty"), "{err}");
    }

    #[test]
    fn rejects_surface_platform_outside_app_platforms() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [ios]
  homeAppId: home
surfaces:
  - lxapp: home
    role: main
    launch: true
    platforms: [macos]
"#;

        let err = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(err.contains("surface 'home'"), "{err}");
        assert!(err.contains("not listed in app.platforms"), "{err}");
    }

    #[test]
    fn rejects_terminal_surface_on_mobile_platforms() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [ios, macos]
  homeAppId: home
capabilities:
  terminal: true
surfaces:
  - lxapp: home
    role: main
    launch: true
  - native: terminal
    role: aside
    edge: bottom
    platforms: [ios]
"#;

        let err = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(err.contains("surface 'terminal'"), "{err}");
        assert!(
            err.contains("only supports platforms: macos, windows"),
            "{err}"
        );
    }

    #[test]
    fn omitted_terminal_surface_platforms_follow_app_platforms() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [ios, macos]
  homeAppId: home
capabilities:
  terminal: true
surfaces:
  - lxapp: home
    role: main
    launch: true
  - native: terminal
    role: aside
    edge: bottom
"#;

        let err = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(err.contains("surface 'terminal'"), "{err}");
        assert!(
            err.contains("only supports platforms: macos, windows"),
            "{err}"
        );
    }

    #[test]
    fn macos_ui_accepts_current_runtime_subset() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "my-app"
                }
            }, {
                "id": "side",
                "role": "aside",
                "attachTo": "main",
                "edge": "right",
                "content": {
                    "kind": "lxapp",
                    "appId": "my-side-app"
                }
            }],
            "activators": [{
                "id": "sideButton",
                "kind": "sidebarItem",
                "hostSurface": "main",
                "action": {
                    "kind": "toggleSurface",
                    "surface": "side"
                }
            }]
        }));

        config.validate().unwrap();
    }

    #[test]
    fn macos_ui_filters_non_macos_platform_surfaces() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "platforms": ["macos"],
                "content": {
                    "kind": "lxapp",
                    "appId": "my-app"
                }
            }, {
                "id": "windowsMain",
                "role": "main",
                "platforms": ["windows"],
                "content": {
                    "kind": "lxapp",
                    "appId": "win-app"
                }
            }, {
                "id": "windowsSide",
                "role": "aside",
                "attachTo": "windowsMain",
                "edge": "right",
                "platforms": ["windows"],
                "content": {
                    "kind": "lxapp",
                    "appId": "win-side"
                }
            }],
            "activators": [{
                "id": "windowsSideButton",
                "kind": "sidebarItem",
                "hostSurface": "windowsMain",
                "action": {
                    "kind": "toggleSurface",
                    "surface": "windowsSide"
                }
            }]
        }));

        config.validate().unwrap();
    }

    #[test]
    fn macos_ui_rejects_duplicate_surface_id_before_platform_filter() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "platforms": ["macos"],
                "content": {
                    "kind": "lxapp",
                    "appId": "my-app"
                }
            }, {
                "id": "main",
                "role": "main",
                "platforms": ["windows"],
                "content": {
                    "kind": "lxapp",
                    "appId": "win-app"
                }
            }],
            "activators": []
        }));

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("duplicate ui surface id 'main'"), "{err}");
    }

    #[test]
    fn macos_ui_accepts_terminal_aside_panel_bottom() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.capabilities.as_mut().unwrap().terminal = true;
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "my-app"
                }
            }, {
                "id": "terminal",
                "role": "aside",
                "attachTo": "main",
                "edge": "bottom",
                "content": {
                    "kind": "native",
                    "name": "terminal"
                }
            }],
            "activators": [{
                "id": "terminalSidebar",
                "kind": "sidebarItem",
                "hostSurface": "main",
                "action": {
                    "kind": "toggleSurface",
                    "surface": "terminal"
                }
            }]
        }));

        config.validate().unwrap();
    }

    #[test]
    fn macos_ui_rejects_terminal_when_capability_disabled() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "my-app"
                }
            }, {
                "id": "terminal",
                "role": "aside",
                "attachTo": "main",
                "edge": "bottom",
                "content": {
                    "kind": "native",
                    "name": "terminal"
                }
            }],
            "activators": [{
                "id": "terminalSidebar",
                "kind": "sidebarItem",
                "hostSurface": "main",
                "action": {
                    "kind": "toggleSurface",
                    "surface": "terminal"
                }
            }]
        }));

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("capabilities.terminal is not enabled"));
    }

    #[test]
    fn macos_ui_rejects_terminal_non_bottom_edge() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.capabilities.as_mut().unwrap().terminal = true;
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "main"
                }
            }, {
                "id": "terminal",
                "role": "aside",
                "attachTo": "main",
                "edge": "right",
                "content": {
                    "kind": "native",
                    "name": "terminal"
                }
            }],
            "activators": []
        }));

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("must use edge 'top' or 'bottom'"));
    }

    #[test]
    fn macos_ui_rejects_terminal_backend() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.capabilities.as_mut().unwrap().terminal = true;
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "main"
                }
            }, {
                "id": "terminal",
                "role": "aside",
                "attachTo": "main",
                "edge": "bottom",
                "content": {
                    "kind": "native",
                    "name": "terminal",
                    "backend": "xterm"
                }
            }],
            "activators": []
        }));

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("must not set content.backend"));
    }

    #[test]
    fn macos_ui_accepts_titlebar_item() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "main"
                }
            }],
            "activators": [{
                "id": "titlebarAction",
                "kind": "titlebarItem",
                "hostSurface": "main",
                "action": {
                    "kind": "openSurface",
                    "surface": "main"
                }
            }]
        }));

        config.validate().unwrap();
    }

    #[test]
    fn macos_ui_rejects_removed_surface_actions() {
        for action_kind in ["closeSurface", "focusSurface"] {
            let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
            config.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
            config.generated_ui = Some(serde_json::json!({
                "launch": {
                    "initialSurface": "main"
                },
                "surfaces": [{
                    "id": "main",
                    "role": "main",
                    "content": {
                        "kind": "lxapp",
                        "appId": "main"
                    }
                }],
                "activators": [{
                    "id": "titlebarAction",
                    "kind": "titlebarItem",
                    "hostSurface": "main",
                    "action": {
                        "kind": action_kind,
                        "surface": "main"
                    }
                }]
            }));

            let err = config.validate().unwrap_err().to_string();
            assert!(err.contains("unknown action.kind"), "{action_kind}: {err}");
        }
    }

    #[test]
    fn macos_ui_rejects_non_macos_activators() {
        for kind in ["trayItem", "deepLink"] {
            let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
            let app = config.app.as_mut().unwrap();
            app.platforms = vec!["macos".to_string()];
            config.generated_ui = Some(serde_json::json!({
                "launch": {
                    "initialSurface": "main"
                },
                "surfaces": [{
                    "id": "main",
                    "role": "main",
                    "content": {
                        "kind": "lxapp",
                        "appId": "main"
                    }
                }],
                "activators": [{
                    "id": kind,
                    "kind": kind,
                    "action": {
                        "kind": "openSurface",
                        "surface": "main"
                    }
                }]
            }));

            let err = config.validate().unwrap_err().to_string();
            assert!(err.contains("unknown kind"), "{kind}: {err}");
        }
    }

    #[test]
    fn macos_ui_rejects_invalid_host_surface_usage() {
        let mut missing_host = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        missing_host.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
        missing_host.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "main"
                }
            }],
            "activators": [{
                "id": "sidebar",
                "kind": "sidebarItem",
                "action": {
                    "kind": "openSurface",
                    "surface": "main"
                }
            }]
        }));
        let err = missing_host.validate().unwrap_err().to_string();
        assert!(err.contains("hostSurface"));

        let mut app_level_host =
            LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        app_level_host.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
        app_level_host.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "main"
                }
            }],
            "activators": [{
                "id": "dock",
                "kind": "appActivation",
                "hostSurface": "main",
                "action": {
                    "kind": "openSurface",
                    "surface": "main"
                }
            }]
        }));
        let err = app_level_host.validate().unwrap_err().to_string();
        assert!(err.contains("cannot set hostSurface"));
    }

    #[test]
    fn macos_ui_rejects_duplicate_content_app_id() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "shared"
                }
            }, {
                "id": "panel",
                "role": "aside",
                "attachTo": "main",
                "edge": "right",
                "content": {
                    "kind": "lxapp",
                    "appId": "shared"
                }
            }],
            "activators": []
        }));

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unique lxapp content.appId"));
    }

    /// Empty declaration with the given role; set exactly one content key.
    fn surface_decl(role: SurfaceRole) -> SurfaceDecl {
        SurfaceDecl {
            lxapp: None,
            page: None,
            url: None,
            native: None,
            role,
            launch: false,
            edge: None,
            size: None,
            tray: None,
            platforms: None,
        }
    }

    fn lxapp_decl(app_id: &str, role: SurfaceRole) -> SurfaceDecl {
        SurfaceDecl {
            lxapp: Some(app_id.into()),
            ..surface_decl(role)
        }
    }

    #[test]
    fn surfaces_maps_showcase_to_internal_ui() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("lingxia-showcase", SurfaceRole::Main)
            },
            SurfaceDecl {
                edge: Some("right".into()),
                ..lxapp_decl("lingxia-chat", SurfaceRole::Aside)
            },
            SurfaceDecl {
                native: Some("terminal".into()),
                // No edge: the terminal defaults to bottom.
                ..surface_decl(SurfaceRole::Aside)
            },
        ];

        let ui = surfaces_to_ui(&surfaces, true, false).unwrap();
        let expected = serde_json::json!({
            "launch": {
                "initialSurface": "lingxia-showcase",
                "openOnLaunch": true
            },
            "surfaces": [
                {
                    "id": "lingxia-showcase",
                    "role": "main",
                    "content": { "kind": "lxapp", "appId": "lingxia-showcase" }
                },
                {
                    "id": "lingxia-chat",
                    "role": "aside",
                    "attachTo": "lingxia-showcase",
                    "edge": "right",
                    "content": { "kind": "lxapp", "appId": "lingxia-chat" }
                },
                {
                    "id": "terminal",
                    "role": "aside",
                    "attachTo": "lingxia-showcase",
                    "edge": "bottom",
                    "size": { "height": 320 },
                    "content": { "kind": "native", "name": "terminal" }
                }
            ],
            "activators": []
        });
        assert_eq!(ui, expected);

        // Full config round-trip: apply_surfaces + validate must accept it.
        let mut config = LingXiaConfig::new_android("lingxia", "com.example", "lingxia-showcase");
        config.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
        config.capabilities.as_mut().unwrap().terminal = true;
        config.generated_ui = None;
        config.surfaces = Some(surfaces);
        config.apply_surfaces().unwrap();
        config.validate().unwrap();
    }

    #[test]
    fn surfaces_maps_tray_to_menubar_item_and_no_launch() {
        let surfaces = vec![SurfaceDecl {
            tray: Some(SurfaceTray {
                icon: Some("icons/tray.svg".into()),
                label: Some("Demo".into()),
                action: Some(SurfaceTrayAction::Activate),
                exclusive: false,
                size: None,
            }),
            ..lxapp_decl("home", SurfaceRole::Main)
        }];

        let ui = surfaces_to_ui(&surfaces, false, false).unwrap();
        let expected = serde_json::json!({
            "launch": {
                "initialSurface": "home",
                "openOnLaunch": false
            },
            "surfaces": [{
                "id": "home",
                "role": "main",
                "content": { "kind": "lxapp", "appId": "home" }
            }],
            "activators": [{
                "id": "homeTray",
                "kind": "menuBarItem",
                "icon": "icons/tray.svg",
                "label": "Demo",
                "action": { "kind": "openSurface", "surface": "home" }
            }]
        });
        assert_eq!(ui, expected);

        let mut config = LingXiaConfig::new_android("demo", "com.example", "home");
        config.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
        config.generated_ui = None;
        config.surfaces = Some(surfaces);
        config.apply_surfaces().unwrap();
        config.validate().unwrap();
    }

    #[test]
    fn surfaces_maps_float_tray_to_anchored_popover() {
        // A pure tray-popover app: one float surface with a tray, no main. It must
        // emit role: float + anchor: activator (the runtime's anchored panel) and
        // launch into the tray with no dock icon.
        let surfaces = vec![SurfaceDecl {
            tray: Some(SurfaceTray {
                icon: Some("icons/tray.svg".into()),
                label: Some("Panel".into()),
                action: None,
                exclusive: true,
                size: Some(SurfaceSize {
                    width: Some(320),
                    height: Some(480),
                }),
            }),
            ..lxapp_decl("panel", SurfaceRole::Float)
        }];

        let ui = surfaces_to_ui(&surfaces, false, false).unwrap();
        let expected = serde_json::json!({
            "launch": {
                "initialSurface": "panel",
                "openOnLaunch": false,
                "hideDockIcon": true
            },
            "surfaces": [{
                "id": "panel",
                "role": "float",
                "anchor": "activator",
                "content": { "kind": "lxapp", "appId": "panel" },
                "size": { "width": 320, "height": 480 }
            }],
            "activators": [{
                "id": "panelTray",
                "kind": "menuBarItem",
                "icon": "icons/tray.svg",
                "label": "Panel",
                "action": { "kind": "toggleSurface", "surface": "panel" }
            }]
        });
        assert_eq!(ui, expected);
    }

    #[test]
    fn surfaces_maps_url_declarations_to_url_content() {
        let surfaces = vec![SurfaceDecl {
            url: Some("https://example.com/docs".into()),
            launch: true,
            ..surface_decl(SurfaceRole::Main)
        }];

        let ui = surfaces_to_ui(&surfaces, false, true).unwrap();
        assert_eq!(
            ui["surfaces"][0],
            serde_json::json!({
                "id": "https://example.com/docs",
                "role": "main",
                "content": { "kind": "url", "url": "https://example.com/docs" }
            })
        );
    }

    #[test]
    fn surfaces_rejects_url_without_browser_capability() {
        let surfaces = vec![SurfaceDecl {
            url: Some("https://example.com".into()),
            launch: true,
            ..surface_decl(SurfaceRole::Main)
        }];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("browser capability"), "{err}");
    }

    #[test]
    fn surfaces_rejects_declarative_url_aside_on_macos() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("home", SurfaceRole::Main)
            },
            SurfaceDecl {
                url: Some("https://example.com/docs".into()),
                ..surface_decl(SurfaceRole::Aside)
            },
        ];

        let err = surfaces_to_ui(&surfaces, false, true)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("declarative url asides are not supported on macOS"),
            "{err}"
        );
    }

    #[test]
    fn capability_only_browser_accepts_url_surface() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [windows]
  homeAppId: home
capabilities:
  browser: true
surfaces:
  - lxapp: home
    role: main
    launch: true
  - url: https://example.com
    role: aside
"#;

        let config = load_config_yaml(yaml).unwrap();
        assert!(config.generated_ui.is_some());
        assert!(config.browser.is_none());
    }

    #[test]
    fn surfaces_reject_plain_http_url() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [windows]
  homeAppId: home
capabilities:
  browser: true
surfaces:
  - lxapp: home
    role: main
    launch: true
  - url: http://example.com
    role: aside
"#;

        let err = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(err.contains("url scheme must be https"), "{err}");
    }

    #[test]
    fn surfaces_rejects_entry_without_content_key() {
        let surfaces = vec![surface_decl(SurfaceRole::Main)];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("exactly one content key"), "{err}");
    }

    #[test]
    fn surfaces_rejects_multiple_content_keys() {
        let surfaces = vec![SurfaceDecl {
            url: Some("https://example.com".into()),
            ..lxapp_decl("home", SurfaceRole::Main)
        }];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("more than one content key"), "{err}");
    }

    #[test]
    fn surfaces_rejects_page_float_for_now() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("home", SurfaceRole::Main)
            },
            SurfaceDecl {
                page: Some("inspector".into()),
                tray: Some(SurfaceTray {
                    icon: None,
                    label: None,
                    action: None,
                    exclusive: false,
                    size: None,
                }),
                ..surface_decl(SurfaceRole::Float)
            },
        ];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("declarative page surfaces are not supported"),
            "{err}"
        );
    }

    #[test]
    fn surfaces_rejects_native_float_for_now() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("home", SurfaceRole::Main)
            },
            SurfaceDecl {
                native: Some("terminal".into()),
                tray: Some(SurfaceTray {
                    icon: None,
                    label: None,
                    action: None,
                    exclusive: false,
                    size: None,
                }),
                ..surface_decl(SurfaceRole::Float)
            },
        ];
        let err = surfaces_to_ui(&surfaces, true, false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("native surfaces do not support role: float"),
            "{err}"
        );
    }

    #[test]
    fn surfaces_accepts_launch_on_url_main() {
        let surfaces = vec![SurfaceDecl {
            url: Some("https://example.com".into()),
            launch: true,
            ..surface_decl(SurfaceRole::Main)
        }];
        let ui = surfaces_to_ui(&surfaces, false, true).unwrap();
        assert_eq!(ui["launch"]["initialSurface"], "https://example.com");
        assert_eq!(ui["launch"]["openOnLaunch"], true);
        assert_eq!(ui["surfaces"][0]["content"]["kind"], "url");
    }

    #[test]
    fn surfaces_resolve_macos_main_separately_from_windows_home() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos, windows]
  homeAppId: home
capabilities:
  browser: true
surfaces:
  - url: https://example.com/macos
    role: main
    launch: true
    platforms: [macos]
  - lxapp: home
    role: main
    launch: true
    platforms: [windows]
"#;

        let config = load_config_yaml(yaml).unwrap();
        let macos = config.resolved_ui_for_platform("macos").unwrap().unwrap();
        let windows = config.resolved_ui_for_platform("windows").unwrap().unwrap();

        assert_eq!(
            macos["launch"]["initialSurface"],
            "https://example.com/macos"
        );
        assert_eq!(macos["surfaces"][0]["content"]["kind"], "url");
        assert_eq!(windows["launch"]["initialSurface"], "home");
        assert_eq!(windows["surfaces"][0]["content"]["kind"], "lxapp");
        assert_eq!(windows["surfaces"][0]["content"]["appId"], "home");
    }

    #[test]
    fn surfaces_accepts_native_terminal_main_on_windows() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [windows]
  homeAppId: home
capabilities:
  terminal: true
surfaces:
  - native: terminal
    role: main
    launch: true
"#;

        let config = load_config_yaml(yaml).unwrap();
        let windows = config.resolved_ui_for_platform("windows").unwrap().unwrap();
        assert_eq!(windows["launch"]["initialSurface"], "terminal");
        assert_eq!(windows["surfaces"][0]["content"]["kind"], "native");
        assert_eq!(windows["surfaces"][0]["content"]["name"], "terminal");
    }

    #[test]
    fn desktop_native_main_accepts_omitted_home_lxapp() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos, windows]
features:
  appService: false
capabilities:
  terminal: true
surfaces:
  - native: terminal
    role: main
    launch: true
"#;

        let config = load_config_yaml(yaml).unwrap();
        assert!(config.app.unwrap().home_app_id.is_none());
    }

    #[test]
    fn host_without_home_rejects_mobile_targets() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [windows, android]
features:
  appService: false
capabilities:
  terminal: true
surfaces:
  - native: terminal
    role: main
    launch: true
    platforms: [windows]
"#;

        let error = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(
            error.contains("homeAppId is required for android"),
            "{error}"
        );
    }

    #[test]
    fn host_without_home_rejects_app_service() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [windows]
capabilities:
  browser: true
surfaces:
  - native: browser
    role: main
    launch: true
"#;

        let error = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(
            error.contains("features.appService must be false"),
            "{error}"
        );
    }

    #[test]
    fn host_without_home_rejects_non_native_main() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [windows]
features:
  appService: false
capabilities:
  browser: true
surfaces:
  - url: https://example.com
    role: main
    launch: true
"#;

        let error = load_config_yaml(yaml).unwrap_err().to_string();
        assert!(error.contains("main must be native"), "{error}");
    }

    #[test]
    fn surfaces_accepts_url_main_on_windows() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [windows]
  homeAppId: home
capabilities:
  browser: true
surfaces:
  - url: https://example.com/windows
    role: main
    launch: true
"#;

        let config = load_config_yaml(yaml).unwrap();
        let windows = config.resolved_ui_for_platform("windows").unwrap().unwrap();
        assert_eq!(
            windows["launch"]["initialSurface"],
            "https://example.com/windows"
        );
        assert_eq!(windows["surfaces"][0]["content"]["kind"], "url");
    }

    #[test]
    fn surfaces_keep_mobile_home_launch_separate_from_desktop_native_main() {
        let yaml = r#"
app:
  projectName: demo
  productName: Demo
  productVersion: 0.1.0
  platforms: [macos, android]
  homeAppId: home
capabilities:
  browser: true
surfaces:
  - native: browser
    role: main
    launch: true
    platforms: [macos]
  - lxapp: home
    role: main
    launch: true
    platforms: [android]
"#;

        let config = load_config_yaml(yaml).unwrap();
        let macos = config.resolved_ui_for_platform("macos").unwrap().unwrap();
        let android = config.resolved_ui_for_platform("android").unwrap().unwrap();

        assert_eq!(macos["launch"]["initialSurface"], "browser");
        assert_eq!(android["launch"]["initialSurface"], "home");
        assert_eq!(android["surfaces"][0]["content"]["appId"], "home");
    }

    #[test]
    fn surfaces_rejects_size_on_main() {
        let surfaces = vec![SurfaceDecl {
            launch: true,
            size: Some(SurfaceSize {
                width: Some(320),
                height: None,
            }),
            ..lxapp_decl("home", SurfaceRole::Main)
        }];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("size is only valid on an aside"), "{err}");
    }

    #[test]
    fn surfaces_rejects_float_without_tray() {
        let surfaces = vec![lxapp_decl("panel", SurfaceRole::Float)];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("tray-anchored popover"), "got: {err}");
    }

    #[test]
    fn surfaces_rejects_unsupported_tray_action() {
        let yaml = r#"
surfaces:
  - lxapp: home
    role: main
    tray:
      action: open
"#;

        let err = yaml::from_str::<LingXiaConfig>(yaml)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown variant"), "{err}");
        assert!(err.contains("toggle") && err.contains("activate"), "{err}");
    }

    #[test]
    fn surfaces_rejects_multiple_tray_entries() {
        let empty_tray = SurfaceTray {
            icon: None,
            label: None,
            action: None,
            exclusive: false,
            size: None,
        };
        let surfaces = vec![
            SurfaceDecl {
                tray: Some(empty_tray.clone()),
                ..lxapp_decl("home", SurfaceRole::Main)
            },
            SurfaceDecl {
                edge: Some("right".into()),
                tray: Some(empty_tray),
                ..lxapp_decl("chat", SurfaceRole::Aside)
            },
        ];

        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("at most one surface may declare tray"),
            "{err}"
        );
    }

    #[test]
    fn surfaces_rejects_native_terminal_without_capability() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("home", SurfaceRole::Main)
            },
            SurfaceDecl {
                native: Some("terminal".into()),
                edge: Some("bottom".into()),
                ..surface_decl(SurfaceRole::Aside)
            },
        ];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("capabilities.terminal"), "{err}");
    }

    #[test]
    fn surfaces_rejects_two_launch_mains() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("a", SurfaceRole::Main)
            },
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("b", SurfaceRole::Main)
            },
        ];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("at most one"), "{err}");
    }

    #[test]
    fn surfaces_rejects_two_declared_mains_even_with_one_launch() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("a", SurfaceRole::Main)
            },
            lxapp_decl("b", SurfaceRole::Main),
        ];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("exactly one declared main"), "{err}");
    }

    #[test]
    fn surfaces_rejects_duplicate_content_key() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("dup", SurfaceRole::Main)
            },
            SurfaceDecl {
                edge: Some("right".into()),
                ..lxapp_decl("dup", SurfaceRole::Aside)
            },
        ];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("duplicate declaration for 'dup'"), "{err}");
    }

    #[test]
    fn surfaces_rejects_same_name_across_content_kinds() {
        // Surface ids share one namespace: an lxapp and a native capability
        // with the same name would emit two surfaces with identical ids.
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("terminal", SurfaceRole::Main)
            },
            SurfaceDecl {
                native: Some("terminal".into()),
                ..surface_decl(SurfaceRole::Aside)
            },
        ];
        let err = surfaces_to_ui(&surfaces, true, false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("duplicate declaration for 'terminal'"),
            "{err}"
        );
    }

    #[test]
    fn surfaces_terminal_partial_size_keeps_default_height() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("home", SurfaceRole::Main)
            },
            SurfaceDecl {
                native: Some("terminal".into()),
                size: Some(SurfaceSize {
                    width: Some(400),
                    height: None,
                }),
                ..surface_decl(SurfaceRole::Aside)
            },
        ];
        let ui = surfaces_to_ui(&surfaces, true, false).unwrap();
        assert_eq!(ui["surfaces"][1]["size"]["width"], 400);
        assert_eq!(ui["surfaces"][1]["size"]["height"], 320);
    }

    #[test]
    fn surfaces_rejects_launch_on_aside() {
        let surfaces = vec![
            SurfaceDecl {
                launch: true,
                ..lxapp_decl("a", SurfaceRole::Main)
            },
            SurfaceDecl {
                launch: true,
                edge: Some("right".into()),
                ..lxapp_decl("b", SurfaceRole::Aside)
            },
        ];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("launch: true is only valid on a main"),
            "{err}"
        );
    }

    #[test]
    fn surfaces_rejects_edge_on_main() {
        let surfaces = vec![SurfaceDecl {
            launch: true,
            edge: Some("right".into()),
            ..lxapp_decl("a", SurfaceRole::Main)
        }];
        let err = surfaces_to_ui(&surfaces, false, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("edge is only valid on an aside"), "{err}");
    }

    #[test]
    fn macos_ui_rejects_unsupported_panel_edge() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.generated_ui = Some(serde_json::json!({
            "launch": {
                "initialSurface": "main"
            },
            "surfaces": [{
                "id": "main",
                "role": "main",
                "content": {
                    "kind": "lxapp",
                    "appId": "main"
                }
            }, {
                "id": "panel",
                "role": "aside",
                "attachTo": "main",
                "edge": "diagonal",
                "content": {
                    "kind": "lxapp",
                    "appId": "panel"
                }
            }],
            "activators": []
        }));

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("unknown edge 'diagonal'"));
    }
}
