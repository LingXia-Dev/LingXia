use crate::error::PlatformError;

#[derive(Debug, Clone, PartialEq)]
pub enum VideoPlayerCommand {
    Play,
    Pause,
    /// Stop playback and release decoder resources immediately if possible.
    Stop,

    /// Seek to a specific time in seconds.
    Seek {
        position: f64,
    },

    /// Provide an external duration for stream/piped playback (seconds).
    /// Use `0` to clear.
    SetDuration {
        duration: f64,
    },

    EnterFullscreen,

    ExitFullscreen,
}

/// Handle to a SameLevel video player instance.
/// Commands go through this handle without needing to re-specify the component ID.
/// Note: Native component lifecycle is owned by the UI layer, not Rust.
pub trait VideoPlayerHandle: Send + Sync {
    /// Dispatch a control command to this player.
    fn dispatch(&self, command: VideoPlayerCommand) -> Result<(), PlatformError>;
}

/// Generic video player handle implementation.
/// Platform implementations provide a dispatch callback.
pub struct VideoPlayerHandleImpl<D>
where
    D: Fn(VideoPlayerCommand) -> Result<(), PlatformError> + Send + Sync,
{
    dispatch_fn: D,
}

impl<D> VideoPlayerHandleImpl<D>
where
    D: Fn(VideoPlayerCommand) -> Result<(), PlatformError> + Send + Sync,
{
    pub fn new(dispatch_fn: D) -> Self {
        Self { dispatch_fn }
    }
}

impl<D> VideoPlayerHandle for VideoPlayerHandleImpl<D>
where
    D: Fn(VideoPlayerCommand) -> Result<(), PlatformError> + Send + Sync,
{
    fn dispatch(&self, command: VideoPlayerCommand) -> Result<(), PlatformError> {
        (self.dispatch_fn)(command)
    }
}

/// Platform-facing API for binding to SameLevel video instances.
/// Note: Native player creation is handled by the UI layer, not Rust.
pub trait VideoPlayerManager: Send + Sync + 'static {
    /// Bind to an existing native player.
    ///
    /// Returns a handle for dispatching commands to the player.
    /// The native player must already be created by the UI layer (native component).
    fn bind_player(&self, component_id: &str) -> Result<Box<dyn VideoPlayerHandle>, PlatformError>;

    /// Set (or update) the callback ID for video player events for a component.
    ///
    /// Default implementation is a no-op for platforms that don't support callbacks.
    fn set_player_callback(
        &self,
        _component_id: &str,
        _callback_id: u64,
    ) -> Result<(), PlatformError> {
        Ok(())
    }
}
