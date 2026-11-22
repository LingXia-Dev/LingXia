//! Display and screen orientation APIs.

use crate::i18n::{js_error_from_platform_error, js_invalid_parameter_error};
use lingxia_platform::traits::device::Device;
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::{
    LxApp, OrientationConfig, publish_app_event, register_app_handler, unregister_app_handler_token,
};
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue, function::Optional};
use std::cell::Cell;
use std::rc::Rc;

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
    // orientation.lock, so we actively emit one to keep JS state in sync.
    // Android already emits orientation events from Activity callbacks; emitting
    // again here would create duplicate events.
    cfg!(target_os = "ios") || cfg!(target_os = "macos") || cfg!(target_env = "ohos")
}

/// One watcher's view of the stream: a value passes only when it differs from
/// the last one that watcher was given.
struct DeliveredOrientation(Cell<&'static str>);

impl DeliveredOrientation {
    fn admit(&self, reported: &str) -> Option<&'static str> {
        let value = normalize_orientation_value(reported)?;
        (self.0.replace(value) != value).then_some(value)
    }
}

/// The lock or declared orientation the current page resolves to; `None` when
/// it follows the screen.
fn resolved_orientation(lxapp: &LxApp) -> Option<&'static str> {
    let current_path = lxapp.peek_current_page_path().unwrap_or_default();
    let current = if current_path.is_empty() {
        lxapp.get_app_orientation()
    } else {
        lxapp.get_page_orientation(&current_path)
    };
    normalize_orientation_value(current.to_label())
}

/// The orientation the current page resolves to: the lock in effect, or the
/// screen's shape when nothing locks it.
fn current_orientation(lxapp: &LxApp) -> &'static str {
    resolved_orientation(lxapp).unwrap_or_else(|| {
        let screen = lxapp.runtime.screen_info();
        if screen.width > screen.height {
            ORIENTATION_LANDSCAPE
        } else {
            ORIENTATION_PORTRAIT
        }
    })
}

/// The orientation in effect: the lock (not how the device is held), or the
/// screen's shape when unlocked.
fn get_device_orientation(ctx: JSContext) -> JSResult<String> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    Ok(current_orientation(&lxapp).to_string())
}

/// Lock this lxapp to `portrait` or `landscape`.
///
/// The lock beats every page's `pageOrientation`; only a full-screen video
/// beats the lock. `unlock()` removes it.
///
/// @throws {E_INVALID_ARG} Any other value.
fn lock_device_orientation(ctx: JSContext, orientation: String) -> JSResult<()> {
    let lock = match orientation.as_str() {
        "portrait" | "landscape" => OrientationConfig::from_label(&orientation),
        _ => {
            return Err(js_invalid_parameter_error(format!(
                "Invalid orientation value: {orientation} (expected portrait or landscape)"
            )));
        }
    };
    apply_orientation_lock(&ctx, lock)
}

/// Remove the lock `lock` placed: each page's `pageOrientation`, then the
/// app's default, applies again.
fn unlock_device_orientation(ctx: JSContext) -> JSResult<()> {
    apply_orientation_lock(&ctx, None)
}

fn apply_orientation_lock(ctx: &JSContext, lock: Option<OrientationConfig>) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp.set_app_orientation(lock);

    lxapp
        .runtime
        .update_orientation_ui(lxapp.appid.clone())
        .map_err(|e| js_error_from_platform_error(&e))?;

    // A page that follows the screen waits for the host's report: the screen
    // size read here predates the window turning.
    if should_emit_orientation_event_after_set()
        && let Some(value) = resolved_orientation(&lxapp)
    {
        emit_orientation_change_event(&lxapp.appid, value);
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

        let delivered = Rc::new(DeliveredOrientation(Cell::new(value)));
        let user = callback.clone();
        let filter = JSFunc::new(&ctx, move |event: JSObject| {
            let reported = event.get::<_, String>("value").unwrap_or_default();
            if let Some(value) = delivered.admit(&reported) {
                let _ = user.call::<_, ()>(None, (value,));
            }
        })?;
        let token = register_app_handler(&ctx, DEVICE_ORIENTATION_CHANGE_EVENT, filter)?;

        let _ = callback.call::<_, ()>(None, (value,));

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
        fn get(ts_return = "DeviceOrientation") = get_device_orientation;
        fn lock(ts_params = "orientation: DeviceOrientation", ts_return = "void") = lock_device_orientation;
        fn unlock(ts_return = "void") = unlock_device_orientation;
        fn watch(
            ts_params = "callback: (value: DeviceOrientation) => void, options?: SignalOptions",
            ts_return = "() => void"
        ) = on_device_orientation_change;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deliver(initial: &'static str, reports: &[&str]) -> Vec<&'static str> {
        let delivered = DeliveredOrientation(Cell::new(initial));
        reports.iter().filter_map(|r| delivered.admit(r)).collect()
    }

    #[test]
    fn a_report_repeating_the_lock_reaches_no_watcher() {
        assert!(deliver("landscape", &["landscape", "landscape"]).is_empty());
        assert_eq!(
            deliver("portrait", &["landscape", "landscape"]),
            vec!["landscape"]
        );
    }

    #[test]
    fn a_stale_shape_after_unlock_is_dropped_and_the_turn_delivered_once() {
        assert_eq!(
            deliver("landscape", &["landscape", "portrait", "portrait"]),
            vec!["portrait"]
        );
        assert_eq!(
            deliver("portrait", &["reverse-portrait", "reverse-landscape"]),
            vec!["landscape"]
        );
        assert!(deliver("portrait", &["upside-down"]).is_empty());
    }
}
