use super::{
    DEFAULT_ABILITY_NAME, HarmonyPlatform, HarmonySigner, ProvisioningManager, SigningConfig,
    SigningMode, project::resolve_harmony_dir, read_bundle_name,
};
use crate::platform::{BuildProfile, Device, DeviceType, InstallConfig, RunConfig};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

impl HarmonyPlatform {
    pub(super) fn install_impl(&self, config: &InstallConfig) -> Result<()> {
        ensure_command("hdc")?;

        let hap_path = if let Some(ref path) = config.artifact_path {
            path.clone()
        } else {
            let harmony_dir = resolve_harmony_dir(&config.project_root, None)?;
            auto_detect_hap(&harmony_dir)?
        };

        if !hap_path.exists() {
            return Err(anyhow!("HAP not found at: {}", hap_path.display()));
        }

        let target_udids = ensure_device_connected(config.device_id.as_deref())?;

        // Install path is strict: always sign first, then install.
        let hap_path = self.sign_before_install(&hap_path, &config.project_root, &target_udids)?;

        println!("  {} Installing HAP: {}", "→".dimmed(), hap_path.display());

        let mut cmd = Command::new("hdc");
        if let Some(ref device_id) = config.device_id {
            cmd.arg("-t").arg(device_id);
        }
        cmd.arg("install").arg("-r").arg(&hap_path);

        let output = cmd.output().context("Failed to execute hdc install")?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || hdc_install_failed(&stdout, &stderr) {
            return Err(anyhow!(
                "hdc install failed:\n{}\n{}",
                stdout.trim(),
                stderr.trim()
            ));
        }

        println!("{}", "  ✓ Installed".green());
        Ok(())
    }

    pub(super) fn uninstall_impl(&self, package_id: &str, device_id: Option<&str>) -> Result<()> {
        ensure_command("hdc")?;

        let mut cmd = Command::new("hdc");
        if let Some(id) = device_id {
            cmd.arg("-t").arg(id);
        }
        cmd.arg("uninstall").arg(package_id);

        let status = cmd.status().context("Failed to execute hdc uninstall")?;
        if !status.success() {
            return Err(anyhow!("Failed to uninstall {}", package_id));
        }

        println!("  {} Uninstalled {}", "✓".green(), package_id);
        Ok(())
    }

    pub(super) fn run_impl(&self, config: &RunConfig) -> Result<()> {
        ensure_command("hdc")?;

        let ability = config
            .main_activity
            .as_deref()
            .unwrap_or(DEFAULT_ABILITY_NAME);

        let mut cmd = Command::new("hdc");
        if let Some(ref device_id) = config.device_id {
            cmd.arg("-t").arg(device_id);
        }
        cmd.arg("shell")
            .arg("aa")
            .arg("start")
            .arg("-a")
            .arg(ability)
            .arg("-b")
            .arg(&config.package_id);

        let output = cmd
            .output()
            .context("Failed to execute hdc shell aa start")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() || stdout.contains("error:") || stderr.contains("error:") {
            let msg = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                stdout.trim().to_string()
            };
            return Err(anyhow!(
                "Failed to launch app (bundle={}, ability={}): {}",
                config.package_id,
                ability,
                msg
            ));
        }

        println!("  {} App launched", "✓".green());
        Ok(())
    }

    pub(super) fn list_devices_impl(&self) -> Result<Vec<Device>> {
        ensure_command("hdc")?;

        let output = Command::new("hdc")
            .arg("list")
            .arg("targets")
            .output()
            .context("Failed to execute hdc list targets")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices: Vec<Device> = stdout
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && *line != "[Empty]")
            .map(|line| Device {
                id: line.to_string(),
                name: None,
                device_type: if line.contains("emulator") || line.starts_with("127.0.0.1") {
                    DeviceType::Emulator
                } else {
                    DeviceType::Physical
                },
                online: true,
            })
            .collect();

        Ok(devices)
    }

    /// Sign input HAP before install regardless of current filename.
    fn sign_before_install(
        &self,
        hap_path: &Path,
        project_root: &Path,
        target_udids: &[String],
    ) -> Result<PathBuf> {
        let source = preferred_resign_source(hap_path);
        let mut output_path = install_signed_output_path(&source);

        if output_path == source {
            output_path = source.with_file_name(format!(
                "{}-resigned.hap",
                source.file_stem().unwrap_or_default().to_string_lossy()
            ));
        }

        self.sign_hap_with_project_config_at(
            &source,
            project_root,
            output_path,
            BuildProfile::Debug,
            target_udids,
        )
    }

    pub(super) fn sign_hap_with_project_config(
        &self,
        input_hap: &Path,
        project_root: &Path,
        build_profile: BuildProfile,
    ) -> Result<PathBuf> {
        let output_path = signed_output_path(input_hap);
        self.sign_hap_with_project_config_at(
            input_hap,
            project_root,
            output_path,
            build_profile,
            &[],
        )
    }

    fn sign_hap_with_project_config_at(
        &self,
        input_hap: &Path,
        project_root: &Path,
        output_path: PathBuf,
        build_profile: BuildProfile,
        target_udids: &[String],
    ) -> Result<PathBuf> {
        let signer = HarmonySigner::new_native();
        let signing = load_signing_config(project_root, build_profile, target_udids)?;

        println!("  {} Signing HAP (Rust native signer)...", "→".dimmed());
        signer
            .sign_hap(&signing, input_hap, &output_path)
            .context("HAP signing failed")?;
        signer
            .verify_hap(&output_path)
            .context("Signed HAP verification failed")?;

        println!(
            "  {} Signed HAP created: {}",
            "✓".green(),
            output_path.display()
        );
        Ok(output_path)
    }
}

fn load_signing_config(
    project_root: &Path,
    build_profile: BuildProfile,
    target_udids: &[String],
) -> Result<SigningConfig> {
    let harmony_dir = resolve_harmony_dir(project_root, None)?;
    let bundle_name = read_bundle_name(&harmony_dir)?;

    let mode = match build_profile {
        BuildProfile::Debug => SigningMode::Debug,
        BuildProfile::Release => SigningMode::Release,
    };

    let mut provisioning = ProvisioningManager::from_storage()?;
    provisioning.prepare_signing_config(&bundle_name, mode, target_udids)
}

pub(super) fn ensure_command(name: &str) -> Result<()> {
    if which::which(name).is_err() {
        return Err(anyhow!(
            "'{}' not found in PATH. Please install HarmonyOS development tools.\n\
             Ensure Harmony command-line tools are in your PATH.",
            name
        ));
    }
    Ok(())
}

fn ensure_device_connected(device_id: Option<&str>) -> Result<Vec<String>> {
    let targets = connected_devices()?;

    if targets.is_empty() {
        return Err(anyhow!(
            "No HarmonyOS device connected. Connect a device via USB or start an emulator."
        ));
    }

    if let Some(id) = device_id {
        if let Some(exact) = targets.iter().find(|candidate| candidate.as_str() == id) {
            return Ok(vec![fetch_harmony_udid(exact)?]);
        }
        if let Some(partial) = targets.iter().find(|candidate| candidate.contains(id)) {
            return Ok(vec![fetch_harmony_udid(partial)?]);
        }

        return Err(anyhow!(
            "Device '{}' not found. Available devices: {}",
            id,
            targets.join(", ")
        ));
    }

    targets
        .iter()
        .map(|target| fetch_harmony_udid(target))
        .collect()
}

fn connected_devices() -> Result<Vec<String>> {
    let output = Command::new("hdc")
        .arg("list")
        .arg("targets")
        .output()
        .context("Failed to execute hdc list targets")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && line != "[Empty]")
        .collect())
}

fn fetch_harmony_udid(target: &str) -> Result<String> {
    let output = Command::new("hdc")
        .arg("-t")
        .arg(target)
        .arg("shell")
        .arg("bm")
        .arg("get")
        .arg("-u")
        .output()
        .with_context(|| format!("Failed to query Harmony UDID for target {target}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "Failed to query Harmony UDID for target {}: {}\n{}",
            target,
            stdout.trim(),
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let udid = stdout
        .split(|ch: char| !ch.is_ascii_hexdigit())
        .find(|token| token.len() >= 32)
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow!(
                "Failed to parse Harmony UDID from hdc output: {}",
                stdout.trim()
            )
        })?;

    Ok(udid)
}

fn auto_detect_hap(harmony_dir: &Path) -> Result<PathBuf> {
    let signed = harmony_dir.join("entry/build/default/outputs/default/entry-default-signed.hap");
    let unsigned =
        harmony_dir.join("entry/build/default/outputs/default/entry-default-unsigned.hap");

    if signed.exists() {
        Ok(signed)
    } else if unsigned.exists() {
        Ok(unsigned)
    } else {
        Err(anyhow!(
            "No HAP found. Build the project first with 'lingxia build --platform harmony'"
        ))
    }
}

fn signed_output_path(input_hap: &Path) -> PathBuf {
    let file_name = input_hap
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if file_name.contains("unsigned") {
        return input_hap.with_file_name(file_name.replace("unsigned", "signed"));
    }
    let stem = input_hap
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    input_hap.with_file_name(format!("{stem}-signed.hap"))
}

fn install_signed_output_path(input_hap: &Path) -> PathBuf {
    let stem = input_hap
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let normalized = stem
        .trim_end_matches("-unsigned")
        .trim_end_matches("-signed")
        .trim_end_matches("-install-signed");
    input_hap.with_file_name(format!("{normalized}-install-signed.hap"))
}

fn preferred_resign_source(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if file_name.contains("unsigned") {
        let signed_candidate = path.with_file_name(file_name.replace("unsigned", "signed"));
        if signed_candidate.exists() {
            return signed_candidate;
        }
    }
    path.to_path_buf()
}

fn hdc_install_failed(stdout: &str, stderr: &str) -> bool {
    fn has_failure_marker(text: &str) -> bool {
        let lower = text.to_ascii_lowercase();
        lower.contains("error:")
            || lower.contains("failed to install")
            || lower.contains("fail to install")
            || lower.contains("install failed")
    }

    has_failure_marker(stdout) || has_failure_marker(stderr)
}
