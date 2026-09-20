#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_error_from_platform_error;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::i18n::js_internal_error;
use crate::i18n::js_service_unavailable_error;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::traits::ui::{ToastIcon, ToastOptions, ToastPosition, UserFeedback};
use lxapp::LxApp;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use rong::JSContextService;
use rong::{FromJSObject, JSContext, JSFunc, JSObject, JSResult};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// The generation guard has to be scoped exactly like the presenter it guards.
// Desktop presents inside the calling lxapp's own WebView, so one counter per
// JS context *is* one lxapp's toast. The mobile hosts share a single
// process-wide overlay, so a toast from any lxapp replaces what is on screen
// and must invalidate every older handle — a per-context counter there would
// let a retained handle dismiss another lxapp's newer toast.
#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Default)]
struct ToastState(Arc<AtomicU64>);
#[cfg(any(target_os = "macos", target_os = "windows"))]
impl JSContextService for ToastState {}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn toast_generation(ctx: &JSContext) -> Arc<AtomicU64> {
    if ctx.get_service::<ToastState>().is_none() {
        ctx.set_service(ToastState::default());
    }
    ctx.get_service::<ToastState>()
        .expect("toast state inserted")
        .0
        .clone()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn toast_generation(_ctx: &JSContext) -> Arc<AtomicU64> {
    static GENERATION: std::sync::LazyLock<Arc<AtomicU64>> =
        std::sync::LazyLock::new(|| Arc::new(AtomicU64::new(0)));
    GENERATION.clone()
}

/// Toast options from JavaScript
#[derive(FromJSObject)]
#[ts_skip]
struct JSToastOptions {
    title: String,
    icon: Option<String>,
    image: Option<String>,
    #[js_name = "durationMs"]
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

/// Presents a toast and resolves a handle once the host accepted it. The handle
/// dismisses only this toast, never a newer one — including a newer one posted
/// by another lxapp onto a host's shared overlay.
async fn show_toast(ctx: JSContext, options: JSToastOptions) -> JSResult<JSObject> {
    let state = toast_generation(&ctx);
    let previous = state.fetch_add(1, Ordering::SeqCst);
    let generation = previous + 1;
    // Invalidate older handles before dispatch so they cannot hide a pending replacement.
    if let Err(error) = present_toast(ctx.clone(), options).await {
        let _ = state.compare_exchange(generation, previous, Ordering::SeqCst, Ordering::SeqCst);
        return Err(error);
    }
    let handle = JSObject::new(&ctx);
    handle.set(
        "dismiss",
        JSFunc::new(&ctx, move |ctx: JSContext| {
            let state = state.clone();
            async move {
                if state
                    .compare_exchange(
                        generation,
                        generation + 1,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    )
                    .is_ok()
                {
                    hide_toast(ctx).await?;
                }
                Ok::<(), rong::RongJSError>(())
            }
        })?,
    )?;
    Ok(handle)
}

/// Resolves after the host accepts presentation, not after the toast expires.
async fn present_toast(ctx: JSContext, options: JSToastOptions) -> JSResult<()> {
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

/// Hides whichever toast is showing.
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
        fn showToast(ts_params = "options: ShowToastOptions", ts_return = "ToastHandle") = show_toast;
        fn hideToast(ts_return = "void") = hide_toast;
    }
}
