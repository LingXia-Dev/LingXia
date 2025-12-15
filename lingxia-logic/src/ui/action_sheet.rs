use crate::{I18nKey, i18n::t};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::UserFeedback;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use serde_json::Value;
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

/// Show action sheet function for JavaScript
async fn show_action_sheet(
    ctx: JSContext,
    options: JSActionSheetOptions,
) -> Result<JSActionSheetResult, RongJSError> {
    let JSActionSheetOptions {
        item_list,
        item_color,
    } = options;

    if item_list.is_empty() {
        return Err(RongJSError::Error("itemList cannot be empty".to_string()));
    }

    let lxapp = LxApp::from_ctx(&ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(RongJSError::Error(
            "LxApp is closed; actionSheet suppressed".to_string(),
        ));
    }

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
        return Err(RongJSError::Error(
            "LxApp is closed; actionSheet suppressed".to_string(),
        ));
    }
    if item_list.is_empty() {
        return Err(RongJSError::Error("itemList cannot be empty".to_string()));
    }

    let cancel_text = cancel_text.unwrap_or_else(|| t(I18nKey::CommonCancel));
    let item_color = item_color.unwrap_or_else(|| "#007AFF".to_string());
    let item_len = item_list.len();

    let (callback_id, receiver) = get_callback();

    lxapp
        .runtime
        .show_action_sheet(item_list, cancel_text, item_color, callback_id)
        .map_err(|e| RongJSError::Error(format!("Failed to show action sheet: {}", e)))?;

    let CallbackResult { success, data } = receiver.await.map_err(|_| {
        RongJSError::Error("Action sheet callback timeout or cancelled".to_string())
    })?;

    if !success {
        return Ok(None);
    }

    let index = serde_json::from_str::<Value>(&data)
        .ok()
        .and_then(|json| json.get("tapIndex").and_then(Value::as_i64))
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
