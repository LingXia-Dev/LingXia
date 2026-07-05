use super::{
    BuildArtifacts, BuildConfig, Device, DeviceType, InstallConfig, Platform, RunConfig,
    resolve_cargo_target_dir,
};
use crate::platform::doctor::{CheckResult, command_version_line};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod env_icon;
pub mod msix;
pub mod signing;

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

/// Generate the Windows app icon as a committed `windows/AppIcon.ico` (the
/// Windows-native exe-icon format), the same way `lingxia icon` emits per-platform
/// resources elsewhere. `lingxia-windows-build::configure_windows_app` embeds it
/// at build time. Source may be SVG (rendered with resvg) or a raster PNG.
pub fn generate_icons(project_root: &Path, source_icon: &Path) -> Result<()> {
    let windows_dir = resolve_windows_dir(project_root)?;
    let out = windows_dir.join("AppIcon.ico");

    let is_svg = source_icon
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    let ico = if is_svg {
        let svg = fs::read_to_string(source_icon)
            .with_context(|| format!("Failed to read {}", source_icon.display()))?;
        crate::r#gen::icons::svg_to_ico_bytes(&svg, crate::r#gen::icons::WINDOWS_ICO_SIZES)?
    } else {
        let png = fs::read(source_icon)
            .with_context(|| format!("Failed to read {}", source_icon.display()))?;
        crate::r#gen::icons::png_to_ico_bytes(&png, crate::r#gen::icons::WINDOWS_ICO_SIZES)?
    };
    fs::write(&out, &ico).with_context(|| format!("Failed to write {}", out.display()))?;
    println!("  Generated {} ({} bytes)", out.display(), ico.len());
    Ok(())
}

impl Platform for WindowsPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        let windows_dir = resolve_windows_dir(&config.project_root)?;
        let cargo_target_dir = resolve_cargo_target_dir(&config.project_root);
        let profile_dir = config.profile.as_str();

        println!(
            "{} Building Windows app from {}",
            "[Windows]".cyan(),
            windows_dir.display()
        );

        let mut command = Command::new("cargo");
        command
            .current_dir(&windows_dir)
            .env("CARGO_TARGET_DIR", &cargo_target_dir)
            .args(["build"]);

        if matches!(config.profile, super::BuildProfile::Release) {
            command.arg("--release");
        }
        if !config.native_default_features {
            command.arg("--no-default-features");
        }
        if !config.native_features.is_empty() {
            command
                .arg("--features")
                .arg(config.native_features.join(","));
        }

        let status = command
            .status()
            .context("Failed to execute cargo build for Windows host app")?;
        if !status.success() {
            return Err(anyhow!("Windows cargo build failed"));
        }

        let exe_name = resolve_windows_executable_name(config, &windows_dir)?;
        let exe_path = cargo_target_dir
            .join(profile_dir)
            .join(executable_file_name(&exe_name));
        if !exe_path.exists() {
            return Err(anyhow!(
                "Windows executable not found after build: {}",
                exe_path.display()
            ));
        }

        Ok(BuildArtifacts::Windows { exe_path })
    }

    fn install(&self, _config: &InstallConfig) -> Result<()> {
        println!(
            "{} Windows apps run directly, no installation needed",
            "info".blue()
        );
        Ok(())
    }

    fn uninstall(&self, _package_id: &str, _device_id: Option<&str>) -> Result<()> {
        Err(anyhow!("Uninstall is not supported for Windows apps"))
    }

    fn run(&self, _config: &RunConfig) -> Result<()> {
        Err(anyhow!(
            "Windows apps run directly from the build output.\n\
             Use 'lingxia dev --platform windows' for the full build-and-run workflow."
        ))
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        Ok(vec![Device {
            id: "localhost".to_string(),
            name: Some("This Windows PC".to_string()),
            device_type: DeviceType::Physical,
            online: true,
        }])
    }
}

pub fn resolve_windows_dir(project_root: &Path) -> Result<PathBuf> {
    let windows_dir = project_root.join("windows");
    if is_windows_manifest(&windows_dir.join("Cargo.toml")) {
        return Ok(windows_dir);
    }
    if is_windows_manifest(&project_root.join("Cargo.toml")) {
        return Ok(project_root.to_path_buf());
    }
    Err(anyhow!(
        "Windows host project not found. Expected windows/Cargo.toml with a lingxia-windows-sdk dependency."
    ))
}

pub fn resolve_windows_assets_dir(project_root: &Path) -> Result<PathBuf> {
    // Generated host assets live under `windows/.lingxia/` (mirrors macOS's
    // `macos/.lingxia/`) so the `windows/` source dir stays free of build output.
    Ok(resolve_windows_dir(project_root)?
        .join(".lingxia")
        .join("assets"))
}

pub fn doctor_checks() -> Vec<CheckResult> {
    vec![check_windows_host(), check_cargo_build()]
}

fn check_windows_host() -> CheckResult {
    if cfg!(target_os = "windows") {
        CheckResult::pass("Windows host", "running on Windows")
    } else {
        CheckResult::warn(
            "Windows host",
            "not running on Windows",
            Some(
                "Build and run Windows host apps on a Windows machine with the MSVC Rust toolchain.",
            ),
        )
    }
}

fn check_cargo_build() -> CheckResult {
    match command_version_line("cargo", &["--version"], false) {
        Some(version) => CheckResult::pass("Cargo", version),
        None => CheckResult::fail(
            "Cargo",
            "cargo not found in PATH".to_string(),
            Some("Install Rust from https://rustup.rs/"),
        ),
    }
}

fn resolve_windows_executable_name(config: &BuildConfig, windows_dir: &Path) -> Result<String> {
    if let Some(name) = config
        .lingxia_config
        .as_ref()
        .and_then(|cfg| cfg.windows.as_ref())
        .and_then(|cfg| cfg.executable_name.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(name.to_string());
    }

    if let Some(name) = config
        .lingxia_config
        .as_ref()
        .and_then(|cfg| cfg.app.as_ref())
        .map(|app| app.project_name.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(name.to_string());
    }

    read_package_name(&windows_dir.join("Cargo.toml"))
        .ok_or_else(|| anyhow!("Unable to infer Windows executable name from Cargo.toml"))
}

fn executable_file_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    }
}

fn is_windows_manifest(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|content| content.contains("[package]") && content.contains("lingxia-windows-sdk"))
        .unwrap_or(false)
}

fn read_package_name(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let manifest: toml::Value = toml::from_str(&content).ok()?;
    manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
}
