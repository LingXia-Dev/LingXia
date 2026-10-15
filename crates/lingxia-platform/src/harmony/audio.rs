use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::audio::{AudioBackend, AudioDeviceCapabilities};
use crate::traits::video_player::VideoPlayerCommand;

fn call(name: &str, args: &[&str]) -> Result<(), PlatformError> {
    lingxia_webview::platform::harmony::tsfn::call_arkts(name, args)
        .map_err(|e| PlatformError::Platform(format!("{name} failed: {e}")))
}

impl AudioBackend for Platform {
    fn audio_capabilities(&self) -> AudioDeviceCapabilities {
        // Phones have a receiver and a proximity sensor; tablets and 2-in-1s do not.
        let phone = super::device::device_type().is_some_and(|kind| kind == "phone");
        AudioDeviceCapabilities {
            earpiece: phone,
            proximity: phone,
        }
    }

    fn audio_attach(&self, event_callback_id: u64) -> Result<(), PlatformError> {
        call("audioAttach", &[&event_callback_id.to_string()])
    }

    fn audio_send(&self, command_json: &str) -> Result<(), PlatformError> {
        // Video playback lives in Rust (OH_AVPlayer), so its pause never visits ArkTS.
        if let Ok(command) = serde_json::from_str::<serde_json::Value>(command_json)
            && command["op"] == "video.pause"
        {
            let component_id = command["componentId"].as_str().unwrap_or_default();
            return super::video_player::dispatch_command(component_id, VideoPlayerCommand::Pause);
        }
        call("audioSend", &[command_json])
    }
}
