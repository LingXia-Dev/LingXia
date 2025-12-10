use crate::error::PlatformError;
use crate::traits::{
    VideoPlayerCommand, VideoPlayerHandle, VideoPlayerHandleImpl, VideoPlayerManager,
};

use super::Platform;

#[cfg(target_os = "ios")]
use super::ffi;

#[cfg(target_os = "ios")]
/// iOS implementation delegates to SameLevel video components.
/// Native player is created by UI layer; this registers callback and returns a handle.
impl VideoPlayerManager for Platform {
    fn bind_player(
        &self,
        component_id: &str,
        event_callback_id: u64,
    ) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        if !ffi::set_video_player_callback(component_id, event_callback_id) {
            return Err(PlatformError::Platform(
                "Failed to register video player callback".to_string(),
            ));
        }

        let cid = component_id.to_string();
        let handle = VideoPlayerHandleImpl::new(move |command| {
            let (name, params_json) = map_command_to_ios(command);
            ffi::dispatch_video_command(&cid, &name, &params_json)
                .then_some(())
                .ok_or_else(|| PlatformError::Platform(format!("Failed to dispatch {}", name)))
        });
        Ok(Box::new(handle))
    }
}

#[cfg(target_os = "ios")]
fn map_command_to_ios(command: VideoPlayerCommand) -> (String, String) {
    const EMPTY: &str = "{}";

    match command {
        VideoPlayerCommand::Play => ("play".into(), EMPTY.into()),
        VideoPlayerCommand::Pause => ("pause".into(), EMPTY.into()),
        VideoPlayerCommand::Stop => ("stop".into(), EMPTY.into()),
        VideoPlayerCommand::Seek { position } => {
            ("seek".into(), format!(r#"{{"time":{}}}"#, position))
        }
        VideoPlayerCommand::EnterFullscreen => ("enterFullscreen".into(), EMPTY.into()),
        VideoPlayerCommand::ExitFullscreen => ("exitFullscreen".into(), EMPTY.into()),
    }
}

#[cfg(not(target_os = "ios"))]
impl VideoPlayerManager for Platform {
    fn bind_player(
        &self,
        _component_id: &str,
        _event_callback_id: u64,
    ) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        Err(PlatformError::Platform(
            "Video player control is not supported on this platform".to_string(),
        ))
    }
}
