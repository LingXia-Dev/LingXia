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

pub(crate) mod calendar;
mod chrome;
mod environment;
mod overlay;
pub use chrome::HostChrome;
pub use environment::{DeviceFacts, obscured, touch_target};
pub use overlay::{
    OverlayAutomationReply, OverlayInput, OverlayLogic, OverlaySurface, WindowId, WindowOverlays,
};
#[cfg(test)]
pub(crate) use environment::test_environment;

pub type Task = Box<dyn FnOnce() + Send>;

/// Viewport and View environment sampled for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameEnvironment {
    pub width: u16,
    pub height: u16,
    /// Device pixels per logical pixel.
    pub scale: f32,
    /// The View `env` snapshot, before the session adds its own keys.
    pub value: crate::protocol::env::Environment,
    /// A tab bar floating over the page, logical pixels: the page draws its material in this shape.
    pub tab_bar: Option<ChromeShape>,
}

/// Host chrome over the page: its box `[x, y, width, height]` from the surface's top left,
/// and its corner radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChromeShape {
    pub rect: [f32; 4],
    pub radius: f32,
}

/// Encodes frames on the View worker and puts them on screen on the main thread.
pub trait Presenter {
    type Frame: Send + 'static;
    /// View worker. `None` when the target cannot take a frame now. The host may drop a
    /// prepared frame without presenting it (it was superseded); dropping the value must
    /// free whatever target it holds.
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
    /// Main thread, before `present`: whether presenting only queues the frame (e.g. to a
    /// raster thread). The host then holds further frames until the adapter reports
    /// [`ViewHost::deferred_presented`](crate::session::host::ViewHost::deferred_presented).
    fn deferred(&self, _frame: &Self::Frame) -> bool {
        false
    }
    /// Main thread: drop memory kept only to speed up coming frames; the page is hidden
    /// or the system is short of memory.
    fn release_transient(&self) {}
}

/// Display-synchronized callbacks.
pub trait FrameScheduler {
    /// Call [`crate::session::host::ViewHost::vsync`] on the next display frame.
    fn request_vsync(&self);
    /// Renderer work on a display frame that needs no View frame, such as showing a playing
    /// video's new frame; true asks for the next display frame too.
    fn tick(&self) -> bool {
        false
    }
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
    /// Resolves `<video src>` under the page's authority; `None` plays sources as given.
    fn video_resolver(&self) -> Option<crate::scene::video::VideoResolver> {
        None
    }
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
    /// Apply an accessibility or automation edit to `node`'s editor: replace its
    /// text, or with `text` absent select `start..end` (-1, -1: the end).
    fn edit_text(&self, node: usize, text: Option<&str>, start: i32, end: i32) -> EditOutcome;
}

/// What became of an [`InputMethod::edit_text`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditOutcome {
    /// Accepted; it shows in a coming frame.
    Applied,
    /// The editor is still connecting; the core sends it again shortly.
    NotReady,
    /// It can never apply (not editable, invalid text or range).
    Failed,
}

/// Pressed/focus feedback, haptics, gesture claims and fling physics.
pub trait Feedback {
    fn apply(&self, effects: &[Effect]);
}

/// Page-level outcomes the embedding host reacts to.
pub trait HostEvents {
    fn loaded(&self);
    fn presented(&self, frame: &FrameSnapshot, timing: &FrameTiming);
    /// The View worker thread: what the View sends Logic for `connection`, in order. It goes
    /// straight to Logic's queue; waiting on the main thread would add a hop per event.
    fn logic(&self, connection: ConnectionTicket, messages: Vec<crate::session::logic::ToLogic>);
    /// What became of Logic's typed state writes for `connection`.
    fn state(&self, connection: ConnectionTicket, signal: crate::session::state::StateSignal);
    /// Restart the Logic connection after a session failure; false when it cannot.
    fn restart(&self, connection: ConnectionTicket) -> bool;
    /// Show host recovery chrome, or clear it with `None`.
    fn recovery(&self, failure: Option<&Failure>);
    /// Put this page on screen in place of the outgoing one, from [`ViewHost::reveal`].
    /// `hold_surface`: no current frame yet, so keep the target hidden until a
    /// later `revealed(false)`.
    ///
    /// [`ViewHost::reveal`]: crate::session::host::ViewHost::reveal
    fn revealed(&self, hold_surface: bool) {
        let _ = hold_surface;
    }
    /// After [`ViewHost::cover`], a frame reached the screen at the current size: drop the
    /// host's snapshot.
    ///
    /// [`ViewHost::cover`]: crate::session::host::ViewHost::cover
    fn uncover(&self) {}
    /// Put `text` on the system clipboard (an accessibility Copy).
    fn copy_text(&self, text: &str) {
        let _ = text;
    }
    /// Show, move or hide (`None`) the floating text menu.
    fn text_menu(&self, menu: Option<&crate::session::host::TextMenu>) {
        let _ = menu;
    }
    fn warn(&self, message: &str);
    /// Main thread: a page shown and already drawn presented a frame `owed` after it was asked
    /// for, past [`crate::session::host::FRAME_GAP`].
    fn frame_gap(&self, _owed: Duration) {}
    /// The View worker thread: a line the page's `console` wrote.
    fn console(&self, level: log::Level, message: &str) {
        log::log!(target: "lingxia_view::console", level, "{message}");
    }
    /// Diagnostic detail (one line per executed click, say); adapters log it at debug level.
    fn debug(&self, _message: &str) {}
    /// Main thread: the host chrome this page's palette asks for, on change and before the
    /// frame that first uses it.
    fn host_chrome(&self, _chrome: &HostChrome) {}
    /// Main thread: reconcile mounted `<webview>` instances with the presented frame.
    fn sync_webviews(&self, _mounts: &[crate::scene::webview::WebViewMount]) {}
    /// Main thread: picture each of `web_ids` at `scale` device pixels per point and hand the
    /// PNGs to [`crate::session::host::ViewHost::webview_picture`]. Without it a web view that
    /// cannot show live draws its box's background instead.
    fn picture_webviews(&self, _web_ids: &[String], _scale: f32) {}
    /// Main thread: exit web-initiated fullscreen on `web_id`.
    fn exit_webview_fullscreen(&self, _web_id: &str) {}
    /// Main thread: tear down all `<webview>` mounts on page reload, park or dispose.
    fn clear_webviews(&self) {}
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
