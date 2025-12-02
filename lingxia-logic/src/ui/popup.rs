use lingxia_platform::{Device, PopupPosition, PopupRequest, ScreenInfo};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSObject, JSResult, RongJSError};

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
        "left" => PopupPosition::Left,
        "right" => PopupPosition::Right,
        _ => PopupPosition::Bottom,
    }
}

fn sanitize_ratio_input(value: Option<f64>) -> Option<f64> {
    match value {
        Some(v) if v.is_finite() => Some(v),
        _ => None,
    }
}

fn clamp_ratio(value: f64) -> f64 {
    if !value.is_finite() {
        1.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn default_width_ratio(position: PopupPosition, screen: &ScreenInfo) -> f64 {
    let min_side = screen.width.min(screen.height);
    let is_tablet = min_side >= 600.0;

    match position {
        PopupPosition::Bottom | PopupPosition::Center => 1.0,
        PopupPosition::Left | PopupPosition::Right => {
            if is_tablet {
                0.4
            } else {
                0.7
            }
        }
    }
}

fn default_height_ratio(position: PopupPosition, screen: &ScreenInfo) -> f64 {
    let min_side = screen.width.min(screen.height);
    let max_side = screen.width.max(screen.height);
    let is_tablet = min_side >= 600.0;

    match position {
        PopupPosition::Bottom => {
            if is_tablet {
                0.45
            } else {
                0.55
            }
        }
        PopupPosition::Center => {
            if is_tablet {
                0.5
            } else if max_side >= 900.0 {
                0.55
            } else if max_side >= 780.0 {
                0.58
            } else {
                0.6
            }
        }
        PopupPosition::Left | PopupPosition::Right => 1.0,
    }
}

fn resolve_popup_ratios(
    width_ratio: Option<f64>,
    height_ratio: Option<f64>,
    position: PopupPosition,
    screen: &ScreenInfo,
) -> (f64, f64) {
    let width =
        sanitize_ratio_input(width_ratio).unwrap_or_else(|| default_width_ratio(position, screen));
    let height = sanitize_ratio_input(height_ratio)
        .unwrap_or_else(|| default_height_ratio(position, screen));

    (clamp_ratio(width), clamp_ratio(height))
}

async fn show_popup(ctx: JSContext, options: JSPopupOptions) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Do not show UI if app is not opened
    if !lxapp.is_opened() {
        return Err(RongJSError::Error(
            "LxApp is closed; popup suppressed".to_string(),
        ));
    }

    let page_svc = lxapp
        .get_or_create_page_in_ctx(&ctx, &options.url)
        .await
        .map_err(|e| RongJSError::Error(format!("Failed to ensure popup page service: {}", e)))?;

    let position = parse_position(options.position);
    let screen = lxapp.runtime.screen_info();
    let (width_ratio, height_ratio) =
        resolve_popup_ratios(options.width_ratio, options.height_ratio, position, &screen);

    let mut request = PopupRequest::new(lxapp.appid.clone(), options.url);
    request.width_ratio = width_ratio;
    request.height_ratio = height_ratio;
    request.position = position;

    lxapp
        .show_popup(request)
        .map_err(|e| RongJSError::Error(format!("Failed to show popup: {}", e)))?;

    let event_emitter = page_svc.get_event_emitter();

    let response = JSObject::new(&ctx);
    response.set("eventEmitter", event_emitter)?;

    Ok(response)
}

fn hide_popup(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if !lxapp.is_opened() {
        return Ok(());
    }

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
