use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::screenshot::{AppScreenshot, WindowInfo};
use async_trait::async_trait;
use base64::Engine as _;
use serde::Deserialize;

/// Envelope shape emitted by `AppScreenshot.ets`. The ArkTS side always
/// reports `success=true` to `onCallback` and encodes the real outcome in
/// this JSON, because the `success=false` branch of
/// `lingxia/src/ffi/harmony.rs::on_callback` parses data as a u32 error
/// code and drops the diagnostic string.
#[derive(Deserialize)]
struct ScreenshotEnvelope {
    ok: bool,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[async_trait]
impl AppScreenshot for Platform {
    async fn list_app_windows(&self) -> Result<Vec<WindowInfo>, PlatformError> {
        // Harmony apps typically have a single window per ability; surface
        // a single "main" entry so the cross-platform API is consistent.
        // (Enumerating via `window.getAllWindows` is feasible if we ever
        // need to drive multi-window setups from a host app.)
        Ok(vec![WindowInfo {
            id: "main".to_string(),
            title: String::new(),
            focused: true,
            main: true,
            visible: true,
            width: 0,
            height: 0,
        }])
    }

    async fn take_app_screenshot(&self, window_id: Option<&str>) -> Result<Vec<u8>, PlatformError> {
        let _ = window_id;
        let envelope_json = crate::rt::native_call(|callback_id| {
            let id = callback_id.to_string();
            lingxia_webview::platform::harmony::tsfn::call_arkts("captureAppScreenshot", &[&id])
                .map_err(|e| {
                    PlatformError::Platform(format!(
                        "Failed to dispatch captureAppScreenshot: {}",
                        e
                    ))
                })
        })
        .await?;

        let envelope: ScreenshotEnvelope = serde_json::from_str(&envelope_json).map_err(|e| {
            PlatformError::Platform(format!(
                "Failed to parse app screenshot envelope: {} (payload: {})",
                e, envelope_json
            ))
        })?;
        if !envelope.ok {
            return Err(PlatformError::Platform(
                envelope
                    .error
                    .unwrap_or_else(|| "unknown error".to_string()),
            ));
        }
        let data = envelope
            .data
            .ok_or_else(|| PlatformError::Platform("envelope ok=true but no data".to_string()))?;
        base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|e| {
                PlatformError::Platform(format!("Failed to decode app screenshot base64: {}", e))
            })
    }
}
