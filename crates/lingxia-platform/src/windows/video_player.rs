//! Video-player command routing for Windows.
//!
//! The logic layer's video context (`lx.createVideoContext`) binds a player
//! by native-component id and dispatches [`VideoPlayerCommand`]s through it.
//! Native players are owned by the UI layer's component host (the shell's
//! `video.native` manager), which the platform layer deliberately cannot
//! see — the host registers a process-wide dispatcher here instead, the
//! same inversion the app-exit and open-url handlers use.

use std::sync::{Arc, OnceLock};

use crate::error::PlatformError;
use crate::traits::video_player::{
    VideoPlayerCommand, VideoPlayerHandle, VideoPlayerHandleImpl, VideoPlayerManager,
};

use super::app::Platform;

/// Routes one command to the native video component with the given id.
/// Returns a human-readable error when the component does not exist.
pub type WindowsVideoCommandDispatcher =
    Arc<dyn Fn(&str, &VideoPlayerCommand) -> Result<(), String> + Send + Sync>;

static DISPATCHER: OnceLock<WindowsVideoCommandDispatcher> = OnceLock::new();

/// Registers the dispatcher that delivers video-player commands to native
/// `video.native` components. Called once by the UI layer when its
/// native-component host installs.
pub fn register_windows_video_command_dispatcher(dispatcher: WindowsVideoCommandDispatcher) {
    if DISPATCHER.set(dispatcher).is_err() {
        log::warn!("a Windows video command dispatcher is already registered; ignoring");
    }
}

impl VideoPlayerManager for Platform {
    fn bind_player(&self, component_id: &str) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        let dispatcher = DISPATCHER
            .get()
            .ok_or_else(|| {
                PlatformError::NotSupported(
                    "no native video host is registered on Windows".to_string(),
                )
            })?
            .clone();
        let component_id = component_id.to_string();
        Ok(Box::new(VideoPlayerHandleImpl::new(move |command| {
            dispatcher(&component_id, &command).map_err(PlatformError::Platform)
        })))
    }
}
