use super::{BuildArtifacts, BuildConfig, Device, DeviceType, InstallConfig, Platform, RunConfig};
use adb_client::{server::ADBServer, ADBDeviceExt};
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Android platform implementation
pub struct AndroidPlatform;

impl AndroidPlatform {
    /// Create a new Android platform instance
    pub fn new() -> Self {
        Self
    }

    /// Detect Android NDK path from environment
    fn detect_ndk_path() -> Result<PathBuf> {
        // 1. Check ANDROID_NDK_HOME environment variable
        if let Ok(ndk_home) = env::var("ANDROID_NDK_HOME") {
            let path = PathBuf::from(ndk_home);
            if path.exists() {
                return Ok(path);
            }
        }

        // 2. Check ANDROID_HOME/ndk/*
        if let Ok(android_home) = env::var("ANDROID_HOME") {
            let ndk_dir = PathBuf::from(android_home).join("ndk");
            if ndk_dir.exists() {
                // Find the latest NDK version
                if let Ok(entries) = std::fs::read_dir(&ndk_dir) {
                    let mut versions: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .collect();
                    versions.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
                    if let Some(latest) = versions.first() {
                        return Ok(latest.path());
                    }
                }
            }
        }

        Err(anyhow!(
            "Android NDK not found. Please set ANDROID_NDK_HOME environment variable"
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
            println!("  {} Skipping native compilation", "⏭️".bold());
            return Ok(());
        }

        println!("{}", "📦 Building Rust libraries...".bold());

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

        println!("  {} Rust build complete", "✓".green());
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
        println!("  → Building for {}...", target.cyan());

        config
            .lingxia_config
            .as_ref()
            .ok_or_else(|| anyhow!("lingxia.config.json is required to build native libraries"))?;
        let rust_lib_name = project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app");
        let rust_lib_dir = project_root.join(format!("{}-lib", rust_lib_name));
        let rust_manifest = rust_lib_dir.join("Cargo.toml");
        if !rust_manifest.exists() {
            return Err(anyhow!(
                "Rust library manifest not found: {}",
                rust_manifest.display()
            ));
        }

        // Get API level from config or default to 33
        let api_level = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.android.as_ref())
            .map(|a| a.get_api_level())
            .unwrap_or(33);

        let (cmake_proc, cc_bin, cxx_bin) = match target {
            "aarch64-linux-android" => (
                "aarch64",
                format!("aarch64-linux-android{}-clang", api_level),
                format!("aarch64-linux-android{}-clang++", api_level),
            ),
            "armv7-linux-androideabi" => (
                "armv7-a",
                format!("armv7a-linux-androideabi{}-clang", api_level),
                format!("armv7a-linux-androideabi{}-clang++", api_level),
            ),
            _ => return Err(anyhow!("Unsupported Android target: {}", target)),
        };

        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--target")
            .arg(target)
            .arg("--manifest-path")
            .arg(&rust_manifest)
            .current_dir(&rust_lib_dir);

        // Add --release flag for release builds (debug is the default)
        if matches!(config.profile, super::BuildProfile::Release) {
            cmd.arg("--release");
        }

        // Add features if specified
        if !config.features.is_empty() {
            cmd.arg("--features").arg(config.features.join(","));
        }

        // Set Android NDK environment variables
        cmd.env("ANDROID_NDK_HOME", ndk_path);
        cmd.env("ANDROID_NDK_ROOT", ndk_path);
        cmd.env("ANDROID_NDK", ndk_path);

        // CMake configuration
        cmd.env(
            "CMAKE_CONFIGURE_ARGS",
            format!(
                "-DCMAKE_TOOLCHAIN_FILE={}/build/cmake/android.toolchain.cmake -DCMAKE_SYSTEM_PROCESSOR={}",
                ndk_path.display(),
                cmake_proc
            ),
        );

        // Clear macOS SDK pollution
        cmd.env_remove("SDKROOT");
        cmd.env_remove("CMAKE_OSX_SYSROOT");
        cmd.env_remove("CMAKE_OSX_ARCHITECTURES");
        cmd.env_remove("MACOSX_DEPLOYMENT_TARGET");
        cmd.env_remove("CMAKE_TOOLCHAIN_FILE");

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

        let status = cmd.status().context("Failed to execute cargo build")?;

        if !status.success() {
            return Err(anyhow!("Rust build failed for target: {}", target));
        }

        Ok(())
    }

    /// Build Gradle project
    fn build_gradle(&self, project_root: &Path, config: &BuildConfig) -> Result<PathBuf> {
        println!("{}", "🔨 Building Gradle project...".bold());

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

        println!("  {} APK: {}", "✓".green(), apk_path.display());
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
        println!();
        println!("{}", "🏗️  Building Android project...".bold().cyan());
        println!();

        // Resolve Android project directory (handle multi-platform layout)
        let android_root = super::detector::resolve_android_dir(&config.project_root);
        println!(
            "  Android directory: {}",
            android_root.display().to_string().cyan()
        );

        // Build Rust libraries
        self.build_rust_library(&config.project_root, config)?;

        // Build Gradle project
        let apk_path = self.build_gradle(&android_root, config)?;

        println!();
        println!("{}", "✅ Build complete!".green().bold());

        Ok(BuildArtifacts::Android { apk_path })
    }

    fn install(&self, config: &InstallConfig) -> Result<()> {
        println!();
        println!("{}", "📲 Installing APK...".bold().cyan());
        println!();

        // Resolve Android project directory
        let android_root = super::detector::resolve_android_dir(&config.project_root);

        // Determine APK path: use provided path or auto-detect
        let apk_path = if let Some(ref path) = config.artifact_path {
            path.clone()
        } else {
            // Auto-detect APK from build output
            self.auto_detect_apk(&android_root)?
        };

        if !apk_path.exists() {
            return Err(anyhow!("APK not found at: {}", apk_path.display()));
        }

        // Create ADB server connection
        let mut server = ADBServer::default();

        // Get device
        let mut device = if let Some(ref device_id) = config.device_id {
            // Get specific device by name
            server
                .get_device_by_name(device_id)
                .context(format!("Failed to get device: {}", device_id))?
        } else {
            // Get the only connected device (errors if 0 or >1 devices)
            server.get_device().context(
                "Failed to get device. Use --device to specify a device if multiple are connected",
            )?
        };

        let device_id = device.identifier.as_deref().unwrap_or("unknown");
        println!("  Installing on device: {}", device_id.cyan());

        // Install APK using adb_client
        device.install(&apk_path).context("Failed to install APK")?;

        println!("  {} APK installed successfully", "✓".green());

        println!();
        println!("{}", "✅ Installation complete!".green().bold());

        Ok(())
    }

    fn run(&self, config: &RunConfig) -> Result<()> {
        println!();
        println!("{}", "🚀 Starting app...".bold().cyan());

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

        println!("  Starting activity: {}", activity.cyan());

        // Start activity using shell_command
        let start_cmd = format!("am start -n {}", activity);
        let mut output = Vec::new();
        device
            .shell_command(&start_cmd, &mut output)
            .context("Failed to start activity")?;

        // Print shell output if available
        if !output.is_empty() {
            let output_str = String::from_utf8_lossy(&output);
            for line in output_str.lines() {
                println!("  {}", line);
            }
        }

        println!("{}", "✅ App started!".green().bold());

        Ok(())
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        // Create ADB server connection
        let mut server = ADBServer::default();

        // Get devices from adb server
        let adb_devices = server
            .devices()
            .context("Failed to get devices from ADB server")?;

        // Convert to our Device type
        let devices = adb_devices
            .into_iter()
            .map(|d| Device {
                id: d.identifier.clone(),
                name: None,
                device_type: if d.identifier.contains("emulator") {
                    DeviceType::Emulator
                } else {
                    DeviceType::Physical
                },
                online: true,
            })
            .collect();

        Ok(devices)
    }

    fn name(&self) -> &str {
        "android"
    }
}
