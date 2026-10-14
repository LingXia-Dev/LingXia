use crate::error::PlatformError;

/// Output hardware the device has, read once per Logic context for
/// `lx.supports`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioDeviceCapabilities {
    pub earpiece: bool,
    pub proximity: bool,
}

/// Native half of the host audio coordinator (`lingxia_media::audio`).
///
/// Commands and events are JSON objects (`{"op": ...}` / `{"event": ...}`);
/// the coordinator owns the protocol. `audio_send` must not call back into the
/// event callback synchronously: the coordinator runs on one worker thread and
/// treats native events as input posted after the command returns.
pub trait AudioBackend: Send + Sync + 'static {
    fn audio_capabilities(&self) -> AudioDeviceCapabilities {
        AudioDeviceCapabilities::default()
    }

    /// Hand native the persistent callback that receives every audio event.
    /// Native answers with an initial `route` event.
    fn audio_attach(&self, _event_callback_id: u64) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "audio is not supported on this platform".to_string(),
        ))
    }

    /// Deliver one command, in order, without waiting for its effect.
    fn audio_send(&self, _command_json: &str) -> Result<(), PlatformError> {
        Err(PlatformError::NotSupported(
            "audio is not supported on this platform".to_string(),
        ))
    }
}

static EVENT_CALLBACK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Remember the coordinator's event callback for natives that live in Rust
/// (the Windows backend, Rust-hosted video players).
pub fn set_audio_event_callback(callback_id: u64) {
    EVENT_CALLBACK.store(callback_id, std::sync::atomic::Ordering::Release);
}

/// Post one native audio event. False until the coordinator has attached.
pub fn emit_audio_event(event_json: String) -> bool {
    let id = EVENT_CALLBACK.load(std::sync::atomic::Ordering::Acquire);
    id != 0 && lingxia_messaging::invoke_callback(id, Ok(event_json))
}

/// A native video started or stopped producing audible output. Muted video
/// reports `false`: it does not take audio focus.
pub fn notify_video_audible(component_id: &str, audible: bool) -> bool {
    emit_audio_event(
        serde_json::json!({
            "event": "video",
            "componentId": component_id,
            "playing": audible,
        })
        .to_string(),
    )
}
