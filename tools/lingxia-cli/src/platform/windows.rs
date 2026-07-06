use super::{
    BuildArtifacts, BuildConfig, Device, DeviceType, InstallConfig, Platform, RunConfig,
    resolve_cargo_target_dir, resolve_lingxia_target_dir,
};
use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
#[cfg(not(target_os = "windows"))]
use crate::platform::detector::PlatformType;
use crate::platform::doctor::{CheckResult, command_version_line};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
        ensure_supported_host()?;
        let windows_dir = resolve_windows_dir(&config.project_root)?;
        let cargo_target_dir = resolve_cargo_target_dir(&config.project_root);
        let assets_dir = resolve_windows_assets_dir(&config.project_root)?;
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
            .env("LINGXIA_WINDOWS_ASSET_DIR", &assets_dir)
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

        sync_assets_next_to_exe(&assets_dir, &exe_path)?;

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

    fn run(&self, config: &RunConfig) -> Result<()> {
        ensure_supported_host()?;
        let _ = (&config.package_id, &config.main_activity, &config.device_id);
        let project_root = current_windows_project_root()?;
        let exe_path = latest_runnable_windows_exe(&project_root)?;
        if config.restart
            && let Some(name) = exe_path.file_name().and_then(|name| name.to_str())
        {
            terminate_windows_process(name);
        }
        let mut command = Command::new(&exe_path);
        if let Some(parent) = exe_path.parent() {
            command.current_dir(parent);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to launch {}", exe_path.display()))?;
        println!("{} App launched -> {}", "✓".green(), exe_path.display());
        Ok(())
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
    let _ = resolve_windows_dir(project_root)?;
    Ok(resolve_lingxia_target_dir(project_root)
        .join("windows")
        .join("assets"))
}

pub fn resolve_windows_build_dir(project_root: &Path) -> Result<PathBuf> {
    let _ = resolve_windows_dir(project_root)?;
    Ok(resolve_lingxia_target_dir(project_root).join("windows"))
}

pub fn doctor_checks() -> Vec<CheckResult> {
    vec![host_doctor_check(), check_cargo_build()]
}

pub(crate) fn ensure_supported_host() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let version = current_windows_version().ok_or_else(|| {
            anyhow!("Unable to determine Windows version. LingXia Windows apps require Windows 10 or later.")
        })?;
        if version.is_supported() {
            return Ok(());
        }
        Err(anyhow!(
            "LingXia Windows apps require Windows 10 or later (detected Windows {}).",
            version
        ))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(crate::platform::host_support::unsupported_host(
            &PlatformType::Windows,
        ))
    }
}

pub(crate) fn host_doctor_check() -> CheckResult {
    #[cfg(target_os = "windows")]
    {
        match current_windows_version() {
            Some(version) if version.is_supported() => {
                CheckResult::pass("Host OS", format!("Windows {version}"))
            }
            Some(version) => CheckResult::fail(
                "Host OS",
                format!("Windows {version}"),
                Some("LingXia Windows apps require Windows 10 or later."),
            ),
            None => CheckResult::fail(
                "Host OS",
                "unable to determine Windows version".to_string(),
                Some("LingXia Windows apps require Windows 10 or later."),
            ),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        CheckResult::fail(
            "Host OS",
            format!(
                "Windows builds are only supported on Windows 10 or later (current: {})",
                std::env::consts::OS
            ),
            None::<String>,
        )
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
struct WindowsVersion {
    major: u32,
    minor: u32,
    build: u32,
}

#[cfg(target_os = "windows")]
impl WindowsVersion {
    fn is_supported(self) -> bool {
        self.major > 10 || (self.major == 10 && self.build >= 10240)
    }
}

#[cfg(target_os = "windows")]
impl std::fmt::Display for WindowsVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.build)
    }
}

#[cfg(target_os = "windows")]
fn current_windows_version() -> Option<WindowsVersion> {
    #[repr(C)]
    struct OsVersionInfoW {
        os_version_info_size: u32,
        major_version: u32,
        minor_version: u32,
        build_number: u32,
        platform_id: u32,
        csd_version: [u16; 128],
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version_information: *mut OsVersionInfoW) -> i32;
    }

    let mut info = OsVersionInfoW {
        os_version_info_size: std::mem::size_of::<OsVersionInfoW>() as u32,
        major_version: 0,
        minor_version: 0,
        build_number: 0,
        platform_id: 0,
        csd_version: [0; 128],
    };
    let status = unsafe { RtlGetVersion(&mut info) };
    (status == 0).then_some(WindowsVersion {
        major: info.major_version,
        minor: info.minor_version,
        build: info.build_number,
    })
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
    resolve_windows_executable_name_from_config(config.lingxia_config.as_ref(), windows_dir)
}

fn resolve_windows_executable_name_from_config(
    config: Option<&LingXiaConfig>,
    windows_dir: &Path,
) -> Result<String> {
    if let Some(name) = config
        .and_then(|cfg| cfg.windows.as_ref())
        .and_then(|cfg| cfg.executable_name.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(name.to_string());
    }

    if let Some(name) = config
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

fn sync_assets_next_to_exe(assets_src: &Path, exe_path: &Path) -> Result<()> {
    if !assets_src.is_dir() {
        return Ok(());
    }
    let exe_dir = exe_path.parent().ok_or_else(|| {
        anyhow!(
            "Windows executable path has no parent: {}",
            exe_path.display()
        )
    })?;
    let assets_dest = exe_dir.join("assets");
    if assets_dest.exists() {
        fs::remove_dir_all(&assets_dest)
            .with_context(|| format!("Failed to clear {}", assets_dest.display()))?;
    }
    crate::platform::apple::copy_dir_recursive(assets_src, &assets_dest)
        .with_context(|| format!("Failed to copy Windows assets to {}", assets_dest.display()))?;
    Ok(())
}

fn current_windows_project_root() -> Result<PathBuf> {
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    super::detector::find_host_project_root(&current_dir, HOST_CONFIG_FILE).ok_or_else(|| {
        anyhow!(
            "No {} found from {}. Run this command inside a LingXia app project.",
            HOST_CONFIG_FILE,
            current_dir.display()
        )
    })
}

pub(crate) fn record_last_build_exe(project_root: &Path, exe_path: &Path) -> Result<()> {
    let marker = last_build_marker_path(project_root)?;
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::write(&marker, exe_path.to_string_lossy().as_bytes())
        .with_context(|| format!("Failed to write {}", marker.display()))?;
    Ok(())
}

fn latest_runnable_windows_exe(project_root: &Path) -> Result<PathBuf> {
    if let Some(path) = read_last_build_exe(project_root)? {
        return Ok(path);
    }

    let windows_dir = resolve_windows_dir(project_root)?;
    let config = LingXiaConfig::load(project_root).ok();
    let exe_name = executable_file_name(&resolve_windows_executable_name_from_config(
        config.as_ref(),
        &windows_dir,
    )?);

    let mut candidates = Vec::new();
    let cargo_target_dir = resolve_cargo_target_dir(project_root);
    candidates.push((0, cargo_target_dir.join("debug").join(&exe_name)));
    candidates.push((1, cargo_target_dir.join("release").join(&exe_name)));

    if let Some(product_name) = config
        .as_ref()
        .and_then(|config| config.app.as_ref())
        .map(|app| app.product_name.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        candidates.push((
            2,
            resolve_windows_build_dir(project_root)?
                .join("dist")
                .join(product_name)
                .join(&exe_name),
        ));
    }

    candidates
        .into_iter()
        .filter(|(_, path)| path.is_file() && path_sibling_assets_app_json(path).is_file())
        .max_by_key(|(priority, path)| {
            let assets_modified = path_sibling_assets_app_json(path)
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok();
            (assets_modified, *priority)
        })
        .map(|(_, path)| path)
        .ok_or_else(|| {
            anyhow!(
                "No runnable Windows build found. Run `lingxia build --platform windows` first."
            )
        })
}

fn read_last_build_exe(project_root: &Path) -> Result<Option<PathBuf>> {
    let marker = last_build_marker_path(project_root)?;
    if !marker.is_file() {
        return Ok(None);
    }
    let value = fs::read_to_string(&marker)
        .with_context(|| format!("Failed to read {}", marker.display()))?;
    let path = PathBuf::from(value.trim());
    if path.is_file() && path_sibling_assets_app_json(&path).is_file() {
        return Ok(Some(path));
    }
    Ok(None)
}

fn last_build_marker_path(project_root: &Path) -> Result<PathBuf> {
    Ok(resolve_windows_build_dir(project_root)?.join("last-build.txt"))
}

fn path_sibling_assets_app_json(exe_path: &Path) -> PathBuf {
    exe_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("assets")
        .join("app.json")
}

fn terminate_windows_process(exe_name: &str) {
    let _ = Command::new("taskkill")
        .args(["/IM", exe_name, "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
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
