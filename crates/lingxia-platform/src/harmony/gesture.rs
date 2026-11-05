//! Touch gestures for automation on HarmonyOS. An app cannot inject into the system input
//! dispatcher, so a gesture goes straight to the page's own touch handling ("view"
//! delivery) through the driver the page host installs: GPU pages take timed touches.
use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::gesture::{
    AppGesture, AppGestureRequest, AppGestureResult, ChromeElement, GestureDelivery, GestureEnd,
    GestureInput, GesturePoint, GestureTarget, HostCapability, PullRefreshIndicator,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Delivers touches to the page a webtag names; installed by the page host.
pub trait GestureDriver: Send + Sync + 'static {
    /// Blocking: runs the timed `points` (page CSS pixels) and returns the events sent and
    /// the held finger, if `end` is `Hold`. `finger` continues an earlier held gesture.
    fn perform(
        &self,
        app_id: &str,
        webtag: &str,
        points: &[GesturePoint],
        end: GestureEnd,
        finger: Option<u64>,
    ) -> Result<(u32, Option<u64>), String>;
    /// The page's visible area `[left, top, width, height]` in page CSS pixels.
    fn viewport(&self, app_id: &str, webtag: &str) -> Result<[f64; 4], String>;
}

static DRIVER: OnceLock<Arc<dyn GestureDriver>> = OnceLock::new();

/// Install the gesture driver; the first call wins.
pub fn set_gesture_driver(driver: Arc<dyn GestureDriver>) {
    let _ = DRIVER.set(driver);
}

/// Each app's presented page and its pull-to-refresh indicator, as the shell last drew them.
type Indicators = HashMap<String, (String, Option<PullRefreshIndicator>)>;
static INDICATORS: Mutex<Option<Indicators>> = Mutex::new(None);

/// Record the indicator of `app_id`'s presented page; `None` when that page has none.
pub fn set_refresh_indicator(app_id: &str, webtag: &str, indicator: Option<PullRefreshIndicator>) {
    INDICATORS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get_or_insert_default()
        .insert(app_id.to_owned(), (webtag.to_owned(), indicator));
}

/// The host's own overlays (toast, modal, action sheet): the only window chrome Harmony can read,
/// and the only window target a tap reaches. Keys name the element to the shell; bounds are vp.
static OVERLAYS: Mutex<Vec<(String, ChromeElement)>> = Mutex::new(Vec::new());

#[derive(serde::Deserialize)]
struct Overlay {
    key: String,
    label: String,
    role: String,
    bounds: [f64; 4],
}

/// Replace the overlay elements with the shell's `[{key,label,role,bounds}]`.
pub fn set_host_overlays(elements: &str) {
    let Ok(elements) = serde_json::from_str::<Vec<Overlay>>(elements) else {
        log::warn!("host overlays: unreadable report");
        return;
    };
    *OVERLAYS.lock().unwrap_or_else(|p| p.into_inner()) = elements
        .into_iter()
        .map(|o| {
            let element = ChromeElement {
                label: o.label,
                role: o.role,
                scope: "window".into(),
                selected: false,
                bounds: o.bounds,
            };
            (o.key, element)
        })
        .collect();
}

/// A tap at window point `at` on a host overlay element: the shell runs that element's press.
fn overlay_tap(
    points: &[GesturePoint],
    end: GestureEnd,
) -> Result<AppGestureResult, PlatformError> {
    let unsupported = || {
        PlatformError::NotSupported(
            "window gestures reach only the host's own overlays on HarmonyOS; apps cannot inject input"
                .into(),
        )
    };
    let (first, last) = (
        points.first().ok_or_else(unsupported)?,
        points.last().ok_or_else(unsupported)?,
    );
    if end != GestureEnd::Up || (first.x - last.x).hypot(first.y - last.y) > 8.0 {
        return Err(unsupported());
    }
    let key = OVERLAYS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .rev()
        .find(|(_, e)| {
            let [x, y, w, h] = e.bounds;
            last.x >= x && last.x <= x + w && last.y >= y && last.y <= y + h
        })
        .map(|(key, _)| key.clone())
        .ok_or_else(unsupported)?;
    lingxia_webview::platform::harmony::tsfn::call_arkts("hostOverlayPress", &[&key])
        .map_err(|e| PlatformError::Platform(format!("host overlay press: {e}")))?;
    Ok(AppGestureResult {
        input: GestureInput::Touch,
        delivered: GestureDelivery::View,
        events: 2,
        duration_ms: last.at_ms,
        finger: None,
    })
}

fn driver() -> Result<Arc<dyn GestureDriver>, PlatformError> {
    DRIVER
        .get()
        .cloned()
        .ok_or_else(|| PlatformError::NotSupported("gestures need a page host".into()))
}

fn page(target: GestureTarget) -> Result<(String, String), PlatformError> {
    match target {
        GestureTarget::Page { app_id, webtag, .. } => Ok((app_id, webtag)),
        GestureTarget::Window { .. } => Err(PlatformError::NotSupported(
            "window gestures need system input injection, which HarmonyOS keeps from apps".into(),
        )),
    }
}

#[async_trait]
impl AppGesture for Platform {
    fn gesture_inputs(&self) -> Vec<GestureInput> {
        if DRIVER.get().is_some() {
            vec![GestureInput::Touch]
        } else {
            Vec::new()
        }
    }

    // Touches go straight to the page, so ArkUI chrome and `Refresh` never see them.
    fn host_capabilities(&self) -> Vec<HostCapability> {
        vec![HostCapability::RefreshIndicator]
    }

    // ArkWeb takes no injected touches.
    fn webview_page_gestures(&self) -> bool {
        false
    }

    async fn perform_app_gesture(
        &self,
        request: AppGestureRequest,
    ) -> Result<AppGestureResult, PlatformError> {
        if let Some(input) = request.input
            && input != GestureInput::Touch
        {
            return Err(PlatformError::NotSupported(format!(
                "{input:?} gestures are not available on HarmonyOS; use touch"
            )));
        }
        if matches!(request.target, GestureTarget::Window { .. }) {
            let points = request
                .shape
                .points()
                .map_err(PlatformError::InvalidParameter)?;
            return overlay_tap(&points, request.end);
        }
        let driver = driver()?;
        let (app_id, webtag) = page(request.target)?;
        let points = request
            .shape
            .points()
            .map_err(PlatformError::InvalidParameter)?;
        let duration_ms = points.last().map_or(0.0, |p| p.at_ms);
        let (end, finger) = (request.end, request.finger);
        let (events, finger) = tokio::task::spawn_blocking(move || {
            driver.perform(&app_id, &webtag, &points, end, finger)
        })
        .await
        .map_err(|e| PlatformError::Platform(format!("gesture worker: {e}")))?
        .map_err(PlatformError::Platform)?;
        Ok(AppGestureResult {
            input: GestureInput::Touch,
            delivered: GestureDelivery::View,
            events,
            duration_ms,
            finger,
        })
    }

    async fn gesture_viewport(&self, target: GestureTarget) -> Result<[f64; 4], PlatformError> {
        let driver = driver()?;
        let (app_id, webtag) = page(target)?;
        tokio::task::spawn_blocking(move || driver.viewport(&app_id, &webtag))
            .await
            .map_err(|e| PlatformError::Platform(format!("gesture worker: {e}")))?
            .map_err(PlatformError::Platform)
    }

    async fn pull_refresh_indicator(
        &self,
        app_id: &str,
        webtag: &str,
    ) -> Result<Option<PullRefreshIndicator>, PlatformError> {
        let indicators = INDICATORS.lock().unwrap_or_else(|p| p.into_inner());
        match indicators.as_ref().and_then(|all| all.get(app_id)) {
            Some((shown, indicator)) if shown == webtag => Ok(indicator.clone()),
            Some(_) => Err(PlatformError::Platform(format!(
                "page {webtag} is not the presented page"
            ))),
            None => Ok(None),
        }
    }

    // Only the host's own overlays are readable: they sit at window scope.
    async fn chrome_elements(
        &self,
        scope: Option<String>,
    ) -> Result<Vec<ChromeElement>, PlatformError> {
        match scope.as_deref() {
            None | Some("window") => Ok(OVERLAYS
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .iter()
                .map(|(_, e)| e.clone())
                .collect()),
            Some(other) => Err(PlatformError::NotSupported(format!(
                "HarmonyOS reads only the host's own overlays (window scope), not {other}"
            ))),
        }
    }
}
