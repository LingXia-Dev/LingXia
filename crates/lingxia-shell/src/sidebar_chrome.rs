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

/// Logical size a first launch opens the main window at before fitting the
/// screen; every desktop platform shares it so one app opens the same way.
pub const DEFAULT_MAIN_WINDOW_SIZE: (f64, f64) = (1200.0, 800.0);

pub const MIN_MAIN_WINDOW_SIZE: (f64, f64) = (480.0, 480.0);

/// Share of the work area a first-launch window may take on a small screen.
const INITIAL_WINDOW_WORK_AREA_SHARE: f64 = 0.85;

/// First-launch main window size for a work area (logical units): the default,
/// shrunk to a share of the work area on smaller screens, never below the
/// minimum unless the work area itself is smaller.
pub fn initial_main_window_size(work_width: f64, work_height: f64) -> (f64, f64) {
    let fit = |default: f64, min: f64, work: f64| {
        if !work.is_finite() || work <= 0.0 {
            return default;
        }
        default
            .min(work * INITIAL_WINDOW_WORK_AREA_SHARE)
            .max(min.min(work))
            .round()
    };
    (
        fit(
            DEFAULT_MAIN_WINDOW_SIZE.0,
            MIN_MAIN_WINDOW_SIZE.0,
            work_width,
        ),
        fit(
            DEFAULT_MAIN_WINDOW_SIZE.1,
            MIN_MAIN_WINDOW_SIZE.1,
            work_height,
        ),
    )
}

/// The user's normal (unmaximized) frame plus whether the window was
/// maximized/zoomed when last seen, so a restore reproduces both.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub maximized: bool,
}

impl WindowFrame {
    pub fn new(x: f64, y: f64, width: f64, height: f64, maximized: bool) -> Option<Self> {
        let frame = Self {
            x,
            y,
            width,
            height,
            maximized,
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

    #[test]
    fn first_launch_size_fits_the_work_area() {
        assert_eq!(initial_main_window_size(2560.0, 1400.0), (1200.0, 800.0));
        // A 1280x800 laptop: 85% of the work area on both axes.
        assert_eq!(initial_main_window_size(1280.0, 760.0), (1088.0, 646.0));
        // Never below the minimum while the work area allows it.
        assert_eq!(initial_main_window_size(500.0, 500.0), (480.0, 480.0));
        // A work area under the minimum is still honored.
        assert_eq!(initial_main_window_size(400.0, 300.0), (400.0, 300.0));
        assert_eq!(initial_main_window_size(0.0, f64::NAN), (1200.0, 800.0));
    }

    #[test]
    fn window_frame_round_trips_without_maximized_when_normal() {
        let frame = WindowFrame::new(1.0, 2.0, 300.0, 400.0, false).unwrap();
        assert!(!serde_json::to_string(&frame).unwrap().contains("maximized"));
        let parsed: WindowFrame =
            serde_json::from_str(r#"{"x":1,"y":2,"width":300,"height":400}"#).unwrap();
        assert_eq!(parsed, frame);
        let parsed: WindowFrame =
            serde_json::from_str(r#"{"x":1,"y":2,"width":300,"height":400,"maximized":true}"#)
                .unwrap();
        assert!(parsed.maximized);
    }

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
                window: WindowFrame::new(40.0, 60.0, 1200.0, 800.0, expanded),
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
                maximized: false,
            }),
        }
        .normalized();

        assert!(!state.sidebar.expanded);
        assert_eq!(state.sidebar.expanded_width, DEFAULT_EXPANDED_SIDEBAR_WIDTH);
        assert_eq!(state.window, None);
    }
}
