use anyhow::{Context, Result, anyhow};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml_ng as yaml;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub const HOST_CONFIG_FILE: &str = "lingxia.yaml";
pub const LXAPP_BUILD_CONFIG_FILE: &str = "lxapp.config.ts";
pub const DEFAULT_CACHE_MAX_AGE_DAYS: u64 = 7;
pub const DEFAULT_CACHE_MAX_SIZE_MB: u64 = 1024;

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
    /// App-level UI config used to generate `ui.json` at build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesConfig>,
}

/// Host app settings (checked into git via `lingxia.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAppConfig {
    /// Project name (technical identifier, used for Rust lib naming, e.g., "myapp" -> "myapp-lib")
    pub project_name: String,

    /// Product name (user-facing display name)
    pub product_name: String,
    pub product_version: String,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_server: Option<String>,

    #[serde(default)]
    #[serde(rename = "lingxiaId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lingxia_id: Option<String>,

    /// Platforms to build for this app (e.g. ["android"]).
    pub platforms: Vec<String>,

    // Keep explicit spelling for "ID" (not "Id") to match runtime `app.json` schema.
    #[serde(rename = "homeLxAppID")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_lxapp_id: Option<String>,

    /// Maximum age in days for cache files before cleanup (default: 7)
    /// Set to 0 to disable automatic cache cleanup
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_max_age_days: Option<u64>,

    /// Maximum cache size in MiB for each lxapp user cache directory (default: 1024)
    /// Set to 0 to disable capacity-based cache cleanup.
    #[serde(default)]
    #[serde(rename = "cacheMaxSizeMB")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_max_size_mb: Option<u64>,
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
pub struct ResourcesConfig {
    /// Path to i18n resources (relative to project root)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i18n: Option<String>,
    /// Path to icon resources (relative to project root)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<String>,
    /// Bundle project directories to build and copy into host resources.
    ///
    /// String entries use the directory path directly and default to copying `dist/`
    /// into a target directory named after `lxapp.json.appId`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundles: Option<Vec<ResourceBundleConfig>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceBundleConfig {
    Path(String),
    Detailed(ResourceBundleDetail),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ResourceBundleType {
    #[default]
    Lxapp,
    Npm,
}

impl ResourceBundleType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lxapp => "lxapp",
            Self::Npm => "npm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceBundleDetail {
    #[serde(rename = "type", default)]
    pub bundle_type: ResourceBundleType,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
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

        let config: LingXiaConfig = yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", config_path.display()))?;
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

    pub fn save_with_comments(&self, project_root: &Path) -> Result<()> {
        let config_path = project_root.join(HOST_CONFIG_FILE);
        let content = self.render_with_comments()?;
        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write {}", HOST_CONFIG_FILE))?;
        Ok(())
    }

    /// Create a default Android config
    #[allow(dead_code)] // Used in tests
    pub fn new_android(project_name: &str, package_id: &str, home_lxapp_id: &str) -> Self {
        Self {
            app: Some(HostAppConfig {
                project_name: project_name.to_string(),
                product_name: project_name.to_string(),
                product_version: "0.0.1".to_string(),
                api_server: None,
                lingxia_id: None,
                platforms: vec!["android".to_string()],
                home_lxapp_id: Some(home_lxapp_id.to_string()),
                cache_max_age_days: Some(DEFAULT_CACHE_MAX_AGE_DAYS),
                cache_max_size_mb: Some(DEFAULT_CACHE_MAX_SIZE_MB),
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
            ui: None,
            resources: Some(ResourcesConfig {
                i18n: None,
                icons: None,
                bundles: Some(vec![ResourceBundleConfig::Detailed(ResourceBundleDetail {
                    bundle_type: ResourceBundleType::Lxapp,
                    path: project_name.to_string(),
                    target: None,
                })]),
            }),
        }
    }

    pub fn splash_path(&self) -> Option<&str> {
        self.ui
            .as_ref()
            .and_then(|ui| ui.get("launch"))
            .and_then(|launch| launch.get("splash"))
            .and_then(|splash| splash.get("path"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
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
                validate_macos_ui_config(ui)?;
            }
        }
        if let Some(resources) = &self.resources
            && let Some(bundles) = &resources.bundles
        {
            for bundle in bundles {
                let (path, bundle_type, target) = match bundle {
                    ResourceBundleConfig::Path(path) => {
                        (path.as_str(), ResourceBundleType::Lxapp, None)
                    }
                    ResourceBundleConfig::Detailed(detail) => (
                        detail.path.as_str(),
                        detail.bundle_type,
                        detail.target.as_deref(),
                    ),
                };
                if path.trim().is_empty() {
                    return Err(anyhow!("resources.bundles path must not be empty"));
                }
                if matches!(bundle_type, ResourceBundleType::Npm)
                    && target.map(str::trim).filter(|s| !s.is_empty()).is_none()
                {
                    return Err(anyhow!(
                        "resources.bundles target is required for bundles with type \"npm\""
                    ));
                }
            }
        }
        if let Some(ui) = &self.ui {
            if !ui.is_object() {
                return Err(anyhow!("ui must be a JSON object"));
            }
            if self
                .ui
                .as_ref()
                .and_then(|ui| ui.get("launch"))
                .and_then(|launch| launch.get("splash"))
                .is_some()
                && self.splash_path().is_none()
            {
                return Err(anyhow!("ui.launch.splash.path must be a non-empty string"));
            }
        }
        Ok(())
    }

    fn render_with_comments(&self) -> Result<String> {
        let mut lines = vec![
            "# LingXia host app configuration",
            "#",
            "# This file is the source of truth for the host app project.",
            "# The CLI reads it and generates runtime app.json and ui.json.",
            "#",
            "# Quick tips:",
            "# - app.platforms must include at least one platform",
            "# - edit this file, not generated runtime files",
            "# - ui controls app-level windows, panels, and activators",
            "",
        ];

        let body = yaml::to_string(self).context("Failed to serialize config")?;
        lines.push(body.trim_end());
        Ok(lines.join("\n") + "\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacosUiSurfaceStyle {
    Window,
    StatusPanel,
    AttachedPanel,
}

#[derive(Debug, Clone)]
struct MacosUiSurface {
    style: MacosUiSurfaceStyle,
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

fn validate_macos_ui_config(ui: &Value) -> Result<()> {
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

        let presentation = obj
            .get("presentation")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("ui.surfaces[{index}].presentation must be an object"))?;
        let style = non_empty_str(
            presentation.get("style"),
            &format!("ui.surfaces[{index}].presentation.style"),
        )?;
        let style = match style {
            "window" => MacosUiSurfaceStyle::Window,
            "statusPanel" => MacosUiSurfaceStyle::StatusPanel,
            "attachedPanel" => MacosUiSurfaceStyle::AttachedPanel,
            "sheet" | "embedded" => {
                return Err(anyhow!(
                    "ui surface '{id}' uses unsupported macOS presentation style '{style}'"
                ));
            }
            other => {
                return Err(anyhow!(
                    "ui surface '{id}' has unknown presentation style '{other}'"
                ));
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
        if kind != "lxapp" {
            return Err(anyhow!(
                "ui surface '{id}' uses unsupported macOS content.kind '{kind}'"
            ));
        }
        let app_id = non_empty_str(
            content.get("appId"),
            &format!("ui.surfaces[{index}].content.appId"),
        )?;
        if !seen_app_ids.insert(app_id.to_string()) {
            return Err(anyhow!(
                "macOS app UI currently requires unique lxapp content.appId values; duplicate '{app_id}'"
            ));
        }

        surface_by_id.insert(
            id.to_string(),
            MacosUiSurface {
                style,
                attach_to: optional_non_empty_str(presentation.get("attachTo")),
                edge: optional_non_empty_str(presentation.get("edge")),
            },
        );
    }

    let Some(initial) = surface_by_id.get(initial_surface) else {
        return Err(anyhow!(
            "ui.launch.initialSurface references unknown surface '{initial_surface}'"
        ));
    };
    if !matches!(
        initial.style,
        MacosUiSurfaceStyle::Window
            | MacosUiSurfaceStyle::StatusPanel
            | MacosUiSurfaceStyle::AttachedPanel
    ) {
        return Err(anyhow!(
            "ui.launch.initialSurface must reference a supported macOS surface"
        ));
    }

    let root_ids = surface_by_id
        .iter()
        .filter_map(|(id, surface)| {
            matches!(
                surface.style,
                MacosUiSurfaceStyle::Window | MacosUiSurfaceStyle::StatusPanel
            )
            .then_some(id.as_str())
        })
        .collect::<Vec<_>>();
    if root_ids.len() != 1 {
        return Err(anyhow!(
            "macOS app UI currently requires exactly one root window or statusPanel surface"
        ));
    }
    let root_id = root_ids[0];

    for (id, surface) in &surface_by_id {
        match surface.style {
            MacosUiSurfaceStyle::Window | MacosUiSurfaceStyle::StatusPanel => {
                if surface.attach_to.is_some() {
                    return Err(anyhow!("root ui surface '{id}' cannot set attachTo"));
                }
            }
            MacosUiSurfaceStyle::AttachedPanel => {
                let parent_id = surface.attach_to.as_deref().ok_or_else(|| {
                    anyhow!("attachedPanel ui surface '{id}' requires presentation.attachTo")
                })?;
                let parent = surface_by_id.get(parent_id).ok_or_else(|| {
                    anyhow!("ui surface '{id}' attaches to unknown surface '{parent_id}'")
                })?;
                if !matches!(
                    parent.style,
                    MacosUiSurfaceStyle::Window | MacosUiSurfaceStyle::StatusPanel
                ) {
                    return Err(anyhow!(
                        "macOS app UI currently does not support attachedPanel -> attachedPanel; surface '{id}' attaches to '{parent_id}'"
                    ));
                }
                if parent_id != root_id {
                    return Err(anyhow!(
                        "macOS app UI currently supports panels attached only to the root surface"
                    ));
                }
                let edge = surface.edge.as_deref().ok_or_else(|| {
                    anyhow!("attachedPanel ui surface '{id}' requires presentation.edge")
                })?;
                match edge {
                    "leading" | "trailing" | "bottom" => {}
                    "top" => {
                        return Err(anyhow!(
                            "macOS app UI currently does not support attachedPanel.edge: top"
                        ));
                    }
                    other => {
                        return Err(anyhow!(
                            "attachedPanel ui surface '{id}' has unknown presentation.edge '{other}'"
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
        assert_eq!(parsed.app.unwrap().product_name, "my-app");
        assert_eq!(parsed.android.unwrap().package_id, "com.example.myapp");
        assert!(matches!(
            parsed.resources.unwrap().bundles.as_deref(),
            Some([ResourceBundleConfig::Detailed(detail)])
                if detail.bundle_type == ResourceBundleType::Lxapp
                    && detail.path == "my-app"
        ));
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
                "presentation": {
                    "style": "window"
                },
                "content": {
                    "kind": "lxapp",
                    "appId": "my-app"
                }
            }, {
                "id": "side",
                "presentation": {
                    "style": "attachedPanel",
                    "attachTo": "main",
                    "edge": "trailing"
                },
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
                "presentation": {
                    "style": "window"
                },
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
                    "presentation": {
                        "style": "window"
                    },
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
                "presentation": {
                    "style": "window"
                },
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
                "presentation": {
                    "style": "window"
                },
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
                "presentation": {
                    "style": "window"
                },
                "content": {
                    "kind": "lxapp",
                    "appId": "shared"
                }
            }, {
                "id": "panel",
                "presentation": {
                    "style": "attachedPanel",
                    "attachTo": "main",
                    "edge": "trailing"
                },
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
                "presentation": {
                    "style": "window"
                },
                "content": {
                    "kind": "lxapp",
                    "appId": "main"
                }
            }, {
                "id": "panel",
                "presentation": {
                    "style": "attachedPanel",
                    "attachTo": "main",
                    "edge": "top"
                },
                "content": {
                    "kind": "lxapp",
                    "appId": "panel"
                }
            }],
            "activators": []
        }));

        let err = config.validate().unwrap_err().to_string();
        assert!(err.contains("attachedPanel.edge: top"));
    }
}
