use crate::config::LingXiaConfig;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod android;
pub mod android_abis;
pub mod apple;
pub mod detector;
pub mod doctor;
pub mod env_badge;
pub mod harmony;
pub mod ios;
pub mod macos;
pub mod spm;
pub mod windows;

pub fn resolve_cargo_target_dir(project_root: &Path) -> PathBuf {
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return if target_dir.is_absolute() {
            target_dir
        } else {
            project_root.join(target_dir)
        };
    }

    if let Some(target_dir) = resolve_cargo_config_target_dir(project_root) {
        return target_dir;
    }

    find_workspace_root(project_root)
        .unwrap_or_else(|| project_root.to_path_buf())
        .join("target")
}

pub(crate) const NATIVE_CLIENT_OUT_ENV: &str = "LINGXIA_NATIVE_CLIENT_OUT";

pub(crate) fn native_client_out_for_host_project(
    project_root: &Path,
    config: &LingXiaConfig,
    framework_override: Option<crate::lxapp::ProjectFramework>,
) -> Result<Option<PathBuf>> {
    let Some(app) = config.app.as_ref() else {
        return Ok(None);
    };
    let Some(resources) = config.resources.as_ref() else {
        return Ok(None);
    };
    let Some(bundle) = resources
        .bundles
        .iter()
        .find(|bundle| bundle.app_id == app.home_app_id)
    else {
        return Ok(None);
    };
    let Some(path) = bundle
        .path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return Ok(None);
    };
    let lxapp_root = project_root.join(path);
    let project = crate::lxapp::Project::discover(&lxapp_root, framework_override)?;
    Ok(Some(crate::lxapp::native_client_output_path(
        &lxapp_root,
        project.framework,
    )))
}

pub(crate) fn set_native_client_codegen_env(cmd: &mut Command, out: Option<&Path>) {
    if let Some(out) = out {
        cmd.env(NATIVE_CLIENT_OUT_ENV, out);
    }
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

fn resolve_cargo_config_target_dir(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let cargo_dir = dir.join(".cargo");
        for name in ["config.toml", "config"] {
            let config_path = cargo_dir.join(name);
            let Some(target_dir) = read_cargo_config_target_dir(&config_path) else {
                continue;
            };
            return Some(if target_dir.is_absolute() {
                target_dir
            } else {
                dir.join(target_dir)
            });
        }
    }
    None
}

fn read_cargo_config_target_dir(path: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(path).ok()?;
    let value = toml::from_str::<toml::Value>(&content).ok()?;
    value
        .get("build")
        .and_then(|build| build.get("target-dir"))
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn manifest_declares_workspace(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|content| content.contains("[workspace]"))
        .unwrap_or(false)
}

/// Whether `project_root` lives inside the LingXia monorepo itself (as opposed
/// to an external user project created via `lingxia new`).
///
/// Used to decide whether the native SDK must be downloaded from a GitHub
/// release (external projects) or is already present in the source tree
/// (in-workspace `examples/*`, which reference the SDK via source paths).
///
/// We require BOTH a `[workspace]` Cargo manifest AND a sibling `lingxia-sdk/`
/// directory next to it, so an unrelated Cargo workspace that happens to
/// contain a LingXia app does not get misclassified.
pub(crate) fn is_inside_lingxia_workspace(project_root: &Path) -> bool {
    let Some(workspace_root) = find_workspace_root(project_root) else {
        return false;
    };
    workspace_root.join("lingxia-sdk").is_dir()
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
    /// Optional `--framework` override forwarded to lxapp project discovery
    /// when resolving the native client codegen output directory.
    pub framework: Option<crate::lxapp::ProjectFramework>,
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
    /// Build only the native Rust library and skip platform packaging. For
    /// harmony this stops after the `.so` (no ohpm/hvigor/.hap) — used by CI to
    /// verify the ohos cross-compile without the gated API-21 HarmonyOS SDK
    /// that `hvigor assembleHap` requires.
    pub native_only: bool,
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
    Windows {
        exe_path: PathBuf,
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
            BuildArtifacts::Windows { exe_path } => exe_path.as_path(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lxapp::ProjectFramework;
    use tempfile::TempDir;

    const AMBIGUOUS_MANIFEST: &str = r#"{
        "appId": "showcase",
        "version": "1.0.0",
        "logic": false,
        "security": {"network":{"trustedDomains":[]},"privileges":[]},
        "pages": [{"name":"home","path":"pages/home/index"}]
    }"#;

    fn write_ambiguous_lxapp_fixture(temp: &TempDir) -> LingXiaConfig {
        let lxapp_root = temp.path().join("showcase");
        fs::create_dir_all(lxapp_root.join("pages/home")).unwrap();
        fs::write(lxapp_root.join("lxapp.json"), AMBIGUOUS_MANIFEST).unwrap();
        fs::write(lxapp_root.join("pages/home/index.tsx"), "export default 0;").unwrap();
        fs::write(lxapp_root.join("pages/home/index.vue"), "<template/>").unwrap();
        LingXiaConfig::new_android("demo", "com.example.demo", "showcase")
    }

    #[test]
    fn native_client_out_errors_without_framework_on_ambiguous_pages() {
        let temp = TempDir::new().unwrap();
        let config = write_ambiguous_lxapp_fixture(&temp);

        let error = native_client_out_for_host_project(temp.path(), &config, None)
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("Pass --framework react|vue|html"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn native_client_out_uses_framework_override_for_ambiguous_pages() {
        let temp = TempDir::new().unwrap();
        let config = write_ambiguous_lxapp_fixture(&temp);

        let out =
            native_client_out_for_host_project(temp.path(), &config, Some(ProjectFramework::React))
                .unwrap()
                .expect("resources.bundles[0].path is set, so an output path must be returned");

        assert_eq!(out, temp.path().join("showcase/.lingxia/native.ts"));
    }

    #[test]
    fn is_inside_lingxia_workspace_requires_sdk_sibling() {
        // A bare workspace with no lingxia-sdk/ is an external project layout.
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        )
        .unwrap();
        let project = temp.path().join("app");
        fs::create_dir_all(&project).unwrap();
        assert!(!is_inside_lingxia_workspace(&project));

        // Adding a sibling lingxia-sdk/ dir marks it as the monorepo.
        fs::create_dir_all(temp.path().join("lingxia-sdk")).unwrap();
        assert!(is_inside_lingxia_workspace(&project));
    }

    #[test]
    fn is_inside_lingxia_workspace_false_without_workspace_manifest() {
        let temp = TempDir::new().unwrap();
        // lingxia-sdk/ present but no [workspace] Cargo.toml anywhere.
        fs::create_dir_all(temp.path().join("lingxia-sdk")).unwrap();
        let project = temp.path().join("app");
        fs::create_dir_all(&project).unwrap();
        assert!(!is_inside_lingxia_workspace(&project));
    }
}
