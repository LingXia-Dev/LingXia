//! Desktop window state remembered across launches.
//!
//! Only the user's own sidebar choice lives here. The adaptive projection — the
//! icon rail a narrow window forces — is derived from the window every launch
//! and must never be written down, or a window that was briefly narrow would
//! teach the app to open as a rail forever.

use serde::{Deserialize, Serialize};

/// Width a first launch opens the sidebar at. The platforms agree on expanded
/// content geometry, not on the exact column width: macOS packs its window
/// chrome tighter, so the same 184 there reads as a slab next to native apps.
#[cfg(target_os = "macos")]
pub const DEFAULT_EXPANDED_SIDEBAR_WIDTH: f64 = 148.0;
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_EXPANDED_SIDEBAR_WIDTH: f64 = 184.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarChrome {
    pub expanded: bool,
    pub expanded_width: f64,
}

impl Default for SidebarChrome {
    fn default() -> Self {
        Self {
            expanded: true,
            expanded_width: DEFAULT_EXPANDED_SIDEBAR_WIDTH,
        }
    }
}

impl SidebarChrome {
    pub fn rail(&self) -> bool {
        !self.expanded
    }

    pub fn with_expanded(expanded: bool, expanded_width: f64) -> Self {
        Self {
            expanded,
            expanded_width,
        }
        .normalized()
    }

    pub(crate) fn normalized(self) -> Self {
        let expanded_width = if self.expanded_width.is_finite() && self.expanded_width > 0.0 {
            self.expanded_width
        } else {
            DEFAULT_EXPANDED_SIDEBAR_WIDTH
        };
        Self {
            expanded: self.expanded,
            expanded_width,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WindowFrame {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        let frame = Self {
            x,
            y,
            width,
            height,
        };
        frame.valid().then_some(frame)
    }

    pub fn valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShellWindowState {
    pub sidebar: SidebarChrome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowFrame>,
}

impl ShellWindowState {
    pub(crate) fn normalized(mut self) -> Self {
        self.sidebar = self.sidebar.normalized();
        self.window = self.window.filter(WindowFrame::valid);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Never having chosen is the expanded case — a first launch shows the
    /// whole sidebar rather than a rail nobody asked for.
    #[test]
    fn the_default_is_expanded() {
        assert!(SidebarChrome::default().expanded);
        assert_eq!(
            SidebarChrome::default().expanded_width,
            DEFAULT_EXPANDED_SIDEBAR_WIDTH
        );
        assert!(!SidebarChrome::default().rail());
    }

    #[test]
    fn window_state_round_trips_through_json() {
        for expanded in [false, true] {
            let saved = ShellWindowState {
                sidebar: SidebarChrome::with_expanded(expanded, 252.5),
                window: WindowFrame::new(40.0, 60.0, 1200.0, 800.0),
            };
            let raw = serde_json::to_string(&saved).expect("serialize");
            let loaded: ShellWindowState = serde_json::from_str(&raw).expect("deserialize");
            assert_eq!(loaded, saved);
            assert_eq!(loaded.sidebar.expanded, expanded);
        }
    }

    #[test]
    fn invalid_geometry_is_dropped_and_width_uses_the_default() {
        let state = ShellWindowState {
            sidebar: SidebarChrome {
                expanded: false,
                expanded_width: -10.0,
            },
            window: Some(WindowFrame {
                x: 10.0,
                y: 20.0,
                width: 0.0,
                height: 600.0,
            }),
        }
        .normalized();

        assert!(!state.sidebar.expanded);
        assert_eq!(state.sidebar.expanded_width, DEFAULT_EXPANDED_SIDEBAR_WIDTH);
        assert_eq!(state.window, None);
    }
}
