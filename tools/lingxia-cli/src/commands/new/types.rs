use std::path::PathBuf;

pub(super) const DEFAULT_PACKAGE_PREFIX: &str = "app.lingxia";
pub(super) const DEFAULT_ICON_BACKGROUND_COLOR: &str = "#FFFFFF";

/// Default logical publish id for a host app: `lingxia.app.<name>`.
/// Distinct from the OS package id (`app.lingxia.<name>`); this one identifies
/// the app to the LingXia server for `lingxia publish`.
pub(super) fn default_lingxia_id(project_name: &str) -> String {
    format!("lingxia.app.{}", project_name.to_lowercase())
}

/// Default lxapp `appId`: `lingxia.lxapp.<name>`. Namespaced so ids don't
/// collide across projects on a shared server. Decoupled from the lxapp's
/// on-disk directory name.
pub(super) fn default_lxapp_app_id(project_name: &str) -> String {
    format!("lingxia.lxapp.{}", project_name.to_lowercase())
}

/// `appId` for an lxapp embedded in a host app: `lingxia.lxapp.<host>.<lxapp>`.
///
/// The host segment keeps ids from colliding across projects on a shared
/// server; the lxapp segment keeps them from colliding *within* one host, which
/// naming after the project alone did not -- every lxapp a host embeds took the
/// host's own id. The two collapse when an lxapp carries the project's name,
/// since repeating it identifies nothing.
pub(super) fn embedded_lxapp_app_id(project_name: &str, lxapp_name: &str) -> String {
    let host = project_name.to_lowercase();
    let lxapp = lxapp_name.to_lowercase();
    if lxapp.is_empty() || lxapp == host {
        return format!("lingxia.lxapp.{host}");
    }
    format!("lingxia.lxapp.{host}.{lxapp}")
}

#[cfg(test)]
mod app_id_tests {
    use super::{default_lxapp_app_id, embedded_lxapp_app_id};

    #[test]
    fn an_embedded_lxapp_is_named_after_the_host_and_itself() {
        assert_eq!(
            embedded_lxapp_app_id("fusheng", "home"),
            "lingxia.lxapp.fusheng.home"
        );
    }

    #[test]
    fn two_lxapps_in_one_host_get_distinct_ids() {
        let home = embedded_lxapp_app_id("fusheng", "home");
        let settings = embedded_lxapp_app_id("fusheng", "settings");
        assert_ne!(
            home, settings,
            "a host embedding two lxapps must not give them one id"
        );
    }

    #[test]
    fn the_same_lxapp_name_in_two_hosts_stays_distinct() {
        assert_ne!(
            embedded_lxapp_app_id("fusheng", "home"),
            embedded_lxapp_app_id("showcase", "home")
        );
    }

    #[test]
    fn an_lxapp_carrying_the_project_name_does_not_repeat_it() {
        assert_eq!(
            embedded_lxapp_app_id("fusheng", "fusheng"),
            "lingxia.lxapp.fusheng"
        );
    }

    #[test]
    fn a_standalone_lxapp_is_still_named_after_its_project() {
        assert_eq!(default_lxapp_app_id("Fusheng"), "lingxia.lxapp.fusheng");
    }
}

#[derive(Debug)]
pub(super) struct ProjectConfig {
    pub(super) name: String,
    pub(super) product_name: String,
    pub(super) project_type: ProjectType,
    pub(super) platforms: Vec<Platform>,
    pub(super) package_id: String,
    pub(super) app_link_hosts: Vec<String>,
    pub(super) target_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ProjectType {
    NativeApp,
    LxApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MainSurface {
    LxApp,
    Terminal,
    Browser,
}

impl MainSurface {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::LxApp => "lxapp",
            Self::Terminal => "terminal",
            Self::Browser => "browser",
        }
    }

    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "lxapp" => Some(Self::LxApp),
            "terminal" => Some(Self::Terminal),
            "browser" => Some(Self::Browser),
            _ => None,
        }
    }

    pub(super) fn is_native(self) -> bool {
        !matches!(self, Self::LxApp)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlMode {
    LxApp,
    Native,
}

impl ControlMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::LxApp => "lxapp",
            Self::Native => "native",
        }
    }

    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "lxapp" => Some(Self::LxApp),
            "native" => Some(Self::Native),
            _ => None,
        }
    }
}

impl ProjectType {
    pub(super) fn as_str(&self) -> &str {
        match self {
            ProjectType::NativeApp => "native-app",
            ProjectType::LxApp => "lxapp",
        }
    }

    pub(super) fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "native-app" | "native" => Some(ProjectType::NativeApp),
            "lxapp" | "miniapp" => Some(ProjectType::LxApp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Platform {
    Android,
    Ios,
    Macos,
    Harmony,
    Windows,
}

impl Platform {
    pub(super) fn as_str(&self) -> &str {
        match self {
            Platform::Android => "android",
            Platform::Ios => "ios",
            Platform::Macos => "macos",
            Platform::Harmony => "harmony",
            Platform::Windows => "windows",
        }
    }

    pub(super) fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "android" => Some(Platform::Android),
            "ios" => Some(Platform::Ios),
            "macos" => Some(Platform::Macos),
            "harmony" | "harmonyos" => Some(Platform::Harmony),
            "windows" | "win" => Some(Platform::Windows),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LxAppInfo {
    /// The namespaced logical id (e.g. `lingxia.lxapp.demo`). Becomes the host's
    /// `homeAppId`, the bundle `appId`, and the surface `id`.
    pub(super) app_id: String,
    /// The lxapp's on-disk directory name (e.g. `lxapp`). Becomes the bundle
    /// `path`. Kept separate so the id can be namespaced without dotting a dir.
    pub(super) dir_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppServiceMode {
    Enabled,
    Disabled,
}

impl AppServiceMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    pub(super) fn enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::{AppServiceMode, ControlMode, MainSurface};

    #[test]
    fn default_logic_mode_keeps_appservice_enabled() {
        assert!(AppServiceMode::Enabled.enabled());
    }

    #[test]
    fn app_service_labels_are_clear() {
        assert_eq!(AppServiceMode::Enabled.label(), "enabled");
        assert_eq!(AppServiceMode::Disabled.label(), "disabled");
    }

    #[test]
    fn scaffold_surface_options_are_strict() {
        assert_eq!(
            MainSurface::from_str("terminal"),
            Some(MainSurface::Terminal)
        );
        assert_eq!(MainSurface::from_str("Browser"), Some(MainSurface::Browser));
        assert_eq!(MainSurface::from_str("url"), None);
        assert_eq!(ControlMode::from_str("native"), Some(ControlMode::Native));
        assert_eq!(ControlMode::from_str("remote"), None);
    }
}
