//! macOS platform implementation.
//!
//! Builds and runs macOS applications using Swift Package Manager.
//! Simpler than iOS - no signing or device deployment needed.

use super::apple::{self, find_workspace_root};
use super::{
    BuildArtifacts, BuildConfig, BuildProfile, Device, InstallConfig, Platform, RunConfig,
};
use crate::config::MacosConfig;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MACOS_ARM_TARGET: &str = "aarch64-apple-darwin";
const MACOS_X86_TARGET: &str = "x86_64-apple-darwin";

/// macOS resources directory relative path within Swift Package
pub const MACOS_RESOURCES_REL_PATH: &str = "Sources/lxapp/Resources";

/// macOS platform implementation
pub struct MacosPlatform;

impl MacosPlatform {
    /// Create a new macOS platform instance
    pub fn new() -> Self {
        Self
    }

    /// Get Rust target for macOS based on architecture
    fn rust_target(arch: &str) -> &'static str {
        match arch {
            "arm64" => MACOS_ARM_TARGET,
            "x86_64" => MACOS_X86_TARGET,
            _ => MACOS_ARM_TARGET, // Default to ARM
        }
    }

    /// Build Rust static library for macOS
    fn build_rust_library(
        &self,
        project_root: &Path,
        workspace_root: &Path,
        config: &BuildConfig,
        arch: &str,
    ) -> Result<PathBuf> {
        let rust_target = Self::rust_target(arch);
        let is_release = matches!(config.profile, BuildProfile::Release);
        let profile_dir = config.profile.as_str();

        if !config.build_native {
            return Ok(workspace_root
                .join("target")
                .join(rust_target)
                .join(profile_dir)
                .join("liblingxia.a"));
        }

        let lingxia_config = config
            .lingxia_config
            .as_ref()
            .ok_or_else(|| anyhow!("lingxia.config.json is required to build native libraries"))?;

        let rust_lib_name = lingxia_config
            .get_rust_lib_name()
            .ok_or_else(|| anyhow!("app.projectName is required in lingxia.config.json"))?;

        let rust_lib_dir = project_root.join(&rust_lib_name);

        apple::build_rust_staticlib(
            workspace_root,
            &rust_lib_dir,
            rust_target,
            is_release,
            &config.features,
            None, // No deployment target for macOS
        )
    }

    /// Build Swift Package for macOS
    fn swift_build_and_get_bin_dir(
        &self,
        macos_dir: &Path,
        workspace_root: &Path,
        profile: BuildProfile,
        arch: &str,
    ) -> Result<PathBuf> {
        println!("{}", "Building Swift Package for macOS...".cyan());

        let is_release = matches!(profile, BuildProfile::Release);
        let build_config = if is_release { "release" } else { "debug" };

        let mut cmd = Command::new("swift");
        cmd.current_dir(macos_dir)
            .env("LINGXIA_PROJECT_ROOT", workspace_root)
            .env("LINGXIA_BUILD_CONFIG", build_config)
            .args(["build", "--show-bin-path"]);

        // Cross-compile if target arch differs from host
        let host_arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        if arch != host_arch {
            cmd.args(["--arch", arch]);
        }

        if is_release {
            cmd.args(["-c", "release"]);
        }

        let output = cmd.output().context("Failed to execute swift build")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Swift build failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let bin_path = stdout.trim();
        if bin_path.is_empty() {
            return Err(anyhow!("swift build --show-bin-path returned empty output"));
        }

        println!("  {} Swift build complete", "✓".green());
        Ok(PathBuf::from(bin_path))
    }

    fn find_executable_in_bin_dir(
        &self,
        bin_dir: &Path,
        preferred_names: &[String],
    ) -> Result<PathBuf> {
        if !bin_dir.exists() {
            return Err(anyhow!("SwiftPM bin dir not found: {}", bin_dir.display()));
        }

        let mut executables = Vec::new();
        for entry in fs::read_dir(bin_dir)? {
            let path = entry?.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            // Skip libraries and build artifacts we might see next to the executable.
            if name.starts_with("lib")
                || name.ends_with(".a")
                || name.ends_with(".dylib")
                || name.ends_with(".o")
                || name.ends_with(".swiftmodule")
                || name.ends_with(".swiftdoc")
                || name.ends_with(".dSYM")
            {
                continue;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let Ok(meta) = path.metadata() else { continue };
                if meta.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }

            executables.push(path);
        }

        if executables.is_empty() {
            return Err(anyhow!("No executable found in {}", bin_dir.display()));
        }

        // Prefer explicitly configured names (and a few derived ones).
        for want in preferred_names {
            if want.trim().is_empty() {
                continue;
            }
            if let Some(found) = executables
                .iter()
                .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(want.as_str()))
            {
                return Ok(found.to_path_buf());
            }
        }

        if executables.len() == 1 {
            return Ok(executables.remove(0));
        }

        executables.sort();
        Ok(executables.remove(0))
    }
}

impl Platform for MacosPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        apple::ensure_macos()?;

        let macos_config = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.macos.as_ref());

        // Default to host architecture
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };

        // Resolve macOS project directory
        let macos_dir = resolve_macos_dir(&config.project_root, macos_config)?;
        let workspace_root = find_workspace_root(&config.project_root)?;

        println!(
            "{} Building macOS app from {}",
            "[macOS]".cyan(),
            macos_dir.display()
        );

        // Generate Swift bridge if needed
        if config.build_native {
            let rust_target = Self::rust_target(arch);
            apple::generate_swift_bridge(&workspace_root, rust_target)?;
        }

        // Prepare SDK resources
        apple::prepare_sdk_resources(&workspace_root, !config.build_native)?;

        // Build Rust static library
        self.build_rust_library(&config.project_root, &workspace_root, config, arch)?;
        if config.build_native {
            let rust_target = Self::rust_target(arch);
            apple::update_spm_rust_link_stamp(
                &workspace_root,
                rust_target,
                config.profile.as_str(),
            )?;
        }

        // Build Swift Package and get bin dir
        let bin_dir =
            self.swift_build_and_get_bin_dir(&macos_dir, &workspace_root, config.profile, arch)?;

        let mut preferred = Vec::new();
        if let Some(ref macos) = macos_config {
            if let Some(ref name) = macos.executable_name {
                preferred.push(name.clone());
            }
        }
        if let Some(ref cfg) = config.lingxia_config {
            if let Some(app) = cfg.app.as_ref() {
                preferred.push(app.project_name.clone());
            }
        }
        if let Some(dir_name) = macos_dir.file_name().and_then(|n| n.to_str()) {
            preferred.push(dir_name.to_string());
        }

        let executable_path = self.find_executable_in_bin_dir(&bin_dir, &preferred)?;

        Ok(BuildArtifacts::MacOs { executable_path })
    }

    fn install(&self, _config: &InstallConfig) -> Result<()> {
        // macOS apps don't need installation - they run directly
        println!(
            "{} macOS apps run directly, no installation needed",
            "ℹ".blue()
        );
        Ok(())
    }

    fn uninstall(&self, _package_id: &str, _device_id: Option<&str>) -> Result<()> {
        Err(anyhow!("Uninstall is not supported for macOS apps"))
    }

    fn run(&self, _config: &RunConfig) -> Result<()> {
        Err(anyhow!(
            "macOS apps run directly from the build output.\n\
             Use 'lingxia dev --platform macos' for the full build-and-run workflow."
        ))
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        // macOS runs on the local machine
        Ok(vec![Device {
            id: "localhost".to_string(),
            name: Some("This Mac".to_string()),
            device_type: super::DeviceType::Physical,
            online: true,
        }])
    }
}

/// Resolve the macOS Swift Package directory (same structure as iOS).
pub(crate) fn resolve_macos_dir(
    project_root: &Path,
    macos_config: Option<&MacosConfig>,
) -> Result<PathBuf> {
    // 1. Check configured path first
    if let Some(config) = macos_config {
        if let Some(ref pkg_path) = config.swift_package_path {
            let configured_dir = project_root.join(pkg_path);
            if configured_dir.join("Package.swift").exists() {
                return Ok(configured_dir);
            }
        }
    }

    // 2. Check multi-platform layout: {projectRoot}/macos/*/Package.swift
    let macos_dir = project_root.join("macos");
    if macos_dir.exists() && macos_dir.is_dir() {
        for entry in fs::read_dir(&macos_dir)? {
            let path = entry?.path();
            if path.is_dir() && path.join("Package.swift").exists() {
                return Ok(path);
            }
        }
    }

    // 3. Fallback to iOS directory (shared codebase)
    let ios_dir = project_root.join("ios");
    if ios_dir.exists() && ios_dir.is_dir() {
        for entry in fs::read_dir(&ios_dir)? {
            let path = entry?.path();
            if path.is_dir() && path.join("Package.swift").exists() {
                return Ok(path);
            }
        }
    }

    // 4. Check root directory
    if project_root.join("Package.swift").exists() {
        return Ok(project_root.to_path_buf());
    }

    Err(anyhow!(
        "macOS Swift Package not found.\n\
         Expected Package.swift in:\n\
         - {}/macos/<package>/\n\
         - {}/ios/<package>/ (shared)\n\
         - {} (root)",
        project_root.display(),
        project_root.display(),
        project_root.display()
    ))
}
