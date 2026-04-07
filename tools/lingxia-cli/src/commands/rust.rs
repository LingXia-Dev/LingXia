use crate::platform::BuildProfile;
use crate::platform::detector::PlatformType;
use crate::platform::doctor::command_version_line;
use anyhow::{Context, Result, anyhow};
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
    let has_tls_ring = requested.iter().any(|f| f == "tls-ring");
    let has_tls_aws = requested.iter().any(|f| f == "tls-aws-lc");
    if has_tls_ring || has_tls_aws {
        return Err(anyhow!(
            "LingXia now selects the TLS backend automatically by target: mobile uses `ring`, desktop uses `aws-lc-rs`. Remove `tls-ring`/`tls-aws-lc` from --features for {}.",
            platform.as_str()
        ));
    }
    Ok(requested.to_vec())
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
    fn apply_cargo_profile_and_features_passes_features_without_touching_defaults() {
        let mut cmd = Command::new("cargo");
        let features = vec!["cloud".to_string()];
        apply_cargo_profile_and_features(&mut cmd, BuildProfile::Debug, &features);

        let args = command_args(&cmd);
        assert!(!args.iter().any(|a| a == "--no-default-features"));
        assert!(args.windows(2).any(|w| w == ["--features", "cloud"]));
    }

    #[test]
    fn resolve_platform_features_rejects_legacy_tls_overrides() {
        let err = resolve_platform_features(&["tls-ring".to_string()], &PlatformType::Android)
            .expect_err("legacy tls feature should be rejected");
        assert!(
            err.to_string()
                .contains("selects the TLS backend automatically"),
            "unexpected error: {err}"
        );
    }
}
