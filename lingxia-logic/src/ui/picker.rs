use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::{PickerType, UserFeedback};
use rong::{FromJSObj, IntoJSValue, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError};
use std::collections::HashMap;
use std::sync::Arc;

/// Cascading column data structure
#[derive(Debug, Clone)]
enum ColumnData {
    /// Static array of strings
    Static(Vec<String>),
    /// Cascading data: key -> options
    Cascading(HashMap<String, Vec<String>>),
}

/// Parsed picker data for different modes
#[derive(Debug, Clone)]
enum PickerData {
    /// Single column picker
    Single(Vec<String>),
    /// Dual column picker (legacy)
    Dual(Vec<String>, Vec<String>),
    /// Cascading picker (new)
    Cascading(Vec<ColumnData>),
}

/// Picker options from JavaScript
#[derive(FromJSObj, Debug)]
struct JSPickerOptions {
    mode: String,
    // Single column mode
    items: Option<Vec<String>>,
    // Multi column mode (supports both regular and cascading)
    columns: Option<rong::JSArray>,

    #[rename = "cancelText"]
    cancel_text: Option<String>,
    #[rename = "cancelButtonColor"]
    cancel_button_color: Option<String>,
    #[rename = "cancelTextColor"]
    cancel_text_color: Option<String>,
    #[rename = "confirmText"]
    confirm_text: Option<String>,
    #[rename = "confirmButtonColor"]
    confirm_button_color: Option<String>,
    #[rename = "confirmTextColor"]
    confirm_text_color: Option<String>,
}

/// Picker result with Vec<i32> index
#[derive(Debug, Clone)]
struct PickerResult {
    index: Vec<i32>,
    cancelled: bool,
    confirmed: bool,
}

impl IntoJSValue<rong::JSEngineValue> for PickerResult {
    fn into_js_value(self, ctx: &rong::JSContext) -> rong::JSEngineValue {
        let obj = JSObject::new(ctx);

        // Convert index based on length for JS compatibility
        if self.index.len() == 1 {
            // Single column: return single number
            obj.set("index", self.index[0]).unwrap();
        } else {
            // Multi column: return array
            obj.set("index", self.index).unwrap();
        }

        obj.set("cancelled", self.cancelled).unwrap();
        obj.set("confirmed", self.confirmed).unwrap();
        obj.into_value()
    }
}

// Blanket implementing to make PickerResult can be used as JSFunc parameter
impl rong::function::JSParameterType for PickerResult {}

/// Parse columns data from JavaScript
fn parse_columns_data(columns_array: &rong::JSArray) -> JSResult<PickerData> {
    // Must have exactly 2 columns for multiSelector
    if columns_array.len() != 2 {
        return Err(RongJSError::Error(
            "multiSelector requires exactly 2 columns".to_string(),
        ));
    }

    // First column: must be array
    let first_column = columns_array
        .get::<Vec<String>>(0)?
        .ok_or_else(|| RongJSError::Error("First column is required".to_string()))?;

    // Second column: check if array or object
    let second_value = columns_array
        .get::<JSValue>(1)?
        .ok_or_else(|| RongJSError::Error("Second column is required".to_string()))?;

    if let Ok(second_array) = second_value.clone().try_into::<Vec<String>>() {
        // Regular dual column (both are arrays)
        Ok(PickerData::Dual(first_column, second_array))
    } else if let Ok(second_object) = second_value.try_into::<JSObject>() {
        // Cascading column (second is object)
        let mut cascading_map = HashMap::new();

        // Get all property names - following the pattern from app.rs
        for key_value in second_object.keys()? {
            if let Ok(key_string) = key_value.try_into::<String>() {
                // Get the array for this key
                if let Ok(array) = second_object.get::<_, Vec<String>>(key_string.as_str()) {
                    cascading_map.insert(key_string, array);
                }
            }
        }

        let columns = vec![
            ColumnData::Static(first_column),
            ColumnData::Cascading(cascading_map),
        ];
        Ok(PickerData::Cascading(columns))
    } else {
        Err(RongJSError::Error(
            "Second column must be array or object".to_string(),
        ))
    }
}

impl From<CallbackResult> for PickerResult {
    fn from(result: CallbackResult) -> Self {
        if result.success {
            if let Ok(picker_data) = serde_json::from_str::<serde_json::Value>(&result.data) {
                let cancelled = picker_data
                    .get("cancel")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let confirmed = picker_data
                    .get("confirm")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Handle both single number and array formats
                let index = if let Some(index_value) = picker_data.get("index") {
                    if let Some(single_index) = index_value.as_i64() {
                        // Single column: convert single number to array
                        vec![single_index as i32]
                    } else if let Some(array) = index_value.as_array() {
                        // Multi column: use array directly
                        array
                            .iter()
                            .filter_map(|v| v.as_i64())
                            .map(|i| i as i32)
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                PickerResult {
                    index,
                    cancelled,
                    confirmed,
                }
            } else {
                PickerResult {
                    index: vec![],
                    cancelled: true,
                    confirmed: false,
                }
            }
        } else {
            PickerResult {
                index: vec![],
                cancelled: true,
                confirmed: false,
            }
        }
    }
}

async fn show_picker(
    ctx: JSContext,
    options: JSPickerOptions,
) -> Result<PickerResult, RongJSError> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    let picker_data = match options.mode.as_str() {
        "selector" => {
            let items = options.items.ok_or_else(|| {
                RongJSError::Error("items is required for selector mode".to_string())
            })?;

            if items.is_empty() {
                return Err(RongJSError::Error("items cannot be empty".to_string()));
            }
            PickerData::Single(items)
        }
        "multiSelector" => {
            let columns_array = options.columns.ok_or_else(|| {
                RongJSError::Error("columns is required for multiSelector mode".to_string())
            })?;

            if columns_array.len() < 2 {
                return Err(RongJSError::Error(
                    "multiSelector requires at least 2 columns".to_string(),
                ));
            }

            parse_columns_data(&columns_array)?
        }
        _ => {
            return Err(RongJSError::Error(
                "mode must be 'selector' or 'multiSelector'".to_string(),
            ));
        }
    };

    // Convert PickerData to PickerType for platform layer
    let picker_type = match picker_data {
        PickerData::Single(items) => PickerType::SingleColumn { items },
        PickerData::Dual(first_column, second_column) => PickerType::DualColumn {
            first_column,
            second_column,
        },
        PickerData::Cascading(columns) => {
            // Extract first column and cascading data
            if columns.len() == 2 {
                if let (ColumnData::Static(first_column), ColumnData::Cascading(cascading_data)) =
                    (&columns[0], &columns[1])
                {
                    PickerType::DualColumnCascading {
                        first_column: first_column.clone(),
                        cascading_data: cascading_data.clone(),
                    }
                } else {
                    return Err(RongJSError::Error(
                        "Invalid cascading picker structure".to_string(),
                    ));
                }
            } else {
                return Err(RongJSError::Error(
                    "Cascading picker must have exactly 2 columns".to_string(),
                ));
            }
        }
    };

    let (callback_id, receiver) = get_callback();
    let cancel_text = options.cancel_text.unwrap_or_else(|| "Cancel".to_string());
    let cancel_button_color = options
        .cancel_button_color
        .unwrap_or_else(|| "#F2F2F2".to_string());
    let cancel_text_color = options
        .cancel_text_color
        .unwrap_or_else(|| "#007AFF".to_string());
    let confirm_text = options
        .confirm_text
        .unwrap_or_else(|| "Confirm".to_string());
    let confirm_button_color = options
        .confirm_button_color
        .unwrap_or_else(|| "#007AFF".to_string());
    let confirm_text_color = options
        .confirm_text_color
        .unwrap_or_else(|| "#FFFFFF".to_string());

    match lxapp.runtime.show_picker(
        picker_type,
        cancel_text,
        cancel_button_color,
        cancel_text_color,
        confirm_text,
        confirm_button_color,
        confirm_text_color,
        callback_id,
    ) {
        Ok(()) => match receiver.await {
            Ok(result) => Ok(PickerResult::from(result)),
            Err(_) => Err(RongJSError::Error(
                "Picker callback timeout or cancelled".to_string(),
            )),
        },
        Err(e) => Err(RongJSError::Error(format!("Failed to show picker: {}", e))),
    }
}
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let show_picker_func = JSFunc::new(ctx, show_picker)?;
    lx::register_js_api(ctx, "showPicker", show_picker_func)?;
    Ok(())
}

