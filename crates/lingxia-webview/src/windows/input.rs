//! Windows input automation over the Chrome DevTools Protocol.
//!
//! Element geometry comes from the shared JS input helper (viewport CSS
//! pixels), and the events themselves go through WebView2's
//! `CallDevToolsProtocolMethod` (`Input.dispatchMouseEvent` /
//! `Input.dispatchKeyEvent` / `Input.insertText`) — trusted browser-level
//! input, the same channel Playwright drives, not page-synthesized JS
//! events. CDP targets the renderer directly, so no OS focus or cursor
//! movement is needed.

use super::*;
use crate::input_helper::build_helper_invocation;
use crate::{
    ClickOptions, PointerButton, PointerInput, PressOptions, ScrollOptions, TypeOptions,
    WebViewInputError,
};
use serde::Deserialize;
use std::time::Duration;

const LEFT_CLICK_SEQUENCE: [(&str, u8); 2] = [("mousePressed", 1), ("mouseReleased", 0)];

#[derive(Debug, Deserialize)]
struct InputHelperElementResult {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    count: usize,
    #[serde(default)]
    index: usize,
    #[serde(default, rename = "centerX")]
    center_x: f64,
    #[serde(default, rename = "centerY")]
    center_y: f64,
    #[serde(default, rename = "viewportWidth")]
    viewport_width: f64,
    #[serde(default, rename = "viewportHeight")]
    viewport_height: f64,
    #[serde(default)]
    visible: bool,
    #[serde(default)]
    editable: bool,
}

/// CDP identity of a non-character key: (key, code, Windows virtual-key).
fn special_key(normalized: &str) -> Option<(&'static str, &'static str, u32)> {
    match normalized {
        "enter" | "return" => Some(("Enter", "Enter", 0x0D)),
        "tab" => Some(("Tab", "Tab", 0x09)),
        "space" => Some((" ", "Space", 0x20)),
        "backspace" | "delete" => Some(("Backspace", "Backspace", 0x08)),
        "forwarddelete" => Some(("Delete", "Delete", 0x2E)),
        "escape" | "esc" => Some(("Escape", "Escape", 0x1B)),
        "home" => Some(("Home", "Home", 0x24)),
        "end" => Some(("End", "End", 0x23)),
        "pageup" => Some(("PageUp", "PageUp", 0x21)),
        "pagedown" => Some(("PageDown", "PageDown", 0x22)),
        "arrowleft" | "left" => Some(("ArrowLeft", "ArrowLeft", 0x25)),
        "arrowup" | "up" => Some(("ArrowUp", "ArrowUp", 0x26)),
        "arrowright" | "right" => Some(("ArrowRight", "ArrowRight", 0x27)),
        "arrowdown" | "down" => Some(("ArrowDown", "ArrowDown", 0x28)),
        _ => None,
    }
}

/// Text a special key inserts (drives the `text` field of the CDP keyDown,
/// which is what makes the renderer commit the edit).
fn special_key_text(code: &str) -> Option<&'static str> {
    match code {
        "Enter" => Some("\r"),
        "Tab" => Some("\t"),
        "Space" => Some(" "),
        _ => None,
    }
}

fn scroll_delta(center: f64, viewport: f64) -> f64 {
    if !center.is_finite() || !viewport.is_finite() || viewport <= 0.0 {
        return 0.0;
    }
    (center - (viewport / 2.0)).clamp(-900.0, 900.0)
}

impl WebViewInner {
    fn cdp(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> std::result::Result<(), WebViewInputError> {
        self.dispatch_cdp_command(method, params)
            .map(|_| ())
            .map_err(|err| WebViewInputError::Platform(err.to_string()))
    }

    fn query_helper_element(
        &self,
        selector: &str,
        index: Option<usize>,
    ) -> std::result::Result<InputHelperElementResult, WebViewInputError> {
        let selector_json = serde_json::to_string(selector)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector: {err}")))?;
        let index_json = serde_json::to_string(&index)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector index: {err}")))?;
        let expr = format!("window.__LingXiaInput.query_box({selector_json}, {index_json})");
        let value = self
            .dispatch_eval_command(build_helper_invocation(&expr))
            .map_err(WebViewInputError::Script)?;
        let result: InputHelperElementResult = serde_json::from_value(value).map_err(|err| {
            WebViewInputError::Platform(format!(
                "Failed to decode input helper element result: {err}"
            ))
        })?;
        if !result.ok {
            return Err(WebViewInputError::ElementNotFound(
                result.error.unwrap_or_else(|| {
                    format!(
                        "selector matched {} element(s), index {} is unavailable: {}",
                        result.count, result.index, selector
                    )
                }),
            ));
        }
        Ok(result)
    }

    fn focus_helper_element(
        &self,
        selector: &str,
        index: Option<usize>,
    ) -> std::result::Result<(), WebViewInputError> {
        let selector_json = serde_json::to_string(selector)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector: {err}")))?;
        let index_json = serde_json::to_string(&index)
            .map_err(|err| WebViewInputError::Platform(format!("Invalid selector index: {err}")))?;
        let expr = format!("window.__LingXiaInput.focus({selector_json}, {index_json})");
        let value = self
            .dispatch_eval_command(build_helper_invocation(&expr))
            .map_err(WebViewInputError::Script)?;
        let result: InputHelperElementResult = serde_json::from_value(value).map_err(|err| {
            WebViewInputError::Platform(format!("Failed to decode input focus result: {err}"))
        })?;
        if result.ok {
            Ok(())
        } else {
            Err(WebViewInputError::ElementNotInteractable(
                result
                    .error
                    .unwrap_or_else(|| format!("Element cannot be focused: {selector}")),
            ))
        }
    }

    fn dispatch_mouse_wheel(
        &self,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    ) -> std::result::Result<(), WebViewInputError> {
        self.cdp(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mouseWheel",
                "x": x,
                "y": y,
                "deltaX": dx,
                "deltaY": dy,
            }),
        )
    }

    async fn ensure_element_visible(
        &self,
        selector: &str,
        index: Option<usize>,
    ) -> std::result::Result<InputHelperElementResult, WebViewInputError> {
        for _ in 0..12 {
            let result = self.query_helper_element(selector, index)?;
            if result.visible {
                return Ok(result);
            }
            let dx = scroll_delta(result.center_x, result.viewport_width);
            let dy = scroll_delta(result.center_y, result.viewport_height);
            if dx.abs() < 1.0 && dy.abs() < 1.0 {
                break;
            }
            self.dispatch_mouse_wheel(
                result.viewport_width / 2.0,
                result.viewport_height / 2.0,
                dx,
                dy,
            )?;
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
        let result = self.query_helper_element(selector, index)?;
        if result.visible {
            Ok(result)
        } else {
            Err(WebViewInputError::ElementNotInteractable(format!(
                "Element not visible: {selector}"
            )))
        }
    }

    fn dispatch_click_at(&self, x: f64, y: f64) -> std::result::Result<(), WebViewInputError> {
        for (kind, buttons) in LEFT_CLICK_SEQUENCE {
            self.cdp(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": kind,
                    "x": x,
                    "y": y,
                    "button": "left",
                    "buttons": buttons,
                    "clickCount": 1,
                }),
            )?;
        }
        Ok(())
    }

    pub(crate) async fn click_inner(
        &self,
        selector: &str,
        options: ClickOptions,
    ) -> std::result::Result<(), WebViewInputError> {
        let result = self.ensure_element_visible(selector, options.index).await?;
        self.dispatch_click_at(result.center_x, result.center_y)
    }

    pub(crate) async fn type_text_inner(
        &self,
        selector: &str,
        text: &str,
        options: TypeOptions,
    ) -> std::result::Result<(), WebViewInputError> {
        let result = self.ensure_element_visible(selector, options.index).await?;
        if !result.editable {
            return Err(WebViewInputError::ElementNotInteractable(format!(
                "Element is not editable: {selector}"
            )));
        }
        self.dispatch_click_at(result.center_x, result.center_y)?;
        tokio::time::sleep(Duration::from_millis(32)).await;
        if options.replace {
            // Select the focused control's content; the native insertText
            // below then replaces the selection.
            let _ = self
                .dispatch_eval_command("document.execCommand('selectAll')".to_string())
                .map_err(WebViewInputError::Script)?;
            if text.is_empty() {
                return self.press_inner("backspace", PressOptions::default()).await;
            }
        }
        if !text.is_empty() {
            self.cdp("Input.insertText", serde_json::json!({ "text": text }))?;
        }
        Ok(())
    }

    pub(crate) async fn press_inner(
        &self,
        key: &str,
        options: PressOptions,
    ) -> std::result::Result<(), WebViewInputError> {
        if let Some(selector) = options.selector.as_deref() {
            self.ensure_element_visible(selector, options.index).await?;
            self.focus_helper_element(selector, options.index)?;
        }
        let normalized = key.trim().to_ascii_lowercase();
        if let Some((key_name, code, virtual_key)) = special_key(&normalized) {
            let mut down = serde_json::json!({
                "type": "rawKeyDown",
                "key": key_name,
                "code": code,
                "windowsVirtualKeyCode": virtual_key,
                "nativeVirtualKeyCode": virtual_key,
            });
            if let Some(text) = special_key_text(code) {
                down["type"] = serde_json::json!("keyDown");
                down["text"] = serde_json::json!(text);
            }
            self.cdp("Input.dispatchKeyEvent", down)?;
            self.cdp(
                "Input.dispatchKeyEvent",
                serde_json::json!({
                    "type": "keyUp",
                    "key": key_name,
                    "code": code,
                    "windowsVirtualKeyCode": virtual_key,
                    "nativeVirtualKeyCode": virtual_key,
                }),
            )
        } else if key.chars().count() == 1 {
            self.cdp("Input.insertText", serde_json::json!({ "text": key }))
        } else {
            Err(WebViewInputError::Unsupported(
                "unsupported Windows key name",
            ))
        }
    }

    pub(crate) async fn scroll_inner(
        &self,
        dx: f64,
        dy: f64,
        _options: ScrollOptions,
    ) -> std::result::Result<(), WebViewInputError> {
        let viewport = self
            .dispatch_eval_command("({w: window.innerWidth, h: window.innerHeight})".to_string())
            .map_err(WebViewInputError::Script)?;
        let width = viewport["w"].as_f64().unwrap_or(0.0);
        let height = viewport["h"].as_f64().unwrap_or(0.0);
        self.dispatch_mouse_wheel(width / 2.0, height / 2.0, dx, dy)
    }

    pub(crate) async fn scroll_to_inner(
        &self,
        selector: &str,
        _options: ScrollOptions,
    ) -> std::result::Result<(), WebViewInputError> {
        let _ = self.ensure_element_visible(selector, None).await?;
        Ok(())
    }
}

/// `code` and Windows virtual-key for a printable ASCII character, which
/// sites read for shortcuts; punctuation keeps an empty code (layout-specific).
fn ascii_key_code(ch: char) -> (String, u32) {
    match ch {
        'a'..='z' | 'A'..='Z' => {
            let upper = ch.to_ascii_uppercase();
            (format!("Key{upper}"), u32::from(upper))
        }
        '0'..='9' => (format!("Digit{ch}"), u32::from(ch)),
        ' ' => ("Space".to_string(), 0x20),
        _ => (String::new(), 0),
    }
}

fn cdp_button(button: PointerButton) -> (&'static str, u8) {
    match button {
        PointerButton::Left => ("left", 1),
        PointerButton::Right => ("right", 2),
        PointerButton::Middle => ("middle", 4),
    }
}

impl WebViewInner {
    fn mouse_event(
        &self,
        kind: &str,
        x: f64,
        y: f64,
        button: &str,
        buttons: u8,
        click_count: u32,
    ) -> std::result::Result<(), WebViewInputError> {
        self.cdp(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": kind,
                "x": x,
                "y": y,
                "button": button,
                "buttons": buttons,
                "clickCount": click_count,
            }),
        )
    }

    pub(crate) async fn trusted_pointer(
        &self,
        input: PointerInput,
    ) -> std::result::Result<(), WebViewInputError> {
        match input {
            PointerInput::Click {
                x,
                y,
                button,
                count,
            } => {
                let (name, mask) = cdp_button(button);
                self.mouse_event("mouseMoved", x, y, "none", 0, 0)?;
                for click in 1..=count.max(1) {
                    self.mouse_event("mousePressed", x, y, name, mask, click)?;
                    self.mouse_event("mouseReleased", x, y, name, 0, click)?;
                }
                Ok(())
            }
            PointerInput::Move { x, y } => self.mouse_event("mouseMoved", x, y, "none", 0, 0),
            PointerInput::Drag { from, to } => {
                self.mouse_event("mouseMoved", from.0, from.1, "none", 0, 0)?;
                self.mouse_event("mousePressed", from.0, from.1, "left", 1, 1)?;
                // Intermediate moves let drag thresholds and dragover fire.
                const STEPS: u32 = 8;
                for step in 1..=STEPS {
                    let t = f64::from(step) / f64::from(STEPS);
                    let x = from.0 + (to.0 - from.0) * t;
                    let y = from.1 + (to.1 - from.1) * t;
                    self.mouse_event("mouseMoved", x, y, "left", 1, 0)?;
                    tokio::time::sleep(Duration::from_millis(16)).await;
                }
                self.mouse_event("mouseReleased", to.0, to.1, "left", 0, 1)
            }
        }
    }

    /// Printable ASCII goes key by key (keydown/keypress/input/keyup); any
    /// other run is committed through an IME composition, as a CJK input
    /// method delivers it.
    pub(crate) async fn trusted_insert_text(
        &self,
        text: &str,
    ) -> std::result::Result<(), WebViewInputError> {
        let mut composed = String::new();
        let flush = |run: &mut String| -> std::result::Result<(), WebViewInputError> {
            if run.is_empty() {
                return Ok(());
            }
            let len = run.encode_utf16().count();
            self.cdp(
                "Input.imeSetComposition",
                serde_json::json!({ "text": run, "selectionStart": len, "selectionEnd": len }),
            )?;
            self.cdp("Input.insertText", serde_json::json!({ "text": run }))?;
            run.clear();
            Ok(())
        };
        for ch in text.chars() {
            if ch.is_ascii_graphic() || ch == ' ' {
                flush(&mut composed)?;
                let key = ch.to_string();
                let (code, virtual_key) = ascii_key_code(ch);
                self.cdp(
                    "Input.dispatchKeyEvent",
                    serde_json::json!({
                        "type": "keyDown",
                        "key": key,
                        "code": code,
                        "text": key,
                        "unmodifiedText": key,
                        "windowsVirtualKeyCode": virtual_key,
                        "nativeVirtualKeyCode": virtual_key,
                    }),
                )?;
                self.cdp(
                    "Input.dispatchKeyEvent",
                    serde_json::json!({
                        "type": "keyUp",
                        "key": key,
                        "code": code,
                        "windowsVirtualKeyCode": virtual_key,
                        "nativeVirtualKeyCode": virtual_key,
                    }),
                )?;
            } else if ch == '\n' {
                flush(&mut composed)?;
                self.press_inner("enter", PressOptions::default()).await?;
            } else if ch == '\t' {
                flush(&mut composed)?;
                self.press_inner("tab", PressOptions::default()).await?;
            } else if ch == '\r' {
                // Part of a CRLF; the LF presses Enter.
            } else {
                composed.push(ch);
            }
        }
        flush(&mut composed)
    }
}

#[cfg(test)]
mod tests {
    use super::{LEFT_CLICK_SEQUENCE, ascii_key_code};

    #[test]
    fn ascii_keys_carry_code_and_virtual_key() {
        assert_eq!(ascii_key_code('a'), ("KeyA".to_string(), 0x41));
        assert_eq!(ascii_key_code('7'), ("Digit7".to_string(), 0x37));
        assert_eq!(ascii_key_code(' '), ("Space".to_string(), 0x20));
        assert_eq!(ascii_key_code('@'), (String::new(), 0));
    }

    #[test]
    fn click_release_clears_button_mask() {
        assert_eq!(LEFT_CLICK_SEQUENCE[0], ("mousePressed", 1));
        assert_eq!(LEFT_CLICK_SEQUENCE[1], ("mouseReleased", 0));
    }
}
