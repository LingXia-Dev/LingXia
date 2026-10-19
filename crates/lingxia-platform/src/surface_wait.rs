//! Bookkeeping for surfaces whose WebView is not ready when a layout plan
//! wants to present them.
//!
//! A plan can be committed many times while one surface's WebView is still
//! being created (every sidebar click, every resize). Each commit must not
//! start another poll for the same surface, and when the WebView does show
//! up it must be presented at the placement the *latest* plan asked for, not
//! the one the first poll was started with. A surface whose WebView never
//! appears must not keep the record alive either: the caller closes it once
//! the wait gives up, so later plans stop queueing on it and the owner's
//! handle learns it is gone.

use std::collections::HashMap;

/// Pending presentations by surface id, each holding the latest placement.
#[derive(Debug, Default)]
pub struct SurfaceWaitRegistry<T> {
    pending: HashMap<String, T>,
}

impl<T> SurfaceWaitRegistry<T> {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Record that `id` wants to be presented as `target` once its WebView
    /// is ready. Returns `true` when the caller must start the wait: a wait
    /// already in flight picks up the newer target by itself.
    pub fn request(&mut self, id: &str, target: T) -> bool {
        let fresh = !self.pending.contains_key(id);
        self.pending.insert(id.to_string(), target);
        fresh
    }

    /// Whether a wait for `id` is in flight.
    pub fn is_pending(&self, id: &str) -> bool {
        self.pending.contains_key(id)
    }

    /// End the wait for `id` and hand back the latest target to present.
    pub fn take(&mut self, id: &str) -> Option<T> {
        self.pending.remove(id)
    }

    /// Drop the wait for `id` without presenting (the surface closed, or the
    /// wait gave up).
    pub fn cancel(&mut self, id: &str) -> bool {
        self.pending.remove(id).is_some()
    }
}

/// What a wait does when its deadline passes with no WebView.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceWaitTimeout {
    /// The surface still exists: close it, so the record does not outlive a
    /// WebView that never came and later plans stop waiting on it.
    CloseSurface,
    /// The surface was closed while waiting; nothing is left to do.
    AlreadyGone,
}

/// Decide the timeout outcome from whether the surface record still exists.
pub fn surface_wait_timeout(surface_exists: bool) -> SurfaceWaitTimeout {
    if surface_exists {
        SurfaceWaitTimeout::CloseSurface
    } else {
        SurfaceWaitTimeout::AlreadyGone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_request_starts_a_wait_and_the_latest_target_wins() {
        let mut waits = SurfaceWaitRegistry::new();
        // The plan that opened the surface starts the wait.
        assert!(waits.request("browser-1", "bottom"));
        // Every later plan while the WebView is still coming does not start
        // another poll, but updates where the surface should land.
        assert!(!waits.request("browser-1", "right"));
        assert!(!waits.request("browser-1", "top"));
        assert!(waits.is_pending("browser-1"));

        // The WebView arrives: one presentation, at the newest placement.
        assert_eq!(waits.take("browser-1"), Some("top"));
        assert!(!waits.is_pending("browser-1"));
        assert_eq!(waits.take("browser-1"), None);

        // A plan after the wait ended starts a fresh wait when needed.
        assert!(waits.request("browser-1", "left"));
    }

    #[test]
    fn a_surface_whose_webview_never_comes_is_closed_and_not_waited_on_again() {
        let mut waits = SurfaceWaitRegistry::new();
        assert!(waits.request("browser-1", "bottom"));
        assert!(!waits.request("browser-1", "bottom"));

        // The deadline passes with the record still there: the wait ends and
        // the caller closes the surface.
        assert!(waits.cancel("browser-1"));
        assert_eq!(surface_wait_timeout(true), SurfaceWaitTimeout::CloseSurface);
        assert!(!waits.is_pending("browser-1"));
        // Once closed there is no record for a later plan to present, and a
        // second cancel is a no-op.
        assert!(!waits.cancel("browser-1"));
        assert_eq!(surface_wait_timeout(false), SurfaceWaitTimeout::AlreadyGone);
    }

    #[test]
    fn closing_a_surface_while_its_webview_is_awaited_ends_the_wait() {
        let mut waits = SurfaceWaitRegistry::new();
        assert!(waits.request("browser-1", "bottom"));
        assert!(waits.request("browser-2", "right"));
        assert!(waits.cancel("browser-1"));
        assert!(!waits.is_pending("browser-1"));
        // Other surfaces' waits are untouched.
        assert_eq!(waits.take("browser-2"), Some("right"));
    }
}
