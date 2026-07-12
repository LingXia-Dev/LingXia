#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_error_from_platform_error;
use crate::i18n::{js_internal_error, js_service_unavailable_error};
use crate::{I18nKey, i18n::t};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::error::PlatformError;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::traits::ui::{ModalOptions, UserFeedback};
use lxapp::LxApp;
use rong::{FromJSObject, IntoJSObject, JSContext, JSResult, RongJSError};
use serde::Deserialize;
use std::sync::Arc;

/// Modal options from JavaScript (compatible with common mini-app APIs)
#[derive(FromJSObject)]
struct JSModalOptions {
    title: Option<String>,
    content: Option<String>,
    #[js_name = "showCancel"]
    show_cancel: Option<bool>,
    #[js_name = "cancelText"]
    cancel_text: Option<String>,
    #[js_name = "cancelColor"]
    cancel_color: Option<String>,
    #[js_name = "confirmText"]
    confirm_text: Option<String>,
    #[js_name = "confirmColor"]
    confirm_color: Option<String>,
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
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
#[derive(Debug, Clone, IntoJSObject)]
struct JSModalResult {
    confirm: bool,
    cancel: bool,
}

#[derive(Debug, Deserialize)]
struct ViewModalResult {
    confirm: bool,
}

/// Show modal function (async)
async fn show_modal(ctx: JSContext, options: JSModalOptions) -> JSResult<JSModalResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(js_service_unavailable_error(
            "LxApp is closed; modal suppressed",
        ));
    }

    present_modal(&lxapp, options).await
}

async fn present_modal(
    lxapp: &Arc<LxApp>,
    options: JSModalOptions,
) -> Result<JSModalResult, RongJSError> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        return present_modal_webview(lxapp, options).await;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        present_modal_native(lxapp, options).await
    }
}

/// macOS: render modal inside the WebView via Logic→View RPC.
#[cfg(any(target_os = "macos", target_os = "windows"))]
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

    let result: ViewModalResult = lxapp
        .call_view_with("ui.showModal", &params)
        .await
        .map_err(|e| js_internal_error(format!("WebView modal failed: {}", e)))?;

    Ok(JSModalResult {
        confirm: result.confirm,
        cancel: !result.confirm,
    })
}

/// Non-macOS: show modal via native platform UI.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
async fn present_modal_native(
    lxapp: &Arc<LxApp>,
    options: JSModalOptions,
) -> Result<JSModalResult, RongJSError> {
    let modal_options = options.into_modal_options();

    match lxapp.runtime.show_modal(modal_options).await {
        Ok(data) => {
            let result: ViewModalResult = serde_json::from_str(&data)
                .map_err(|e| js_internal_error(format!("Modal callback invalid payload: {}", e)))?;
            Ok(JSModalResult {
                confirm: result.confirm,
                cancel: !result.confirm,
            })
        }
        Err(PlatformError::BusinessError(2000)) => Ok(JSModalResult {
            confirm: false,
            cancel: true,
        }),
        Err(e) => Err(js_error_from_platform_error(&e)),
    }
}

/// Initialize modal functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn showModal(
            ts_params = "options: ShowModalOptions",
            ts_return = "Promise<ModalResult>"
        ) = show_modal;
    }
}
