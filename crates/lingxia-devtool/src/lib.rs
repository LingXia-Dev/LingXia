//! Native entry library for LingXia development tools.
//!
//! This crate owns devtool-specific bootstrap behavior for Apple hosts such as
//! LingXia Runner while reusing the shared LingXia runtime.

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use lingxia::apple::*;

const HOME_LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";

struct DevtoolAddon;

impl lingxia::HostAddon for DevtoolAddon {
    fn before_init(&self) {
        let Ok(raw_path) = std::env::var(HOME_LXAPP_PATH_ENV) else {
            return;
        };

        let path = raw_path.trim();
        if path.is_empty() {
            log::warn!("{HOME_LXAPP_PATH_ENV} is set but empty; ignoring");
            return;
        }

        if lxapp::configure_home_lxapp_dev_path(path).is_none() {
            log::warn!(
                "Failed to initialize home lxapp dev path from {}={}",
                HOME_LXAPP_PATH_ENV,
                path
            );
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_install_host_addon() {
    lingxia::install_host_addon(Box::new(DevtoolAddon));
}
