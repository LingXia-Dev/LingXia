//! Touch gestures for automation on HarmonyOS. An app cannot inject into the system input
//! dispatcher, so a gesture goes straight to the page's own touch handling ("view"
//! delivery) through the driver the page host installs: GPU pages take timed touches.
use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::gesture::{
    AppGesture, AppGestureRequest, AppGestureResult, GestureDelivery, GestureEnd, GestureInput,
    GesturePoint, GestureTarget, HostCapability, PullRefreshIndicator,
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
}
