use semver::Version;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();
const APP_STATE_DIR: &str = "app_state";

#[derive(Debug, Error)]
pub enum AppContextError {
    #[error("invalid app.json: {0}")]
    InvalidJson(String),
    #[error("invalid app config: {0}")]
    InvalidConfig(String),
}

/// Build-time environment version baked into `app.json`.
///
/// Wire-compatible with `lingxia_update::ReleaseType` — both serialize as
/// lowercase `"developer" | "preview" | "release"`. Defined locally here
/// (rather than imported) to keep `lingxia-app-context` free of additional
/// crate dependencies; the JSON contract is what callers rely on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EnvVersion {
    #[default]
    Release,
    Preview,
    Developer,
}

impl EnvVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Preview => "preview",
            Self::Developer => "developer",
        }
    }
}

impl std::fmt::Display for EnvVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Opaque sRGB color used by the host theme wire format.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeColor(u32);

impl ThemeColor {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.len() != 7 || !value.starts_with('#') {
            return Err("theme colors must use opaque #RRGGBB syntax".to_string());
        }
        let rgb = u32::from_str_radix(&value[1..], 16)
            .map_err(|_| "theme colors must use opaque #RRGGBB syntax".to_string())?;
        Ok(Self(rgb))
    }

    pub const fn rgb(self) -> u32 {
        self.0
    }
}

impl std::fmt::Debug for ThemeColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ThemeColor(#{:06X})", self.0)
    }
}

impl std::fmt::Display for ThemeColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:06X}", self.0)
    }
}

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_background_color: Option<ThemeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_background_color: Option<ThemeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<ThemeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted_foreground_color: Option<ThemeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<ThemeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator_color: Option<ThemeColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_background_color: Option<ThemeColor>,
}

impl ThemeStyle {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ThemeConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<ThemeStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark: Option<ThemeStyle>,
}

impl ThemeConfig {
    pub fn normalized(mut self) -> Option<Self> {
        self.light = self.light.filter(|style| !style.is_empty());
        self.dark = self.dark.filter(|style| !style.is_empty());
        (self.light.is_some() || self.dark.is_some()).then_some(self)
    }

    pub fn style(&self, dark: bool) -> Option<&ThemeStyle> {
        if dark {
            self.dark.as_ref()
        } else {
            self.light.as_ref()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(rename = "productName")]
    pub product_name: String,
    #[serde(rename = "productVersion")]
    pub product_version: String,

    #[serde(rename = "lingxiaId", default)]
    pub lingxia_id: Option<String>,

    #[serde(rename = "lingxiaServer", default)]
    pub lingxia_server: Option<String>,

    /// The environment this build was produced for. Defaults to [`EnvVersion::Release`]
    /// when missing, matching pre-envVersion app.json artifacts.
    #[serde(rename = "envVersion", default)]
    pub env_version: EnvVersion,

    #[serde(
        rename = "homeAppId",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub home_app_id: String,

    #[serde(
        rename = "homeAppVersion",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub home_app_version: String,

    #[serde(rename = "cacheMaxSizeMB", default = "default_cache_max_size_mb")]
    pub cache_max_size_mb: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub splash: Option<SplashConfig>,

    #[serde(rename = "devWsUrl", default, skip_serializing_if = "Option::is_none")]
    pub dev_ws_url: Option<String>,

    #[serde(
        rename = "devBundleBaseUrl",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub dev_bundle_base_url: Option<String>,

    #[serde(rename = "appLinks", default, skip_serializing_if = "Option::is_none")]
    pub app_links: Option<AppLinksConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemeConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<CapabilitiesConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panels: Option<PanelsConfig>,
}

/// The `capabilities:` section, shared verbatim between the CLI (parsing
/// `lingxia.yaml`, writing `app.json`) and the runtime (reading `app.json`) —
/// one definition so a capability can never exist on one side only.
/// `deny_unknown_fields` gives lingxia.yaml typo errors; the runtime always
/// reads an app.json generated by the same CLI build, so it never sees fields
/// this struct lacks.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilitiesConfig {
    #[serde(default)]
    pub notifications: bool,
    /// The product in-app browser, with its newtab / settings / downloads pages
    /// and browser shell runtime. Opt-in and cross-platform.
    #[serde(default)]
    pub browser: bool,
    #[serde(default)]
    pub terminal: bool,
    /// Opt-in HTTP proxy for the in-app browser (desktop). Requires browser.
    #[serde(default)]
    pub proxy: bool,
    /// Allows the trusted home lxapp to launch and manage OS processes. The
    /// lxapp must also declare the `process` security privilege.
    #[serde(default)]
    pub process: bool,
    /// Unlocks `lx.app.autostart` (launch at system startup). macOS/Windows
    /// only; enabling is always a runtime user decision, never automatic.
    #[serde(default)]
    pub autostart: bool,
    /// Lets a command line or agent skill on the same machine drive this
    /// product's own windows, and unlocks the product's command line. Desktop
    /// only. The local socket it needs is derived, not declared: which IPC
    /// carries this is plumbing, and a capability list says what a product can
    /// do.
    #[serde(default)]
    pub app_use: bool,
    /// Extends that to the whole machine: screenshots of any window, synthetic
    /// input, the accessibility tree. Named for what the user is granting,
    /// because they will be asked — macOS prompts for Accessibility and Screen
    /// Recording, and the entry they see in System Settings is this product.
    #[serde(default)]
    pub computer_use: bool,
    /// Extends it to the in-app browser. Requires `browser`.
    #[serde(default)]
    pub browser_use: bool,
}

impl CapabilitiesConfig {
    /// Whether anything needs the local control socket. Derived rather than
    /// declared: no product should have to know the transport's name to say
    /// what it wants.
    pub fn needs_control_socket(&self) -> bool {
        self.app_use_effective() || self.browser_use
    }

    /// Whether this product's own windows may be driven.
    ///
    /// `computerUse` implies it. Not for symmetry — because it already
    /// contains it: an agent that may screenshot any window and post input to
    /// any window can reach this product's through the wider door. Requiring
    /// both would add no protection and one failure mode, where a product
    /// declares `computerUse`, forgets `appUse`, and `myapp computer
    /// screenshot` works while `myapp screenshot` is refused.
    ///
    /// `browserUse` does not imply it: driving browser tabs reaches no native
    /// window, and "open pages, don't touch my chrome" is a real choice.
    pub fn app_use_effective(&self) -> bool {
        self.app_use || self.computer_use
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AppLinksConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
}

/// Runtime half of `splash:`. Images and colors are platform resources; only
/// the minimum hold time is a runtime decision, and the upper bound is a
/// framework constant that hosts deliberately cannot configure.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SplashConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_duration: Option<u32>,
}

/// Default minimum hold, in milliseconds. Long enough that a fast first render
/// does not flash the cover, short enough not to feel like a delay.
pub const DEFAULT_SPLASH_MIN_DURATION_MS: u32 = 600;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageConfig {
    #[serde(rename = "tempMaxSizeMB")]
    #[serde(default = "default_temp_max_size_mb")]
    pub temp_max_size_mb: u64,
    #[serde(rename = "cacheMaxSizeMB")]
    #[serde(default = "default_cache_max_size_mb")]
    pub cache_max_size_mb: u64,
    #[serde(rename = "dataMaxSizeMB")]
    #[serde(default = "default_data_max_size_mb")]
    pub data_max_size_mb: u64,
    #[serde(rename = "appStorageMaxSizeMB")]
    #[serde(default = "default_app_storage_max_size_mb")]
    pub app_storage_max_size_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PanelsConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<PanelItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelPosition {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PanelItem {
    pub id: String,
    pub label: String,
    pub icon: String,
    #[serde(default = "default_panel_position")]
    pub position: PanelPosition,
    pub content: PanelContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PanelContentKind {
    #[default]
    LxApp,
    Terminal,
}

impl PanelContentKind {
    pub fn is_lxapp(self) -> bool {
        self == PanelContentKind::LxApp
    }
}

fn is_lxapp_panel_content_kind(kind: &PanelContentKind) -> bool {
    kind.is_lxapp()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PanelContent {
    #[serde(default, skip_serializing_if = "is_lxapp_panel_content_kind")]
    pub kind: PanelContentKind,
    #[serde(rename = "appId")]
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

fn default_cache_max_size_mb() -> u64 {
    2048
}

fn default_temp_max_size_mb() -> u64 {
    1024
}

fn default_data_max_size_mb() -> u64 {
    4096
}

fn default_app_storage_max_size_mb() -> u64 {
    16384
}

fn default_panel_position() -> PanelPosition {
    PanelPosition::Right
}

impl AppConfig {
    pub fn parse_and_validate(content: &str) -> Result<Self, AppContextError> {
        let mut config: Self = serde_json::from_str(content).map_err(|e| {
            AppContextError::InvalidJson(format!("Failed to parse app.json: {}", e))
        })?;
        config.theme = config.theme.take().and_then(ThemeConfig::normalized);
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), AppContextError> {
        if self.product_name.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "productName is mandatory and cannot be empty".to_string(),
            ));
        }
        if self.product_version.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "productVersion is mandatory and cannot be empty".to_string(),
            ));
        }
        Version::parse(&self.product_version).map_err(|_| {
            AppContextError::InvalidConfig(
                "productVersion must be a semantic version (major.minor.patch)".to_string(),
            )
        })?;
        if self.home_app_id.is_empty() != self.home_app_version.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "homeAppId and homeAppVersion must either both be set or both be omitted"
                    .to_string(),
            ));
        }
        if !self.home_app_version.is_empty() {
            Version::parse(&self.home_app_version).map_err(|_| {
                AppContextError::InvalidConfig(
                    "homeAppVersion must be a semantic version (major.minor.patch)".to_string(),
                )
            })?;
        }
        validate_panels(self.panels.as_ref())
    }
}

pub fn set_app_config(config: AppConfig) -> Result<(), AppContextError> {
    if let Some(existing) = APP_CONFIG.get() {
        if existing == &config {
            return Ok(());
        }
        return Err(AppContextError::InvalidConfig(
            "app config is already initialized with different values".to_string(),
        ));
    }

    APP_CONFIG
        .set(config)
        .map_err(|_| {
            AppContextError::InvalidConfig(
                "app config was initialized concurrently with different values".to_string(),
            )
        })
        .map(|_| ())
}

pub fn app_config() -> Option<&'static AppConfig> {
    APP_CONFIG.get()
}

pub fn theme() -> Option<&'static ThemeConfig> {
    APP_CONFIG.get().and_then(|config| config.theme.as_ref())
}

/// Wall-clock origin for cold-start timing. First touched while the runtime
/// loads `app.json`, which is early enough to stand in for process start.
static STARTUP: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

/// Start the cold-start clock. Idempotent; call as early as possible.
pub fn mark_startup() {
    let _ = *STARTUP;
}

pub fn since_startup() -> std::time::Duration {
    STARTUP.elapsed()
}

/// A host's per-launch override of the minimum hold (`u32::MAX` = unset).
/// Bounded by the platforms' 6s dismissal timeout, which stays absolute.
static SPLASH_MIN_DURATION_OVERRIDE: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(u32::MAX);
const SPLASH_HOLD_CAP_MS: u32 = 6_000;

/// Override the configured minimum hold for this launch, from the host's
/// splash selection hook.
pub fn set_splash_min_duration_override(ms: u32) {
    SPLASH_MIN_DURATION_OVERRIDE.store(
        ms.min(SPLASH_HOLD_CAP_MS),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// How long the splash must stay up before a ready signal may dismiss it.
pub fn splash_min_duration() -> std::time::Duration {
    let overridden = SPLASH_MIN_DURATION_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed);
    if overridden != u32::MAX {
        return std::time::Duration::from_millis(u64::from(overridden));
    }
    let ms = APP_CONFIG
        .get()
        .and_then(|config| config.splash.as_ref())
        .and_then(|splash| splash.min_duration)
        .unwrap_or(DEFAULT_SPLASH_MIN_DURATION_MS);
    std::time::Duration::from_millis(u64::from(ms))
}

pub fn product_name() -> Option<&'static str> {
    APP_CONFIG.get().map(|c| c.product_name.as_str())
}

pub fn home_app_id() -> Option<&'static str> {
    APP_CONFIG
        .get()
        .map(|c| c.home_app_id.as_str())
        .filter(|value| !value.is_empty())
}

pub fn home_app_version() -> Option<&'static str> {
    APP_CONFIG
        .get()
        .map(|c| c.home_app_version.as_str())
        .filter(|value| !value.is_empty())
}

pub fn product_version() -> Option<&'static str> {
    APP_CONFIG.get().map(|c| c.product_version.as_str())
}

pub fn lingxia_id() -> Option<&'static str> {
    APP_CONFIG
        .get()
        .and_then(|c| c.lingxia_id.as_deref())
        .filter(|s| !s.is_empty())
}

/// Active environment version baked into the running build. Defaults to
/// [`EnvVersion::Release`] before [`set_app_config`] is called and for any
/// `app.json` produced before the envVersion field existed.
pub fn env_version() -> EnvVersion {
    APP_CONFIG.get().map(|c| c.env_version).unwrap_or_default()
}

pub fn notifications_enabled() -> bool {
    APP_CONFIG
        .get()
        .and_then(|c| c.capabilities.as_ref())
        .map(|capabilities| capabilities.notifications)
        .unwrap_or(false)
}

pub fn browser_enabled() -> bool {
    APP_CONFIG
        .get()
        .and_then(|config| config.capabilities.as_ref())
        .map(|capabilities| capabilities.browser)
        .unwrap_or(false)
}

pub fn autostart_enabled() -> bool {
    APP_CONFIG
        .get()
        .and_then(|c| c.capabilities.as_ref())
        .map(|capabilities| capabilities.autostart)
        .unwrap_or(false)
}

pub fn terminal_enabled() -> bool {
    APP_CONFIG
        .get()
        .and_then(|c| c.capabilities.as_ref())
        .map(|capabilities| capabilities.terminal)
        .unwrap_or(false)
}

/// What the host *binary* was compiled with, recorded once at boot.
///
/// A capability is available only when the build carries it and the app
/// declares it in `lingxia.yaml`; the declaration accessors above answer the
/// second half. Defaults to all-false so a host that never records its build
/// (tests, tools) reports nothing rather than over-promising.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HostBuild {
    pub browser: bool,
    pub terminal: bool,
    pub proxy: bool,
}

static HOST_BUILD: OnceLock<HostBuild> = OnceLock::new();

/// Records the host build's capabilities. Idempotent; the first call wins.
pub fn set_host_build(build: HostBuild) {
    let _ = HOST_BUILD.set(build);
}

pub fn host_build() -> HostBuild {
    HOST_BUILD.get().copied().unwrap_or_default()
}

/// The one place each host capability is decided. `lx.supports()`, the FFI
/// capability bitmask, and the optional `lx.*` members all read these, so they
/// cannot drift apart.
pub mod capability {
    /// What the binary carries, independent of what an app declared. The
    /// native SDKs' capability bitmask reports this.
    pub mod build {
        pub fn browser() -> bool {
            super::super::host_build().browser
        }

        pub fn terminal() -> bool {
            super::super::host_build().terminal
        }

        pub fn proxy() -> bool {
            super::super::host_build().proxy
        }

        /// Notifications are a platform fact rather than a build feature.
        pub fn notifications() -> bool {
            cfg!(any(target_os = "ios", target_env = "ohos"))
        }
    }

    /// Managed browser tabs and the browser shell.
    pub fn browser() -> bool {
        build::browser() && super::browser_enabled()
    }

    /// Host notifications.
    pub fn notifications() -> bool {
        build::notifications() && super::notifications_enabled()
    }

    /// The terminal product surface and `lx.terminal`.
    pub fn terminal() -> bool {
        build::terminal() && super::terminal_enabled()
    }

    /// Browser proxy configuration.
    pub fn proxy() -> bool {
        build::proxy() && super::browser_enabled()
    }
}

pub fn process_enabled() -> bool {
    APP_CONFIG
        .get()
        .and_then(|c| c.capabilities.as_ref())
        .map(|capabilities| capabilities.process)
        .unwrap_or(false)
}

pub fn temp_max_size_bytes() -> u64 {
    const MIB: u64 = 1024 * 1024;
    APP_CONFIG
        .get()
        .and_then(|c| c.storage.as_ref().map(|storage| storage.temp_max_size_mb))
        .unwrap_or_else(default_temp_max_size_mb)
        .saturating_mul(MIB)
}

pub fn cache_max_size_bytes() -> u64 {
    const MIB: u64 = 1024 * 1024;
    APP_CONFIG
        .get()
        .map(|c| {
            c.storage
                .as_ref()
                .map(|storage| storage.cache_max_size_mb)
                .unwrap_or(c.cache_max_size_mb)
        })
        .unwrap_or_else(default_cache_max_size_mb)
        .saturating_mul(MIB)
}

pub fn data_max_size_bytes() -> u64 {
    const MIB: u64 = 1024 * 1024;
    APP_CONFIG
        .get()
        .and_then(|c| c.storage.as_ref().map(|storage| storage.data_max_size_mb))
        .unwrap_or_else(default_data_max_size_mb)
        .saturating_mul(MIB)
}

pub fn app_storage_max_size_bytes() -> u64 {
    const MIB: u64 = 1024 * 1024;
    APP_CONFIG
        .get()
        .and_then(|c| {
            c.storage
                .as_ref()
                .map(|storage| storage.app_storage_max_size_mb)
        })
        .unwrap_or_else(default_app_storage_max_size_mb)
        .saturating_mul(MIB)
}

pub fn app_state_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(APP_STATE_DIR)
}

pub fn app_state_file(app_data_dir: &Path, name: &str) -> PathBuf {
    app_state_dir(app_data_dir).join(name)
}

fn validate_panels(panels: Option<&PanelsConfig>) -> Result<(), AppContextError> {
    let Some(panels) = panels else {
        return Ok(());
    };

    let mut ids = HashSet::new();
    let mut positions = HashSet::new();
    let mut app_ids = HashSet::new();

    for item in &panels.items {
        if item.id.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "panels.items[].id cannot be empty".to_string(),
            ));
        }
        if item.label.is_empty() {
            return Err(AppContextError::InvalidConfig(format!(
                "panel '{}' label cannot be empty",
                item.id
            )));
        }
        if item.content.kind == PanelContentKind::LxApp && item.content.app_id.is_empty() {
            return Err(AppContextError::InvalidConfig(format!(
                "panel '{}' content.appId cannot be empty",
                item.id
            )));
        }
        if !ids.insert(item.id.clone()) {
            return Err(AppContextError::InvalidConfig(format!(
                "duplicate panel id '{}'",
                item.id
            )));
        }
        if !positions.insert(item.position) {
            return Err(AppContextError::InvalidConfig(format!(
                "only one panel is supported at position '{}'",
                panel_position_name(item.position)
            )));
        }
        if item.content.kind == PanelContentKind::LxApp
            && !app_ids.insert(item.content.app_id.clone())
        {
            return Err(AppContextError::InvalidConfig(format!(
                "panel appId '{}' must be unique",
                item.content.app_id
            )));
        }
    }

    Ok(())
}

fn panel_position_name(position: PanelPosition) -> &'static str {
    match position {
        PanelPosition::Left => "left",
        PanelPosition::Right => "right",
        PanelPosition::Top => "top",
        PanelPosition::Bottom => "bottom",
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, AppContextError, ThemeColor, ThemeConfig, set_app_config};

    fn test_config(product_name: &str) -> AppConfig {
        AppConfig {
            product_name: product_name.to_string(),
            product_version: "1.0.0".to_string(),
            lingxia_id: Some("lingxia".to_string()),
            lingxia_server: None,
            env_version: super::EnvVersion::Release,
            home_app_id: "home".to_string(),
            home_app_version: "1.0.0".to_string(),
            cache_max_size_mb: 1024,
            storage: None,
            splash: None,
            dev_ws_url: None,
            dev_bundle_base_url: None,
            app_links: None,
            theme: None,
            capabilities: None,
            panels: None,
        }
    }

    #[test]
    fn set_app_config_rejects_mismatched_value_after_initialization() {
        let cfg = test_config("LingXia");
        assert!(set_app_config(cfg.clone()).is_ok());
        assert!(set_app_config(cfg).is_ok());
        let err = set_app_config(test_config("Other")).unwrap_err();
        assert!(matches!(err, AppContextError::InvalidConfig(_)));
    }

    #[test]
    fn host_without_home_lxapp_is_valid() {
        let mut config = test_config("Web Host");
        config.home_app_id.clear();
        config.home_app_version.clear();

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("homeAppId"));
        assert!(!json.contains("homeAppVersion"));
        assert!(AppConfig::parse_and_validate(&json).is_ok());
    }

    #[test]
    fn home_lxapp_identity_must_be_complete() {
        let mut config = test_config("Broken Host");
        config.home_app_version.clear();

        let error = config.validate().unwrap_err();
        assert!(matches!(error, AppContextError::InvalidConfig(_)));
    }

    #[test]
    fn theme_colors_validate_and_serialize_canonically() {
        let config = AppConfig::parse_and_validate(
            r##"{
                "productName": "Theme Test",
                "productVersion": "1.0.0",
                "theme": {
                    "light": { "accentColor": "#a1b2c3" },
                    "dark": { "separatorColor": "#343840" }
                }
            }"##,
        )
        .expect("valid theme");

        let light = config
            .theme
            .as_ref()
            .and_then(|theme| theme.light.as_ref())
            .expect("light style");
        assert_eq!(light.accent_color.map(ThemeColor::rgb), Some(0xA1B2C3));

        let json = serde_json::to_string(&config).expect("serialize app config");
        assert!(json.contains("#A1B2C3"));
    }

    #[test]
    fn theme_rejects_alpha_and_unknown_fields() {
        for theme in [
            r##"{ "light": { "accentColor": "#80A1B2C3" } }"##,
            r##"{ "light": { "sidebarBackgroundColor": "#A1B2C3" } }"##,
            r##"{ "highContrast": { "accentColor": "#A1B2C3" } }"##,
        ] {
            let json = format!(
                r#"{{ "productName": "Theme Test", "productVersion": "1.0.0", "theme": {theme} }}"#
            );
            assert!(AppConfig::parse_and_validate(&json).is_err(), "{theme}");
        }
    }

    #[test]
    fn empty_theme_blocks_normalize_to_absence() {
        let theme: ThemeConfig =
            serde_json::from_str(r#"{ "light": {}, "dark": {} }"#).expect("parse empty theme");
        assert!(theme.normalized().is_none());
    }
}
