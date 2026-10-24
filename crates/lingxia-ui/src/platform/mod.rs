//! The contract a platform implements to host GPU Views.
//!
//! The shared core owns lifecycle tickets ([`crate::session::lifecycle`]),
//! input and accessibility policy ([`crate::interaction`],
//! [`crate::semantics`]) and frame preparation ([`crate::ViewSession`]). A
//! platform supplies only the OS mechanics below and feeds OS events back as
//! ingress calls ([`crate::interaction::view::ViewInteraction::pointer`],
//! `key`, `accessibility`, `explore`). [`crate::session::controller`] composes
//! the pieces for a single-threaded host and is the reference for adapters
//! that split preparation and presentation across threads.
use crate::DisplayList;
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{AccessibilityEffect, Dispatch, Effect};
use crate::semantics::projection::Projection;
use crate::session::lifecycle::SurfaceTicket;

#[cfg(target_os = "android")]
pub mod android;

/// Puts prepared pixels on screen. Returns false when the target was not ready.
pub trait Presenter {
    fn present(&mut self, display: DisplayList, surface: SurfaceTicket) -> Result<bool, String>;
}

/// Loads authorized image bytes without blocking; completion arrives through
/// [`crate::session::controller::ViewController::image_loaded`] with the same id.
pub trait ImageProvider {
    fn fetch(&mut self, id: u64, source: &str);
    /// The request became obsolete; drop its transport.
    fn cancel(&mut self, id: u64);
}

/// Publishes the shared semantic projection through the OS accessibility API.
pub trait AccessibilitySink {
    fn publish(&mut self, projection: &Projection<'_>, events: &[AccessibilityEffect]);
}

/// The OS text input connection; editor policy itself stays in the session.
pub trait InputMethod {
    /// Reconcile the input connection with a presented frame's focused editor.
    fn presented(&mut self, frame: &PresentedFrame);
    /// Focus or place the caret in an editor ([`Dispatch::Editor`]).
    fn focus(&mut self, editor: &Dispatch);
}

/// Pressed/focus feedback, haptics, gesture claims and fling physics.
pub trait Feedback {
    fn apply(&mut self, effects: &[Effect]);
}

/// Monotonic time for animation and presentation scheduling.
pub trait Clock {
    fn now(&self) -> std::time::Duration;
}

/// Everything a platform implements.
pub trait PlatformAdapter:
    Presenter + ImageProvider + AccessibilitySink + InputMethod + Feedback + Clock
{
}
impl<T: Presenter + ImageProvider + AccessibilitySink + InputMethod + Feedback + Clock>
    PlatformAdapter for T
{
}
