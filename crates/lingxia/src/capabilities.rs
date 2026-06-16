#![cfg_attr(target_os = "windows", allow(dead_code))]

pub const CAP_SHELL: u32 = 0x1;
pub const CAP_NOTIFICATIONS: u32 = 0x2;

pub(crate) fn app_capabilities() -> u32 {
    let mut caps = 0;
    if shell_enabled() {
        caps |= CAP_SHELL;
    }
    if notifications_supported() && lingxia_app_context::notifications_enabled() {
        caps |= CAP_NOTIFICATIONS;
    }
    caps
}

fn shell_enabled() -> bool {
    cfg!(feature = "shell-runtime")
}

fn notifications_supported() -> bool {
    cfg!(any(target_os = "ios", target_env = "ohos"))
}
