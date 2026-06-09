use super::{
    BuildArtifacts, BuildConfig, Device, DeviceType, InstallConfig, Platform, RunConfig,
    resolve_cargo_target_dir,
};
use crate::config::{LingXiaConfig, ResolvedEnv};
use crate::platform::doctor::{CheckResult, command_version_line};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for WindowsPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        let windows_dir = resolve_windows_dir(&config.project_root)?;
        let cargo_target_dir = resolve_cargo_target_dir(&config.project_root);
        let profile_dir = config.profile.as_str();
        let runtime_env = config
            .lingxia_config
            .as_ref()
            .map(|cfg| windows_runtime_env(&config.project_root, cfg, &config.resolved_env))
            .transpose()?
            .unwrap_or_default();

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

        for (key, value) in &runtime_env {
            command.env(key, value);
        }

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
        "Windows host project not found. Expected windows/Cargo.toml with a lingxia-windows dependency."
    ))
}

pub fn resolve_windows_assets_dir(project_root: &Path) -> Result<PathBuf> {
    Ok(resolve_windows_dir(project_root)?.join("assets"))
}

pub fn windows_runtime_env(
    project_root: &Path,
    config: &LingXiaConfig,
    resolved_env: &ResolvedEnv,
) -> Result<Vec<(String, String)>> {
    Ok(vec![
        (
            "LINGXIA_ASSET_DIR".to_string(),
            resolve_windows_assets_dir(project_root)?
                .to_string_lossy()
                .to_string(),
        ),
        (
            "LINGXIA_APP_ID".to_string(),
            resolve_windows_app_id(config, resolved_env),
        ),
        (
            "LINGXIA_PRODUCT_NAME".to_string(),
            resolve_windows_product_name(config),
        ),
    ])
}

pub fn resolve_windows_app_id(config: &LingXiaConfig, resolved_env: &ResolvedEnv) -> String {
    let base = config
        .windows
        .as_ref()
        .and_then(|cfg| cfg.app_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| config.android.as_ref().map(|cfg| cfg.package_id.as_str()))
        .or_else(|| config.ios.as_ref().map(|cfg| cfg.bundle_id.as_str()))
        .or_else(|| config.app.as_ref().map(|app| app.home_app_id.as_str()))
        .unwrap_or("app.lingxia.windows");

    match resolved_env.effective_package_id_suffix() {
        Some(suffix) => format!("{base}{suffix}"),
        None => base.to_string(),
    }
}

pub fn resolve_windows_product_name(config: &LingXiaConfig) -> String {
    config
        .app
        .as_ref()
        .map(|app| app.product_name.clone())
        .unwrap_or_else(|| "LingXia".to_string())
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
        .map(|content| content.contains("[package]") && content.contains("lingxia-windows"))
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
