use crate::{I18nKey, i18n::t};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::{ModalOptions, UserFeedback};
use lxapp::{LxApp, lx};
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

impl JSModalOptions {
    fn into_modal_options(self) -> ModalOptions {
        ModalOptions {
            title: self.title.unwrap_or_default(),
            content: self.content.unwrap_or_default(),
            show_cancel: self.show_cancel.unwrap_or(true),
            cancel_text: self.cancel_text.unwrap_or_else(|| t(I18nKey::CommonCancel)),
            cancel_color: self.cancel_color,
            confirm_text: self
                .confirm_text
                .unwrap_or_else(|| t(I18nKey::CommonConfirm)),
            confirm_color: self.confirm_color,
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
        let data = match result {
            CallbackResult::Success(data) => data,
            // Error code 2000 = user cancelled
            CallbackResult::Error(_) => {
                return JSModalResult {
                    confirm: false,
                    cancel: true,
                };
            }
        };

        // Success callback contains confirm result
        match serde_json::from_str::<Value>(&data) {
            Ok(json) => JSModalResult {
                confirm: json
                    .get("confirm")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
                cancel: false,
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
    let modal_options = options.into_modal_options();

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
