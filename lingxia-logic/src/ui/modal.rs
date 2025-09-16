use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::{ModalOptions, UserFeedback};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::sync::Arc;

/// Modal options from JavaScript (compatible with WeChat mini-program API)
#[derive(FromJSObj)]
struct JSModalOptions {
    title: Option<String>,
    content: Option<String>,
    show_cancel: Option<bool>,
    cancel_text: Option<String>,
    cancel_color: Option<String>,
    confirm_text: Option<String>,
    confirm_color: Option<String>,
}

impl From<JSModalOptions> for ModalOptions {
    fn from(js_options: JSModalOptions) -> Self {
        ModalOptions {
            title: js_options.title.unwrap_or_else(|| "Title".to_string()),
            content: js_options.content.unwrap_or_default(),
            show_cancel: js_options.show_cancel.unwrap_or(true),
            cancel_text: js_options
                .cancel_text
                .unwrap_or_else(|| "Cancel".to_string()),
            cancel_color: js_options.cancel_color,
            confirm_text: js_options
                .confirm_text
                .unwrap_or_else(|| "Confirm".to_string()),
            confirm_color: js_options.confirm_color,
        }
    }
}

/// JavaScript ModalResult for return value
#[derive(Debug, Clone, IntoJSObj)]
struct JSModalResult {
    confirm: bool,
    cancel: bool,
}

impl From<CallbackResult> for JSModalResult {
    fn from(result: CallbackResult) -> Self {
        if result.success {
            // Parse JSON data for modal result
            if let Ok(modal_data) = serde_json::from_str::<serde_json::Value>(&result.data) {
                JSModalResult {
                    confirm: modal_data
                        .get("confirm")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    cancel: modal_data
                        .get("cancel")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                }
            } else {
                // Fallback: assume confirm if success
                JSModalResult {
                    confirm: true,
                    cancel: false,
                }
            }
        } else {
            // Error or cancel
            JSModalResult {
                confirm: false,
                cancel: true,
            }
        }
    }
}

/// Show modal function (async)
async fn show_modal(ctx: JSContext, options: JSModalOptions) -> JSResult<JSModalResult> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let modal_options: ModalOptions = options.into();

    // Get callback ID and receiver
    let (callback_id, receiver) = get_callback();

    // Call runtime interface with callback ID
    match lxapp.runtime.show_modal(modal_options, callback_id) {
        Ok(()) => {
            // Wait for result from callback
            match receiver.await {
                Ok(result) => Ok(result.into()),
                Err(_) => Err(RongJSError::Error(
                    "Modal callback timeout or cancelled".to_string(),
                )),
            }
        }
        Err(e) => Err(RongJSError::Error(format!("Failed to show modal: {}", e))),
    }
}

/// Initialize modal functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register showModal function
    let show_modal_func = JSFunc::new(ctx, show_modal)?;
    lx::register_js_api(ctx, "showModal", show_modal_func)?;

    Ok(())
}
