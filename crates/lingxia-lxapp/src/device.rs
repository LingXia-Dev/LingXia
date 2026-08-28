//! Simulated-environment control shared by the devtool (`lxdev runner`) and
//! the `lx.automation()` host tier.
//!
//! The host runner owns the device presets and the window frame; it registers
//! a [`DeviceController`] here at startup. Both automation front-ends call the
//! `device_list` / `device_get` / `device_set` helpers, so neither embeds
//! runner specifics and the two can never drift.

use std::sync::OnceLock;

/// Simulated system appearance of the runner's device screen.
///
/// `System` follows the host OS; `Light`/`Dark` pin the scheme for the
/// simulated device only. The runner applies this at the WebView host level so
/// pages observe it through `prefers-color-scheme` — never through a DOM
/// override, which belongs to apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    fn default_system() -> Self {
        Appearance::System
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }
}

impl std::str::FromStr for Appearance {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "system" => Ok(Appearance::System),
            "light" => Ok(Appearance::Light),
            "dark" => Ok(Appearance::Dark),
            other => Err(format!(
                "unknown appearance: {other} (expected system|light|dark)"
            )),
        }
    }
}

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
    /// Simulated system appearance of the device screen.
    #[serde(default = "Appearance::default_system")]
    pub appearance: Appearance,
    /// Whether the simulated host capsule is enabled. This is the setting, not
    /// per-device visibility: a desktop preset draws no phone chrome either
    /// way. Defaults to true — the capsule is real host chrome for every
    /// non-home lxapp, so hiding it is the opt-in.
    #[serde(default = "default_capsule")]
    pub capsule: bool,
}

fn default_capsule() -> bool {
    true
}

/// Host-provided controller for switching the simulated device. Implemented by
/// the runner binary (which owns the device presets and window frame) and
/// registered via [`register_device_controller`]; the `device_*` helpers call
/// through this indirection so callers stay platform-agnostic.
pub trait DeviceController: Send + Sync {
    fn list(&self) -> Result<Vec<DeviceEntry>, String>;
    fn get(&self) -> Result<DeviceState, String>;
    /// Partial update: only the provided fields change. `id: None` keeps the
    /// current preset, so orientation or appearance can flip on their own.
    fn set(
        &self,
        id: Option<&str>,
        landscape: Option<bool>,
        appearance: Option<Appearance>,
        capsule: Option<bool>,
    ) -> Result<DeviceState, String>;
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
    device_controller()?.list()
}

/// Report the currently selected device and orientation.
pub fn device_get() -> Result<DeviceState, String> {
    device_controller()?.get()
}

/// Update the simulated environment; only the provided fields change.
pub fn device_set(
    id: Option<&str>,
    landscape: Option<bool>,
    appearance: Option<Appearance>,
    capsule: Option<bool>,
) -> Result<DeviceState, String> {
    device_controller()?.set(id, landscape, appearance, capsule)
}
