//! Launch-at-login registration for native hosts, including those without JS Logic.
//!
//! Requires `capabilities.autostart: true` in `lingxia.yaml`. Declaring the
//! capability never registers the app; only [`set_enabled`](crate::app::autostart::set_enabled)
//! does. Supported on Windows and macOS 13+; other hosts report unsupported.
//! Queries and updates can block on the OS. From an async route or a UI thread,
//! run them through [`crate::task::spawn_blocking`].

use lingxia_platform::traits::app_runtime::AppRuntime;

/// Whether this host declared the capability and its OS supports it.
/// Does not read the live registration or change any system setting.
pub fn is_supported() -> bool {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        lingxia_app_context::autostart_enabled() && lingxia_platform::autostart_supported()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

fn require_support() -> crate::Result<()> {
    if !lingxia_app_context::autostart_enabled() {
        return Err(crate::Error::invalid_request(
            "this host did not declare capabilities.autostart",
        ));
    }
    if !is_supported() {
        return Err(crate::Error::invalid_request(
            "autostart is not supported on this host",
        ));
    }
    Ok(())
}

/// Read the current OS registration, including changes made outside the app.
/// Unsupported hosts return an error, not a cached or assumed disabled state.
pub fn is_enabled() -> crate::Result<bool> {
    require_support()?;
    crate::runtime::platform()?
        .autostart_is_enabled()
        .map_err(crate::Error::from)
}

/// Register or unregister this host as a per-user startup item. Idempotent.
/// The OS may still require user approval; read [`is_enabled`] afterwards
/// to display the effective state instead of assuming the requested value.
pub fn set_enabled(enabled: bool) -> crate::Result<()> {
    require_support()?;
    crate::runtime::platform()?
        .autostart_set_enabled(enabled)
        .map_err(crate::Error::from)
}
