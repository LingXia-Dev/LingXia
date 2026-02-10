//! Device management commands: list, uninstall, launch.

use crate::platform;
use crate::platform::RunConfig;
use crate::platform::detector::PlatformType;
use anyhow::{Result, anyhow};
use colored::Colorize;

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
    bundle_id: &str,
    device: Option<String>,
    platform_arg: Option<String>,
) -> Result<()> {
    let platform_type = resolve_single_platform(platform_arg)?;
    let p = platform::detector::create_platform(&platform_type)?;

    p.uninstall(bundle_id, device.as_deref())?;

    println!("{} {} uninstalled", "✓".green(), bundle_id);
    Ok(())
}

/// Launch an installed app on a device
pub fn launch(bundle_id: &str, device: Option<String>, platform_arg: Option<String>) -> Result<()> {
    let platform_type = resolve_single_platform(platform_arg)?;
    let p = platform::detector::create_platform(&platform_type)?;

    let run_config = RunConfig {
        package_id: bundle_id.to_string(),
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
        let project_root = std::env::current_dir()?;
        platform::detector::detect_platform_type(&project_root)
            .map_err(|_| anyhow!("Could not detect platform. Please specify with --platform"))
    }
}
