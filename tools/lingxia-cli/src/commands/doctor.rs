use crate::config::{HOST_CONFIG_FILE, LingXiaConfig};
use crate::commands::rust::cargo_version_line;
use crate::platform::detector::PlatformType;
use crate::platform::doctor::{CheckResult, CheckStatus, command_version_line};
use crate::platform::{android, detector, harmony, ios, macos};
use anyhow::Result;
use colored::Colorize;
use std::env;

/// Execute environment doctor checks.
///
/// If `platforms` is empty:
/// - Prefer platforms from `lingxia.config.json` when available.
/// - Otherwise, check all supported platforms.
pub fn execute(platforms: Vec<String>) -> Result<()> {
    println!("{}", "🔍 LingXia Doctor".bold().cyan());
    println!();

    let mut all_checks = Vec::new();

    println!("{}", "[common]".bold().cyan());
    for check in common_checks() {
        print_check(&check);
        all_checks.push(check);
    }
    println!();

    let target_platforms = resolve_target_platforms(platforms)?;
    for platform in target_platforms {
        println!("{}", format!("[{}]", platform.as_str()).bold().cyan());
        let checks = platform_checks(&platform);
        for check in checks {
            print_check(&check);
            all_checks.push(check);
        }
        println!();
    }

    let failed = all_checks
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .count();
    let warned = all_checks
        .iter()
        .filter(|c| c.status == CheckStatus::Warn)
        .count();
    let passed = all_checks
        .iter()
        .filter(|c| c.status == CheckStatus::Pass)
        .count();

    println!(
        "{} passed, {} warnings, {} failed",
        passed.to_string().green(),
        warned.to_string().yellow(),
        failed.to_string().red()
    );
    if failed == 0 {
        println!("{}", "✅ Doctor finished with no blocking issues.".green());
    } else {
        println!(
            "{}",
            "⚠️  Doctor found blocking issues. Fix failed checks first.".yellow()
        );
    }

    Ok(())
}

fn common_checks() -> Vec<CheckResult> {
    vec![check_rust(), check_cargo()]
}

fn check_rust() -> CheckResult {
    match command_version_line("rustc", &["--version"], false) {
        Some(version) => CheckResult::pass("Rust", version),
        None => CheckResult::fail(
            "Rust",
            "rustc not found in PATH".to_string(),
            Some("Install Rust from https://rustup.rs/"),
        ),
    }
}

fn check_cargo() -> CheckResult {
    match cargo_version_line() {
        Some(version) => CheckResult::pass("Cargo", version),
        None => CheckResult::fail(
            "Cargo",
            "cargo not found in PATH".to_string(),
            Some("Install Rust toolchain from https://rustup.rs/"),
        ),
    }
}

fn resolve_target_platforms(requested: Vec<String>) -> Result<Vec<PlatformType>> {
    if !requested.is_empty() {
        return parse_requested_platforms(requested);
    }

    if let Some(platforms) = platforms_from_project_config()? {
        return Ok(platforms);
    }

    Ok(vec![
        PlatformType::Android,
        PlatformType::Ios,
        PlatformType::MacOs,
        PlatformType::Harmony,
    ])
}

fn parse_requested_platforms(requested: Vec<String>) -> Result<Vec<PlatformType>> {
    let mut parsed = Vec::new();
    for item in requested {
        if item.eq_ignore_ascii_case("all") {
            for platform in [
                PlatformType::Android,
                PlatformType::Ios,
                PlatformType::MacOs,
                PlatformType::Harmony,
            ] {
                if !parsed.contains(&platform) {
                    parsed.push(platform);
                }
            }
            continue;
        }

        let platform: PlatformType = item.parse()?;
        if !parsed.contains(&platform) {
            parsed.push(platform);
        }
    }
    Ok(parsed)
}

fn platforms_from_project_config() -> Result<Option<Vec<PlatformType>>> {
    let current_dir = env::current_dir()?;
    let config_root = if current_dir.join(HOST_CONFIG_FILE).exists() {
        Some(current_dir.clone())
    } else if let Some(ctx) =
        detector::find_apple_swift_package_context(&current_dir, HOST_CONFIG_FILE)?
    {
        Some(ctx.host_project_root)
    } else {
        detector::find_host_project_root(&current_dir, HOST_CONFIG_FILE)
    };

    let Some(config_root) = config_root else {
        return Ok(None);
    };

    let config = match LingXiaConfig::load(&config_root) {
        Ok(cfg) => cfg,
        Err(_) => return Ok(None),
    };

    let Some(app) = config.app else {
        return Ok(None);
    };

    let mut platforms = Vec::new();
    for platform_name in app.platforms {
        let Ok(platform) = platform_name.parse::<PlatformType>() else {
            continue;
        };
        if !platforms.contains(&platform) {
            platforms.push(platform);
        }
    }

    if platforms.is_empty() {
        return Ok(None);
    }

    Ok(Some(platforms))
}

fn platform_checks(platform: &PlatformType) -> Vec<CheckResult> {
    match platform {
        PlatformType::Android => android::doctor_checks(),
        PlatformType::Ios => ios::doctor_checks(),
        PlatformType::MacOs => macos::doctor_checks(),
        PlatformType::Harmony => harmony::doctor_checks(),
    }
}

fn print_check(check: &CheckResult) {
    let (symbol, colorized_name) = match check.status {
        CheckStatus::Pass => ("✓".green(), check.name.green()),
        CheckStatus::Warn => ("⚠".yellow(), check.name.yellow()),
        CheckStatus::Fail => ("✗".red(), check.name.red()),
    };
    println!("  {} {}: {}", symbol, colorized_name, check.detail);

    if let Some(hint) = &check.hint {
        let mut lines = hint.lines();
        if let Some(first) = lines.next() {
            println!("    {} {}", "Hint:".dimmed(), first.dimmed());
        }
        for line in lines {
            println!("          {}", line.dimmed());
        }
    }
}
