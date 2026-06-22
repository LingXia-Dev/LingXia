#![cfg_attr(target_os = "windows", allow(dead_code))]

pub const CAP_BROWSER: u32 = 0x1;
pub const CAP_NOTIFICATIONS: u32 = 0x2;
pub const CAP_TERMINAL: u32 = 0x4;
pub const CAP_PROXY: u32 = 0x8;

pub(crate) fn app_capabilities() -> u32 {
    let mut caps = 0;
    if browser_enabled() {
        caps |= CAP_BROWSER;
    }
    if notifications_supported() && lingxia_app_context::notifications_enabled() {
        caps |= CAP_NOTIFICATIONS;
    }
    if terminal_enabled() {
        caps |= CAP_TERMINAL;
    }
    if proxy_enabled() {
        caps |= CAP_PROXY;
    }
    caps
}

fn browser_enabled() -> bool {
    cfg!(feature = "browser-shell")
}

fn terminal_enabled() -> bool {
    cfg!(feature = "terminal-runtime")
}

fn proxy_enabled() -> bool {
    cfg!(feature = "proxy")
}

fn notifications_supported() -> bool {
    cfg!(any(target_os = "ios", target_env = "ohos"))
}
