use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::UserFeedback;
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

impl From<CallbackResult> for JSActionSheetResult {
    fn from(result: CallbackResult) -> Self {
        if !result.success {
            return JSActionSheetResult { tap_index: -1 };
        }

        match serde_json::from_str::<Value>(&result.data) {
            Ok(json) => JSActionSheetResult {
                tap_index: json.get("tapIndex").and_then(Value::as_i64).unwrap_or(-1) as i32,
            },
            Err(_) => JSActionSheetResult { tap_index: -1 },
        }
    }
}

/// Show action sheet function for JavaScript
async fn show_action_sheet(
    ctx: JSContext,
    options: JSActionSheetOptions,
) -> Result<JSActionSheetResult, RongJSError> {
    // Validate parameters
    if options.item_list.is_empty() {
        return Err(RongJSError::Error("itemList cannot be empty".to_string()));
    }

    // Extract parameters with defaults
    let cancel_text = "Cancel".to_string();
    let item_color = options.item_color.unwrap_or_else(|| "#007AFF".to_string());

    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    // Get callback ID and receiver
    let (callback_id, receiver) = get_callback();

    // Call runtime interface with callback ID
    match lxapp
        .runtime
        .show_action_sheet(options.item_list, cancel_text, item_color, callback_id)
    {
        Ok(()) => {
            // Wait for result from callback
            match receiver.await {
                Ok(result) => Ok(result.into()),
                Err(_) => Err(RongJSError::Error(
                    "Action sheet callback timeout or cancelled".to_string(),
                )),
            }
        }
        Err(e) => Err(RongJSError::Error(format!(
            "Failed to show action sheet: {}",
            e
        ))),
    }
}

/// Initialize action sheet functions
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register showActionSheet function
    let show_action_sheet_func = JSFunc::new(ctx, show_action_sheet)?;
    lx::register_js_api(ctx, "showActionSheet", show_action_sheet_func)?;

    Ok(())
}
