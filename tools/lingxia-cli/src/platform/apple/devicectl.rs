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
    /// Fail fast with a fix when the active developer directory has no devicectl.
    pub fn ensure_available() -> Result<()> {
        let found = Command::new("xcrun")
            .args(["--find", "devicectl"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if found {
            return Ok(());
        }
        let dir = super::active_developer_dir().unwrap_or_else(|| "not set".to_string());
        Err(anyhow!(
            "devicectl not found: active developer directory is {dir}.\n\
             Installing to iOS devices requires full Xcode 15+.\n{}",
            super::select_xcode_hint()
        ))
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
        Self::ensure_available()?;
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_seconds as u64);

        println!(
            "Waiting for device to connect (timeout: {}s)...",
            timeout_seconds
        );

        loop {
            // Only an empty list is worth waiting on; a devicectl failure won't heal by polling.
            if let Some(device) = Self::list_devices()?.into_iter().find(|d| d.is_available()) {
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
    pub fn launch_app(bundle_id: &str, device_identifier: &str, restart: bool) -> Result<()> {
        println!("{}", "Launching app...".cyan());

        let mut cmd = Command::new("xcrun");
        cmd.args([
            "devicectl",
            "device",
            "process",
            "launch",
            "--device",
            device_identifier,
        ]);
        if restart {
            cmd.arg("--terminate-existing");
        }
        cmd.arg(bundle_id);

        let output = cmd.output().context("Failed to launch app")?;

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

#[derive(Debug, Deserialize)]
struct AppListOutput {
    result: AppListResult,
}

#[derive(Debug, Deserialize)]
struct AppListResult {
    apps: Vec<InstalledApp>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstalledApp {
    bundle_identifier: String,
}

// =============================================================================
// Unified API (requires Xcode 15+ for devicectl)
// =============================================================================

/// Install an app to a connected iOS device.
///
/// Requires Xcode 15+ (uses devicectl).
pub fn install_app(app_path: &Path, device_id: Option<&str>) -> Result<()> {
    DeviceCtl::ensure_available()?;

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
    DeviceCtl::ensure_available()?;

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
/// `bundle_id` is the canonical bundle id from lingxia.yaml. Xcode builds
/// can apply an env suffix (`.dev`) — auto-detect which
/// variant is installed and target that, so callers don't have to track
/// env state.
///
/// Requires Xcode 15+ (uses devicectl).
pub fn launch_app(bundle_id: &str, device_id: Option<&str>, restart: bool) -> Result<()> {
    DeviceCtl::ensure_available()?;

    let device_identifier = if let Some(id) = device_id {
        DeviceCtl::get_device(id)?.identifier
    } else {
        DeviceCtl::wait_for_device(30)?.identifier
    };
    let installed_id =
        resolve_installed_bundle_id(bundle_id, &device_identifier)?.ok_or_else(|| {
            anyhow!(
                "No installed app matching {bundle_id} (or {bundle_id}.<env>) on device. \
                 Run `lingxia install` first."
            )
        })?;
    DeviceCtl::launch_app(&installed_id, &device_identifier, restart)
}

/// Whether exactly `bundle_id` (env suffix included) is installed on the device.
pub fn app_installed(bundle_id: &str, device_id: Option<&str>) -> Result<bool> {
    DeviceCtl::ensure_available()?;
    let device_identifier = if let Some(id) = device_id {
        DeviceCtl::get_device(id)?.identifier
    } else {
        DeviceCtl::wait_for_device(30)?.identifier
    };
    Ok(installed_bundle_ids(&device_identifier)?
        .iter()
        .any(|installed| installed == bundle_id))
}

/// Pick the installed bundle id matching `base_id` (canonical) or
/// `base_id.<single-segment>` (env-suffixed via Xcode build settings).
/// When several coexist, prefer the canonical id.
fn resolve_installed_bundle_id(base_id: &str, device_identifier: &str) -> Result<Option<String>> {
    let prefix = format!("{base_id}.");
    let mut matches: Vec<String> = installed_bundle_ids(device_identifier)?
        .into_iter()
        .filter(|pkg| {
            pkg == base_id
                || pkg
                    .strip_prefix(&prefix)
                    .map(|rest| !rest.is_empty() && !rest.contains('.'))
                    .unwrap_or(false)
        })
        .collect();
    if matches.is_empty() {
        return Ok(None);
    }
    if let Some(idx) = matches.iter().position(|p| p == base_id) {
        return Ok(Some(matches.swap_remove(idx)));
    }
    if matches.len() > 1 {
        eprintln!(
            "{} multiple installed variants of {base_id}: {:?} — picking {}",
            "warn:".yellow(),
            matches,
            matches[0]
        );
    }
    Ok(Some(matches.remove(0)))
}

fn installed_bundle_ids(device_identifier: &str) -> Result<Vec<String>> {
    let output = Command::new("xcrun")
        .args([
            "devicectl",
            "device",
            "info",
            "apps",
            "--device",
            device_identifier,
            "--json-output",
            "-",
        ])
        .output()
        .context("Failed to run devicectl info apps")?;
    if !output.status.success() {
        return Err(anyhow!(
            "devicectl info apps failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let parsed: AppListOutput = serde_json::from_slice(&output.stdout)
        .context("Failed to parse devicectl info apps JSON")?;
    Ok(parsed
        .result
        .apps
        .into_iter()
        .map(|a| a.bundle_identifier)
        .collect())
}

/// List connected iOS devices.
///
/// Requires Xcode 15+ (uses devicectl).
#[allow(dead_code)]
pub fn list_devices() -> Result<Vec<crate::platform::Device>> {
    DeviceCtl::ensure_available()?;

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

/// Whether the device reaches this Mac by cable only: devicectl says wired and
/// its pairing service shows up on no interface of the Mac's own networks.
pub fn cable_only(device_id: Option<&str>) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    let device = match device_id {
        Some(id) => DeviceCtl::get_device(id).ok(),
        None => DeviceCtl::list_devices()
            .ok()
            .and_then(|devices| devices.into_iter().find(|d| d.is_available())),
    };
    if !device.is_some_and(|d| d.is_usb()) {
        return false;
    }
    seen_only_over_cable(
        &pairing_sightings(std::time::Duration::from_secs(2)),
        &lan_interfaces(),
    )
}

/// `(interface index, instance)` sightings: a pairing instance that appears
/// off every LAN interface is a cabled phone; one also on a LAN interface is on Wi-Fi.
pub(crate) fn seen_only_over_cable(
    sightings: &[(u32, String)],
    lan: &std::collections::HashSet<u32>,
) -> bool {
    let on_lan = |name: &str| {
        sightings
            .iter()
            .any(|(index, other)| other == name && lan.contains(index))
    };
    let mut cabled = sightings
        .iter()
        .filter(|(index, _)| !lan.contains(index))
        .peekable();
    cabled.peek().is_some() && cabled.all(|(_, name)| !on_lan(name))
}

/// One `dns-sd -B` result line as `(interface index, instance)`.
pub(crate) fn parse_browse_line(line: &str) -> Option<(u32, String)> {
    let mut fields = line.split_whitespace();
    let _time = fields.next()?;
    if fields.next()? != "Add" {
        return None;
    }
    let _flags = fields.next()?;
    let index = fields.next()?.parse().ok()?;
    let _domain = fields.next()?;
    let _service = fields.next()?;
    let instance = fields.collect::<Vec<_>>().join(" ");
    (!instance.is_empty()).then_some((index, instance))
}

/// Browses `_remotepairing._tcp`, which a paired iPhone advertises on every link it shares with this Mac.
fn pairing_sightings(window: std::time::Duration) -> Vec<(u32, String)> {
    use std::io::BufRead;
    let Ok(mut child) = Command::new("dns-sd")
        .args(["-B", "_remotepairing._tcp", "local."])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        stdout
            .map(|out| {
                std::io::BufReader::new(out)
                    .lines()
                    .map_while(Result::ok)
                    .filter_map(|line| parse_browse_line(&line))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    std::thread::sleep(window);
    let _ = child.kill();
    let _ = child.wait();
    reader.join().unwrap_or_default()
}

/// Indices of interfaces holding a routable IPv4 address: the Mac's own networks,
/// not the link-local USB link to a phone.
#[cfg(target_os = "macos")]
fn lan_interfaces() -> std::collections::HashSet<u32> {
    let mut indices = std::collections::HashSet::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills a list we only read and then free.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return indices;
        }
        let mut cursor = head;
        while let Some(entry) = cursor.as_ref() {
            if let Some(addr) = entry.ifa_addr.as_ref()
                && i32::from(addr.sa_family) == libc::AF_INET
            {
                let sin = &*(entry.ifa_addr as *const libc::sockaddr_in);
                let ip = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                let index = libc::if_nametoindex(entry.ifa_name);
                if !ip.is_loopback() && !ip.is_link_local() && index != 0 {
                    indices.insert(index);
                }
            }
            cursor = entry.ifa_next;
        }
        libc::freeifaddrs(head);
    }
    indices
}

#[cfg(not(target_os = "macos"))]
fn lan_interfaces() -> std::collections::HashSet<u32> {
    std::collections::HashSet::new()
}
