use super::{
    BuildArtifacts, BuildConfig, Device, DeviceType, InstallConfig, Platform, RunConfig,
    resolve_cargo_target_dir,
};
use crate::commands::rust::run_cargo_build_for_target;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod doctor;
pub use doctor::doctor_checks;

/// Android platform implementation
pub struct AndroidPlatform;

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
                if !config.native_default_features {
                    cmd.arg("--no-default-features");
                }
                if !config.native_features.is_empty() {
                    cmd.arg("--features").arg(config.native_features.join(","));
                }

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
    fn build_gradle(
        &self,
        project_root: &Path,
        config: &BuildConfig,
        icon_overlay: Option<&LauncherIconOverlay>,
    ) -> Result<PathBuf> {
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

        // Inject env-version overrides via Gradle project properties. Android
        // projects are expected to consume these in app/build.gradle(.kts).
        // Empty suffix props are still passed so the build is deterministic.
        let app_id_suffix = config
            .resolved_env
            .effective_package_id_suffix()
            .unwrap_or("");
        let app_name = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .map(|app| app.product_name.clone())
            .unwrap_or_default();
        let app_id_arg = format!("-Plingxia.applicationIdSuffix={app_id_suffix}");
        let app_name_arg = format!("-Plingxia.appName={app_name}");

        let mut command = Command::new(&gradlew);
        command
            .arg(task)
            .arg(app_id_arg)
            .arg(app_name_arg)
            .current_dir(project_root);
        if let Some(overlay) = icon_overlay {
            command.arg(format!(
                "-Plingxia.resOverlayDir={}",
                overlay.res_overlay_dir.to_string_lossy()
            ));
            // Manifest placeholders need both icon and roundIcon resolved.
            // For projects without a round icon, fall back to the standard
            // icon so the placeholder still resolves to something valid.
            let round_resource = if overlay.has_round_icon {
                format!("{}_round", overlay.icon_resource_name)
            } else {
                overlay.icon_resource_name.clone()
            };
            command.arg(format!(
                "-Plingxia.appIcon=@mipmap/{}",
                overlay.icon_resource_name
            ));
            command.arg(format!("-Plingxia.appRoundIcon=@mipmap/{round_resource}"));
        }
        let status = command.status().context("Failed to execute gradlew")?;

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
        let app_link_hosts = config
            .lingxia_config
            .as_ref()
            .and_then(|config| config.app_links.as_ref())
            .map(|app_links| app_links.hosts.as_slice())
            .unwrap_or(&[]);
        if sync_android_app_links(&android_root, app_link_hosts)? {
            println!(
                "{} Synced Android AppLinks to AndroidManifest.xml",
                "[Android]".cyan()
            );
        }

        // Build Rust libraries
        self.build_rust_library(&config.project_root, config)?;

        // Stage env-version overlay resources outside the source tree and let
        // Gradle merge them via sourceSets.main.res.srcDirs. No source-tree
        // mutation, no Drop-based rollback to fail on SIGKILL.
        let icon_overlay = prepare_launcher_icon_overlay(&android_root, config)?;

        // Build Gradle project
        let apk_path = self.build_gradle(&android_root, config, icon_overlay.as_ref())?;

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

        let device_id = resolve_adb_device_id(config.device_id.as_deref())?;

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
                if let Err(err) = adb_uninstall_package(Some(device_id.as_str()), &package_id) {
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

        if !config.quiet {
            println!("Installing ({:.1} MB) with adb...", size_mb);
        }
        install_with_adb(Some(device_id.as_str()), &apk_path, file_size, config.quiet)?;
        println!("{} APK → {}", "✓ Installed".green(), apk_path.display());
        Ok(())
    }

    fn uninstall(&self, package_id: &str, device_id: Option<&str>) -> Result<()> {
        let device_id = resolve_adb_device_id(device_id)?;
        adb_uninstall_package(Some(device_id.as_str()), package_id)?;

        Ok(())
    }

    fn run(&self, config: &RunConfig) -> Result<()> {
        let activity = config
            .main_activity
            .as_ref()
            .map(|activity| format!("{}/{}", config.package_id, activity))
            .unwrap_or_else(|| format!("{}/{}.MainActivity", config.package_id, config.package_id));

        let device_id = resolve_adb_device_id(config.device_id.as_deref())?;
        let args = if config.restart {
            vec!["am", "start", "-S", "-n", activity.as_str()]
        } else {
            vec!["am", "start", "-n", activity.as_str()]
        };

        run_adb_shell_checked(Some(device_id.as_str()), &args, "adb shell am start")?;

        println!("{}", "✓ App launched".green());

        Ok(())
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        list_adb_devices()
    }
}

/// Result of staging launcher-icon overlay resources. The Gradle template
/// reads both fields:
/// - `res_overlay_dir` is added to `sourceSets.main.res.srcDirs` so AGP picks
///   up the new (uniquely-named) drawables/mipmaps.
/// - `icon_resource_name` (e.g. `ic_launcher_lingxia_env`) flows into the
///   manifest via `manifestPlaceholders`, swapping which icon the launcher
///   shows. We deliberately *don't* override the existing `ic_launcher.xml`
///   in place — Gradle's resource merger treats duplicate qualified names as
///   build errors, not silent overrides.
struct LauncherIconOverlay {
    res_overlay_dir: PathBuf,
    icon_resource_name: String,
    has_round_icon: bool,
}

/// Generate the env-version launcher-icon overlay resources to a staging
/// directory outside the source tree. Returns the overlay descriptor if an
/// overlay was produced, or `None` when no badge applies (release env, no
/// adaptive icon to badge, etc.).
///
/// Nothing under the user's git tree is modified, so SIGKILL/abort can never
/// leave the project dirty.
fn prepare_launcher_icon_overlay(
    android_root: &Path,
    config: &BuildConfig,
) -> Result<Option<LauncherIconOverlay>> {
    let Some((badge, accent)) = android_env_icon_badge(config.resolved_env.version) else {
        return Ok(None);
    };
    let res_dir = android_root.join("app/src/main/res");
    let icon_path = res_dir.join("mipmap-anydpi-v26/ic_launcher.xml");
    let round_icon_path = res_dir.join("mipmap-anydpi-v26/ic_launcher_round.xml");
    if !icon_path.exists() {
        return Ok(None);
    }

    let icon_content = fs::read_to_string(&icon_path)
        .with_context(|| format!("Failed to read {}", icon_path.display()))?;
    let foreground = extract_adaptive_icon_foreground(&icon_content)
        .unwrap_or_else(|| "@mipmap/ic_launcher_foreground".to_string());
    let background = extract_adaptive_icon_background(&icon_content)
        .unwrap_or_else(|| "@color/ic_launcher_background".to_string());
    if !mipmap_resource_exists(&res_dir, &foreground) {
        return Ok(None);
    }

    // Stage under <android_root>/.lingxia/overlay/<env>/res so the dir lives
    // alongside iOS's `.lingxia/` build outputs. Gradle's `clean` won't touch
    // this, but we wipe per-env on every build so stale resources never leak.
    let staging_root = android_root
        .join(".lingxia")
        .join("overlay")
        .join(config.resolved_env.version.as_str());
    let staging_res = staging_root.join("res");
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .with_context(|| format!("Failed to clean {}", staging_root.display()))?;
    }

    let icon_resource_name = "ic_launcher_lingxia_env".to_string();
    let round_icon_resource_name = format!("{icon_resource_name}_round");
    let adaptive_icon = android_env_adaptive_icon_xml(&background);
    write_overlay_file(
        &staging_res.join(format!("mipmap-anydpi-v26/{icon_resource_name}.xml")),
        adaptive_icon.as_bytes(),
    )?;
    let has_round_icon = round_icon_path.exists();
    if has_round_icon {
        write_overlay_file(
            &staging_res.join(format!("mipmap-anydpi-v26/{round_icon_resource_name}.xml")),
            adaptive_icon.as_bytes(),
        )?;
    }
    write_overlay_file(
        &staging_res.join("drawable/lingxia_env_icon_foreground.xml"),
        android_env_icon_foreground_xml(&foreground, accent, badge).as_bytes(),
    )?;

    Ok(Some(LauncherIconOverlay {
        res_overlay_dir: staging_res,
        icon_resource_name,
        has_round_icon,
    }))
}

fn write_overlay_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("Failed to write {}", path.display()))
}

/// Check whether the launcher foreground reference resolves under any
/// density bucket. Earlier versions only probed `mipmap-mdpi`, which broke
/// projects that ship icons in higher-density buckets only.
fn mipmap_resource_exists(res_dir: &Path, drawable_ref: &str) -> bool {
    let Some(name) = drawable_ref.strip_prefix("@mipmap/") else {
        // `@drawable/...` or anything custom: assume the user knows what they
        // are doing; AGP will fail link-time with a clear error otherwise.
        return true;
    };
    const DENSITY_DIRS: &[&str] = &[
        "mipmap-mdpi",
        "mipmap-hdpi",
        "mipmap-xhdpi",
        "mipmap-xxhdpi",
        "mipmap-xxxhdpi",
        "mipmap-anydpi",
        "mipmap-anydpi-v26",
    ];
    const EXTS: &[&str] = &["webp", "png", "xml"];
    DENSITY_DIRS.iter().any(|density| {
        EXTS.iter()
            .any(|ext| res_dir.join(format!("{density}/{name}.{ext}")).exists())
    })
}

fn android_env_icon_badge(
    version: crate::config::EnvVersion,
) -> Option<(&'static str, &'static str)> {
    match version {
        crate::config::EnvVersion::Developer => Some(("D", "#D32F2F")),
        crate::config::EnvVersion::Preview => Some(("P", "#D32F2F")),
        crate::config::EnvVersion::Release => None,
    }
}

fn extract_adaptive_icon_foreground(content: &str) -> Option<String> {
    extract_adaptive_icon_drawable(content, "foreground")
}

fn extract_adaptive_icon_background(content: &str) -> Option<String> {
    extract_adaptive_icon_drawable(content, "background")
}

fn extract_adaptive_icon_drawable(content: &str, tag_name: &str) -> Option<String> {
    let tag_start = content.find(&format!("<{tag_name}"))?;
    let tag_end = content[tag_start..].find('>')? + tag_start;
    let tag = &content[tag_start..tag_end];
    let attr_start = tag.find("android:drawable=")? + "android:drawable=".len();
    let quote = tag.as_bytes().get(attr_start).copied()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let value_start = attr_start + 1;
    let value_end = tag[value_start..].find(quote as char)? + value_start;
    Some(tag[value_start..value_end].to_string())
}

fn android_env_adaptive_icon_xml(background: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="{background}" />
    <foreground android:drawable="@drawable/lingxia_env_icon_foreground" />
</adaptive-icon>
"#
    )
}

fn android_env_icon_foreground_xml(foreground: &str, accent: &str, badge: &str) -> String {
    format!(
        r###"<?xml version="1.0" encoding="utf-8"?>
<layer-list xmlns:android="http://schemas.android.com/apk/res/android">
    <item android:drawable="{foreground}" />
    <item
        android:width="42dp"
        android:height="42dp"
        android:gravity="bottom|end"
        android:bottom="14dp"
        android:right="14dp">
        <shape android:shape="oval">
            <solid android:color="{accent}" />
            <stroke android:width="4dp" android:color="#FFFFFF" />
        </shape>
    </item>
    <item
        android:width="42dp"
        android:height="42dp"
        android:gravity="bottom|end"
        android:bottom="14dp"
        android:right="14dp">
        <vector
            android:width="42dp"
            android:height="42dp"
            android:viewportWidth="42"
            android:viewportHeight="42">
            {badge_path}
        </vector>
    </item>
</layer-list>
"###,
        badge_path = android_env_icon_badge_path(badge),
    )
}

fn android_env_icon_badge_path(badge: &str) -> &'static str {
    match badge {
        "D" => {
            r##"<path
                android:fillColor="#FFFFFFFF"
                android:pathData="M12,10 L22,10 C29,10 34,15 34,21 C34,27 29,32 22,32 L12,32 Z M18,16 L18,26 L22,26 C25.5,26 28,24 28,21 C28,18 25.5,16 22,16 Z" />"##
        }
        "P" => {
            r##"<path
                android:fillColor="#FFFFFFFF"
                android:pathData="M13,10 L25,10 C30,10 34,14 34,19 C34,24 30,28 25,28 L19,28 L19,32 L13,32 Z M19,16 L19,22 L24,22 C26.5,22 28,20.8 28,19 C28,17.2 26.5,16 24,16 Z" />"##
        }
        _ => "",
    }
}

fn install_with_adb(
    device_id: Option<&str>,
    apk_path: &Path,
    file_size: u64,
    quiet: bool,
) -> Result<()> {
    let output = match stream_install_with_adb(device_id, apk_path, file_size, quiet) {
        Ok(output) => output,
        Err(stream_err) => {
            if !quiet {
                eprintln!(
                    "{} adb streaming install failed, falling back to adb install: {}",
                    "Warning:".yellow(),
                    stream_err
                );
            }
            install_with_classic_adb(device_id, apk_path, file_size, quiet)?
        }
    };
    if output.lines().any(|line| line.trim() == "Success") {
        return Ok(());
    }
    Err(anyhow!("adb install failed: {}", output.trim()))
}

fn stream_install_with_adb(
    device_id: Option<&str>,
    apk_path: &Path,
    file_size: u64,
    quiet: bool,
) -> Result<String> {
    let progress = (!quiet).then(|| transfer_progress_bar(file_size));
    let mut child = adb_command(device_id)
        .arg("shell")
        .args(["pm", "install", "-r", "-S", &file_size.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| "Failed to execute adb streaming install")?;

    let write_result = (|| -> Result<()> {
        let mut apk = fs::File::open(apk_path)
            .with_context(|| format!("Failed to open APK {}", apk_path.display()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to open adb install stdin"))?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = apk
                .read(&mut buffer)
                .with_context(|| format!("Failed to read APK {}", apk_path.display()))?;
            if read == 0 {
                break;
            }
            stdin
                .write_all(&buffer[..read])
                .with_context(|| "Failed to stream APK to adb")?;
            if let Some(progress) = progress.as_ref() {
                progress.inc(read as u64);
            }
        }
        drop(stdin);
        if let Some(progress) = progress.as_ref() {
            progress.set_position(file_size);
            progress.set_message("Installing APK...");
        }
        Ok(())
    })();

    let output = child
        .wait_with_output()
        .with_context(|| "Failed to wait for adb streaming install")?;
    finish_progress(progress);
    write_result?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        return Ok(stdout);
    }
    Err(anyhow!(
        "adb install failed\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    ))
}

fn install_with_classic_adb(
    device_id: Option<&str>,
    apk_path: &Path,
    file_size: u64,
    quiet: bool,
) -> Result<String> {
    let spinner = (!quiet).then(|| {
        status_spinner(&format!(
            "Transferring & installing APK ({})...",
            format_transfer_size(file_size)
        ))
    });
    let result = run_adb_checked(
        device_id,
        &["install", "-r", apk_path.to_string_lossy().as_ref()],
        "adb install",
    );
    finish_spinner(spinner);
    result
}

fn adb_command(device_id: Option<&str>) -> Command {
    let mut command = Command::new("adb");
    if let Some(device_id) = device_id {
        command.arg("-s").arg(device_id);
    }
    command
}

fn run_adb_shell_checked(device_id: Option<&str>, args: &[&str], label: &str) -> Result<()> {
    let output = adb_command(device_id)
        .arg("shell")
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute {label}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(anyhow!(
        "{label} failed\nstdout: {}\nstderr: {}",
        stdout.trim(),
        stderr.trim()
    ))
}

fn run_adb_checked(device_id: Option<&str>, args: &[&str], label: &str) -> Result<String> {
    let output = adb_command(device_id)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute {label}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        return Ok(stdout);
    }
    Err(anyhow!(
        "{label} failed\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    ))
}

fn list_adb_devices() -> Result<Vec<Device>> {
    let output = run_adb_checked(None, &["devices", "-l"], "adb devices -l")?;
    let mut devices = Vec::new();

    for line in output.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(id) = parts.next() else {
            continue;
        };
        let Some(state) = parts.next() else {
            continue;
        };

        let name = line
            .split_whitespace()
            .find_map(|part| part.strip_prefix("model:"))
            .filter(|value| !value.is_empty())
            .map(|value| value.replace('_', " "));

        devices.push(Device {
            id: id.to_string(),
            name,
            device_type: if id.starts_with("emulator-") {
                DeviceType::Emulator
            } else {
                DeviceType::Physical
            },
            online: state == "device",
        });
    }

    Ok(devices)
}

fn resolve_adb_device_id(device_id: Option<&str>) -> Result<String> {
    if let Some(device_id) = device_id {
        return Ok(device_id.to_string());
    }

    let online_devices = list_adb_devices()?
        .into_iter()
        .filter(|device| device.online)
        .collect::<Vec<_>>();

    match online_devices.as_slice() {
        [] => Err(anyhow!("No Android devices connected")),
        [device] => Ok(device.id.clone()),
        _ => Err(anyhow!(
            "Multiple Android devices connected. Use --device to specify a target device"
        )),
    }
}

fn status_spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner.set_message(message.to_string());
    spinner
}

fn finish_spinner(spinner: Option<ProgressBar>) {
    if let Some(spinner) = spinner {
        spinner.finish_and_clear();
    }
}

fn transfer_progress_bar(total_bytes: u64) -> ProgressBar {
    let progress = ProgressBar::new(total_bytes);
    progress.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg} {bytes}/{total_bytes}")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    progress.enable_steady_tick(std::time::Duration::from_millis(80));
    progress.set_message("Transferring APK...");
    progress
}

fn finish_progress(progress: Option<ProgressBar>) {
    if let Some(progress) = progress {
        progress.finish_and_clear();
    }
}

fn format_transfer_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    format!("{mb:.1} MB")
}

fn adb_uninstall_package(device_id: Option<&str>, package_id: &str) -> Result<()> {
    let output = run_adb_checked(device_id, &["uninstall", package_id], "adb uninstall")?;
    if output.lines().any(|line| line.trim() == "Success") {
        return Ok(());
    }
    Err(anyhow!("adb uninstall failed: {}", output.trim()))
}

const APPLINKS_BEGIN: &str = "            <!-- LingXia AppLinks BEGIN -->";
const APPLINKS_END: &str = "            <!-- LingXia AppLinks END -->";

fn sync_android_app_links(android_root: &Path, hosts: &[String]) -> Result<bool> {
    let manifest_path = android_root.join("app/src/main/AndroidManifest.xml");
    if !manifest_path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let block = render_android_applinks_block(hosts);
    let updated = if content.contains(APPLINKS_BEGIN) && content.contains(APPLINKS_END) {
        replace_managed_applink_block(&content, &block)?
    } else if block.is_empty() {
        content.clone()
    } else {
        insert_applink_block(&content, &block)?
    };
    if updated == content {
        return Ok(false);
    }
    fs::write(&manifest_path, updated)
        .with_context(|| format!("Failed to write {}", manifest_path.display()))?;
    Ok(true)
}

fn render_android_applinks_block(hosts: &[String]) -> String {
    if hosts.is_empty() {
        return String::new();
    }
    let filters = hosts
        .iter()
        .map(|host| {
            format!(
                r#"            <intent-filter android:autoVerify="true">
                <action android:name="android.intent.action.VIEW" />
                <category android:name="android.intent.category.DEFAULT" />
                <category android:name="android.intent.category.BROWSABLE" />
                <data android:scheme="https" android:host="{host}" />
            </intent-filter>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{APPLINKS_BEGIN}\n{filters}\n{APPLINKS_END}")
}

fn replace_managed_applink_block(content: &str, block: &str) -> Result<String> {
    let start = content
        .find(APPLINKS_BEGIN)
        .ok_or_else(|| anyhow!("AndroidManifest.xml AppLinks begin marker not found"))?;
    let end = content
        .find(APPLINKS_END)
        .ok_or_else(|| anyhow!("AndroidManifest.xml AppLinks end marker not found"))?
        + APPLINKS_END.len();
    let mut updated = String::new();
    updated.push_str(&content[..start]);
    if !block.is_empty() {
        updated.push_str(block);
    }
    updated.push_str(&content[end..]);
    Ok(updated)
}

fn insert_applink_block(content: &str, block: &str) -> Result<String> {
    let insert_at = find_launcher_activity_end(content).ok_or_else(|| {
        anyhow!("AndroidManifest.xml missing launcher activity for AppLinks insertion")
    })?;
    let mut updated = String::new();
    updated.push_str(&content[..insert_at]);
    updated.push_str(block);
    updated.push('\n');
    updated.push_str(&content[insert_at..]);
    Ok(updated)
}

fn find_launcher_activity_end(content: &str) -> Option<usize> {
    let mut offset = 0;
    while let Some(relative_start) = content[offset..].find("<activity") {
        let start = offset + relative_start;
        let after_activity = content.as_bytes().get(start + "<activity".len()).copied();
        if !matches!(after_activity, Some(b' ' | b'\n' | b'\r' | b'\t' | b'>')) {
            offset = start + "<activity".len();
            continue;
        }
        let Some(relative_end) = content[start..].find("</activity>") else {
            return None;
        };
        let end = start + relative_end;
        let block = &content[start..end];
        if block.contains("android.intent.action.MAIN")
            && block.contains("android.intent.category.LAUNCHER")
        {
            return Some(end);
        }
        offset = end + "</activity>".len();
    }
    None
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

#[cfg(test)]
mod tests {
    use super::{
        android_env_adaptive_icon_xml, android_env_icon_foreground_xml,
        extract_adaptive_icon_background, extract_adaptive_icon_foreground,
    };

    #[test]
    fn env_overlay_extracts_adaptive_icon_foreground() {
        let icon = r#"<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
</adaptive-icon>"#;

        assert_eq!(
            extract_adaptive_icon_foreground(icon).as_deref(),
            Some("@mipmap/ic_launcher_foreground")
        );
        assert_eq!(
            extract_adaptive_icon_background(icon).as_deref(),
            Some("@color/ic_launcher_background")
        );
    }

    #[test]
    fn env_overlay_preserves_adaptive_icon_background() {
        let icon = android_env_adaptive_icon_xml("@color/ic_launcher_background");
        assert!(icon.contains(r#"android:drawable="@color/ic_launcher_background""#));
        assert!(icon.contains(r#"android:drawable="@drawable/lingxia_env_icon_foreground""#));
    }

    #[test]
    fn env_overlay_generates_badged_foreground_drawable() {
        let drawable =
            android_env_icon_foreground_xml("@mipmap/ic_launcher_foreground", "#D32F2F", "D");
        assert!(drawable.contains(r#"android:drawable="@mipmap/ic_launcher_foreground""#));
        assert!(drawable.contains(r##"android:color="#D32F2F""##));
        assert!(drawable.contains("android:viewportWidth=\"42\""));
    }
}
