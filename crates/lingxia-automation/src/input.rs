//! `page.pointer` / `page.key` — app-window input at page coordinates, the JS
//! mapping of `lxdev lxapp page pointer|key`. Both dispatch the same platform
//! requests as the devtool handlers (`app.mouse` / `app.keyboard` RPCs), so
//! coordinates, buttons, and the modifier vocabulary can never drift.

use crate::resolve::json_to_js;
#[cfg(target_os = "windows")]
use crate::resolve::resolve_lxapp_by_id;
use crate::{auto_err, require_host_context};
use lingxia_platform::traits::{keyboard, mouse};
use rong::{FromJSObject, HostError, JSContext, JSResult, JSValue, js_class, js_method};
use std::sync::Arc;

fn illegal_ctor() -> rong::RongJSError {
    HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into()
}

/// Parse the `at: [x, y]` coordinate form (page CSS pixels).
fn point(at: &[f64], flag: &str) -> JSResult<(f64, f64)> {
    match at {
        [x, y] if x.is_finite() && y.is_finite() => Ok((*x, *y)),
        [_, _] => Err(auto_err(format!("{flag}: coordinates must be finite"))),
        _ => Err(auto_err(format!("{flag}: expected [x, y]"))),
    }
}

async fn app_mouse(
    ctx: &JSContext,
    window: Option<String>,
    action: mouse::AppMouseAction,
) -> JSResult<JSValue> {
    require_host_context(ctx)?;
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
    require_host_context(ctx)?;
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

#[js_class(clone)]
pub(crate) struct JSPagePointer {
    appid: Arc<str>,
}

impl JSPagePointer {
    pub(crate) fn new(appid: Arc<str>) -> Self {
        Self { appid }
    }

    fn page_point(
        &self,
        at: &[f64],
        flag: &str,
        window: Option<String>,
    ) -> JSResult<(f64, f64, Option<String>)> {
        let (x, y) = point(at, flag)?;

        #[cfg(target_os = "windows")]
        {
            let app = resolve_lxapp_by_id(self.appid.as_ref())?;
            let (page, _) = lxapp::automation::resolve_page(&app, None).map_err(auto_err)?;
            let content = lingxia_windows_contract::find_webview_content_window(&page.webtag())
                .ok_or_else(|| auto_err("current page is not attached to a Windows host window"))?;
            let actual_window = content.window.to_string();
            if let Some(requested) = window.as_deref()
                && !windows_window_id_matches(requested, content.window)
            {
                return Err(auto_err(format!(
                    "window {requested} does not host the current page (expected {actual_window})"
                )));
            }
            Ok((
                f64::from(content.content_left) + x * content.scale,
                f64::from(content.content_top) + y * content.scale,
                Some(actual_window),
            ))
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = &self.appid;
            Ok((x, y, window))
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_window_id_matches(raw: &str, expected: isize) -> bool {
    let raw = raw.trim();
    raw.parse::<usize>()
        .ok()
        .or_else(|| {
            raw.strip_prefix("0x")
                .or_else(|| raw.strip_prefix("0X"))
                .and_then(|hex| usize::from_str_radix(hex, 16).ok())
        })
        .is_some_and(|value| value == expected as usize)
}

#[derive(FromJSObject)]
struct PointerAt {
    /// Target coordinate as `[x, y]` in page (CSS) pixels.
    at: Vec<f64>,
    window: Option<String>,
}

#[derive(FromJSObject)]
struct PointerButtonAt {
    at: Vec<f64>,
    button: Option<String>,
    window: Option<String>,
}

#[derive(FromJSObject)]
struct PointerClick {
    at: Vec<f64>,
    button: Option<String>,
    count: Option<u8>,
    window: Option<String>,
}

#[derive(FromJSObject)]
struct PointerDrag {
    from: Vec<f64>,
    to: Vec<f64>,
    button: Option<String>,
    window: Option<String>,
}

#[derive(FromJSObject)]
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
        let (x, y, window) = self.page_point(&o.at, "at", o.window)?;
        app_mouse(&ctx, window, mouse::AppMouseAction::Move { x, y }).await
    }

    #[js_method]
    async fn down(&self, ctx: JSContext, o: PointerButtonAt) -> JSResult<JSValue> {
        let (x, y, window) = self.page_point(&o.at, "at", o.window)?;
        let button = mouse_button(&o.button)?;
        app_mouse(&ctx, window, mouse::AppMouseAction::Down { x, y, button }).await
    }

    #[js_method]
    async fn up(&self, ctx: JSContext, o: PointerButtonAt) -> JSResult<JSValue> {
        let (x, y, window) = self.page_point(&o.at, "at", o.window)?;
        let button = mouse_button(&o.button)?;
        app_mouse(&ctx, window, mouse::AppMouseAction::Up { x, y, button }).await
    }

    #[js_method]
    async fn click(&self, ctx: JSContext, o: PointerClick) -> JSResult<JSValue> {
        let (x, y, window) = self.page_point(&o.at, "at", o.window)?;
        let button = mouse_button(&o.button)?;
        let click_count = o.count.unwrap_or(1);
        if click_count == 0 {
            return Err(auto_err("count must be greater than zero"));
        }
        app_mouse(
            &ctx,
            window,
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
        let (from_x, from_y, window) = self.page_point(&o.from, "from", o.window)?;
        let (to_x, to_y, _) = self.page_point(&o.to, "to", window.clone())?;
        let button = mouse_button(&o.button)?;
        app_mouse(
            &ctx,
            window,
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
        let (x, y, window) = self.page_point(&o.at, "at", o.window)?;
        app_mouse(
            &ctx,
            window,
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

#[js_class(clone)]
pub(crate) struct JSPageKey {
    lxapp: std::sync::Weak<lxapp::LxApp>,
}

impl JSPageKey {
    pub(crate) fn new(lxapp: std::sync::Weak<lxapp::LxApp>) -> Self {
        Self { lxapp }
    }

    /// This lxapp when its current page renders on the GPU.
    fn gpu_app(&self) -> Option<Arc<lxapp::LxApp>> {
        self.lxapp
            .upgrade()
            .filter(|app| lxapp::automation::page_is_gpu(app, None))
    }
}

/// A window key press as the DOM key name a GPU page's focused element takes. A GPU page
/// is driven in-process so a covered window is never activated; only Shift chords exist there.
fn gpu_key_name(key: &str, modifiers: &[keyboard::AppKeyboardModifier]) -> Result<String, String> {
    let mut shift = false;
    for modifier in modifiers {
        match modifier {
            keyboard::AppKeyboardModifier::Shift => shift = true,
            other => {
                return Err(format!(
                    "E_UNSUPPORTED: modifier {other:?} on a GPU page; press the control's key directly"
                ));
            }
        }
    }
    let key = key.trim();
    let name = match key.to_ascii_lowercase().as_str() {
        "return" | "enter" => "Enter",
        "tab" => "Tab",
        "escape" | "esc" => "Escape",
        "space" => "Space",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        _ if key.chars().count() == 1 => key,
        _ => return Err(format!("E_UNSUPPORTED: key {key:?} on a GPU page")),
    };
    Ok(if shift {
        format!("Shift+{name}")
    } else {
        name.to_owned()
    })
}

#[cfg(test)]
mod gpu_key_tests {
    use super::gpu_key_name;
    use lingxia_platform::traits::keyboard::AppKeyboardModifier;

    #[test]
    fn window_key_names_map_to_the_gpu_page_vocabulary() {
        assert_eq!(gpu_key_name("return", &[]).unwrap(), "Enter");
        assert_eq!(gpu_key_name("down", &[]).unwrap(), "ArrowDown");
        assert_eq!(gpu_key_name("ArrowLeft", &[]).unwrap(), "ArrowLeft");
        assert_eq!(gpu_key_name("End", &[]).unwrap(), "End");
        assert_eq!(gpu_key_name("s", &[]).unwrap(), "s");
        assert_eq!(
            gpu_key_name("tab", &[AppKeyboardModifier::Shift]).unwrap(),
            "Shift+Tab"
        );
    }

    #[test]
    fn a_chord_a_gpu_page_cannot_take_is_refused_not_sent_to_the_window() {
        let err = gpu_key_name("a", &[AppKeyboardModifier::Command]).unwrap_err();
        assert!(err.starts_with("E_UNSUPPORTED"), "{err}");
        let err = gpu_key_name("backspace", &[]).unwrap_err();
        assert!(err.starts_with("E_UNSUPPORTED"), "{err}");
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::windows_window_id_matches;

    #[test]
    fn windows_window_ids_accept_decimal_and_hex_handles() {
        assert!(windows_window_id_matches("12322762", 12_322_762));
        assert!(windows_window_id_matches("0xBC07CA", 12_322_762));
        assert!(!windows_window_id_matches("0xBC07CB", 12_322_762));
    }
}

#[derive(FromJSObject)]
struct KeyType {
    text: String,
    raw: Option<bool>,
    window: Option<String>,
}

#[derive(FromJSObject)]
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
        if self.gpu_app().is_some() {
            require_host_context(&ctx)?;
            return Err(auto_err(
                "E_UNSUPPORTED: a GPU page takes text through locator type or fill",
            ));
        }
        app_keyboard(
            &ctx,
            o.window,
            keyboard::AppKeyboardAction::Type {
                text: o.text,
                raw: o.raw.unwrap_or(false),
            },
        )
        .await
    }

    #[js_method]
    async fn press(&self, ctx: JSContext, o: KeyPress) -> JSResult<JSValue> {
        let modifiers = keyboard_modifiers(&o.modifiers)?;
        if let Some(app) = self.gpu_app() {
            require_host_context(&ctx)?;
            let key = gpu_key_name(&o.key, &modifiers).map_err(auto_err)?;
            lxapp::automation::page_press(&app, None, &key, None, None)
                .await
                .map_err(auto_err)?;
            use lingxia_platform::traits::screenshot::AppScreenshot;
            let platform =
                lxapp::get_platform().ok_or_else(|| auto_err("platform is not initialized"))?;
            let window = platform
                .resolve_app_window(o.window.as_deref())
                .await
                .map_err(|err| auto_err(err.to_string()))?;
            return json_to_js(
                &ctx,
                &serde_json::json!({ "window_id": window.id, "action": "press" }),
            );
        }
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
