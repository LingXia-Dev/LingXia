use super::app::Platform;
use super::with_env;
use crate::error::PlatformError;
use crate::traits::screenshot::{AppScreenshot, WindowInfo};
use async_trait::async_trait;
use base64::Engine as _;
use jni::objects::{JClass, JValue};
use jni::{jni_sig, jni_str};
use serde::Deserialize;

/// Envelope shape emitted by `AppScreenshot.kt`. The Java side always
/// reports `success=true` to `NativeApi.onCallback` and encodes the real
/// outcome in this JSON so failure messages survive the JNI hop intact
/// (the `success=false` branch of `on_callback` only carries a u32 code).
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
        // Android apps are activity-per-window; tracking richer info requires
        // ActivityLifecycleCallbacks instrumentation. For now report a single
        // "main" entry so the cross-platform API stays consistent — the
        // window selector is ignored on Android anyway.
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
        // Mobile is single-window; selector is informational only.
        let _ = window_id;
        let envelope_json = crate::rt::native_call(|callback_id| {
            let screenshot_class: &JClass =
                super::get_cached_class(super::CachedClass::AppScreenshot)
                    .map_err(|e| PlatformError::Platform(e.to_string()))?;
            with_env(|env| -> Result<(), PlatformError> {
                env.call_static_method(
                    screenshot_class,
                    jni_str!("captureWindow"),
                    jni_sig!("(J)V"),
                    &[JValue::Long(callback_id as i64)],
                )?;
                Ok(())
            })
            .map_err(|e| {
                PlatformError::Platform(format!("Failed to dispatch captureWindow: {}", e))
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
