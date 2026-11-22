//! `lx.device.permission.get(name)` and `lx.device.permission.request(name)`.

use crate::i18n::js_invalid_parameter_error;
use rong::{HostError, JSContext, JSObject, JSResult, RongJSError};

const NAMES: [&str; 4] = ["location", "camera", "photos", "notifications"];

fn checked(name: &str) -> JSResult<&str> {
    if NAMES.contains(&name) {
        Ok(name)
    } else {
        Err(js_invalid_parameter_error(format!(
            "unknown permission '{name}' (expected location, camera, photos or notifications)"
        )))
    }
}

/// The hosts answer only notification permission today; the others still
/// prompt implicitly on first use and report refusal as a rejection.
fn not_yet(member: &str, name: &str) -> RongJSError {
    HostError::new(
        rong::error::E_NOT_SUPPORTED,
        format!("{member}('{name}') is not available on this host yet; the API that needs it asks when called"),
    )
    .into()
}

/// Read a permission's state without prompting, to decide before showing a
/// button that would prompt.
///
/// @example
/// ```ts
/// if (await lx.device.permission.get('notifications') === 'prompt') showEnableButton();
/// ```
/// @supports host.notification
/// @throws {E_INVALID_ARG} A name outside `PermissionName`.
/// @throws {E_NOT_SUPPORTED} `location`, `camera` or `photos`, which no host reports yet; `notifications` where the host does not declare them.
async fn permission(ctx: JSContext, name: String) -> JSResult<String> {
    match checked(&name)? {
        "notifications" => crate::app::notification_permission(ctx).await,
        other => Err(not_yet("lx.device.permission.get", other)),
    }
}

/// Ask for a permission. Prompts where the OS has a prompt and the person has
/// not answered; otherwise resolves the current setting. Rejects when the
/// prompt is left unanswered.
///
/// @permission notifications
/// @supports host.notification
/// @throws {E_INVALID_ARG} A name outside `PermissionName`.
/// @throws {E_PERMISSION_DENIED} A guest lxapp asks while it is not on screen, or a second time this launch.
/// @throws {E_NOT_SUPPORTED} `location`, `camera` or `photos`, which no host asks for yet; `notifications` where the host does not declare them.
async fn request_permission(ctx: JSContext, name: String) -> JSResult<String> {
    match checked(&name)? {
        "notifications" => crate::app::request_notification_permission(ctx).await,
        other => Err(not_yet("lx.device.permission.request", other)),
    }
}

/// Read and ask for the device permissions an lxapp needs.
fn namespace(ctx: &JSContext) -> JSResult<JSObject> {
    crate::namespace::child(ctx, &crate::namespace::device(ctx)?, "permission")
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_property(ctx)?;
    register_api(ctx)
}

rong::js_api! {
    fn register_property(ctx) {
        namespace DeviceApi = crate::namespace::device(ctx)?;
        const permission: "PermissionApi" = namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_api(ctx) {
        namespace PermissionApi = namespace(ctx)?;
        fn get(
            ts_params = "name: PermissionName",
            ts_return = "Promise<PermissionState>"
        ) = permission;
        fn request(
            ts_params = "name: PermissionName",
            ts_return = "Promise<'granted' | 'denied'>"
        ) = request_permission;
    }
}
