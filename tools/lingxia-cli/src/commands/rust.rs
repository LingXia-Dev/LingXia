use crate::platform::BuildProfile;
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

/// Apply common cargo build switches for profile.
pub fn apply_cargo_profile(cmd: &mut Command, profile: BuildProfile) {
    if matches!(profile, BuildProfile::Release) {
        cmd.arg("--release");
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

    apply_cargo_profile(&mut cmd, profile);
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

    apply_cargo_profile(&mut cmd, profile);
    configure(&mut cmd);

    let status = cmd.status().context("Failed to execute cargo rustc")?;
    if !status.success() {
        return Err(anyhow!("Rust build failed for target: {}", target));
    }
    Ok(())
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
    fn apply_cargo_profile_only_adds_release_when_requested() {
        let mut cmd = Command::new("cargo");
        apply_cargo_profile(&mut cmd, BuildProfile::Debug);

        let args = command_args(&cmd);
        assert!(!args.iter().any(|a| a == "--no-default-features"));
        assert!(!args.iter().any(|a| a == "--release"));
    }
}
