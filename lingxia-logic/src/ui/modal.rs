use crate::{I18nKey, i18n::t};
#[cfg(not(target_os = "macos"))]
use lingxia_messaging::{CallbackResult, get_callback};
#[cfg(not(target_os = "macos"))]
use lingxia_platform::traits::ui::{ModalOptions, UserFeedback};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, HostError, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::Value;
use std::sync::Arc;

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

#[cfg(not(target_os = "macos"))]
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

#[cfg(not(target_os = "macos"))]
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
                confirm: json.get("confirm").and_then(Value::as_bool).unwrap_or(true),
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

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(
            HostError::new(rong::error::E_INTERNAL, "LxApp is closed; modal suppressed").into(),
        );
    }

    present_modal(&lxapp, options).await
}

async fn present_modal(
    lxapp: &Arc<LxApp>,
    options: JSModalOptions,
) -> Result<JSModalResult, RongJSError> {
    #[cfg(target_os = "macos")]
    {
        return present_modal_webview(lxapp, options).await;
    }

    #[cfg(not(target_os = "macos"))]
    {
        present_modal_native(lxapp, options).await
    }
}

/// macOS: render modal inside the WebView via Logic→View RPC.
#[cfg(target_os = "macos")]
async fn present_modal_webview(
    lxapp: &Arc<LxApp>,
    options: JSModalOptions,
) -> Result<JSModalResult, RongJSError> {
    let params = serde_json::json!({
        "title": options.title.unwrap_or_default(),
        "content": options.content.unwrap_or_default(),
        "showCancel": options.show_cancel.unwrap_or(true),
        "cancelText": options.cancel_text.unwrap_or_else(|| t(I18nKey::CommonCancel)),
        "cancelColor": options.cancel_color,
        "confirmText": options.confirm_text.unwrap_or_else(|| t(I18nKey::CommonConfirm)),
        "confirmColor": options.confirm_color,
    });

    let result = lxapp
        .call_current_page_view("ui.showModal", Some(params))
        .await
        .map_err(|e| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("WebView modal failed: {}", e),
            )
        })?;

    let confirm = result
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(JSModalResult {
        confirm,
        cancel: !confirm,
    })
}

/// Non-macOS: show modal via native platform UI.
#[cfg(not(target_os = "macos"))]
async fn present_modal_native(
    lxapp: &Arc<LxApp>,
    options: JSModalOptions,
) -> Result<JSModalResult, RongJSError> {
    let modal_options = options.into_modal_options();
    let (callback_id, receiver) = get_callback();

    lxapp
        .runtime
        .show_modal(modal_options, callback_id)
        .map_err(|e| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("Failed to show modal: {}", e),
            )
        })?;

    let result = receiver.await.map_err(|_| {
        HostError::new(
            rong::error::E_INTERNAL,
            "Modal callback timeout or cancelled",
        )
    })?;

    Ok(result.into())
}

/// Initialize modal functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register showModal function
    let show_modal_func = JSFunc::new(ctx, show_modal)?;
    lx::register_js_api(ctx, "showModal", show_modal_func)?;

    Ok(())
}
