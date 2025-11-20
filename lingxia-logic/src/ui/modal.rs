use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::{ModalOptions, UserFeedback};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::Value;

/// Modal options from JavaScript (compatible with WeChat mini-program API)
#[derive(FromJSObj)]
struct JSModalOptions {
    title: Option<String>,
    content: Option<String>,
    #[rename = "showCancel"]
    show_cancel: Option<bool>,
    #[rename = "cancelText"]
    cancel_text: Option<String>,
    #[rename = "cancelColor"]
    cancel_color: Option<String>,
    #[rename = "confirmText"]
    confirm_text: Option<String>,
    #[rename = "confirmColor"]
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
        if !result.success {
            return JSModalResult {
                confirm: false,
                cancel: true,
            };
        }

        match serde_json::from_str::<Value>(&result.data) {
            Ok(json) => JSModalResult {
                confirm: json
                    .get("confirm")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                cancel: json.get("cancel").and_then(Value::as_bool).unwrap_or(false),
            },
            Err(_) => JSModalResult {
                confirm: true,
                cancel: false,
            },
        }
    }
}

/// Show modal function (async)
async fn show_modal(ctx: JSContext, options: JSModalOptions) -> JSResult<JSModalResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let modal_options: ModalOptions = options.into();

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(RongJSError::Error(
            "LxApp is closed; modal suppressed".to_string(),
        ));
    }

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
