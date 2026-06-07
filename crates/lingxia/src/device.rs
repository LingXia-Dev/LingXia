//! Device, screen, and system-setting APIs for native Rust code.
//!
//! These functions expose host device information and simple device actions in
//! Rust-friendly types. Platform-specific failures, unsupported capabilities,
//! and permission denials are surfaced through [`crate::Error`].
//!
//! # Example
//!
//! ```ignore
//! use lingxia::device;
//!
//! let info = device::info()?;
//! log::info!("running on {} {}", info.brand, info.model);
//! ```

use lingxia_platform::ScreenInfo;
use lingxia_platform::traits::device::Device;
use lingxia_platform::traits::location::Location;
use lingxia_platform::traits::wifi::Wifi;
use serde::Serialize;

/// Static device identity reported by the host platform.
///
/// The values are platform supplied and may be empty when a host cannot provide
/// a particular field.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub brand: String,
    pub model: String,
    pub market_name: String,
    pub os_name: String,
    pub os_version: String,
}

impl From<lingxia_platform::DeviceInfo> for DeviceInfo {
    fn from(info: lingxia_platform::DeviceInfo) -> Self {
        Self {
            brand: info.brand,
            model: info.model,
            market_name: info.market_name,
            os_name: info.os_name,
            os_version: info.os_version,
        }
    }
}

/// Coarse system toggles reported by the host platform.
///
/// Bluetooth is intentionally omitted: there is no platform trait for it.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    pub wifi_enabled: bool,
    pub location_enabled: bool,
}

/// Returns the host device's static identity (brand, model, OS, ...).
pub fn info() -> crate::Result<DeviceInfo> {
    Ok(crate::runtime::platform()?.device_info().into())
}

/// Returns the main screen geometry in logical pixels plus its scale factor.
pub fn screen() -> crate::Result<ScreenInfo> {
    Ok(crate::runtime::platform()?.screen_info())
}

/// Triggers device haptics.
///
/// `long` requests a sustained vibration where the host supports it.
pub fn vibrate(long: bool) -> crate::Result<()> {
    crate::runtime::platform()?
        .vibrate(long)
        .map_err(Into::into)
}

/// Places a phone call to `number` via the host platform.
pub fn make_phone_call(number: &str) -> crate::Result<()> {
    crate::runtime::platform()?
        .make_phone_call(number)
        .map_err(Into::into)
}

/// Reports coarse Wi-Fi and location system toggles.
pub fn system_settings() -> crate::Result<SystemSettings> {
    let runtime = crate::runtime::platform()?;
    let wifi_enabled = runtime.is_wifi_enabled().map_err(crate::Error::from)?;
    let location_enabled = runtime.is_location_enabled().map_err(crate::Error::from)?;
    Ok(SystemSettings {
        wifi_enabled,
        location_enabled,
    })
}
