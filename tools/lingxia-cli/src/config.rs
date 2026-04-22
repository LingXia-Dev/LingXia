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
    pub features: Option<FeaturesConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<CapabilitiesConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<ShellConfig>,
    /// App-level UI config used to generate `ui.json` at build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<Value>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_max_age_days: Option<u64>,
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
    pub lingxia_server: Option<String>,

    #[serde(default)]
    #[serde(rename = "lingxiaId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lingxia_id: Option<String>,

    /// Platforms to build for this app (e.g. ["android"]).
    pub platforms: Vec<String>,

    #[serde(rename = "homeAppId")]
    pub home_app_id: String,
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

    pub fn shell_enabled(&self, _platform: &str) -> bool {
        self.features
            .as_ref()
            .map(|features| features.shell)
            .unwrap_or(false)
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
            features.push("js-lxapp".to_string());
        }
        if self.shell_enabled(platform) {
            features.push("shell".to_string());
            features.push("webview-input".to_string());
        }
        if self.devtools_enabled() {
            features.push("devtools".to_string());
        }
        features
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

    /// Create a default Android config
    #[allow(dead_code)] // Used in tests
    pub fn new_android(project_name: &str, package_id: &str, home_app_id: &str) -> Self {
        Self {
            app: Some(HostAppConfig {
                project_name: project_name.to_string(),
                product_name: project_name.to_string(),
                product_version: "0.0.1".to_string(),
                lingxia_server: None,
                lingxia_id: None,
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
            features: Some(FeaturesConfig::default()),
            capabilities: Some(CapabilitiesConfig::default()),
            shell: None,
            ui: None,
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
            if app.home_app_id.trim().is_empty() {
                return Err(anyhow!("app.homeAppId must not be empty"));
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
        if let Some(features) = &self.features
            && features.shell
            && !features.app_service
        {
            return Err(anyhow!(
                "features.shell requires features.appService because shell uses AppService-backed lxapps"
            ));
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacosUiSurfaceStyle {
    Window,
    StatusPanel,
    AttachPanel,
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
            "attachPanel" => MacosUiSurfaceStyle::AttachPanel,
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
            | MacosUiSurfaceStyle::AttachPanel
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
            MacosUiSurfaceStyle::AttachPanel => {
                let parent_id = surface.attach_to.as_deref().ok_or_else(|| {
                    anyhow!("attachPanel ui surface '{id}' requires presentation.attachTo")
                })?;
                let parent = surface_by_id.get(parent_id).ok_or_else(|| {
                    anyhow!("ui surface '{id}' attaches to unknown surface '{parent_id}'")
                })?;
                if !matches!(
                    parent.style,
                    MacosUiSurfaceStyle::Window | MacosUiSurfaceStyle::StatusPanel
                ) {
                    return Err(anyhow!(
                        "macOS app UI currently does not support attachPanel -> attachPanel; surface '{id}' attaches to '{parent_id}'"
                    ));
                }
                if parent_id != root_id {
                    return Err(anyhow!(
                        "macOS app UI currently supports panels attached only to the root surface"
                    ));
                }
                let edge = surface.edge.as_deref().ok_or_else(|| {
                    anyhow!("attachPanel ui surface '{id}' requires presentation.edge")
                })?;
                match edge {
                    "leading" | "trailing" | "bottom" => {}
                    "top" => {
                        return Err(anyhow!(
                            "macOS app UI currently does not support attachPanel.edge: top"
                        ));
                    }
                    other => {
                        return Err(anyhow!(
                            "attachPanel ui surface '{id}' has unknown presentation.edge '{other}'"
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
        let app = parsed.app.unwrap();
        assert_eq!(app.product_name, "my-app");
        assert_eq!(app.home_app_id, "my-app");
        assert_eq!(parsed.android.unwrap().package_id, "com.example.myapp");
        let resources = parsed.resources.unwrap();
        assert_eq!(resources.bundles[0].app_id, "my-app");
        assert_eq!(resources.bundles[0].path.as_deref(), Some("my-app"));
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
                    "style": "attachPanel",
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
                    "style": "attachPanel",
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
                    "style": "attachPanel",
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
        assert!(err.contains("attachPanel.edge: top"));
    }
}
