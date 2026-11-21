//! Display and screen orientation APIs.

use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::device::Device;
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::{
    LxApp, OrientationConfig, publish_app_event, register_app_handler, unregister_app_handler_token,
};
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};
use std::cell::Cell;

const DEVICE_ORIENTATION_CHANGE_EVENT: &str = "DeviceOrientationChange";
const ORIENTATION_PORTRAIT: &str = "portrait";
const ORIENTATION_LANDSCAPE: &str = "landscape";

fn normalize_orientation_value(value: &str) -> Option<&'static str> {
    match value {
        "portrait" | "reverse-portrait" => Some(ORIENTATION_PORTRAIT),
        "landscape" | "reverse-landscape" => Some(ORIENTATION_LANDSCAPE),
        _ => None,
    }
}

fn emit_orientation_change_event(appid: &str, value: &str) {
    let payload = format!(r#"{{"value":"{}"}}"#, value);
    let _ = publish_app_event(appid, DEVICE_ORIENTATION_CHANGE_EVENT, Some(payload));
}

#[inline]
fn should_emit_orientation_event_after_set() -> bool {
    // iOS/Harmony may not deliver a host orientation callback immediately after
    // orientation.set, so we actively emit one to keep JS state in sync.
    // Android already emits orientation events from Activity callbacks; emitting
    // again here would create duplicate events.
    cfg!(target_os = "ios") || cfg!(target_os = "macos") || cfg!(target_env = "ohos")
}

/// The orientation the current page resolves to: the lock in effect, or the
/// screen's shape when nothing locks it.
fn current_orientation(lxapp: &LxApp) -> &'static str {
    let current_path = lxapp.peek_current_page_path().unwrap_or_default();
    let current = if current_path.is_empty() {
        lxapp.get_app_orientation()
    } else {
        lxapp.get_page_orientation(&current_path)
    };
    normalize_orientation_value(current.to_label()).unwrap_or_else(|| {
        let screen = lxapp.runtime.screen_info();
        if screen.width > screen.height {
            ORIENTATION_LANDSCAPE
        } else {
            ORIENTATION_PORTRAIT
        }
    })
}

/// Lock this lxapp to `portrait` or `landscape`; `auto` removes the lock.
///
/// The lock beats every page's `pageOrientation`; only a full-screen video
/// beats the lock. With it removed, each page's own setting applies again.
///
/// @throws {E_INVALID_ARG} Any other value.
fn set_device_orientation(ctx: JSContext, orientation: String) -> JSResult<()> {
    let lock = match orientation.as_str() {
        "auto" => None,
        "portrait" | "landscape" => OrientationConfig::from_label(&orientation),
        _ => {
            return Err(js_invalid_parameter_error(format!(
                "Invalid orientation value: {orientation} (expected portrait, landscape or auto)"
            )));
        }
    };

    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp.set_app_orientation(lock);

    lxapp
        .runtime
        .update_orientation_ui(lxapp.appid.clone())
        .map_err(|e| js_error_from_platform_error(&e))?;

    if should_emit_orientation_event_after_set() {
        emit_orientation_change_event(&lxapp.appid, current_orientation(&lxapp));
    }

    Ok(())
}

/// Delivers the orientation in effect before returning, then each change: the
/// lock (not how the device is held), or the screen's shape when unlocked.
fn on_device_orientation_change(
    ctx: JSContext,
    callback: JSFunc,
    options: Optional<JSValue>,
) -> JSResult<JSFunc> {
    let scope = ctx.clone();
    lxapp::lx::subscribe(&scope, options.0, move || {
        let lxapp = LxApp::from_ctx(&ctx)?;
        let value = current_orientation(&lxapp);

        let token = register_app_handler(&ctx, DEVICE_ORIENTATION_CHANGE_EVENT, callback.clone())?;

        let payload = JSObject::new(&ctx);
        payload.set("value", value)?;
        let _ = callback.call::<_, ()>(None, (payload,));

        let off_ctx = ctx.clone();
        let unsubscribed = Cell::new(false);
        JSFunc::new(&ctx, move || {
            if unsubscribed.replace(true) {
                return;
            }
            unregister_app_handler_token(&off_ctx, DEVICE_ORIENTATION_CHANGE_EVENT, token);
        })
    })
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace OrientationApi = crate::namespace::orientation(ctx)?;
        fn set(ts_params = "orientation: DeviceOrientation | 'auto'", ts_return = "void") = set_device_orientation;
        fn watch(
            ts_params = "callback: (event: DeviceOrientationChangeEvent) => void, options?: SignalOptions",
            ts_return = "() => void"
        ) = on_device_orientation_change;
    }
}
