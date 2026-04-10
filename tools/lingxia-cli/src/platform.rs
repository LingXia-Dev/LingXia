use crate::config::LingXiaConfig;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod android;
pub mod android_abis;
pub mod apple;
pub mod detector;
pub mod doctor;
pub mod harmony;
pub mod ios;
pub mod macos;
pub mod spm;

/// Platform-specific build configuration
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub project_root: PathBuf,
    pub profile: BuildProfile,
    pub features: Vec<String>,
    /// Whether to build native Rust libraries
    pub build_native: bool,
    pub targets: Vec<String>,
    /// Optional project configuration
    pub lingxia_config: Option<LingXiaConfig>,
    /// Sign and package as IPA (iOS only)
    pub ipa: bool,
    /// Package host app for update delivery (macOS .app.zip)
    pub package: bool,
    /// Package macOS app bundle as DMG (macOS only)
    pub dmg: bool,
    /// Requested macOS architecture for native build (`arm64` or `x86_64`)
    pub macos_arch: Option<String>,
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
    /// Whether to reinstall app before install (best effort).
    pub reinstall: bool,
    /// Suppress progress UI for automation-friendly installs.
    pub quiet: bool,
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

    /// Uninstall an app from a device
    fn uninstall(&self, package_id: &str, device_id: Option<&str>) -> Result<()>;

    /// Run the installed app on a device
    fn run(&self, config: &RunConfig) -> Result<()>;

    /// List available devices
    fn list_devices(&self) -> Result<Vec<Device>>;
}

/// Build artifacts produced by a platform build
#[derive(Debug, Clone)]
#[allow(dead_code)] // Some variants will be used in the future
pub enum BuildArtifacts {
    Android {
        apk_path: PathBuf,
    },
    Ios {
        app_path: PathBuf,
        ipa_path: Option<PathBuf>,
    },
    MacOs {
        app_path: PathBuf,
        update_zip_path: Option<PathBuf>,
        dmg_path: Option<PathBuf>,
    },
    Harmony {
        hap_path: PathBuf,
    },
}

impl BuildArtifacts {
    /// Get the primary artifact path regardless of platform.
    ///
    /// For macOS the priority is: update zip > dmg > app bundle.
    /// This matches the publish workflow where the update zip is the preferred
    /// deliverable after `lingxia package`.
    pub fn path(&self) -> &Path {
        match self {
            BuildArtifacts::Android { apk_path } => apk_path.as_path(),
            BuildArtifacts::Ios { app_path, ipa_path } => {
                ipa_path.as_deref().unwrap_or(app_path.as_path())
            }
            BuildArtifacts::MacOs {
                app_path,
                update_zip_path,
                dmg_path,
            } => update_zip_path
                .as_deref()
                .or(dmg_path.as_deref())
                .unwrap_or(app_path.as_path()),
            BuildArtifacts::Harmony { hap_path } => hap_path.as_path(),
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
