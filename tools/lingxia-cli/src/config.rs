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

/// Host project configuration (native app project)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<ShellConfig>,
    /// Generated UI structure (`ui.json`). Built from `surfaces` at load time;
    /// never authored directly, so it is not read from the yaml.
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub ui: Option<Value>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeaturesConfig {
    #[serde(default = "default_true")]
    pub app_service: bool,
    #[serde(default)]
    pub shell: bool,
    #[serde(default)]
    pub devtools: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            app_service: true,
            shell: false,
            devtools: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesConfig {
    #[serde(default)]
    pub notifications: bool,
    #[serde(default)]
    pub terminal: bool,
    /// Which window edge the built-in terminal panel docks to: `bottom`
    /// (default) or `top`. Only meaningful when `terminal` is enabled.
    #[serde(default)]
    pub terminal_edge: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShellConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webui: Option<ShellWebUiConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShellWebUiConfig {
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
// This is INPUT schema only. `surfaces_to_ui` maps it into the existing
// internal `ui` JSON structure (`launch`/`surfaces`/`activators`) that the
// macOS runtime already consumes, so no runtime/Swift code changes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceRender {
    #[default]
    Lxapp,
    Native,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceRole {
    Main,
    Aside,
    Float,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceDecl {
    /// Surface id. For `render: lxapp` this doubles as the lxapp `appId`.
    pub id: String,
    #[serde(default)]
    pub render: SurfaceRender,
    pub role: SurfaceRole,
    /// At most one `role: main` may set `launch: true` (the initial surface).
    #[serde(default)]
    pub launch: bool,
    /// Required for `role: aside`. One of left|right|top|bottom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<String>,
    /// Inline sidebar entry; clicking it toggles this surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<SurfaceSidebar>,
    /// Inline tray/menubar entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tray: Option<SurfaceTray>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceSidebar {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Placement within the sidebar: `top` (default) or `bottom`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceTray {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

/// Map the `surfaces:` declaration into the internal `ui` JSON structure
/// the runtime consumes. Produces the SAME shape the `ui:` block does.
///
/// Mapping:
/// - `role: main` + `render: lxapp` -> surface `role: main`,
///   content `{ kind: lxapp, appId: <id> }`. `launch: true` ->
///   `launch.initialSurface = <id>`.
/// - `role: aside` + `render: lxapp` -> surface `role: aside`,
///   `attachTo: <main>`, edge `left|right|top|bottom` carried through verbatim.
/// - `role: aside` + `render: native` (id `terminal`) -> the terminal surface,
///   emitted in the EXACT shape `assets/ui.rs::add_terminal_ui` produces so
///   the auto-inject guard skips it (no double inject) and output is identical.
/// - `sidebar` -> a `sidebarItem` activator toggling the surface.
/// - `tray` -> a `menuBarItem` activator (closest existing kind).
fn surfaces_to_ui(surfaces: &[SurfaceDecl], terminal_enabled: bool) -> Result<Value> {
    // `role: float` is not supported. Reject it up front with
    // a clear message rather than mis-mapping it.
    if let Some(float) = surfaces.iter().find(|s| s.role == SurfaceRole::Float) {
        return Err(anyhow!(
            "surface '{}' uses role: float which is not supported",
            float.id.trim()
        ));
    }
    // Identify the launch main.
    let mains: Vec<&SurfaceDecl> = surfaces
        .iter()
        .filter(|s| s.role == SurfaceRole::Main)
        .collect();
    let launch_mains: Vec<&SurfaceDecl> = mains.iter().copied().filter(|s| s.launch).collect();
    if launch_mains.len() > 1 {
        return Err(anyhow!(
            "surfaces: at most one main surface may set launch: true"
        ));
    }
    let launch_main = launch_mains
        .first()
        .copied()
        .or_else(|| mains.first().copied())
        .ok_or_else(|| anyhow!("surfaces: requires exactly one main surface"))?;
    let launch_id = launch_main.id.trim().to_string();
    if launch_id.is_empty() {
        return Err(anyhow!("surfaces[].id must not be empty"));
    }

    // Structural validation: unique ids, and fields that only make sense on a
    // given role.
    let mut seen_ids = std::collections::HashSet::new();
    for surface in surfaces {
        let id = surface.id.trim();
        if id.is_empty() {
            return Err(anyhow!("surfaces[].id must not be empty"));
        }
        if !seen_ids.insert(id) {
            return Err(anyhow!("surfaces: duplicate surface id '{id}'"));
        }
        if surface.launch && surface.role != SurfaceRole::Main {
            return Err(anyhow!(
                "surface '{id}': launch: true is only valid on a main surface"
            ));
        }
        if surface.edge.is_some() && surface.role != SurfaceRole::Aside {
            return Err(anyhow!(
                "surface '{id}': edge is only valid on an aside surface"
            ));
        }
    }

    let mut out_surfaces: Vec<Value> = Vec::new();
    let mut out_activators: Vec<Value> = Vec::new();

    for surface in surfaces {
        let id = surface.id.trim();
        if id.is_empty() {
            return Err(anyhow!("surfaces[].id must not be empty"));
        }
        match surface.role {
            SurfaceRole::Float => {
                return Err(anyhow!(
                    "surface '{id}' uses role: float which is not supported"
                ));
            }
            SurfaceRole::Main => {
                if surface.render == SurfaceRender::Native {
                    return Err(anyhow!(
                        "surface '{id}': role: main with render: native is not supported"
                    ));
                }
                out_surfaces.push(json!({
                    "id": id,
                    "role": "main",
                    "content": { "kind": "lxapp", "appId": id }
                }));
            }
            SurfaceRole::Aside => {
                let edge = surface
                    .edge
                    .as_deref()
                    .map(str::trim)
                    .filter(|e| !e.is_empty())
                    .ok_or_else(|| anyhow!("aside surface '{id}' requires an edge"))?;
                match surface.render {
                    SurfaceRender::Lxapp => {
                        let mapped_edge = map_edge(edge, id)?;
                        out_surfaces.push(json!({
                            "id": id,
                            "role": "aside",
                            "attachTo": launch_id,
                            "edge": mapped_edge,
                            "content": { "kind": "lxapp", "appId": id }
                        }));
                    }
                    SurfaceRender::Native => {
                        // The only native surface currently supported is the
                        // built-in terminal. Emit it in the EXACT shape that
                        // `assets/ui.rs::add_terminal_ui` produces so the
                        // downstream auto-inject is a no-op (double-inject guard)
                        // and the generated JSON is byte-identical to today's.
                        if id != "terminal" {
                            return Err(anyhow!(
                                "native surface '{id}' is not supported; only the built-in 'terminal' surface is available"
                            ));
                        }
                        if !terminal_enabled {
                            return Err(anyhow!(
                                "surface '{id}' uses render: native (terminal) but capabilities.terminal is not enabled"
                            ));
                        }
                        if edge != "bottom" && edge != "top" {
                            return Err(anyhow!(
                                "terminal surface '{id}' must use edge 'top' or 'bottom'"
                            ));
                        }
                        out_surfaces.push(json!({
                            "id": "terminal",
                            "role": "aside",
                            "attachTo": launch_id,
                            "edge": edge,
                            "size": { "height": 320 },
                            "content": { "kind": "terminal" }
                        }));
                        // The terminal surface is always available once declared
                        // (openable programmatically); a sidebar entry is opt-in.
                        // Emit `terminalSidebar` only when the surface declares a
                        // `sidebar:` block. Its icon defaults to the host-provided
                        // built-in when omitted, so authors never write an internal
                        // sentinel; a supplied icon is a normal repo-relative path.
                        if let Some(sidebar) = &surface.sidebar {
                            let terminal_icon = sidebar
                                .icon
                                .as_deref()
                                .unwrap_or("__lingxia_builtin__/terminal.svg");
                            out_activators.push(json!({
                                "id": "terminalSidebar",
                                "kind": "sidebarItem",
                                "hostSurface": launch_id,
                                "label": "Terminal",
                                "icon": terminal_icon,
                                "action": { "kind": "toggleSurface", "surface": "terminal" }
                            }));
                        }
                        // A native terminal carries its sidebar implicitly; skip
                        // the generic sidebar/tray emission below for it.
                        continue;
                    }
                }
            }
        }

        if let Some(sidebar) = &surface.sidebar {
            let mut activator = Map::new();
            activator.insert("id".into(), json!(format!("{id}Sidebar")));
            activator.insert("kind".into(), json!("sidebarItem"));
            activator.insert("hostSurface".into(), json!(launch_id));
            let label = sidebar
                .label
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(id);
            activator.insert("label".into(), json!(label));
            if let Some(icon) = sidebar
                .icon
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                activator.insert("icon".into(), json!(icon));
            }
            if let Some(section) = sidebar
                .section
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if section != "top" && section != "bottom" {
                    return Err(anyhow!(
                        "surface '{id}' sidebar.section must be 'top' or 'bottom'"
                    ));
                }
                // The internal activator schema has no first-class placement
                // field today; carry `section` through verbatim (runtime ignores
                // unknown keys, defaulting to top).
                activator.insert("section".into(), json!(section));
            }
            activator.insert(
                "action".into(),
                json!({ "kind": "toggleSurface", "surface": id }),
            );
            out_activators.push(Value::Object(activator));
        }

        if let Some(tray) = &surface.tray {
            // The internal schema's closest existing kind is `menuBarItem`.
            // (There is no dedicated status/tray runtime kind today.)
            let mut activator = Map::new();
            activator.insert("id".into(), json!(format!("{id}Tray")));
            activator.insert("kind".into(), json!("menuBarItem"));
            if let Some(icon) = tray
                .icon
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                activator.insert("icon".into(), json!(icon));
            }
            let action_kind = tray
                .action
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("toggleSurface");
            activator.insert(
                "action".into(),
                json!({ "kind": action_kind, "surface": id }),
            );
            out_activators.push(Value::Object(activator));
        }
    }

    Ok(json!({
        "launch": { "initialSurface": launch_id },
        "surfaces": out_surfaces,
        "activators": out_activators
    }))
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

/// Host app settings (checked into git via `lingxia.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostAppConfig {
    /// Project name (technical identifier, used for Rust lib naming, e.g., "myapp" -> "myapp-lib")
    pub project_name: String,

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

    #[serde(rename = "homeAppId")]
    pub home_app_id: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_target: Option<String>, // e.g., "17.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swift_version: Option<String>,
    /// SwiftPM target name for resources lookup.
    /// If omitted, CLI will try app.projectName or infer from Sources/ when unambiguous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacosConfig {
    /// Bundle identifier (e.g., "app.lingxia.example")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,

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
}

impl LingXiaConfig {
    /// Get the project name from config
    pub fn get_project_name(&self) -> Option<&str> {
        self.app.as_ref().map(|app| app.project_name.as_str())
    }

    /// Get the Rust library directory name (e.g., "myproject-lib")
    pub fn get_rust_lib_name(&self) -> Option<String> {
        self.get_project_name().map(|name| format!("{}-lib", name))
    }

    pub fn app_service_enabled(&self) -> bool {
        self.features
            .as_ref()
            .map(|features| features.app_service)
            .unwrap_or(true)
    }

    pub fn shell_enabled(&self, platform: &str) -> bool {
        let shell_requested = self
            .features
            .as_ref()
            .map(|features| features.shell)
            .unwrap_or(false);
        (shell_requested || self.terminal_enabled(platform))
            && matches!(platform, "macos" | "windows")
    }

    pub fn terminal_enabled(&self, platform: &str) -> bool {
        let terminal_requested = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.terminal)
            .unwrap_or(false);
        terminal_requested && matches!(platform, "macos" | "windows")
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
        if self.shell_enabled(platform) {
            features.push("shell-runtime".to_string());
        }
        if self.terminal_enabled(platform) {
            features.push("terminal-runtime".to_string());
        }
        if self.shell_enabled(platform) {
            features.push("webview-input".to_string());
        }
        if self.devtools_enabled() {
            features.push("devtools".to_string());
        }
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
                product_name: project_name.to_string(),
                product_version: "0.0.1".to_string(),
                lingxia_server: Some(LingxiaServer::Single("https://api.example.com".to_string())),
                lingxia_id: None,
                package_id_suffix: None,
                platforms: vec!["android".to_string()],
                home_app_id: home_app_id.to_string(),
            }),
            android: Some(AndroidConfig {
                package_id: package_id.to_string(),
                min_sdk: Some(28),
                target_sdk: Some(35),
                compile_sdk: Some(35),
                ndk_version: None, // Auto-detect
                api_level: None,   // Derive from minSdk/targetSdk
            }),
            ios: None,
            macos: None,
            harmony: None,
            windows: None,
            features: Some(FeaturesConfig::default()),
            capabilities: Some(CapabilitiesConfig::default()),
            shell: None,
            ui: None,
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
        // `surfaces` is independent of `capabilities`; terminal availability is
        // gated by `capabilities.terminal` (any-platform truthiness here, the
        // per-platform gating stays in `terminal_enabled`).
        let terminal_enabled = self
            .capabilities
            .as_ref()
            .map(|capabilities| capabilities.terminal)
            .unwrap_or(false);
        self.ui = Some(surfaces_to_ui(surfaces, terminal_enabled)?);
        Ok(())
    }

    fn validate(&self) -> Result<()> {
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
            if app.platforms.is_empty() {
                return Err(anyhow!("app.platforms must include at least one platform"));
            }
            let home_app_id = app.home_app_id.trim();
            if home_app_id.is_empty() {
                return Err(anyhow!("app.homeAppId must not be empty"));
            }
            if is_sdk_reserved_app_id(home_app_id) {
                return Err(anyhow!(
                    "app.homeAppId '{}' is an SDK-reserved appId. Pick a different id \
                     for your home app (e.g. the project's reverse-domain identifier).",
                    app.home_app_id
                ));
            }
            if let Some(server) = app.lingxia_server.as_ref() {
                validate_lingxia_server(server)?;
            }
            if let Some(over) = app.package_id_suffix.as_ref() {
                validate_package_id_suffix_overrides(over)?;
            }
            let has_macos = app
                .platforms
                .iter()
                .any(|platform| platform.eq_ignore_ascii_case("macos"));
            if has_macos {
                let Some(ui) = &self.ui else {
                    return Err(anyhow!(
                        "ui is required for macOS host app projects; define ui.launch, ui.surfaces, and ui.activators"
                    ));
                };
                validate_macos_ui_config(ui, self.terminal_enabled("macos"))?;
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
                if is_sdk_reserved_app_id(app_id) {
                    return Err(anyhow!(
                        "resources.bundles[{app_id}] uses an SDK-reserved appId. \
                         To customize the in-app browser webui, use `shell.webui.path` \
                         (or `shell.webui.package`) instead of declaring \
                         `{app_id}` as a resource bundle."
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
        if let Some(webui) = self.shell.as_ref().and_then(|shell| shell.webui.as_ref()) {
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
                    "shell.webui must use either path or package, not both"
                ));
            }
            if webui
                .version
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| value.is_empty())
            {
                return Err(anyhow!("shell.webui.version must not be empty"));
            }
        }
        if let Some(ui) = &self.ui
            && !ui.is_object()
        {
            return Err(anyhow!("ui must be a JSON object"));
        }
        Ok(())
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
    Terminal,
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

/// App IDs reserved for SDK-internal hosts that ship their own customization API.
/// These must not appear in `resources.bundles` or `app.homeAppId`; the SDK provides
/// dedicated config keys (e.g. `shell.webui.*` for the in-app browser webui).
///
/// Source of truth for each entry (kept in sync manually to avoid pulling the
/// full browser runtime into the CLI build):
/// - `crate::host_assets::SHELL_WEBUI_APP_ID` mirrors `lingxia_browser::BUILTIN_BROWSER_APPID`.
const SDK_RESERVED_APP_IDS: &[&str] = &[crate::host_assets::SHELL_WEBUI_APP_ID];

fn is_sdk_reserved_app_id(app_id: &str) -> bool {
    SDK_RESERVED_APP_IDS.contains(&app_id)
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

fn validate_macos_ui_config(ui: &Value, terminal_enabled: bool) -> Result<()> {
    let ui_obj = ui
        .as_object()
        .ok_or_else(|| anyhow!("ui must be a JSON object"))?;
    let launch = ui_obj
        .get("launch")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("ui.launch must be an object"))?;
    let initial_surface = non_empty_str(launch.get("initialSurface"), "ui.launch.initialSurface")?;
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

    let mut surface_by_id = HashMap::<String, MacosUiSurface>::new();
    let mut seen_app_ids = HashSet::<String>::new();

    for (index, surface) in surfaces.iter().enumerate() {
        let obj = surface
            .as_object()
            .ok_or_else(|| anyhow!("ui.surfaces[{index}] must be an object"))?;
        let id = non_empty_str(obj.get("id"), &format!("ui.surfaces[{index}].id"))?;
        if surface_by_id.contains_key(id) {
            return Err(anyhow!("duplicate ui surface id '{id}'"));
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
            "terminal" => {
                if !terminal_enabled {
                    return Err(anyhow!(
                        "ui surface '{id}' uses terminal content but capabilities.terminal is not enabled"
                    ));
                }
                if optional_non_empty_str(content.get("backend")).is_some() {
                    return Err(anyhow!(
                        "ui surface '{id}' must not set content.backend; terminal runtime is selected internally"
                    ));
                }
                MacosUiContentKind::Terminal
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

    let Some(initial) = surface_by_id.get(initial_surface) else {
        return Err(anyhow!(
            "ui.launch.initialSurface references unknown surface '{initial_surface}'"
        ));
    };
    if !matches!(
        initial.role,
        MacosUiSurfaceRole::Main | MacosUiSurfaceRole::Float | MacosUiSurfaceRole::Aside
    ) {
        return Err(anyhow!(
            "ui.launch.initialSurface must reference a supported macOS surface"
        ));
    }

    let root_ids = surface_by_id
        .iter()
        .filter_map(|(id, surface)| {
            matches!(
                surface.role,
                MacosUiSurfaceRole::Main | MacosUiSurfaceRole::Float
            )
            .then_some(id.as_str())
        })
        .collect::<Vec<_>>();
    if root_ids.len() != 1 {
        return Err(anyhow!(
            "macOS app UI currently requires exactly one root main or float surface"
        ));
    }
    let root_id = root_ids[0];

    for (id, surface) in &surface_by_id {
        if surface.content_kind == MacosUiContentKind::Terminal {
            if surface.role != MacosUiSurfaceRole::Aside {
                return Err(anyhow!("terminal ui surface '{id}' must use role 'aside'"));
            }
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
                if !matches!(
                    parent.role,
                    MacosUiSurfaceRole::Main | MacosUiSurfaceRole::Float
                ) {
                    return Err(anyhow!(
                        "macOS app UI currently does not support aside -> aside; surface '{id}' attaches to '{parent_id}'"
                    ));
                }
                if parent_id != root_id {
                    return Err(anyhow!(
                        "macOS app UI currently supports asides attached only to the root surface"
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
        if !seen_activator_ids.insert(id.to_string()) {
            return Err(anyhow!("duplicate ui activator id '{id}'"));
        }
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
            "toggleSurface" | "openSurface" | "closeSurface" | "focusSurface" => {}
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
        if !surface_by_id.contains_key(action_surface) {
            return Err(anyhow!(
                "ui activator '{id}' references unknown surface '{action_surface}'"
            ));
        }

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
                if !surface_by_id.contains_key(host_surface) {
                    return Err(anyhow!(
                        "ui activator '{id}' references unknown hostSurface '{host_surface}'"
                    ));
                }
            }
            other => {
                return Err(anyhow!("ui activator '{id}' has unknown kind '{other}'"));
            }
        }
    }

    Ok(())
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
    use tempfile::TempDir;

    #[test]
    fn test_android_api_level_derivation() {
        let config = AndroidConfig {
            package_id: "com.example.app".to_string(),
            min_sdk: Some(28),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: None,
        };
        assert_eq!(config.get_api_level(), 28);

        let config_explicit = AndroidConfig {
            package_id: "com.example.app".to_string(),
            min_sdk: Some(28),
            target_sdk: Some(35),
            compile_sdk: Some(35),
            ndk_version: None,
            api_level: Some(33),
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
        assert_eq!(app.home_app_id, "my-app");
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
                path: Some("./my-shell-webui".to_string()),
                package: None,
                version: None,
            });

        let err = config
            .validate()
            .expect_err("validate must reject reserved appId");
        let msg = err.to_string();
        assert!(
            msg.contains("app.lingxia.browser") && msg.contains("shell.webui"),
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
    fn shell_feature_is_only_effective_on_macos() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.features.as_mut().unwrap().shell = true;

        assert!(config.shell_enabled("macos"));
        assert!(!config.shell_enabled("android"));
        assert!(!config.shell_enabled("ios"));
        assert!(!config.shell_enabled("harmony"));

        assert_eq!(
            config.native_features_for_platform("macos"),
            vec![
                "standard".to_string(),
                "shell-runtime".to_string(),
                "webview-input".to_string(),
            ]
        );
        assert_eq!(
            config.native_features_for_platform("harmony"),
            vec!["standard".to_string()]
        );
    }

    #[test]
    fn terminal_capability_enables_macos_and_windows_runtime() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        config.capabilities.as_mut().unwrap().terminal = true;

        assert!(config.shell_enabled("macos"));
        assert!(config.terminal_enabled("windows"));
        assert!(!config.shell_enabled("android"));
        assert_eq!(
            config.native_features_for_platform("macos"),
            vec![
                "standard".to_string(),
                "shell-runtime".to_string(),
                "terminal-runtime".to_string(),
                "webview-input".to_string(),
            ]
        );
        assert_eq!(
            config.native_features_for_platform("windows"),
            vec![
                "standard".to_string(),
                "shell-runtime".to_string(),
                "terminal-runtime".to_string(),
                "webview-input".to_string(),
            ]
        );
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
        let config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");

        config.save(temp.path()).unwrap();

        let loaded = LingXiaConfig::load(temp.path()).unwrap();
        assert_eq!(loaded.app.as_ref().unwrap().project_name, "my-app");
        assert!(temp.path().join(HOST_CONFIG_FILE).exists());
    }

    #[test]
    fn macos_host_requires_ui() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("ui is required for macOS host app projects"));
    }

    #[test]
    fn macos_ui_accepts_current_runtime_subset() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.ui = Some(serde_json::json!({
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
    fn macos_ui_accepts_terminal_attach_panel_bottom() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.capabilities.as_mut().unwrap().terminal = true;
        config.ui = Some(serde_json::json!({
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
                    "kind": "terminal"
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
        config.ui = Some(serde_json::json!({
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
                    "kind": "terminal"
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
        config.ui = Some(serde_json::json!({
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
                    "kind": "terminal"
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
        config.ui = Some(serde_json::json!({
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
                    "kind": "terminal",
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
        config.ui = Some(serde_json::json!({
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
                    "kind": "focusSurface",
                    "surface": "main"
                }
            }]
        }));

        config.validate().unwrap();
    }

    #[test]
    fn macos_ui_rejects_non_macos_activators() {
        for kind in ["trayItem", "deepLink"] {
            let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
            let app = config.app.as_mut().unwrap();
            app.platforms = vec!["macos".to_string()];
            config.ui = Some(serde_json::json!({
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
                        "kind": "focusSurface",
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
        missing_host.ui = Some(serde_json::json!({
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
                    "kind": "focusSurface",
                    "surface": "main"
                }
            }]
        }));
        let err = missing_host.validate().unwrap_err().to_string();
        assert!(err.contains("hostSurface"));

        let mut app_level_host =
            LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        app_level_host.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
        app_level_host.ui = Some(serde_json::json!({
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
                    "kind": "focusSurface",
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
        config.ui = Some(serde_json::json!({
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

    #[test]
    fn surfaces_maps_showcase_to_internal_ui() {
        let surfaces = vec![
            SurfaceDecl {
                id: "lingxia-showcase".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Main,
                launch: true,
                edge: None,
                sidebar: None,
                tray: None,
            },
            SurfaceDecl {
                id: "lingxia-chat".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Aside,
                launch: false,
                edge: Some("right".into()),
                sidebar: Some(SurfaceSidebar {
                    icon: Some("icons/chat.svg".into()),
                    label: Some("AI Chat".into()),
                    section: None,
                }),
                tray: None,
            },
            SurfaceDecl {
                id: "terminal".into(),
                render: SurfaceRender::Native,
                role: SurfaceRole::Aside,
                launch: false,
                edge: Some("bottom".into()),
                sidebar: Some(SurfaceSidebar {
                    icon: Some("__lingxia_builtin__/terminal.svg".into()),
                    label: None,
                    section: None,
                }),
                tray: None,
            },
        ];

        let ui = surfaces_to_ui(&surfaces, true).unwrap();
        let expected = serde_json::json!({
            "launch": { "initialSurface": "lingxia-showcase" },
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
                    "content": { "kind": "terminal" }
                }
            ],
            "activators": [
                {
                    "id": "lingxia-chatSidebar",
                    "kind": "sidebarItem",
                    "hostSurface": "lingxia-showcase",
                    "label": "AI Chat",
                    "icon": "icons/chat.svg",
                    "action": { "kind": "toggleSurface", "surface": "lingxia-chat" }
                },
                {
                    "id": "terminalSidebar",
                    "kind": "sidebarItem",
                    "hostSurface": "lingxia-showcase",
                    "label": "Terminal",
                    "icon": "__lingxia_builtin__/terminal.svg",
                    "action": { "kind": "toggleSurface", "surface": "terminal" }
                }
            ]
        });
        assert_eq!(ui, expected);

        // Full config round-trip: apply_surfaces + validate must accept it.
        let mut config = LingXiaConfig::new_android("lingxia", "com.example", "lingxia-showcase");
        config.app.as_mut().unwrap().platforms = vec!["macos".to_string()];
        config.capabilities.as_mut().unwrap().terminal = true;
        config.ui = None;
        config.surfaces = Some(surfaces);
        config.apply_surfaces().unwrap();
        config.validate().unwrap();
    }

    #[test]
    fn surfaces_rejects_float_role() {
        let surfaces = vec![SurfaceDecl {
            id: "popup".into(),
            render: SurfaceRender::Lxapp,
            role: SurfaceRole::Float,
            launch: false,
            edge: None,
            sidebar: None,
            tray: None,
        }];
        let err = surfaces_to_ui(&surfaces, false).unwrap_err().to_string();
        assert!(err.contains("not supported"), "{err}");
    }

    #[test]
    fn surfaces_rejects_native_terminal_without_capability() {
        let surfaces = vec![
            SurfaceDecl {
                id: "home".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Main,
                launch: true,
                edge: None,
                sidebar: None,
                tray: None,
            },
            SurfaceDecl {
                id: "terminal".into(),
                render: SurfaceRender::Native,
                role: SurfaceRole::Aside,
                launch: false,
                edge: Some("bottom".into()),
                sidebar: None,
                tray: None,
            },
        ];
        let err = surfaces_to_ui(&surfaces, false).unwrap_err().to_string();
        assert!(err.contains("capabilities.terminal"), "{err}");
    }

    #[test]
    fn surfaces_rejects_two_launch_mains() {
        let surfaces = vec![
            SurfaceDecl {
                id: "a".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Main,
                launch: true,
                edge: None,
                sidebar: None,
                tray: None,
            },
            SurfaceDecl {
                id: "b".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Main,
                launch: true,
                edge: None,
                sidebar: None,
                tray: None,
            },
        ];
        let err = surfaces_to_ui(&surfaces, false).unwrap_err().to_string();
        assert!(err.contains("at most one"), "{err}");
    }

    #[test]
    fn surfaces_rejects_duplicate_id() {
        let surfaces = vec![
            SurfaceDecl {
                id: "dup".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Main,
                launch: true,
                edge: None,
                sidebar: None,
                tray: None,
            },
            SurfaceDecl {
                id: "dup".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Aside,
                launch: false,
                edge: Some("right".into()),
                sidebar: None,
                tray: None,
            },
        ];
        let err = surfaces_to_ui(&surfaces, false).unwrap_err().to_string();
        assert!(err.contains("duplicate surface id"), "{err}");
    }

    #[test]
    fn surfaces_rejects_launch_on_aside() {
        let surfaces = vec![
            SurfaceDecl {
                id: "a".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Main,
                launch: true,
                edge: None,
                sidebar: None,
                tray: None,
            },
            SurfaceDecl {
                id: "b".into(),
                render: SurfaceRender::Lxapp,
                role: SurfaceRole::Aside,
                launch: true,
                edge: Some("right".into()),
                sidebar: None,
                tray: None,
            },
        ];
        let err = surfaces_to_ui(&surfaces, false).unwrap_err().to_string();
        assert!(err.contains("launch: true is only valid on a main"), "{err}");
    }

    #[test]
    fn surfaces_rejects_edge_on_main() {
        let surfaces = vec![SurfaceDecl {
            id: "a".into(),
            render: SurfaceRender::Lxapp,
            role: SurfaceRole::Main,
            launch: true,
            edge: Some("right".into()),
            sidebar: None,
            tray: None,
        }];
        let err = surfaces_to_ui(&surfaces, false).unwrap_err().to_string();
        assert!(err.contains("edge is only valid on an aside"), "{err}");
    }

    #[test]
    fn macos_ui_rejects_unsupported_panel_edge() {
        let mut config = LingXiaConfig::new_android("my-app", "com.example.myapp", "my-app");
        let app = config.app.as_mut().unwrap();
        app.platforms = vec!["macos".to_string()];
        config.ui = Some(serde_json::json!({
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
