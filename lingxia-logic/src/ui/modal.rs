use lingxia_lxapp::{LxApp, lx};
use lingxia_platform::{ModalOptions, ModalResult, UserFeedback};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::sync::Arc;

/// Modal options from JavaScript (compatible with WeChat mini-program API)
#[derive(FromJSObj)]
struct JSModalOptions {
    title: Option<String>,
    content: Option<String>,
    show_cancel: Option<bool>,
    cancel_text: Option<String>,
    cancel_color: Option<String>,
    confirm_text: Option<String>,
    confirm_color: Option<String>,
    editable: Option<bool>,
    placeholder_text: Option<String>,
}

impl From<JSModalOptions> for ModalOptions {
    fn from(js_options: JSModalOptions) -> Self {
        ModalOptions {
            title: js_options.title.unwrap_or_else(|| "提示".to_string()),
            content: js_options.content.unwrap_or_default(),
            show_cancel: js_options.show_cancel.unwrap_or(true),
            cancel_text: js_options.cancel_text.unwrap_or_else(|| "取消".to_string()),
            cancel_color: js_options.cancel_color,
            confirm_text: js_options
                .confirm_text
                .unwrap_or_else(|| "确定".to_string()),
            confirm_color: js_options.confirm_color,
            editable: js_options.editable.unwrap_or(false),
            placeholder_text: js_options.placeholder_text.unwrap_or_default(),
        }
    }
}

/// JavaScript ModalResult for return value
#[derive(Debug, Clone, IntoJSObj)]
struct JSModalResult {
    confirm: bool,
    cancel: bool,
    content: String,
}

impl From<ModalResult> for JSModalResult {
    fn from(result: ModalResult) -> Self {
        JSModalResult {
            confirm: result.confirm,
            cancel: result.cancel,
            content: result.content,
        }
    }
}

/// Show modal function
fn show_modal(ctx: JSContext, options: JSModalOptions) -> JSResult<JSModalResult> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let modal_options: ModalOptions = options.into();

    match lxapp.runtime.show_modal(modal_options) {
        Ok(result) => Ok(result.into()),
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
