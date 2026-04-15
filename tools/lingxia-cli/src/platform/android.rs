use super::{
    BuildArtifacts, BuildConfig, Device, DeviceType, InstallConfig, Platform, RunConfig,
    resolve_cargo_target_dir,
};
use crate::commands::rust::run_cargo_build_for_target;
use adb_client::{ADBDeviceExt, server::ADBServer};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

mod doctor;
pub use doctor::doctor_checks;

/// Android platform implementation
pub struct AndroidPlatform;

fn format_install_progress(uploaded: u64, total: u64) -> String {
    let uploaded_mb = uploaded as f64 / 1024.0 / 1024.0;
    let total_mb = total as f64 / 1024.0 / 1024.0;
    let percent = if total > 0 {
        (uploaded as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    format!(
        "Installing {:.1}% ({:.1}/{:.1} MB)...",
        percent, uploaded_mb, total_mb
    )
}

impl AndroidPlatform {
    fn unsupported_target_error(target: &str) -> anyhow::Error {
        anyhow!(
            "Unsupported Android target: {}.\n\
Supported Rust target triples:\n\
  - aarch64-linux-android\n\
  - armv7-linux-androideabi",
            target
        )
    }

    /// Normalize Android target aliases to Rust target triples.
    fn normalize_target(target: &str) -> Option<&'static str> {
        match target {
            "aarch64-linux-android" | "arm64-v8a" => Some("aarch64-linux-android"),
            "armv7-linux-androideabi" | "armv7a-linux-androideabi" | "armeabi-v7a" => {
                Some("armv7-linux-androideabi")
            }
            _ => None,
        }
    }

    /// Create a new Android platform instance
    pub fn new() -> Self {
        Self
    }

    /// Detect Android NDK path from environment
    fn detect_ndk_path() -> Result<PathBuf> {
        if let Ok(value) = env::var("ANDROID_NDK_ROOT") {
            let path = PathBuf::from(&value);
            if path.exists() {
                return Ok(path);
            }
            return Err(anyhow!(
                "ANDROID_NDK_ROOT is set to '{}' but path does not exist",
                value
            ));
        }

        Err(anyhow!(
            "Android NDK not found. Set ANDROID_NDK_ROOT (for example: $ANDROID_SDK_ROOT/ndk/28.2.13676358)"
        ))
    }

    /// Get NDK host platform string (darwin-x86_64, linux-x86_64, windows-x86_64)
    fn get_ndk_host_platform() -> Result<&'static str> {
        #[cfg(target_os = "macos")]
        return Ok("darwin-x86_64");

        #[cfg(target_os = "linux")]
        return Ok("linux-x86_64");

        #[cfg(target_os = "windows")]
        return Ok("windows-x86_64");

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Err(anyhow!("Unsupported host platform for Android NDK"));
    }

    /// Build Rust library for Android
    fn build_rust_library(&self, project_root: &Path, config: &BuildConfig) -> Result<()> {
        if !config.build_native {
            return Ok(());
        }

        println!("{}", "Compiling native code...".cyan());

        let ndk_path = Self::detect_ndk_path()?;
        let host_platform = Self::get_ndk_host_platform()?;
        let toolchain_base = ndk_path.join(format!("toolchains/llvm/prebuilt/{}", host_platform));

        if !toolchain_base.exists() {
            return Err(anyhow!(
                "NDK toolchain not found at: {}",
                toolchain_base.display()
            ));
        }

        // Build for each target
        for target in &config.targets {
            self.build_rust_target(project_root, config, &ndk_path, &toolchain_base, target)?;
        }

        Ok(())
    }

    /// Build Rust for a specific target
    fn build_rust_target(
        &self,
        project_root: &Path,
        config: &BuildConfig,
        ndk_path: &Path,
        toolchain_base: &Path,
        target: &str,
    ) -> Result<()> {
        let normalized_target =
            Self::normalize_target(target).ok_or_else(|| Self::unsupported_target_error(target))?;
        let target = normalized_target;

        let lingxia_config = config
            .lingxia_config
            .as_ref()
            .ok_or_else(|| anyhow!("lingxia.yaml is required to build native libraries"))?;

        let rust_lib_name = lingxia_config
            .get_rust_lib_name()
            .ok_or_else(|| anyhow!("app.projectName is required in lingxia.yaml"))?;
        let rust_lib_dir = project_root.join(&rust_lib_name);
        let rust_manifest = rust_lib_dir.join("Cargo.toml");
        if !rust_manifest.exists() {
            return Err(anyhow!(
                "Rust library manifest not found: {}",
                rust_manifest.display()
            ));
        }

        // Get API level from config or default to 33 for arm64, 21 for armv7
        let default_api_level = if target == "armv7-linux-androideabi" {
            21
        } else {
            33
        };
        let api_level = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.android.as_ref())
            .map(|a| a.get_api_level())
            .unwrap_or(default_api_level);

        let (cc_bin, cxx_bin) = match target {
            "aarch64-linux-android" => (
                format!("aarch64-linux-android{}-clang", api_level),
                format!("aarch64-linux-android{}-clang++", api_level),
            ),
            "armv7-linux-androideabi" => (
                format!("armv7a-linux-androideabi{}-clang", api_level),
                format!("armv7a-linux-androideabi{}-clang++", api_level),
            ),
            _ => return Err(Self::unsupported_target_error(target)),
        };

        let target_dir = resolve_cargo_target_dir(project_root);
        run_cargo_build_for_target(
            &rust_manifest,
            &rust_lib_dir,
            &target_dir,
            target,
            None,
            config.profile,
            |cmd| {
                // Set Android NDK environment variables
                cmd.env("ANDROID_NDK_ROOT", ndk_path);
                cmd.env("ANDROID_API_LEVEL", api_level.to_string());

                // Clear macOS SDK pollution
                cmd.env_remove("SDKROOT");
                cmd.env_remove("MACOSX_DEPLOYMENT_TARGET");

                // Set target-specific toolchain
                let bin_dir = toolchain_base.join("bin");
                let ar_path = bin_dir.join("llvm-ar");
                let cc_path = bin_dir.join(&cc_bin);
                let cxx_path = bin_dir.join(&cxx_bin);

                let target_upper = target.to_uppercase().replace('-', "_");
                let target_env = target.replace('-', "_");
                cmd.env(format!("AR_{}", target_env), &ar_path);
                cmd.env(format!("CARGO_TARGET_{}_LINKER", target_upper), &cc_path);
                cmd.env(format!("CC_{}", target_env), &cc_path);
                cmd.env(format!("CXX_{}", target_env), &cxx_path);

                // Old Android (API < 23) requires DT_HASH, not just DT_GNU_HASH
                if target == "armv7-linux-androideabi" {
                    cmd.env(
                        format!("CARGO_TARGET_{}_RUSTFLAGS", target_upper),
                        "-C link-arg=-Wl,--hash-style=both",
                    );
                }
            },
        )?;

        // Copy .so file to jniLibs directory
        let profile_dir = if matches!(config.profile, super::BuildProfile::Release) {
            "release"
        } else {
            "debug"
        };
        let so_path = target_dir
            .join(target)
            .join(profile_dir)
            .join("liblingxia.so");

        if so_path.exists() {
            let abi = match target {
                "aarch64-linux-android" => "arm64-v8a",
                "armv7-linux-androideabi" => "armeabi-v7a",
                _ => return Err(anyhow!("Unknown ABI for target: {}", target)),
            };
            let android_root = super::detector::resolve_android_dir(project_root);
            let jni_dir = android_root.join(format!("app/src/main/jniLibs/{}", abi));
            std::fs::create_dir_all(&jni_dir)?;
            let dest = jni_dir.join("liblingxia.so");
            std::fs::copy(&so_path, &dest)?;
        }

        Ok(())
    }

    /// Build Gradle project
    fn build_gradle(&self, project_root: &Path, config: &BuildConfig) -> Result<PathBuf> {
        println!("{}", "Building APK...".cyan());

        let gradlew = if cfg!(windows) {
            project_root.join("gradlew.bat")
        } else {
            project_root.join("gradlew")
        };

        if !gradlew.exists() {
            return Err(anyhow!(
                "Gradle wrapper not found at: {}",
                gradlew.display()
            ));
        }

        let task = match config.profile {
            super::BuildProfile::Debug => "assembleDebug",
            super::BuildProfile::Release => "assembleRelease",
        };

        let status = Command::new(&gradlew)
            .arg(task)
            .current_dir(project_root)
            .status()
            .context("Failed to execute gradlew")?;

        if !status.success() {
            return Err(anyhow!("Gradle build failed"));
        }

        // Find the built APK
        let profile_name = config.profile.as_str();
        let apk_path = project_root
            .join("app")
            .join("build")
            .join("outputs")
            .join("apk")
            .join(profile_name)
            .join(format!("app-{}.apk", profile_name));

        if !apk_path.exists() {
            return Err(anyhow!("APK not found at: {}", apk_path.display()));
        }

        Ok(apk_path)
    }

    /// Auto-detect APK path from build output
    fn auto_detect_apk(&self, android_root: &Path) -> Result<PathBuf> {
        let debug_apk = android_root.join("app/build/outputs/apk/debug/app-debug.apk");
        let release_apk = android_root.join("app/build/outputs/apk/release/app-release.apk");

        if release_apk.exists() {
            Ok(release_apk)
        } else if debug_apk.exists() {
            Ok(debug_apk)
        } else {
            Err(anyhow!(
                "No APK found. Build the project first with 'lingxia build'"
            ))
        }
    }
}

impl Platform for AndroidPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        // Resolve Android project directory (handle multi-platform layout)
        let android_root = super::detector::resolve_android_dir(&config.project_root);

        // Build Rust libraries
        self.build_rust_library(&config.project_root, config)?;

        // Build Gradle project
        let apk_path = self.build_gradle(&android_root, config)?;

        Ok(BuildArtifacts::Android { apk_path })
    }

    fn install(&self, config: &InstallConfig) -> Result<()> {
        // Resolve Android project directory
        let android_root = super::detector::resolve_android_dir(&config.project_root);

        // Determine APK path: use provided path or auto-detect
        let apk_path = if let Some(ref path) = config.artifact_path {
            path.clone()
        } else {
            self.auto_detect_apk(&android_root)?
        };

        if !apk_path.exists() {
            return Err(anyhow!("APK not found at: {}", apk_path.display()));
        }

        // Get APK file size
        let file_size = std::fs::metadata(&apk_path).map(|m| m.len()).unwrap_or(0);
        let size_mb = file_size as f64 / 1024.0 / 1024.0;

        // Create ADB server connection
        let mut server = ADBServer::default();

        // Get device
        let mut device = if let Some(ref device_id) = config.device_id {
            server
                .get_device_by_name(device_id)
                .context(format!("Failed to get device: {}", device_id))?
        } else {
            server.get_device().context(
                "Failed to get device. Use --device to specify a device if multiple are connected",
            )?
        };

        if config.reinstall {
            let spinner = (!config.quiet).then(|| {
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.cyan} {msg}")
                        .unwrap()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
                );
                spinner.enable_steady_tick(std::time::Duration::from_millis(80));
                spinner
            });
            let package_id = infer_android_package_id_for_uninstall(&config.project_root);
            if let Some(package_id) = package_id {
                if let Some(spinner) = spinner.as_ref() {
                    spinner.set_message(format!("Uninstalling {package_id}..."));
                }
                if let Err(err) = device.uninstall(&package_id, None) {
                    eprintln!(
                        "{} failed to uninstall {} before install: {}",
                        "Warning:".yellow(),
                        package_id,
                        err
                    );
                }
            } else {
                eprintln!(
                    "{} could not resolve Android package id for --reinstall; continuing install",
                    "Warning:".yellow()
                );
            }
            if let Some(spinner) = spinner {
                spinner.finish_and_clear();
            }
        }

        // Install APK
        let install_bar = (!config.quiet).then(|| {
            let bar = ProgressBar::new(file_size.max(1));
            bar.set_style(
                ProgressStyle::with_template(
                    "{spinner:.cyan} [{bar:30.cyan/blue}] {percent:>3}% {msg}",
                )
                .unwrap()
                .progress_chars("=>-"),
            );
            bar.set_message(format!("Installing ({:.1} MB)...", size_mb));
            bar.enable_steady_tick(std::time::Duration::from_millis(80));
            bar
        });
        let progress_bar = install_bar.clone();
        let mut on_progress = move |uploaded: u64, total: u64| {
            if let Some(progress_bar) = progress_bar.as_ref() {
                let bounded_total = total.max(1);
                progress_bar.set_length(bounded_total);
                progress_bar.set_position(uploaded.min(bounded_total));
                progress_bar.set_message(format_install_progress(uploaded, total));
            }
        };
        let result = device.install_with_progress(&apk_path, None, Some(&mut on_progress));

        if let Some(install_bar) = install_bar {
            install_bar.finish_and_clear();
        }

        match result {
            Ok(_) => {
                println!("{}", "✓ Installed".green());
                Ok(())
            }
            Err(e) => Err(anyhow!("Installation failed: {}", e)),
        }
    }

    fn uninstall(&self, package_id: &str, device_id: Option<&str>) -> Result<()> {
        let mut server = ADBServer::default();

        let mut device = if let Some(id) = device_id {
            server
                .get_device_by_name(id)
                .context(format!("Failed to get device: {}", id))?
        } else {
            server.get_device().context(
                "Failed to get device. Use --device to specify a device if multiple are connected",
            )?
        };

        device
            .uninstall(package_id, None)
            .context(format!("Failed to uninstall {}", package_id))?;

        Ok(())
    }

    fn run(&self, config: &RunConfig) -> Result<()> {
        // Create ADB server connection
        let mut server = ADBServer::default();

        // Get device
        let mut device = if let Some(ref device_id) = config.device_id {
            server
                .get_device_by_name(device_id)
                .context(format!("Failed to get device: {}", device_id))?
        } else {
            server.get_device().context(
                "Failed to get device. Use --device to specify a device if multiple are connected",
            )?
        };

        let activity = if let Some(ref activity) = config.main_activity {
            format!("{}/{}", config.package_id, activity)
        } else {
            format!("{}/{}.MainActivity", config.package_id, config.package_id)
        };

        // Start activity using shell_command
        let start_cmd = format!("am start -n {}", activity);
        let mut output = Vec::new();
        device
            .shell_command(&start_cmd, Some(&mut output), None)
            .context("Failed to start activity")?;

        println!("{}", "✓ App launched".green());

        Ok(())
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        let mut server = ADBServer::default();

        let adb_devices = server
            .devices_long()
            .context("Failed to get devices from ADB server")?;

        let devices = adb_devices
            .into_iter()
            .map(|d| {
                let name = if d.model.is_empty() {
                    None
                } else {
                    Some(d.model.replace('_', " "))
                };
                Device {
                    id: d.identifier.clone(),
                    name,
                    device_type: if d.identifier.contains("emulator") {
                        DeviceType::Emulator
                    } else {
                        DeviceType::Physical
                    },
                    online: true,
                }
            })
            .collect();

        Ok(devices)
    }
}

fn infer_android_package_id_for_uninstall(project_root: &Path) -> Option<String> {
    crate::config::LingXiaConfig::load(project_root)
        .ok()
        .and_then(|c| c.android.map(|a| a.package_id))
}

/// Generate Android app icons
///
/// # Arguments
/// * `project_root` - Project root directory
/// * `source_icon` - Path to source icon image
/// * `background_color` - Hex color for adaptive icon background (e.g., "#FFFFFF")
/// * `legacy` - Whether to generate legacy icons for minSdk < 26
pub fn generate_icons(
    project_root: &Path,
    source_icon: &Path,
    background_color: &str,
    legacy: bool,
) -> Result<()> {
    let android_res = resolve_android_assets_dir(project_root);

    if !android_res.exists() {
        anyhow::bail!(
            "Android res directory not found: {}. Make sure you're in an Android project.",
            android_res.display()
        );
    }

    crate::appicon::generate_android_icons(source_icon, &android_res, background_color, legacy)
}

/// Resolve Android assets/res directory
fn resolve_android_assets_dir(project_root: &Path) -> PathBuf {
    project_root.join("android/app/src/main/res")
}
