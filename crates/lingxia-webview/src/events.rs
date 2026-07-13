//! Typed WebView delegate events: correlated navigation lifecycle, observable
//! state snapshots, and the canonical derived-state folds every consumer must
//! use instead of hand-rolled equivalents.

pub(crate) mod normalizer;

/// Register a read-only observer for a WebView's delivered events
/// (automation waits, devtools). Observers run after the delegate, in
/// registration order, on the same delivery drain.
pub use normalizer::add_observer;

use crate::traits::LoadError;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Process-unique identity of one accepted top-level navigation attempt.
///
/// Allocated by the event normalizer from a process-wide monotonic sequence;
/// never reused within a process, never persistent across launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavigationId(u64);

static NAVIGATION_ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl NavigationId {
    /// Allocate the next process-wide id. Normalizer-internal.
    pub(crate) fn next() -> Self {
        Self(NAVIGATION_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed))
    }

    pub fn get(self) -> u64 {
        self.0
    }

    /// Construct an arbitrary id in consumer unit tests.
    #[cfg(feature = "test-support")]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

/// Formats as `nav#42` for logs and diagnostics.
impl fmt::Display for NavigationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nav#{}", self.0)
    }
}

/// Why an active navigation attempt terminated without success or failure.
/// Cancellation is control flow, not a load error: it must never surface
/// error UI or count as a failed visit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationCancellationReason {
    /// A newer navigation replaced this attempt.
    Superseded,
    /// The caller explicitly stopped loading.
    Stopped,
    /// The WebView was destroyed while the attempt was active.
    WebViewDestroyed,
    /// The backend reported cancellation but cannot distinguish the cause.
    Other,
}

/// Top-level navigation lifecycle. Every `Started` receives exactly one
/// terminal `Succeeded`, `Failed`, or `Cancelled` with the same id.
///
/// - `requested_url` is the initially requested URL — non-empty, never
///   updated on redirects, and not the final URL.
/// - `Succeeded.final_url` is the non-empty top-level URL after redirects and
///   is authoritative for persistence (`Location` state is authoritative for
///   live display).
/// - `Failed.error.failing_url` is the one authoritative failure URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationEvent {
    Started {
        id: NavigationId,
        requested_url: String,
    },
    Succeeded {
        id: NavigationId,
        final_url: String,
    },
    Failed {
        id: NavigationId,
        error: LoadError,
    },
    Cancelled {
        id: NavigationId,
        reason: NavigationCancellationReason,
    },
}

impl NavigationEvent {
    pub fn id(&self) -> NavigationId {
        match self {
            NavigationEvent::Started { id, .. }
            | NavigationEvent::Succeeded { id, .. }
            | NavigationEvent::Failed { id, .. }
            | NavigationEvent::Cancelled { id, .. } => *id,
        }
    }

    pub fn is_terminal(&self) -> bool {
        !matches!(self, NavigationEvent::Started { .. })
    }
}

/// Observable WebView state snapshots. Not lifecycle transitions: `Location`
/// alone is never evidence of a successful visit, and `None` explicitly
/// clears a previously reported title/favicon (empty strings and empty byte
/// arrays are not sentinels).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebViewStateChange {
    Location {
        url: String,
    },
    Title {
        /// `None` means the current document has no reported title.
        title: Option<String>,
    },
    Favicon {
        /// PNG bytes. `None` explicitly clears a previously reported favicon.
        png_bytes: Option<Vec<u8>>,
    },
    BackForwardAvailability {
        can_go_back: bool,
        can_go_forward: bool,
    },
}

/// A borrowed view of one delivered event, for read-only observers
/// (automation waits, devtools) that watch a WebView without owning it.
/// (`WebViewEvent` is taken by the creation-stage event in `webview.rs`.)
pub enum WebViewObservedEvent<'a> {
    Navigation(&'a NavigationEvent),
    State(&'a WebViewStateChange),
}

/// Read-only event observer. Observers run after the delegate returns, in
/// registration order, on the same delivery drain; they cannot affect
/// delivery and must not block.
pub type WebViewEventObserver = Arc<dyn Fn(WebViewObservedEvent<'_>) + Send + Sync>;

/// Attempt bookkeeping every consumer otherwise re-implements: because
/// attempts may overlap (WebView2), a terminal event for an older attempt
/// must not clear loading UI for the newest one.
#[derive(Debug, Default)]
pub struct NavigationProgress {
    newest: Option<NavigationId>,
    newest_terminal: bool,
}

impl NavigationProgress {
    /// Fold one event into the progress state.
    pub fn apply(&mut self, event: &NavigationEvent) {
        match event {
            NavigationEvent::Started { id, .. } => {
                self.newest = Some(*id);
                self.newest_terminal = false;
            }
            terminal => {
                if self.newest == Some(terminal.id()) {
                    self.newest_terminal = true;
                }
            }
        }
    }

    /// True while the newest attempt has no terminal event.
    pub fn is_loading(&self) -> bool {
        self.newest.is_some() && !self.newest_terminal
    }

    /// The newest attempt, until its terminal arrives.
    pub fn current(&self) -> Option<NavigationId> {
        if self.newest_terminal {
            None
        } else {
            self.newest
        }
    }

    /// Whether `id` is the newest attempt (terminal or not).
    pub fn is_current(&self, id: NavigationId) -> bool {
        self.newest == Some(id)
    }
}

/// Fold of `WebViewStateChange` into the current observed state, including
/// the `None`-clears semantics, so all consumers interpret clearing the same
/// way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservedWebViewState {
    pub url: Option<String>,
    pub title: Option<String>,
    pub favicon_png: Option<Vec<u8>>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

impl ObservedWebViewState {
    /// Fold one change into the state. Takes the change by value so owned
    /// payloads are retained without cloning.
    pub fn apply(&mut self, change: WebViewStateChange) {
        match change {
            WebViewStateChange::Location { url } => self.url = Some(url),
            WebViewStateChange::Title { title } => self.title = title,
            WebViewStateChange::Favicon { png_bytes } => self.favicon_png = png_bytes,
            WebViewStateChange::BackForwardAvailability {
                can_go_back,
                can_go_forward,
            } => {
                self.can_go_back = can_go_back;
                self.can_go_forward = can_go_forward;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{LoadError, LoadErrorKind};

    fn id(raw: u64) -> NavigationId {
        NavigationId(raw)
    }

    fn started(raw: u64) -> NavigationEvent {
        NavigationEvent::Started {
            id: id(raw),
            requested_url: format!("https://example.com/{raw}"),
        }
    }

    fn succeeded(raw: u64) -> NavigationEvent {
        NavigationEvent::Succeeded {
            id: id(raw),
            final_url: format!("https://example.com/{raw}"),
        }
    }

    #[test]
    fn navigation_id_displays_for_diagnostics() {
        assert_eq!(id(42).to_string(), "nav#42");
    }

    #[test]
    fn progress_tracks_single_attempt() {
        let mut progress = NavigationProgress::default();
        assert!(!progress.is_loading());
        progress.apply(&started(1));
        assert!(progress.is_loading());
        assert_eq!(progress.current(), Some(id(1)));
        progress.apply(&succeeded(1));
        assert!(!progress.is_loading());
        assert_eq!(progress.current(), None);
        assert!(progress.is_current(id(1)));
    }

    #[test]
    fn terminal_for_older_attempt_keeps_newest_loading() {
        let mut progress = NavigationProgress::default();
        progress.apply(&started(1));
        progress.apply(&started(2));
        progress.apply(&NavigationEvent::Cancelled {
            id: id(1),
            reason: NavigationCancellationReason::Superseded,
        });
        assert!(progress.is_loading());
        assert_eq!(progress.current(), Some(id(2)));
        assert!(!progress.is_current(id(1)));
    }

    #[test]
    fn failed_terminal_ends_loading_for_current_attempt() {
        let mut progress = NavigationProgress::default();
        progress.apply(&started(1));
        progress.apply(&NavigationEvent::Failed {
            id: id(1),
            error: LoadError {
                failing_url: Some("https://example.com/1".into()),
                kind: LoadErrorKind::Network,
                description: "boom".into(),
            },
        });
        assert!(!progress.is_loading());
    }

    #[test]
    fn observed_state_applies_none_clears() {
        let mut state = ObservedWebViewState::default();
        state.apply(WebViewStateChange::Title {
            title: Some("Example".into()),
        });
        state.apply(WebViewStateChange::Favicon {
            png_bytes: Some(vec![1, 2, 3]),
        });
        state.apply(WebViewStateChange::Location {
            url: "https://example.com/".into(),
        });
        state.apply(WebViewStateChange::BackForwardAvailability {
            can_go_back: true,
            can_go_forward: false,
        });
        assert_eq!(state.title.as_deref(), Some("Example"));
        assert_eq!(state.favicon_png.as_deref(), Some(&[1u8, 2, 3][..]));
        assert!(state.can_go_back);

        state.apply(WebViewStateChange::Title { title: None });
        state.apply(WebViewStateChange::Favicon { png_bytes: None });
        assert_eq!(state.title, None);
        assert_eq!(state.favicon_png, None);
        assert_eq!(state.url.as_deref(), Some("https://example.com/"));
    }
}
