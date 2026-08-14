use crate::dismissal::{canceled, completed};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_error_from_platform_error;
use crate::i18n::{js_internal_error, js_invalid_parameter_error, js_service_unavailable_error};
use crate::{I18nKey, i18n::t};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::error::PlatformError;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_platform::traits::ui::UserFeedback;
use lxapp::LxApp;
use rong::{FromJSObject, JSContext, JSObject, JSResult, RongJSError};
use serde::Deserialize;
use std::sync::Arc;

/// Action sheet options from JavaScript
#[derive(FromJSObject)]
#[ts_skip]
struct JSActionSheetOptions {
    #[js_name = "itemList"]
    item_list: Vec<String>,
    #[js_name = "itemColor"]
    item_color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ViewActionSheetResult {
    #[serde(rename = "tapIndex")]
    tap_index: i64,
}

fn classify_action_sheet_index(index: i64, item_len: usize) -> Result<Option<usize>, RongJSError> {
    if index == -1 {
        return Ok(None);
    }
    if index < -1 {
        return Err(js_internal_error(format!(
            "ActionSheet callback invalid payload: tapIndex {index} is not a cancellation or item index"
        )));
    }

    let index = usize::try_from(index).map_err(|_| {
        js_internal_error(format!(
            "ActionSheet callback invalid payload: tapIndex {index} cannot be represented"
        ))
    })?;
    if index >= item_len {
        return Err(js_internal_error(format!(
            "ActionSheet callback invalid payload: tapIndex {index} is outside itemList length {item_len}"
        )));
    }

    Ok(Some(index))
}

/// Show action sheet function for JavaScript
async fn show_action_sheet(
    ctx: JSContext,
    options: JSActionSheetOptions,
) -> Result<JSObject, RongJSError> {
    let JSActionSheetOptions {
        item_list,
        item_color,
    } = options;
    let lxapp = LxApp::from_ctx(&ctx)?;

    let Some(index) = present_action_sheet(&lxapp, item_list, None, item_color).await? else {
        return canceled(&ctx);
    };

    let result = completed(&ctx)?;
    result.set("index", index as u32)?;
    Ok(result)
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

    classify_action_sheet_index(result.tap_index, item_len)
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

    let result: ViewActionSheetResult = serde_json::from_str(&data).map_err(|error| {
        js_internal_error(format!("ActionSheet callback invalid payload: {error}"))
    })?;
    classify_action_sheet_index(result.tap_index, item_len)
}

/// Initialize action sheet functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn showActionSheet(
            ts_params = "options: ShowActionSheetOptions",
            ts_return = "Promise<ActionSheetResult>"
        ) = show_action_sheet;
    }
}

#[cfg(test)]
mod tests {
    use super::classify_action_sheet_index;

    #[test]
    fn classifies_only_minus_one_as_canceled() {
        assert_eq!(classify_action_sheet_index(-1, 2).unwrap(), None);
        assert!(classify_action_sheet_index(-2, 2).is_err());
    }

    #[test]
    fn accepts_only_indexes_within_item_list() {
        assert_eq!(classify_action_sheet_index(0, 2).unwrap(), Some(0));
        assert_eq!(classify_action_sheet_index(1, 2).unwrap(), Some(1));
        assert!(classify_action_sheet_index(2, 2).is_err());
    }
}
