use crate::error::PlatformError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Keyboard modifier used by app-level devtool input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppKeyboardModifier {
    Command,
    Shift,
    Option,
    Control,
}

/// App-window keyboard action delivered to the focused native control.
///
/// This is app/window-level, not LxApp-level: it drives whatever control
/// currently holds first responder (a native NSTextField address bar, a
/// WebView input, etc.), matching [`AppMouse`](super::mouse::AppMouse).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppKeyboardAction {
    /// Type literal text; drives `insertText:` on the focused control.
    Type { text: String },
    /// Press a named key (e.g. `return`, `tab`, `escape`) with modifiers.
    Press {
        key: String,
        #[serde(default)]
        modifiers: Vec<AppKeyboardModifier>,
    },
}

impl AppKeyboardAction {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Type { .. } => "type",
            Self::Press { .. } => "press",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppKeyboardRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    pub action: AppKeyboardAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppKeyboardResult {
    pub window_id: String,
    pub action: String,
    /// Reliability of requested modifier chords, when modifiers were present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier_reliability: Option<String>,
}

/// Dispatch keyboard input to the host app's focused window.
///
/// Symmetric to [`AppMouse`](super::mouse::AppMouse): app/window-level so it
/// can target native chrome, WebViews, and desktop shells consistently.
#[async_trait]
pub trait AppKeyboard: Send + Sync {
    async fn perform_app_keyboard(
        &self,
        request: AppKeyboardRequest,
    ) -> Result<AppKeyboardResult, PlatformError> {
        let _ = request;
        Err(PlatformError::NotSupported(
            "app keyboard input is not implemented for this platform".to_string(),
        ))
    }
}
