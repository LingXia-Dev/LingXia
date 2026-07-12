#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_error_from_platform_error;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::i18n::js_internal_error;
use crate::i18n::js_service_unavailable_error;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::traits::ui::{ToastIcon, ToastOptions, ToastPosition, UserFeedback};
use lxapp::LxApp;
use rong::{FromJSObject, JSContext, JSResult};

/// Toast options from JavaScript
#[derive(FromJSObject)]
#[ts_skip]
struct JSToastOptions {
    title: String,
    icon: Option<String>,
    image: Option<String>,
    duration: Option<f64>,
    mask: Option<bool>,
    position: Option<String>,
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn convert_string_to_toast_icon(icon: &str) -> ToastIcon {
    match icon.to_lowercase().as_str() {
        "success" => ToastIcon::Success,
        "error" => ToastIcon::Error,
        "loading" => ToastIcon::Loading,
        "none" => ToastIcon::None,
        _ => ToastIcon::None,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn convert_string_to_toast_position(position: &str) -> ToastPosition {
    match position.to_lowercase().as_str() {
        "top" => ToastPosition::Top,
        "bottom" => ToastPosition::Bottom,
        "center" => ToastPosition::Center,
        _ => ToastPosition::Center,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl From<JSToastOptions> for ToastOptions {
    fn from(js_options: JSToastOptions) -> Self {
        // Convert duration from milliseconds (JS) to seconds (native platforms)
        let duration_seconds = js_options.duration.unwrap_or(1500.0) / 1000.0;

        ToastOptions {
            title: js_options.title,
            icon: convert_string_to_toast_icon(js_options.icon.as_deref().unwrap_or("none")),
            image: js_options.image.filter(|s| !s.is_empty()),
            duration: duration_seconds,
            mask: js_options.mask.unwrap_or(false),
            position: convert_string_to_toast_position(
                js_options.position.as_deref().unwrap_or("center"),
            ),
        }
    }
}

/// Show toast function
async fn show_toast(ctx: JSContext, options: JSToastOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(js_service_unavailable_error(
            "LxApp is closed; toast suppressed",
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let params = serde_json::json!({
            "title": options.title,
            "icon": options.icon.as_deref().unwrap_or("none"),
            "image": options.image,
            "duration": options.duration.unwrap_or(1500.0),
            "mask": options.mask.unwrap_or(false),
            "position": options.position.as_deref().unwrap_or("center"),
        });

        let _: () = lxapp
            .call_view_with("ui.showToast", &params)
            .await
            .map_err(|e| js_internal_error(format!("WebView toast failed: {}", e)))?;

        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let toast_options: ToastOptions = options.into();
        lxapp
            .runtime
            .show_toast(toast_options)
            .map_err(|e| js_error_from_platform_error(&e))?;

        Ok(())
    }
}

/// Hide toast function
async fn hide_toast(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    if !lxapp.is_opened() {
        return Err(js_service_unavailable_error(
            "LxApp is closed; hideToast suppressed",
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let _: () = lxapp
            .call_view("ui.hideToast")
            .await
            .map_err(|e| js_internal_error(format!("WebView hideToast failed: {}", e)))?;

        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        lxapp
            .runtime
            .hide_toast()
            .map_err(|e| js_error_from_platform_error(&e))?;

        Ok(())
    }
}

/// Initialize toast functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn showToast(ts_params = "options: ShowToastOptions", ts_return = "void") = show_toast;
        fn hideToast(ts_return = "void") = hide_toast;
    }
}
