#[cfg(not(target_os = "macos"))]
use lingxia_platform::traits::ui::{ToastIcon, ToastOptions, ToastPosition, UserFeedback};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, HostError, JSContext, JSFunc, JSResult};

/// Toast options from JavaScript
#[derive(FromJSObj)]
struct JSToastOptions {
    title: String,
    icon: Option<String>,
    image: Option<String>,
    duration: Option<f64>,
    mask: Option<bool>,
    position: Option<String>,
}

#[cfg(not(target_os = "macos"))]
fn convert_string_to_toast_icon(icon: &str) -> ToastIcon {
    match icon.to_lowercase().as_str() {
        "success" => ToastIcon::Success,
        "error" => ToastIcon::Error,
        "loading" => ToastIcon::Loading,
        "none" => ToastIcon::None,
        _ => ToastIcon::None,
    }
}

#[cfg(not(target_os = "macos"))]
fn convert_string_to_toast_position(position: &str) -> ToastPosition {
    match position.to_lowercase().as_str() {
        "top" => ToastPosition::Top,
        "bottom" => ToastPosition::Bottom,
        "center" => ToastPosition::Center,
        _ => ToastPosition::Center,
    }
}

#[cfg(not(target_os = "macos"))]
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
        return Err(
            HostError::new(rong::error::E_INTERNAL, "LxApp is closed; toast suppressed").into(),
        );
    }

    #[cfg(target_os = "macos")]
    {
        let params = serde_json::json!({
            "title": options.title,
            "icon": options.icon.as_deref().unwrap_or("none"),
            "image": options.image,
            "duration": options.duration.unwrap_or(1500.0),
            "mask": options.mask.unwrap_or(false),
            "position": options.position.as_deref().unwrap_or("center"),
        });

        lxapp
            .call_current_page_view("ui.showToast", Some(params))
            .await
            .map_err(|e| {
                HostError::new(
                    rong::error::E_INTERNAL,
                    format!("WebView toast failed: {}", e),
                )
            })?;

        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let toast_options: ToastOptions = options.into();
        lxapp.runtime.show_toast(toast_options).map_err(|e| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to show toast: {}", e),
            )
        })?;

        Ok(())
    }
}

/// Hide toast function
async fn hide_toast(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    #[cfg(target_os = "macos")]
    {
        lxapp
            .call_current_page_view("ui.hideToast", None)
            .await
            .map_err(|e| {
                HostError::new(
                    rong::error::E_INTERNAL,
                    format!("WebView hideToast failed: {}", e),
                )
            })?;

        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        lxapp.runtime.hide_toast().map_err(|e| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to hide toast: {}", e),
            )
        })?;

        Ok(())
    }
}

/// Initialize toast functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register showToast function
    let show_toast_func = JSFunc::new(ctx, show_toast)?;
    lx::register_js_api(ctx, "showToast", show_toast_func)?;

    // Register hideToast function
    let hide_toast_func = JSFunc::new(ctx, hide_toast)?;
    lx::register_js_api(ctx, "hideToast", hide_toast_func)?;

    Ok(())
}
