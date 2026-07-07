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
    use super::AppServiceMode;

    #[test]
    fn default_logic_mode_keeps_appservice_enabled() {
        assert!(AppServiceMode::Enabled.enabled());
    }

    #[test]
    fn app_service_labels_are_clear() {
        assert_eq!(AppServiceMode::Enabled.label(), "enabled");
        assert_eq!(AppServiceMode::Disabled.label(), "disabled");
    }
}
