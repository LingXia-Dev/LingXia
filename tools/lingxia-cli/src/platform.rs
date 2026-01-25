use crate::config::LingXiaConfig;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod android;
pub mod detector;

/// Platform-specific build configuration
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub project_root: PathBuf,
    pub profile: BuildProfile,
    pub features: Vec<String>,
    pub skip_native: bool,
    pub targets: Vec<String>,
    /// Optional project configuration
    pub lingxia_config: Option<LingXiaConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    pub fn as_str(&self) -> &str {
        match self {
            BuildProfile::Debug => "debug",
            BuildProfile::Release => "release",
        }
    }
}

/// Platform-specific install configuration
#[derive(Debug, Clone)]
pub struct InstallConfig {
    pub project_root: PathBuf,
    /// Optional artifact path (auto-detected if None)
    pub artifact_path: Option<PathBuf>,
    pub device_id: Option<String>,
}

/// Platform-specific run configuration
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub package_id: String,
    pub main_activity: Option<String>,
    pub device_id: Option<String>,
}

/// Platform trait for build, install, and run operations
pub trait Platform: Send + Sync {
    /// Build the project
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts>;

    /// Install the built artifacts to a device
    fn install(&self, config: &InstallConfig) -> Result<()>;

    /// Run the installed app on a device
    fn run(&self, config: &RunConfig) -> Result<()>;

    /// List available devices
    #[allow(dead_code)]
    fn list_devices(&self) -> Result<Vec<Device>>;

    /// Platform name
    #[allow(dead_code)]
    fn name(&self) -> &str;
}

/// Build artifacts produced by a platform build
#[derive(Debug, Clone)]
#[allow(dead_code)] // iOS and Harmony variants will be used in the future
pub enum BuildArtifacts {
    Android { apk_path: PathBuf },
    Ios { app_path: PathBuf },
    Harmony { hap_path: PathBuf },
}

impl BuildArtifacts {
    /// Get the artifact path regardless of platform
    pub fn path(&self) -> &Path {
        match self {
            BuildArtifacts::Android { apk_path } => apk_path.as_path(),
            BuildArtifacts::Ios { app_path } => app_path.as_path(),
            BuildArtifacts::Harmony { hap_path } => hap_path.as_path(),
        }
    }

    /// Get platform name
    pub fn platform_name(&self) -> &str {
        match self {
            BuildArtifacts::Android { .. } => "Android",
            BuildArtifacts::Ios { .. } => "iOS",
            BuildArtifacts::Harmony { .. } => "HarmonyOS",
        }
    }
}

/// Device information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Device {
    pub id: String,
    pub name: Option<String>,
    pub device_type: DeviceType,
    pub online: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum DeviceType {
    Physical,
    Emulator,
    Simulator,
}

impl Device {
    #[allow(dead_code)]
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.name {
            format!("{} ({})", name, self.id)
        } else {
            self.id.clone()
        }
    }
}
