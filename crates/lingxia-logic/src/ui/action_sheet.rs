#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_error_from_platform_error;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::i18n::js_internal_error;
use crate::i18n::{js_invalid_parameter_error, js_service_unavailable_error};
use crate::{I18nKey, i18n::t};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::error::PlatformError;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::traits::ui::UserFeedback;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde::Deserialize;
use std::sync::Arc;

/// Action sheet options from JavaScript
#[derive(FromJSObj)]
struct JSActionSheetOptions {
    #[rename = "itemList"]
    item_list: Vec<String>,
    #[rename = "itemColor"]
    item_color: Option<String>,
}

/// JavaScript ActionSheetResult for return value
#[derive(Debug, Clone, IntoJSObj)]
struct JSActionSheetResult {
    #[rename = "tapIndex"]
    tap_index: i32,
}

#[derive(Debug, Deserialize)]
struct ViewActionSheetResult {
    #[serde(rename = "tapIndex")]
    tap_index: i64,
}

/// Show action sheet function for JavaScript
async fn show_action_sheet(
    ctx: JSContext,
    options: JSActionSheetOptions,
) -> Result<JSActionSheetResult, RongJSError> {
    let JSActionSheetOptions {
        item_list,
        item_color,
    } = options;
    let lxapp = LxApp::from_ctx(&ctx)?;

    let selected_index = present_action_sheet(&lxapp, item_list, None, item_color).await?;
    let tap_index = selected_index.map(|idx| idx as i32).unwrap_or(-1);

    Ok(JSActionSheetResult { tap_index })
}

pub(crate) async fn present_action_sheet(
    lxapp: &Arc<LxApp>,
    item_list: Vec<String>,
    cancel_text: Option<String>,
    item_color: Option<String>,
) -> Result<Option<usize>, RongJSError> {
    if !lxapp.is_opened() {
        return Err(js_service_unavailable_error(
            "LxApp is closed; actionSheet suppressed",
        ));
    }
    if item_list.is_empty() {
        return Err(js_invalid_parameter_error("itemList cannot be empty"));
    }

    let cancel_text = cancel_text.unwrap_or_else(|| t(I18nKey::CommonCancel));
    let item_color = item_color.unwrap_or_else(|| "#007AFF".to_string());
    let item_len = item_list.len();

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        return present_action_sheet_webview(lxapp, item_list, cancel_text, item_color, item_len)
            .await;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        present_action_sheet_native(lxapp, item_list, cancel_text, item_color, item_len).await
    }
}

/// macOS: render action sheet inside the WebView via Logic→View RPC.
#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn present_action_sheet_webview(
    lxapp: &Arc<LxApp>,
    item_list: Vec<String>,
    cancel_text: String,
    item_color: String,
    item_len: usize,
) -> Result<Option<usize>, RongJSError> {
    let params = serde_json::json!({
        "itemList": item_list,
        "cancelText": cancel_text,
        "itemColor": item_color,
    });

    let result: ViewActionSheetResult =
        lxapp
            .call_view_with("ui.showActionSheet", &params)
            .await
            .map_err(|e| js_internal_error(format!("WebView action sheet failed: {}", e)))?;

    let index = result.tap_index;

    if index < 0 {
        return Ok(None);
    }

    let idx = index as usize;
    if idx >= item_len {
        return Ok(None);
    }

    Ok(Some(idx))
}

/// Non-macOS: show action sheet via native platform UI.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
async fn present_action_sheet_native(
    lxapp: &Arc<LxApp>,
    item_list: Vec<String>,
    cancel_text: String,
    item_color: String,
    item_len: usize,
) -> Result<Option<usize>, RongJSError> {
    let data = match lxapp
        .runtime
        .show_action_sheet(item_list, cancel_text, item_color)
        .await
    {
        Ok(data) => data,
        Err(PlatformError::BusinessError(2000)) => return Ok(None),
        Err(e) => return Err(js_error_from_platform_error(&e)),
    };

    let index = serde_json::from_str::<ViewActionSheetResult>(&data)
        .map(|result| result.tap_index)
        .unwrap_or(-1);

    if index < 0 {
        return Ok(None);
    }

    let idx = index as usize;
    if idx >= item_len {
        return Ok(None);
    }

    Ok(Some(idx))
}

/// Initialize action sheet functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register showActionSheet function
    let show_action_sheet_func = JSFunc::new(ctx, show_action_sheet)?;
    lx::register_js_api(ctx, "showActionSheet", show_action_sheet_func)?;

    Ok(())
}
