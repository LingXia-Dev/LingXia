//! `page.pointer` / `page.key` — app-window input at page coordinates, the JS
//! mapping of `lxdev lxapp page pointer|key`. Both dispatch the same platform
//! requests as the devtool handlers (`app.mouse` / `app.keyboard` RPCs), so
//! coordinates, buttons, and the modifier vocabulary can never drift.

use crate::auto_err;
use crate::resolve::json_to_js;
use lingxia_platform::traits::{keyboard, mouse};
use rong::{FromJSObj, HostError, JSContext, JSResult, JSValue, js_class, js_export, js_method};

fn illegal_ctor() -> rong::RongJSError {
    HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into()
}

/// Parse the `at: [x, y]` coordinate form (page CSS pixels).
fn point(at: &[f64], flag: &str) -> JSResult<(f64, f64)> {
    match at {
        [x, y] => Ok((*x, *y)),
        _ => Err(auto_err(format!("{flag}: expected [x, y]"))),
    }
}

async fn app_mouse(
    ctx: &JSContext,
    window: Option<String>,
    action: mouse::AppMouseAction,
) -> JSResult<JSValue> {
    use lingxia_platform::traits::mouse::AppMouse;
    let platform = lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))?;
    let result = platform
        .perform_app_mouse(mouse::AppMouseRequest {
            window_id: window,
            action,
        })
        .await
        .map_err(|err| auto_err(err.to_string()))?;
    let json = serde_json::to_value(&result).map_err(|err| auto_err(err.to_string()))?;
    json_to_js(ctx, &json)
}

async fn app_keyboard(
    ctx: &JSContext,
    window: Option<String>,
    action: keyboard::AppKeyboardAction,
) -> JSResult<JSValue> {
    use lingxia_platform::traits::keyboard::AppKeyboard;
    let platform = lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))?;
    let result = platform
        .perform_app_keyboard(keyboard::AppKeyboardRequest {
            window_id: window,
            action,
        })
        .await
        .map_err(|err| auto_err(err.to_string()))?;
    let json = serde_json::to_value(&result).map_err(|err| auto_err(err.to_string()))?;
    json_to_js(ctx, &json)
}

fn mouse_button(raw: &Option<String>) -> JSResult<mouse::AppMouseButton> {
    match raw.as_deref().map(str::trim) {
        None | Some("") | Some("left") => Ok(mouse::AppMouseButton::Left),
        Some("right") => Ok(mouse::AppMouseButton::Right),
        Some("middle") => Ok(mouse::AppMouseButton::Middle),
        Some(other) => Err(auto_err(format!(
            "unknown button '{other}' (expected left | right | middle)"
        ))),
    }
}

/// The canonical cross-platform modifier vocabulary (`ctrl | shift | alt |
/// meta`); `meta` maps to the platform meta key (Command / Windows key).
fn keyboard_modifiers(raw: &Option<Vec<String>>) -> JSResult<Vec<keyboard::AppKeyboardModifier>> {
    raw.iter()
        .flatten()
        .map(|value| match value.trim() {
            "ctrl" => Ok(keyboard::AppKeyboardModifier::Control),
            "shift" => Ok(keyboard::AppKeyboardModifier::Shift),
            "alt" => Ok(keyboard::AppKeyboardModifier::Option),
            "meta" => Ok(keyboard::AppKeyboardModifier::Command),
            other => Err(auto_err(format!(
                "unknown modifier '{other}' (expected ctrl | shift | alt | meta)"
            ))),
        })
        .collect()
}

// ===================== page.pointer.* =====================

#[js_export]
pub(crate) struct JSPagePointer {}

impl JSPagePointer {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj)]
struct PointerAt {
    /// Target coordinate as `[x, y]` in page (CSS) pixels.
    at: Vec<f64>,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct PointerButtonAt {
    at: Vec<f64>,
    button: Option<String>,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct PointerClick {
    at: Vec<f64>,
    button: Option<String>,
    count: Option<u8>,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct PointerDrag {
    from: Vec<f64>,
    to: Vec<f64>,
    button: Option<String>,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct PointerScroll {
    at: Vec<f64>,
    dx: Option<f64>,
    dy: Option<f64>,
    window: Option<String>,
}

#[js_class(rename = "PagePointer")]
impl JSPagePointer {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method(rename = "move")]
    async fn pointer_move(&self, ctx: JSContext, o: PointerAt) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        app_mouse(&ctx, o.window, mouse::AppMouseAction::Move { x, y }).await
    }

    #[js_method]
    async fn down(&self, ctx: JSContext, o: PointerButtonAt) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let button = mouse_button(&o.button)?;
        app_mouse(&ctx, o.window, mouse::AppMouseAction::Down { x, y, button }).await
    }

    #[js_method]
    async fn up(&self, ctx: JSContext, o: PointerButtonAt) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let button = mouse_button(&o.button)?;
        app_mouse(&ctx, o.window, mouse::AppMouseAction::Up { x, y, button }).await
    }

    #[js_method]
    async fn click(&self, ctx: JSContext, o: PointerClick) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        let button = mouse_button(&o.button)?;
        let click_count = o.count.unwrap_or(1);
        if click_count == 0 {
            return Err(auto_err("count must be greater than zero"));
        }
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Click {
                x,
                y,
                button,
                click_count,
            },
        )
        .await
    }

    #[js_method]
    async fn drag(&self, ctx: JSContext, o: PointerDrag) -> JSResult<JSValue> {
        let (from_x, from_y) = point(&o.from, "from")?;
        let (to_x, to_y) = point(&o.to, "to")?;
        let button = mouse_button(&o.button)?;
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Drag {
                from_x,
                from_y,
                to_x,
                to_y,
                button,
            },
        )
        .await
    }

    #[js_method]
    async fn scroll(&self, ctx: JSContext, o: PointerScroll) -> JSResult<JSValue> {
        let (x, y) = point(&o.at, "at")?;
        app_mouse(
            &ctx,
            o.window,
            mouse::AppMouseAction::Scroll {
                x,
                y,
                dx: o.dx.unwrap_or(0.0),
                dy: o.dy.unwrap_or(0.0),
            },
        )
        .await
    }
}

// ===================== page.key.* =====================

#[js_export]
pub(crate) struct JSPageKey {}

impl JSPageKey {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObj)]
struct KeyType {
    text: String,
    window: Option<String>,
}

#[derive(FromJSObj)]
struct KeyPress {
    key: String,
    modifiers: Option<Vec<String>>,
    window: Option<String>,
}

#[js_class(rename = "PageKey")]
impl JSPageKey {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(illegal_ctor())
    }

    #[js_method(rename = "type")]
    async fn key_type(&self, ctx: JSContext, o: KeyType) -> JSResult<JSValue> {
        app_keyboard(
            &ctx,
            o.window,
            keyboard::AppKeyboardAction::Type { text: o.text },
        )
        .await
    }

    #[js_method]
    async fn press(&self, ctx: JSContext, o: KeyPress) -> JSResult<JSValue> {
        let modifiers = keyboard_modifiers(&o.modifiers)?;
        app_keyboard(
            &ctx,
            o.window,
            keyboard::AppKeyboardAction::Press {
                key: o.key,
                modifiers,
            },
        )
        .await
    }
}
