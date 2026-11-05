//! Real pointer gestures: a timed path injected into the host window, so page
//! chrome (pull to refresh, scroll edges, flings, edge back) handles it as it
//! would a user's.
use crate::error::PlatformError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The device a gesture imitates; `None` in a request picks the platform's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GestureInput {
    Touch,
    Mouse,
    Trackpad,
}

/// Host automation beyond page gestures; a spec that `requires` one is skipped where it is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HostCapability {
    /// [`AppGesture::chrome_elements`] reads the host's bars and window controls.
    Chrome,
    /// [`AppGesture::pull_refresh_indicator`] reads a page's pull-to-refresh indicator.
    RefreshIndicator,
    /// Injected input reaches the host's own views around the page: chrome, pull to refresh.
    HostInput,
}

/// Where the gesture lands. Page points are page CSS pixels of the page
/// instance `webtag` names; window points are the window-content unit of
/// [`AppMouseAction`](super::mouse::AppMouseAction).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GestureTarget {
    Page {
        app_id: String,
        webtag: String,
        /// The page's native view where the platform cannot find it by
        /// `webtag` (macOS WebView pages: the `WKWebView`).
        #[serde(default, skip_serializing)]
        view: Option<usize>,
    },
    Window {
        window_id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GestureEnd {
    /// Lift the pointer at the last point.
    #[default]
    Up,
    /// Cancel the gesture (Android `ACTION_CANCEL`, a cancelled trackpad phase).
    Cancel,
    /// Keep the pointer down; the result's `finger` continues it.
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlingDirection {
    Up,
    Down,
    Left,
    Right,
}

/// What the pointer does, before it is expanded into timed points.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GestureShape {
    /// Straight move from `from` to `to` over `duration_ms`, then still for
    /// `hold_ms` so the release carries no velocity.
    Line {
        from: [f64; 2],
        to: [f64; 2],
        duration_ms: f64,
        #[serde(default)]
        steps: Option<u32>,
        #[serde(default)]
        hold_ms: f64,
    },
    /// Released while moving at `velocity` px/s in `direction` (the finger's).
    Fling {
        from: [f64; 2],
        direction: FlingDirection,
        velocity: f64,
    },
    /// The pointer stays at `at` for `duration_ms`.
    Hold { at: [f64; 2], duration_ms: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppGestureRequest {
    pub target: GestureTarget,
    #[serde(default)]
    pub input: Option<GestureInput>,
    /// A held pointer from an earlier `Hold` result; the shape continues it
    /// without a new down.
    #[serde(default)]
    pub finger: Option<u64>,
    pub shape: GestureShape,
    #[serde(default)]
    pub end: GestureEnd,
}

/// How the events reached the app: `Window` through the platform's input
/// dispatch, `View` straight into the target view's handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GestureDelivery {
    Window,
    View,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppGestureResult {
    pub input: GestureInput,
    pub delivered: GestureDelivery,
    pub events: u32,
    pub duration_ms: f64,
    /// Set when the request ended with `Hold`.
    pub finger: Option<u64>,
}

/// A page's pull-to-refresh indicator; `offset` is page CSS pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PullRefreshIndicator {
    pub phase: String,
    pub offset: f64,
}

/// Host chrome a user can press, found through the host's own view or
/// accessibility tree; `bounds` is `[x, y, width, height]` in window points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChromeElement {
    pub label: String,
    pub role: String,
    /// `tabBar`, `navigationBar`, `capsule`, `sidebar`, `toolbar` or `window`.
    pub scope: String,
    pub selected: bool,
    pub bounds: [f64; 4],
}

/// One timed point; `at_ms` counts from the gesture's first point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GesturePoint {
    pub x: f64,
    pub y: f64,
    pub at_ms: f64,
}

/// Interval between moves when the request names no step count.
pub const FRAME_MS: f64 = 16.0;
const FLING_MS: f64 = 80.0;
const MAX_POINTS: usize = 2_000;

impl GestureShape {
    /// The timed points of the shape, starting at `at_ms = 0`.
    pub fn points(&self) -> Result<Vec<GesturePoint>, String> {
        let finite = |p: &[f64; 2], name: &str| {
            if p.iter().all(|v| v.is_finite()) {
                Ok(())
            } else {
                Err(format!("{name}: coordinates must be finite"))
            }
        };
        let duration = |ms: f64, name: &str| {
            if ms.is_finite() && (0.0..=60_000.0).contains(&ms) {
                Ok(ms)
            } else {
                Err(format!("{name} must be between 0 and 60000 ms"))
            }
        };
        let mut out = Vec::new();
        match self {
            GestureShape::Line {
                from,
                to,
                duration_ms,
                steps,
                hold_ms,
            } => {
                finite(from, "from")?;
                finite(to, "to")?;
                let total = duration(*duration_ms, "durationMs")?;
                let hold = duration(*hold_ms, "hold")?;
                let steps = match steps {
                    Some(0) => return Err("steps must be at least 1".into()),
                    Some(n) => *n as usize,
                    None => ((total / FRAME_MS).ceil() as usize).max(1),
                };
                out.push(point(*from, 0.0));
                for i in 1..=steps {
                    let t = i as f64 / steps as f64;
                    out.push(point(lerp(*from, *to, t), total * t));
                }
                push_hold(&mut out, *to, total, hold);
            }
            GestureShape::Fling {
                from,
                direction,
                velocity,
            } => {
                finite(from, "from")?;
                if !velocity.is_finite() || *velocity <= 0.0 || *velocity > 50_000.0 {
                    return Err("velocity must be between 0 and 50000 px/s".into());
                }
                let travel = velocity * FLING_MS / 1000.0;
                let (dx, dy) = match direction {
                    FlingDirection::Up => (0.0, -travel),
                    FlingDirection::Down => (0.0, travel),
                    FlingDirection::Left => (-travel, 0.0),
                    FlingDirection::Right => (travel, 0.0),
                };
                let to = [from[0] + dx, from[1] + dy];
                let steps = (FLING_MS / 8.0) as usize;
                out.push(point(*from, 0.0));
                for i in 1..=steps {
                    let t = i as f64 / steps as f64;
                    out.push(point(lerp(*from, to, t), FLING_MS * t));
                }
            }
            GestureShape::Hold { at, duration_ms } => {
                finite(at, "at")?;
                let total = duration(*duration_ms, "durationMs")?;
                out.push(point(*at, 0.0));
                push_hold(&mut out, *at, 0.0, total);
            }
        }
        if out.len() > MAX_POINTS {
            return Err(format!(
                "gesture has {} points (max {MAX_POINTS})",
                out.len()
            ));
        }
        Ok(out)
    }

    /// Fling start as the far side of a region of `size`, so the travel fits.
    pub fn fling_start(direction: FlingDirection, size: [f64; 2]) -> [f64; 2] {
        let [w, h] = size;
        match direction {
            FlingDirection::Up => [w / 2.0, h * 0.8],
            FlingDirection::Down => [w / 2.0, h * 0.2],
            FlingDirection::Left => [w * 0.8, h / 2.0],
            FlingDirection::Right => [w * 0.2, h / 2.0],
        }
    }
}

fn point(p: [f64; 2], at_ms: f64) -> GesturePoint {
    GesturePoint {
        x: p[0],
        y: p[1],
        at_ms,
    }
}

fn lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// A still pointer still reports moves on a touch screen; repeating the point
/// is what lets a velocity tracker settle to zero before the release.
fn push_hold(out: &mut Vec<GesturePoint>, at: [f64; 2], start: f64, hold: f64) {
    let steps = (hold / FRAME_MS).ceil() as usize;
    for i in 1..=steps {
        out.push(point(at, start + (hold * i as f64 / steps as f64)));
    }
}

/// Real gesture injection into the host window.
#[async_trait]
pub trait AppGesture: Send + Sync {
    /// The devices this host can imitate; empty where it cannot inject.
    fn gesture_inputs(&self) -> Vec<GestureInput> {
        Vec::new()
    }

    /// False where page gestures reach GPU pages only (iOS has no public touch
    /// injection into a WebView).
    fn webview_page_gestures(&self) -> bool {
        true
    }

    fn host_capabilities(&self) -> Vec<HostCapability> {
        Vec::new()
    }

    async fn perform_app_gesture(
        &self,
        request: AppGestureRequest,
    ) -> Result<AppGestureResult, PlatformError> {
        let _ = request;
        Err(PlatformError::NotSupported(
            "gestures are not implemented for this platform".to_string(),
        ))
    }

    /// The visible part of the target as `[left, top, width, height]` in its
    /// own coordinates, so a gesture can start on screen.
    async fn gesture_viewport(&self, target: GestureTarget) -> Result<[f64; 4], PlatformError> {
        let _ = target;
        Err(PlatformError::NotSupported(
            "gestures are not implemented for this platform".to_string(),
        ))
    }

    /// Turn the host window to `landscape` (wider than tall) or portrait, as
    /// a desktop user resizes it; returns the new content size in points.
    async fn orient_window(&self, landscape: bool) -> Result<[f64; 2], PlatformError> {
        let _ = landscape;
        Err(PlatformError::NotSupported(
            "this host turns with its device: rotate it through the device fixture".to_string(),
        ))
    }

    /// Pressable host chrome, optionally limited to one `scope`.
    async fn chrome_elements(
        &self,
        scope: Option<String>,
    ) -> Result<Vec<ChromeElement>, PlatformError> {
        let _ = scope;
        Err(PlatformError::NotSupported(
            "host chrome is not readable on this platform".to_string(),
        ))
    }

    /// The pull-to-refresh indicator of a page, or `None` when it has none.
    async fn pull_refresh_indicator(
        &self,
        app_id: &str,
        webtag: &str,
    ) -> Result<Option<PullRefreshIndicator>, PlatformError> {
        let _ = (app_id, webtag);
        Err(PlatformError::NotSupported(
            "the pull-to-refresh indicator is not readable on this platform".to_string(),
        ))
    }

    /// A back swipe from the window's `edge`, as the system recognises one.
    async fn perform_edge_back(
        &self,
        window_id: Option<String>,
        edge: FlingDirection,
    ) -> Result<AppGestureResult, PlatformError> {
        let _ = (window_id, edge);
        Err(PlatformError::NotSupported(
            "edge back is not available on this platform".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_is_spaced_one_frame_apart_and_holds_still() {
        let points = GestureShape::Line {
            from: [10.0, 100.0],
            to: [10.0, 260.0],
            duration_ms: 320.0,
            steps: None,
            hold_ms: 48.0,
        }
        .points()
        .unwrap();
        assert_eq!(points.len(), 1 + 20 + 3);
        assert_eq!(points[0], point([10.0, 100.0], 0.0));
        assert_eq!(points[20], point([10.0, 260.0], 320.0));
        assert!(points.windows(2).all(|w| w[1].at_ms > w[0].at_ms));
        assert!(points[21..].iter().all(|p| p.y == 260.0));
        assert_eq!(points.last().unwrap().at_ms, 368.0);
    }

    #[test]
    fn explicit_steps_win() {
        let points = GestureShape::Line {
            from: [0.0, 0.0],
            to: [100.0, 0.0],
            duration_ms: 100.0,
            steps: Some(4),
            hold_ms: 0.0,
        }
        .points()
        .unwrap();
        let xs: Vec<f64> = points.iter().map(|p| p.x).collect();
        assert_eq!(xs, vec![0.0, 25.0, 50.0, 75.0, 100.0]);
    }

    #[test]
    fn fling_releases_at_the_requested_velocity() {
        let points = GestureShape::Fling {
            from: [200.0, 600.0],
            direction: FlingDirection::Up,
            velocity: 4000.0,
        }
        .points()
        .unwrap();
        let last = points[points.len() - 1];
        let prev = points[points.len() - 2];
        let velocity = (last.y - prev.y) / ((last.at_ms - prev.at_ms) / 1000.0);
        assert!((velocity + 4000.0).abs() < 1e-6, "{velocity}");
        assert_eq!(last.y, 600.0 - 320.0);
    }

    #[test]
    fn invalid_shapes_are_refused() {
        let line = |duration_ms, steps| GestureShape::Line {
            from: [0.0, 0.0],
            to: [f64::NAN, 0.0],
            duration_ms,
            steps,
            hold_ms: 0.0,
        };
        assert!(line(100.0, None).points().is_err());
        let bad_steps = GestureShape::Line {
            from: [0.0, 0.0],
            to: [1.0, 0.0],
            duration_ms: 10.0,
            steps: Some(0),
            hold_ms: 0.0,
        };
        assert!(bad_steps.points().is_err());
        let slow = GestureShape::Fling {
            from: [0.0, 0.0],
            direction: FlingDirection::Down,
            velocity: 0.0,
        };
        assert!(slow.points().is_err());
        let long = GestureShape::Hold {
            at: [0.0, 0.0],
            duration_ms: 120_000.0,
        };
        assert!(long.points().is_err());
    }

    #[test]
    fn requests_round_trip_as_json() {
        let request: AppGestureRequest = serde_json::from_value(serde_json::json!({
            "target": { "kind": "page", "app_id": "a", "webtag": "w" },
            "shape": { "kind": "line", "from": [1, 2], "to": [3, 4], "duration_ms": 100 },
        }))
        .unwrap();
        assert_eq!(request.end, GestureEnd::Up);
        assert!(request.input.is_none());
        assert!(matches!(request.shape, GestureShape::Line { hold_ms, .. } if hold_ms == 0.0));
    }
}
