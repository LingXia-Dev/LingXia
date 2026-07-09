//! Simulated-device control shared by the devtool (`lxdev lxapp device`) and
//! the `lx.automation()` host tier.
//!
//! The host runner owns the device presets and the window frame; it registers
//! a [`DeviceController`] here at startup. Both automation front-ends call the
//! `device_list` / `device_get` / `device_set` helpers, so neither embeds
//! runner specifics and the two can never drift.

use std::sync::OnceLock;

/// A device preset the host runner can simulate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceEntry {
    /// Stable preset id (e.g. "iphone-15-pro").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Form-factor group ("phone" | "tablet" | "desktop").
    pub group: String,
    /// Logical width in points.
    pub width: u32,
    /// Logical height in points.
    pub height: u32,
    /// True for the currently selected device.
    pub current: bool,
}

/// The active device selection reported by the host runner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceState {
    /// Selected preset id.
    pub id: String,
    /// Selected preset name.
    pub name: String,
    /// Form-factor group.
    pub group: String,
    /// Logical width in points (accounts for orientation).
    pub width: u32,
    /// Logical height in points (accounts for orientation).
    pub height: u32,
    /// True when the device is rotated to landscape.
    pub landscape: bool,
}

/// Host-provided controller for switching the simulated device. Implemented by
/// the runner binary (which owns the device presets and window frame) and
/// registered via [`register_device_controller`]; the `device_*` helpers call
/// through this indirection so callers stay platform-agnostic.
pub trait DeviceController: Send + Sync {
    fn list(&self) -> Vec<DeviceEntry>;
    fn get(&self) -> DeviceState;
    fn set(&self, id: &str, landscape: Option<bool>) -> Result<DeviceState, String>;
}

static DEVICE_CONTROLLER: OnceLock<Box<dyn DeviceController>> = OnceLock::new();

/// Registers the host device controller for this process. First registration
/// wins; later ones are ignored.
pub fn register_device_controller(controller: Box<dyn DeviceController>) {
    if DEVICE_CONTROLLER.set(controller).is_err() {
        crate::warn!("device controller already registered; ignoring");
    }
}

fn device_controller() -> Result<&'static dyn DeviceController, String> {
    DEVICE_CONTROLLER
        .get()
        .map(|c| c.as_ref())
        .ok_or_else(|| "device switching is not supported by this host".to_string())
}

/// List the device presets the host runner offers.
pub fn device_list() -> Result<Vec<DeviceEntry>, String> {
    Ok(device_controller()?.list())
}

/// Report the currently selected device and orientation.
pub fn device_get() -> Result<DeviceState, String> {
    Ok(device_controller()?.get())
}

/// Switch the simulated device by preset id and/or orientation.
pub fn device_set(id: &str, landscape: Option<bool>) -> Result<DeviceState, String> {
    device_controller()?.set(id, landscape)
}
