#![cfg_attr(target_os = "windows", allow(dead_code))]

use lingxia_app_context::{HostBuild, capability};

pub const CAP_BROWSER: u32 = 0x1;
pub const CAP_NOTIFICATIONS: u32 = 0x2;
pub const CAP_TERMINAL: u32 = 0x4;
pub const CAP_PROXY: u32 = 0x8;

/// The build half of every host capability, recorded into the app context at
/// boot so `lx.supports()` and this bitmask answer from one source.
pub(crate) fn host_build() -> HostBuild {
    HostBuild {
        browser: cfg!(feature = "browser-shell"),
        terminal: cfg!(feature = "terminal-runtime"),
        proxy: cfg!(feature = "proxy"),
    }
}

/// The native SDKs' view of the same capability registry, encoded as bits.
pub(crate) fn app_capabilities() -> u32 {
    // The bits used to be compile-time constants; they now read state recorded
    // during `init_with_platform`. Every SDK asks after init returns, so this
    // only fires if a future caller asks earlier — and answering 0 silently
    // would look like a host that simply has no capabilities.
    debug_assert!(
        lingxia_app_context::host_build_recorded(),
        "app_capabilities() read before set_host_build()"
    );
    let mut caps = 0;
    if capability::build::browser() {
        caps |= CAP_BROWSER;
    }
    if capability::notifications() {
        caps |= CAP_NOTIFICATIONS;
    }
    if capability::build::terminal() {
        caps |= CAP_TERMINAL;
    }
    if capability::build::proxy() {
        caps |= CAP_PROXY;
    }
    caps
}
