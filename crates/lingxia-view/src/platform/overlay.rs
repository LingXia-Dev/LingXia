//! The window overlay surface: a transparent View surface over one window, above every LingXia
//! surface in it and below the system viewers. It knows nothing of what its page shows; its Logic
//! is whoever opened it.
use super::Task;
use crate::session::logic::{ToLogic, ToView};
use crate::session::state::StateWrite;
use std::sync::Arc;
use std::time::Duration;

/// A host window, as the overlay host names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

/// Where input on the overlay goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayInput {
    /// Every touch, click and wheel reaches the window below, as if the overlay were not there.
    PassThrough,
    /// The overlay takes all input over the whole window.
    Block,
}

/// What the overlay's page sends its Logic, in order, from the View worker.
pub type OverlayLogic = Arc<dyn Fn(Vec<ToLogic>) + Send + Sync>;

/// The host's overlays. Any thread.
pub trait WindowOverlays: Send + Sync + 'static {
    /// The window showing `appid`'s presentation; the main window for `None` or an lxapp with none.
    fn window_of(&self, appid: Option<&str>) -> Option<WindowId>;
    /// Put a transparent overlay over `window` running the View page `module`, whose Logic is
    /// `logic`. It starts passing input through, without focus, and takes the window's
    /// environment (insets, keyboard, appearance, text scale, size) as a page does. `logic` is
    /// called from the View worker, never inside `open`.
    fn open(
        &self,
        window: WindowId,
        module: &'static str,
        logic: OverlayLogic,
    ) -> Result<Box<dyn OverlaySurface>, String>;
    /// Run `task` once, `delay` from now, on any thread but never inside this call.
    fn after(&self, delay: Duration, task: Task);
}

/// One open overlay; dropping it removes the surface and ends its page.
pub trait OverlaySurface: Send + Sync {
    /// A state write for the page, applied in order with [`Self::send`].
    fn write_state(&self, write: StateWrite);
    fn send(&self, message: ToView);
    fn set_input(&self, input: OverlayInput);
    /// Focused, the overlay takes key focus and the soft keyboard goes; unfocused, focus returns
    /// to what had it.
    fn set_focus(&self, focused: bool);
    /// An automation request as a GPU page answers it (`ViewHost::automation_request`, plus
    /// `{op: "screenshot"}`), answered once from any thread after the presented frame.
    fn automation(&self, request: serde_json::Value, reply: OverlayAutomationReply);
}

/// Answers one overlay automation request.
pub type OverlayAutomationReply = Box<dyn FnOnce(Result<serde_json::Value, String>) + Send>;
