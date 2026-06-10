use crate::error::PlatformError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Mouse button used by app-level devtool input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

/// App-window mouse action.
///
/// Coordinates are logical points in the target window content area, with
/// origin at the top-left corner of the same visual content captured by
/// `AppScreenshot::take_app_screenshot`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppMouseAction {
    Move {
        x: f64,
        y: f64,
    },
    Down {
        x: f64,
        y: f64,
        #[serde(default)]
        button: AppMouseButton,
    },
    Up {
        x: f64,
        y: f64,
        #[serde(default)]
        button: AppMouseButton,
    },
    Click {
        x: f64,
        y: f64,
        #[serde(default)]
        button: AppMouseButton,
        #[serde(default = "default_click_count")]
        click_count: u8,
    },
    Drag {
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
        #[serde(default)]
        button: AppMouseButton,
    },
    Scroll {
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    },
}

fn default_click_count() -> u8 {
    1
}

impl AppMouseAction {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Move { .. } => "move",
            Self::Down { .. } => "down",
            Self::Up { .. } => "up",
            Self::Click { .. } => "click",
            Self::Drag { .. } => "drag",
            Self::Scroll { .. } => "scroll",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMouseRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    pub action: AppMouseAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMouseResult {
    pub window_id: String,
    pub action: String,
}

/// Dispatch mouse input to the host app's top-level window.
///
/// This is intentionally app/window-level, not LxApp-level: it can target
/// native chrome, overlays, WebViews, and future desktop shells consistently.
#[async_trait]
pub trait AppMouse: Send + Sync {
    async fn perform_app_mouse(
        &self,
        request: AppMouseRequest,
    ) -> Result<AppMouseResult, PlatformError> {
        let _ = request;
        Err(PlatformError::NotSupported(
            "app mouse input is not implemented for this platform".to_string(),
        ))
    }
}
