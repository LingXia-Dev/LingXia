use std::path::PathBuf;

pub(super) const DEFAULT_PACKAGE_PREFIX: &str = "app.lingxia";
pub(super) const DEFAULT_ICON_BACKGROUND_COLOR: &str = "#FFFFFF";

#[derive(Debug)]
pub(super) struct ProjectConfig {
    pub(super) name: String,
    pub(super) product_name: String,
    pub(super) project_type: ProjectType,
    pub(super) platforms: Vec<Platform>,
    pub(super) package_id: String,
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
}

impl Platform {
    pub(super) fn as_str(&self) -> &str {
        match self {
            Platform::Android => "android",
            Platform::Ios => "ios",
            Platform::Macos => "macos",
            Platform::Harmony => "harmony",
        }
    }

    pub(super) fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "android" => Some(Platform::Android),
            "ios" => Some(Platform::Ios),
            "macos" => Some(Platform::Macos),
            "harmony" | "harmonyos" => Some(Platform::Harmony),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LxAppInfo {
    pub(super) app_id: String,
}
