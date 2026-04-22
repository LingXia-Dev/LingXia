//! Device management commands: list, uninstall, launch.

use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
use crate::platform;
use crate::platform::RunConfig;
use crate::platform::detector::PlatformType;
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env;
use std::path::PathBuf;

/// List connected devices
pub fn list_devices(platform_arg: Option<String>) -> Result<()> {
    let platforms = resolve_platforms(platform_arg)?;

    let mut found_any = false;
    let mut first = true;

    for platform_type in platforms {
        if !first {
            println!();
        }
        first = false;
        println!("{}", platform_type.as_str().cyan().bold());

        match platform::detector::create_platform(&platform_type) {
            Ok(p) => match p.list_devices() {
                Ok(devices) if !devices.is_empty() => {
                    found_any = true;
                    for d in devices {
                        if let Some(name) = &d.name {
                            println!("  {} {}  {}", "•".green(), name, d.id.dimmed());
                        } else {
                            println!("  {} {}", "•".green(), d.id);
                        }
                    }
                }
                Ok(_) => println!("  {}", "No devices connected".dimmed()),
                Err(e) => println!("  {} {}", "✗".red(), e),
            },
            Err(_) => {
                println!("  {}", "Not available on this host".dimmed());
            }
        }
    }

    if !found_any {
        println!();
        println!(
            "{}",
            "No devices found. Connect a device and try again.".yellow()
        );
    }

    Ok(())
}

/// Uninstall an app from a device
pub fn uninstall(
    bundle_id: Option<&str>,
    device: Option<String>,
    platform_arg: Option<String>,
) -> Result<()> {
    let platform_type = resolve_single_platform(platform_arg)?;
    let bundle_id = resolve_package_id(bundle_id, &platform_type, "uninstall")?;
    let p = platform::detector::create_platform(&platform_type)?;

    p.uninstall(&bundle_id, device.as_deref())?;

    println!("{} {} uninstalled", "✓".green(), bundle_id);
    Ok(())
}

/// Launch an installed app on a device
pub fn launch(
    bundle_id: Option<&str>,
    device: Option<String>,
    platform_arg: Option<String>,
) -> Result<()> {
    let platform_type = resolve_single_platform(platform_arg.clone())?;
    let bundle_id = resolve_package_id(bundle_id, &platform_type, "launch")?;
    let p = platform::detector::create_platform(&platform_type)?;

    let run_config = RunConfig {
        package_id: bundle_id,
        main_activity: None,
        device_id: device,
    };

    p.run(&run_config)?;

    Ok(())
}

// =============================================================================
// Platform resolution
// =============================================================================

fn resolve_platforms(platform_arg: Option<String>) -> Result<Vec<PlatformType>> {
    if let Some(p) = platform_arg {
        Ok(vec![p.parse()?])
    } else {
        // Default: show all supported device platforms.
        Ok(vec![
            PlatformType::Android,
            PlatformType::Ios,
            PlatformType::Harmony,
        ])
    }
}

fn resolve_single_platform(platform_arg: Option<String>) -> Result<PlatformType> {
    if let Some(p) = platform_arg {
        p.parse()
    } else {
        // Try to detect from project config
        let project_root = current_project_root()?;
        platform::detector::detect_platform_type(&project_root)
    }
}

fn resolve_package_id(
    bundle_id: Option<&str>,
    platform_type: &PlatformType,
    action: &str,
) -> Result<String> {
    if let Some(bundle_id) = bundle_id {
        return Ok(bundle_id.to_string());
    }

    let project_root = current_project_root()?;
    let config = LingXiaConfig::load(&project_root).map_err(|_| {
        anyhow!(
            "Bundle ID / Package ID is required when LingXia config cannot be resolved from {}.",
            project_root.display()
        )
    })?;

    infer_package_id_from_config(&config, platform_type).ok_or_else(|| {
        anyhow!(
            "Could not infer {} identifier for {} from {}. Pass <BUNDLE_ID> explicitly.",
            action,
            platform_type.as_str(),
            project_root.join(HOST_CONFIG_FILE).display()
        )
    })
}

fn current_project_root() -> Result<PathBuf> {
    let current_dir = env::current_dir()?;
    Ok(
        platform::detector::find_host_project_root(&current_dir, HOST_CONFIG_FILE)
            .unwrap_or(current_dir),
    )
}

fn infer_package_id_from_config(
    config: &LingXiaConfig,
    platform_type: &PlatformType,
) -> Option<String> {
    match platform_type {
        PlatformType::Android => config.android.as_ref().map(|cfg| cfg.package_id.clone()),
        PlatformType::Ios => config.ios.as_ref().map(|cfg| cfg.bundle_id.clone()),
        PlatformType::Harmony => config.harmony.as_ref().map(|cfg| cfg.bundle_name.clone()),
        PlatformType::MacOs => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AndroidConfig, HarmonyConfig, HostAppConfig, IosConfig};

    fn sample_config() -> LingXiaConfig {
        LingXiaConfig {
            app: Some(HostAppConfig {
                project_name: "demo".into(),
                product_name: "Demo".into(),
                product_version: "0.1.0".into(),
                lingxia_server: None,
                lingxia_id: None,
                platforms: vec!["android".into(), "ios".into(), "harmony".into()],
                home_app_id: "demo-home".into(),
            }),
            android: Some(AndroidConfig {
                package_id: "com.example.demo".into(),
                min_sdk: None,
                target_sdk: None,
                compile_sdk: None,
                ndk_version: None,
                api_level: None,
            }),
            ios: Some(IosConfig {
                bundle_id: "app.example.demo".into(),
                deployment_target: None,
                swift_version: None,
                target_name: None,
            }),
            macos: None,
            harmony: Some(HarmonyConfig {
                bundle_name: "com.example.demo.hm".into(),
                compatible_sdk_version: None,
                target_sdk_version: None,
            }),
            features: None,
            capabilities: None,
            shell: None,
            ui: None,
            app_links: None,
            storage: None,
            resources: None,
        }
    }

    #[test]
    fn infer_launch_package_id_from_config_by_platform() {
        let config = sample_config();
        assert_eq!(
            infer_package_id_from_config(&config, &PlatformType::Android).as_deref(),
            Some("com.example.demo")
        );
        assert_eq!(
            infer_package_id_from_config(&config, &PlatformType::Ios).as_deref(),
            Some("app.example.demo")
        );
        assert_eq!(
            infer_package_id_from_config(&config, &PlatformType::Harmony).as_deref(),
            Some("com.example.demo.hm")
        );
    }
}
