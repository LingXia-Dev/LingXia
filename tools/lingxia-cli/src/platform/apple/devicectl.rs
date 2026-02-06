//! Device management using xcrun devicectl (Xcode 15+).
//!
//! Uses Apple's `devicectl` command to manage iOS devices,
//! install apps, and launch applications.

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Device controller for iOS devices using xcrun devicectl (Xcode 15+).
pub struct DeviceCtl;

impl DeviceCtl {
    /// Check if devicectl is available (requires Xcode 15+)
    pub fn is_available() -> bool {
        Command::new("xcrun")
            .args(["devicectl", "--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// List all connected iOS devices
    pub fn list_devices() -> Result<Vec<ConnectedDevice>> {
        let output = Command::new("xcrun")
            .args(["devicectl", "list", "devices", "--json-output", "-"])
            .output()
            .context("Failed to list devices")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to list devices: {}", stderr));
        }

        // Parse JSON output
        let json_output: DeviceListOutput =
            serde_json::from_slice(&output.stdout).context("Failed to parse device list output")?;

        Ok(json_output.result.devices)
    }

    /// Get a specific device by UDID
    pub fn get_device(udid: &str) -> Result<ConnectedDevice> {
        let devices = Self::list_devices()?;
        devices
            .into_iter()
            .find(|d| d.udid() == Some(udid) || d.identifier == udid)
            .ok_or_else(|| anyhow!("Device not found: {}", udid))
    }

    /// Wait for a device to be connected
    pub fn wait_for_device(timeout_seconds: u32) -> Result<ConnectedDevice> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_seconds as u64);

        println!(
            "Waiting for device to connect (timeout: {}s)...",
            timeout_seconds
        );

        loop {
            if let Ok(devices) = Self::list_devices()
                && let Some(device) = devices.into_iter().find(|d| d.is_available())
            {
                return Ok(device);
            }

            if start.elapsed() > timeout {
                return Err(anyhow!(
                    "Timeout waiting for device after {} seconds",
                    timeout_seconds
                ));
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    /// Install an app bundle to a device
    pub fn install_app(app_path: &Path, device_identifier: &str) -> Result<()> {
        println!("{}", "Installing app to device...".cyan());

        if !app_path.exists() {
            return Err(anyhow!("App bundle not found: {}", app_path.display()));
        }

        let output = Command::new("xcrun")
            .args([
                "devicectl",
                "device",
                "install",
                "app",
                "--device",
                device_identifier,
                app_path.to_str().unwrap(),
            ])
            .output()
            .context("Failed to install app")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow!(
                "Failed to install app:\nstderr: {}\nstdout: {}",
                stderr,
                stdout
            ));
        }

        println!("  {} App installed successfully", "✓".green());
        Ok(())
    }

    /// Launch an installed app on a device
    pub fn launch_app(bundle_id: &str, device_identifier: &str) -> Result<()> {
        println!("{}", "Launching app...".cyan());

        let output = Command::new("xcrun")
            .args([
                "devicectl",
                "device",
                "process",
                "launch",
                "--device",
                device_identifier,
                bundle_id,
            ])
            .output()
            .context("Failed to launch app")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to launch app: {}", stderr));
        }

        println!("  {} App launched", "✓".green());
        Ok(())
    }
}

// =============================================================================
// JSON Output Types for devicectl
// =============================================================================

#[derive(Debug, Deserialize)]
struct DeviceListOutput {
    result: DeviceListResult,
}

#[derive(Debug, Deserialize)]
struct DeviceListResult {
    devices: Vec<ConnectedDevice>,
}

/// A connected iOS device
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedDevice {
    /// Device identifier (used in devicectl commands)
    pub identifier: String,
    /// Connection properties
    #[serde(default)]
    pub connection_properties: Option<ConnectionProperties>,
    /// Device properties
    #[serde(default)]
    pub device_properties: Option<DeviceProperties>,
    /// Hardware properties
    #[serde(default)]
    pub hardware_properties: Option<HardwareProperties>,
}

impl ConnectedDevice {
    /// Get the device UDID
    pub fn udid(&self) -> Option<&str> {
        self.hardware_properties
            .as_ref()
            .and_then(|p| p.udid.as_deref())
    }

    /// Get the device name
    pub fn name(&self) -> Option<&str> {
        self.device_properties
            .as_ref()
            .and_then(|p| p.name.as_deref())
    }

    /// Get the device model
    pub fn model(&self) -> Option<&str> {
        self.hardware_properties
            .as_ref()
            .and_then(|p| p.marketing_name.as_deref())
    }

    /// Get the OS version
    pub fn os_version(&self) -> Option<&str> {
        self.device_properties
            .as_ref()
            .and_then(|p| p.os_version_number.as_deref())
    }

    /// Check if the device is available for use
    pub fn is_available(&self) -> bool {
        self.connection_properties
            .as_ref()
            .map(|p| p.transport_type.is_some())
            .unwrap_or(false)
    }

    /// Check if connected via USB
    pub fn is_usb(&self) -> bool {
        self.connection_properties
            .as_ref()
            .and_then(|p| p.transport_type.as_deref())
            .map(|t| t.to_lowercase().contains("usb") || t.to_lowercase().contains("wired"))
            .unwrap_or(false)
    }

    /// Check if connected via WiFi
    pub fn is_wifi(&self) -> bool {
        self.connection_properties
            .as_ref()
            .and_then(|p| p.transport_type.as_deref())
            .map(|t| t.to_lowercase().contains("wifi") || t.to_lowercase().contains("network"))
            .unwrap_or(false)
    }

    /// Get a human-readable description
    pub fn description(&self) -> String {
        let name = self.name().unwrap_or("Unknown");
        let model = self.model().unwrap_or("iOS Device");
        let version = self.os_version().unwrap_or("?");
        let connection = if self.is_usb() {
            "USB"
        } else if self.is_wifi() {
            "WiFi"
        } else {
            "?"
        };

        format!("{} ({}) - iOS {} [{}]", name, model, version, connection)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProperties {
    pub transport_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProperties {
    pub name: Option<String>,
    pub os_version_number: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProperties {
    pub udid: Option<String>,
    pub marketing_name: Option<String>,
}

// =============================================================================
// Unified API (requires Xcode 15+ for devicectl)
// =============================================================================

/// Install an app to a connected iOS device.
///
/// Requires Xcode 15+ (uses devicectl).
pub fn install_app(app_path: &Path, device_id: Option<&str>) -> Result<()> {
    if !DeviceCtl::is_available() {
        return Err(anyhow!(
            "devicectl not found. Please install Xcode 15 or later."
        ));
    }

    let device_identifier = if let Some(id) = device_id {
        DeviceCtl::get_device(id)?.identifier
    } else {
        DeviceCtl::wait_for_device(30)?.identifier
    };
    DeviceCtl::install_app(app_path, &device_identifier)
}

/// Uninstall an app from a connected iOS device.
///
/// Requires Xcode 15+ (uses devicectl).
pub fn uninstall_app(bundle_id: &str, device_id: Option<&str>) -> Result<()> {
    if !DeviceCtl::is_available() {
        return Err(anyhow!(
            "devicectl not found. Please install Xcode 15 or later."
        ));
    }

    let device_identifier = if let Some(id) = device_id {
        DeviceCtl::get_device(id)?.identifier
    } else {
        DeviceCtl::wait_for_device(30)?.identifier
    };

    let output = Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "uninstall",
            "app",
            "--device",
            &device_identifier,
            bundle_id,
        ])
        .output()
        .context("Failed to run devicectl uninstall")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Failed to uninstall: {}", stderr));
    }

    Ok(())
}

/// Launch an app on a connected iOS device.
///
/// Requires Xcode 15+ (uses devicectl).
pub fn launch_app(bundle_id: &str, device_id: Option<&str>) -> Result<()> {
    if !DeviceCtl::is_available() {
        return Err(anyhow!(
            "devicectl not found. Please install Xcode 15 or later."
        ));
    }

    let device_identifier = if let Some(id) = device_id {
        DeviceCtl::get_device(id)?.identifier
    } else {
        DeviceCtl::wait_for_device(30)?.identifier
    };
    DeviceCtl::launch_app(bundle_id, &device_identifier)
}

/// List connected iOS devices.
///
/// Requires Xcode 15+ (uses devicectl).
#[allow(dead_code)]
pub fn list_devices() -> Result<Vec<crate::platform::Device>> {
    if !DeviceCtl::is_available() {
        return Err(anyhow!(
            "devicectl not found. Please install Xcode 15 or later."
        ));
    }

    let devices = DeviceCtl::list_devices()?;
    Ok(devices
        .into_iter()
        .filter(|d| d.is_available())
        .map(|d| crate::platform::Device {
            id: d.udid().unwrap_or(&d.identifier).to_string(),
            name: d.name().map(|s| s.to_string()),
            device_type: crate::platform::DeviceType::Physical,
            online: true,
        })
        .collect())
}
