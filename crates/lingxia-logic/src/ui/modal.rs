#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::dismissal::USER_DISMISSED;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_error_from_platform_error;
use crate::i18n::{js_internal_error, js_service_unavailable_error};
use crate::{I18nKey, i18n::t};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::error::PlatformError;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::traits::ui::{ModalOptions, UserFeedback};
use lxapp::LxApp;
use lxapp::dialogs::{DialogDecision, ModalShown};
use rong::{FromJSObject, JSContext, JSResult, JSValue, RongJSError};
use serde::Deserialize;
use std::sync::Arc;

/// `lx.ui.alert` / `lx.ui.confirm` options; a bare string is the title.
#[derive(FromJSObject)]
#[ts_skip]
struct JSDialogOptions {
    title: Option<String>,
    message: Option<String>,
    tone: Option<String>,
    #[js_name = "confirmText"]
    confirm_text: Option<String>,
    #[js_name = "cancelText"]
    cancel_text: Option<String>,
}

struct JSModalOptions {
    title: Option<String>,
    content: Option<String>,
    show_cancel: Option<bool>,
    cancel_text: Option<String>,
    cancel_color: Option<String>,
    confirm_text: Option<String>,
    confirm_color: Option<String>,
}

fn dialog_options(value: JSValue, show_cancel: bool) -> JSResult<JSModalOptions> {
    let options = if value.is_string() {
        JSDialogOptions {
            title: Some(value.to_rust::<String>()?),
            message: None,
            tone: None,
            confirm_text: None,
            cancel_text: None,
        }
    } else {
        value.to_rust::<JSDialogOptions>()?
    };
    let confirm_color = super::tone_color(options.tone.as_deref())?;
    Ok(JSModalOptions {
        title: options.title,
        content: options.message,
        show_cancel: Some(show_cancel),
        cancel_text: options.cancel_text.filter(|_| show_cancel),
        cancel_color: None,
        confirm_text: options.confirm_text,
        confirm_color,
    })
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

#[derive(Debug, Deserialize)]
struct ViewModalResult {
    confirm: bool,
}

/// Resolves true when the user confirmed, false when they dismissed or
/// canceled. Rejects when presentation fails.
async fn show_modal(ctx: JSContext, options: JSModalOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(js_service_unavailable_error(
            "LxApp is closed; modal suppressed",
        ));
    }

    let confirmed = match watched_modal(&lxapp.appid, &options) {
        DialogDecision::Present => present_modal(&lxapp, options).await?,
        DialogDecision::Draw(id) => {
            let result = present_modal(&lxapp, options).await;
            if let Some(hook) = lxapp::dialogs::dialog_hook() {
                hook.modal_closed(&lxapp.appid, id, result.as_ref().ok().copied());
            }
            result?
        }
        DialogDecision::Answer(confirm) => confirm,
        DialogDecision::Refuse(message) => return Err(js_service_unavailable_error(message)),
    };
    Ok(confirmed)
}

/// A test run watching the app records the modal, or answers it instead
/// of the user once its spec queued answers.
fn watched_modal(appid: &str, options: &JSModalOptions) -> DialogDecision<bool> {
    let Some(hook) = lxapp::dialogs::dialog_hook() else {
        return DialogDecision::Present;
    };
    let show_cancel = options.show_cancel.unwrap_or(true);
    hook.modal(
        appid,
        &ModalShown {
            title: options.title.clone().unwrap_or_default(),
            content: options.content.clone().unwrap_or_default(),
            confirm_text: options.confirm_text.clone(),
            cancel_text: options.cancel_text.clone().filter(|_| show_cancel),
            show_cancel,
        },
    )
}

/// Acknowledgement-only dialog. Resolves when the user dismisses it.
///
/// @throws {E_INVALID_STATE} No page can show a dialog now.
/// @throws {E_INTERNAL} The dialog could not be shown.
async fn alert(ctx: JSContext, options: JSValue) -> JSResult<()> {
    show_modal(ctx, dialog_options(options, false)?).await?;
    Ok(())
}

/// Ask a yes/no question. Dismissal resolves false; presentation failure rejects.
///
/// @throws {E_INVALID_STATE} No page can show a dialog now.
/// @throws {E_INTERNAL} The dialog could not be shown.
async fn confirm(ctx: JSContext, options: JSValue) -> JSResult<bool> {
    show_modal(ctx, dialog_options(options, true)?).await
}

/// Resolves true when the user confirmed.
async fn present_modal(lxapp: &Arc<LxApp>, options: JSModalOptions) -> Result<bool, RongJSError> {
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
) -> Result<bool, RongJSError> {
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

    Ok(result.confirm)
}

/// Non-macOS: show modal via native platform UI.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
async fn present_modal_native(
    lxapp: &Arc<LxApp>,
    options: JSModalOptions,
) -> Result<bool, RongJSError> {
    let modal_options = options.into_modal_options();

    match lxapp.runtime.show_modal(modal_options).await {
        Ok(data) => {
            let result: ViewModalResult = serde_json::from_str(&data)
                .map_err(|e| js_internal_error(format!("Modal callback invalid payload: {}", e)))?;
            Ok(result.confirm)
        }
        Err(PlatformError::BusinessError(USER_DISMISSED)) => Ok(false),
        Err(e) => Err(js_error_from_platform_error(&e)),
    }
}

/// Initialize modal functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace UiApi = super::ui_namespace(ctx)?;
        fn alert(ts_params = "options: string | AlertOptions", ts_return = "Promise<void>") = alert;
        fn confirm(
            ts_params = "options: string | ConfirmOptions",
            ts_return = "Promise<boolean>"
        ) = confirm;
    }
}
