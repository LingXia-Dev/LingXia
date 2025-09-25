use lingxia_lxapp::{LxApp, lx};
use lingxia_platform::{PopupPosition, PopupRequest};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::sync::Arc;

#[derive(FromJSObj)]
struct JSPopupOptions {
    url: String,
    #[rename = "widthRatio"]
    width_ratio: Option<f64>,
    #[rename = "heightRatio"]
    height_ratio: Option<f64>,
    position: Option<String>,
}

fn parse_position(value: Option<String>) -> PopupPosition {
    match value
        .unwrap_or_else(|| "bottom".to_string())
        .to_lowercase()
        .as_str()
    {
        "center" => PopupPosition::Center,
        "bottom" => PopupPosition::Bottom,
        _ => PopupPosition::Bottom,
    }
}

fn show_popup(ctx: JSContext, options: JSPopupOptions) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    let mut request = PopupRequest::new(lxapp.appid.clone(), options.url);
    if let Some(width) = options.width_ratio {
        request.width_ratio = width;
    }
    if let Some(height) = options.height_ratio {
        request.height_ratio = height;
    }
    request.position = parse_position(options.position);

    lxapp
        .show_popup(request)
        .map_err(|e| RongJSError::Error(format!("Failed to show popup: {}", e)))
}

fn hide_popup(ctx: JSContext) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    lxapp
        .hide_popup()
        .map_err(|e| RongJSError::Error(format!("Failed to hide popup: {}", e)))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let show_popup_func = JSFunc::new(ctx, show_popup)?;
    lx::register_js_api(ctx, "showPopup", show_popup_func)?;

    let hide_popup_func = JSFunc::new(ctx, hide_popup)?;
    lx::register_js_api(ctx, "hidePopup", hide_popup_func)?;

    Ok(())
}
