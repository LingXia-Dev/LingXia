use crate::config::LingXiaConfig;
use anyhow::Result;
use std::fs;
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

pub fn resolve_cargo_target_dir(project_root: &Path) -> PathBuf {
    find_workspace_root(project_root)
        .unwrap_or_else(|| project_root.to_path_buf())
        .join("target")
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let manifest = dir.join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        if manifest_declares_workspace(&manifest) {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn manifest_declares_workspace(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|content| content.contains("[workspace]"))
        .unwrap_or(false)
}

/// Platform-specific build configuration
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub project_root: PathBuf,
    pub profile: BuildProfile,
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
    /// Extra Rust features enabled for native app builds.
    pub native_features: Vec<String>,
    /// Build native crate with Cargo default features enabled.
    pub native_default_features: bool,
    /// Resolved environment-version context. Platform builders apply
    /// `package_id_suffix` to package/bundle IDs for side-by-side installs
    /// without source-tree mutation.
    pub resolved_env: crate::config::ResolvedEnv,
    /// Set by the multi-phase `commands::build` orchestrator only for
    /// platforms that report `Platform::hoists_native_build() == true`,
    /// signalling that Phase 1's `build_rust_library` already produced the
    /// native artifacts. Platforms that opt into Phase 1 MUST honor this
    /// flag inside `build` and skip the inline native build + stamp update
    /// so cargo is not invoked twice. Platforms that don't opt in never
    /// see this set to true and need not check it.
    pub skip_native_build: bool,
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
    /// Restart the app by terminating an existing instance before launch.
    pub restart: bool,
}

/// Platform trait for build, install, and run operations
pub trait Platform: Send + Sync {
    /// Build the project
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts>;

    /// Phase 1 entry point: build only the native Rust library (and any
    /// linker stamps tied to it) before the lxapp asset build. Lets cargo's
    /// `build.rs` checks (e.g. cloud-types drift) fail fast before the
    /// slower JS bundle build runs.
    ///
    /// To opt into Phase 1 a platform overrides this AND returns `true`
    /// from `hoists_native_build`. The orchestrator then sets
    /// `BuildConfig::skip_native_build` for `build`, and the platform's
    /// `build` impl must honor that flag.
    ///
    /// Default is a no-op for platforms (e.g. Harmony) whose native build
    /// is structurally coupled to per-env staging.
    fn build_rust_library(&self, _config: &BuildConfig) -> Result<()> {
        Ok(())
    }

    /// Returns `true` iff a successful `build_rust_library` produces the
    /// same native artifacts that `build` would otherwise produce inline.
    /// When `true`, the multi-phase orchestrator runs `build_rust_library`
    /// in Phase 1 and sets `BuildConfig::skip_native_build` so `build` does
    /// not redo the work in Phase 3. Override to `true` alongside any
    /// non-default `build_rust_library` impl.
    fn hoists_native_build(&self) -> bool {
        false
    }

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
