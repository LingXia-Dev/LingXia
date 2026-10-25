//! The contract a platform implements to host GPU Views.
//!
//! [`crate::session::host::ViewHost`] owns the View worker, frame sequencing,
//! lifecycle tickets, input, image and bridge policy. A platform supplies only
//! the OS mechanics below and feeds OS events back as `ViewHost` calls. Unless
//! a method says otherwise the host calls it on the platform's main thread, and
//! never while holding its own locks, so an adapter may call back into the host.
use crate::DisplayList;
use crate::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use crate::session::host::{Failure, FrameSnapshot, FrameTiming};
use crate::session::lifecycle::{CommitTicket, ConnectionTicket, SurfaceTicket};
use std::time::Duration;

#[cfg(any(target_os = "android", feature = "jvm"))]
pub mod android;

pub type Task = Box<dyn FnOnce() + Send>;

/// Viewport and View environment sampled for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameEnvironment {
    pub width: u16,
    pub height: u16,
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// The View `env` object (theme, insets, keyboard, text scale, os, pointer, ...).
    pub value: serde_json::Value,
}

/// Encodes frames on the View worker and puts them on screen on the main thread.
pub trait Presenter {
    type Frame: Send + 'static;
    /// View worker. `None` when the target cannot take a frame now.
    fn prepare(
        &self,
        display: DisplayList,
        surface: SurfaceTicket,
        generation: CommitTicket,
    ) -> Result<Option<Self::Frame>, String>;
    /// View worker. Backend metrics merged into the frame snapshot.
    fn metrics(&self, _frame: &Self::Frame) -> Option<serde_json::Value> {
        None
    }
    /// False when the target was lost; the host prepares again.
    fn present(&self, frame: Self::Frame) -> Result<bool, String>;
}

/// Display-synchronized callbacks.
pub trait FrameScheduler {
    /// Call [`crate::session::host::ViewHost::vsync`] on the next display frame.
    fn request_vsync(&self);
}

/// Callable from any thread.
pub trait MainThread {
    fn post(&self, task: Task);
    fn post_after(&self, delay: Duration, task: Task);
}

pub trait EnvironmentSource {
    /// `None` until the surface is measured.
    fn environment(&self) -> Option<FrameEnvironment>;
}

/// Loads authorized image bytes without blocking; completion arrives through
/// [`crate::session::host::ViewHost::image_loaded`] with the same id.
pub trait ImageProvider {
    fn fetch(&self, id: u64, source: &str);
    /// The request became obsolete; drop its transport.
    fn cancel(&self, id: u64);
}

/// Publishes accessibility events; the OS pulls nodes through
/// [`crate::session::host::ViewHost::semantics`].
pub trait AccessibilitySink {
    fn announce(&self, events: &[AccessibilityEffect]);
    /// An activation ran a click handler.
    fn clicked(&self);
}

/// The OS text input connection; editor policy itself stays in the session.
pub trait InputMethod {
    /// Reconcile the input connection with a presented frame's focused editor.
    fn reconcile(&self, frame: &FrameSnapshot);
    /// Focus or place the caret in an editor ([`Dispatch::Editor`]).
    fn focus(&self, editor: &Dispatch, frame: &FrameSnapshot);
    /// A non-editor activation is about to run; end text input.
    fn activation(&self);
    /// The page instance ended; drop the connection without blurring.
    fn reset(&self);
}

/// Pressed/focus feedback, haptics, gesture claims and fling physics.
pub trait Feedback {
    fn apply(&self, effects: &[Effect]);
}

/// Page-level outcomes the embedding host reacts to.
pub trait HostEvents {
    fn loaded(&self);
    fn presented(&self, frame: &FrameSnapshot, timing: &FrameTiming);
    /// An outbound Logic bridge message for `connection`.
    fn message(&self, connection: ConnectionTicket, message: &str);
    /// Restart the Logic connection after a session failure; false when it cannot.
    fn restart(&self, connection: ConnectionTicket) -> bool;
    /// Show host recovery chrome, or clear it with `None`.
    fn recovery(&self, failure: Option<&Failure>);
    /// The bridge has not become ready within the bootstrap deadline, or it has now.
    fn bootstrap_overdue(&self, overdue: bool);
    fn warn(&self, message: &str);
}

/// Monotonic time for animation and presentation scheduling. Any thread.
pub trait Clock {
    fn now(&self) -> Duration;
}

/// Everything a platform implements.
pub trait PlatformAdapter:
    Presenter
    + FrameScheduler
    + MainThread
    + EnvironmentSource
    + ImageProvider
    + AccessibilitySink
    + InputMethod
    + Feedback
    + HostEvents
    + Clock
    + Send
    + Sync
    + 'static
{
}
impl<
    T: Presenter
        + FrameScheduler
        + MainThread
        + EnvironmentSource
        + ImageProvider
        + AccessibilitySink
        + InputMethod
        + Feedback
        + HostEvents
        + Clock
        + Send
        + Sync
        + 'static,
> PlatformAdapter for T
{
}
