use crate::platform::BuildProfile;
use crate::platform::detector::PlatformType;
use crate::platform::doctor::command_version_line;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::path::Path;
use std::process::Command;

/// Parse build profile from `--release` style flags.
pub fn resolve_build_profile(release: bool) -> BuildProfile {
    if release {
        BuildProfile::Release
    } else {
        BuildProfile::Debug
    }
}

/// Apply common cargo build switches for profile/features.
pub fn apply_cargo_profile_and_features(
    cmd: &mut Command,
    profile: BuildProfile,
    features: &[String],
) {
    if matches!(profile, BuildProfile::Release) {
        cmd.arg("--release");
    }
    let has_tls_ring = features.iter().any(|f| f == "tls-ring");
    let has_tls_aws = features.iter().any(|f| f == "tls-aws-lc");
    // When tls-ring is explicitly selected, disable default features to avoid
    // pulling in default tls-aws-lc and enabling both backends.
    if has_tls_ring && !has_tls_aws {
        cmd.arg("--no-default-features");
    }

    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }
}

pub fn cargo_version_line() -> Option<String> {
    command_version_line("cargo", &["--version"], false)
}

/// Execute `cargo build` for a target with shared LingXia defaults.
pub fn run_cargo_build_for_target<F>(
    manifest_path: &Path,
    working_dir: &Path,
    target_dir: &Path,
    target: &str,
    package: Option<&str>,
    profile: BuildProfile,
    features: &[String],
    configure: F,
) -> Result<()>
where
    F: FnOnce(&mut Command),
{
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--target")
        .arg(target)
        .arg("--manifest-path")
        .arg(manifest_path)
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(working_dir);

    if let Some(package_name) = package {
        cmd.arg("-p").arg(package_name);
    }

    apply_cargo_profile_and_features(&mut cmd, profile, features);
    configure(&mut cmd);

    let status = cmd.status().context("Failed to execute cargo build")?;
    if !status.success() {
        return Err(anyhow!("Rust build failed for target: {}", target));
    }
    Ok(())
}

/// Execute `cargo rustc --crate-type=staticlib` for a target with shared LingXia defaults.
pub fn run_cargo_rustc_staticlib_for_target<F>(
    manifest_path: &Path,
    working_dir: &Path,
    target_dir: &Path,
    target: &str,
    profile: BuildProfile,
    features: &[String],
    configure: F,
) -> Result<()>
where
    F: FnOnce(&mut Command),
{
    let mut cmd = Command::new("cargo");
    cmd.arg("rustc")
        .arg("--crate-type=staticlib")
        .arg("--target")
        .arg(target)
        .arg("--manifest-path")
        .arg(manifest_path)
        .env("CARGO_TARGET_DIR", target_dir)
        .current_dir(working_dir);

    apply_cargo_profile_and_features(&mut cmd, profile, features);
    configure(&mut cmd);

    let status = cmd.status().context("Failed to execute cargo rustc")?;
    if !status.success() {
        return Err(anyhow!("Rust build failed for target: {}", target));
    }
    Ok(())
}

pub fn resolve_platform_features(
    requested: &[String],
    platform: &PlatformType,
) -> Result<Vec<String>> {
    let mut features = requested.to_vec();
    let has_tls_ring = features.iter().any(|f| f == "tls-ring");
    let has_tls_aws = features.iter().any(|f| f == "tls-aws-lc");
    if has_tls_ring && has_tls_aws {
        return Err(anyhow!(
            "Conflicting TLS features: `tls-ring` and `tls-aws-lc` cannot both be enabled."
        ));
    }

    let is_mobile_platform = matches!(
        platform,
        PlatformType::Android | PlatformType::Ios | PlatformType::Harmony
    );
    if is_mobile_platform && !has_tls_ring && !has_tls_aws {
        features.push("tls-ring".to_string());
        println!(
            "{} {}: auto-enabled feature `tls-ring`",
            "ℹ".blue(),
            platform.as_str()
        );
    }

    Ok(features)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn apply_cargo_profile_and_features_disables_defaults_for_tls_ring_only() {
        let mut cmd = Command::new("cargo");
        let features = vec!["tls-ring".to_string()];
        apply_cargo_profile_and_features(&mut cmd, BuildProfile::Debug, &features);

        let args = command_args(&cmd);
        assert!(args.iter().any(|a| a == "--no-default-features"));
        assert!(args.windows(2).any(|w| w == ["--features", "tls-ring"]));
    }

    #[test]
    fn apply_cargo_profile_and_features_keeps_defaults_for_tls_aws_only() {
        let mut cmd = Command::new("cargo");
        let features = vec!["tls-aws-lc".to_string()];
        apply_cargo_profile_and_features(&mut cmd, BuildProfile::Debug, &features);

        let args = command_args(&cmd);
        assert!(!args.iter().any(|a| a == "--no-default-features"));
        assert!(args.windows(2).any(|w| w == ["--features", "tls-aws-lc"]));
    }

    #[test]
    fn resolve_platform_features_auto_adds_tls_ring_on_mobile() {
        let features = resolve_platform_features(&[], &PlatformType::Android).unwrap();
        assert!(features.iter().any(|f| f == "tls-ring"));
    }
}
