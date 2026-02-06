use super::{DEFAULT_ABILITY_NAME, HarmonyPlatform, project::resolve_harmony_dir};
use crate::platform::{Device, DeviceType, InstallConfig, RunConfig};
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

        ensure_device_connected(config.device_id.as_deref())?;
        println!("  {} Installing HAP: {}", "→".dimmed(), hap_path.display());

        let mut cmd = Command::new("hdc");
        if let Some(ref device_id) = config.device_id {
            cmd.arg("-t").arg(device_id);
        }
        cmd.arg("install").arg("-r").arg(&hap_path);

        let output = cmd.output().context("Failed to execute hdc install")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
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
}

pub(super) fn ensure_command(name: &str) -> Result<()> {
    if which::which(name).is_err() {
        return Err(anyhow!(
            "'{}' not found in PATH. Please install HarmonyOS development tools.\n\
             Ensure DevEco Studio command-line tools are in your PATH.",
            name
        ));
    }
    Ok(())
}

fn ensure_device_connected(device_id: Option<&str>) -> Result<()> {
    let output = Command::new("hdc")
        .arg("list")
        .arg("targets")
        .output()
        .context("Failed to execute hdc list targets")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices: Vec<&str> = stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && *l != "[Empty]")
        .collect();

    if devices.is_empty() {
        return Err(anyhow!(
            "No HarmonyOS device connected. Connect a device via USB or start an emulator."
        ));
    }

    if let Some(id) = device_id
        && !devices.iter().any(|d| d.contains(id))
    {
        return Err(anyhow!(
            "Device '{}' not found. Available devices: {}",
            id,
            devices.join(", ")
        ));
    }

    Ok(())
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
