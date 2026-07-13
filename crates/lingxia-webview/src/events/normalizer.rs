//! Per-WebView event normalizer: adapters submit [`NativeSignal`]s captured on
//! the native callback thread; the normalizer owns navigation identity,
//! exactly-once terminals, orphan synthesis, document generations, state
//! coalescing, and flattened FIFO delivery to the delegate and observers.

use super::{
    NavigationCancellationReason, NavigationEvent, NavigationId, WebViewEventObserver,
    WebViewObservedEvent, WebViewStateChange,
};
use crate::traits::LoadError;
use crate::webview::{WebTag, find_webview_delegate};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

/// Backend-private correlation key (e.g. WebView2 navigation ID, WKNavigation
/// identity). Meaningless outside one adapter.
pub(crate) type NativeKey = u64;

// Variants are constructed per-platform (e.g. only WebView2 submits
// NavigationSuppressed; Apple never submits FaviconChanged), so no single
// cfg constructs the full set.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum NativeSignal {
    NavigationStarted {
        key: Option<NativeKey>,
        url: String,
    },
    /// Policy rejected this native navigation before loading; its follow-up
    /// completion callbacks are expected and must not be diagnosed.
    NavigationSuppressed {
        key: Option<NativeKey>,
    },
    /// Commit evidence: the displayed document was replaced.
    DocumentCommitted,
    NavigationFinished {
        key: Option<NativeKey>,
        result: NativeNavigationResult,
    },
    LocationChanged {
        url: String,
    },
    TitleChanged {
        title: Option<String>,
    },
    FaviconChanged {
        png_bytes: Option<Vec<u8>>,
    },
    BackForwardChanged {
        can_go_back: bool,
        can_go_forward: bool,
    },
    Destroyed,
}

#[derive(Debug)]
pub(crate) enum NativeNavigationResult {
    Succeeded { final_url: String },
    Failed(LoadError),
    Cancelled(Option<NavigationCancellationReason>),
}

enum Output {
    Nav(NavigationEvent),
    State(WebViewStateChange),
}

/// Navigation identity and exactly-once terminal bookkeeping.
#[derive(Default)]
struct NavigationTracker {
    by_key: HashMap<NativeKey, NavigationId>,
    keyless_active: Option<NavigationId>,
    /// Start order, for teardown draining.
    active: Vec<NavigationId>,
    suppressed_keys: HashSet<NativeKey>,
    /// Keyless policy suppressions: consume that many keyless finishes.
    suppressed_keyless: u32,
    /// A keyless attempt just failed; backends that also emit a bare
    /// page-finished for the failed load (Android, ArkWeb) have that late
    /// success consumed instead of synthesizing a bogus lifecycle.
    consume_next_orphan_success: bool,
    /// Recently terminated native keys: duplicate completion callbacks for
    /// an already-terminal attempt are dropped, not re-synthesized.
    recent_terminated: VecDeque<NativeKey>,
}

impl NavigationTracker {
    fn start(&mut self, webtag: &WebTag, key: Option<NativeKey>, url: String) -> Vec<Output> {
        let mut out = Vec::new();
        match key {
            Some(key) => {
                if self.by_key.contains_key(&key) {
                    // Redirect restart with the same native id: same attempt.
                    return out;
                }
                let id = NavigationId::next();
                self.by_key.insert(key, id);
                self.active.push(id);
                out.push(Output::Nav(NavigationEvent::Started {
                    id,
                    requested_url: url,
                }));
            }
            None => {
                // An ID-less backend has one correlatable attempt: a new start
                // unambiguously supersedes the previous one.
                if let Some(old) = self.keyless_active.take() {
                    self.retire(old);
                    log::debug!("{webtag}: {old} superseded by a newer navigation");
                    out.push(Output::Nav(NavigationEvent::Cancelled {
                        id: old,
                        reason: NavigationCancellationReason::Superseded,
                    }));
                }
                let id = NavigationId::next();
                self.keyless_active = Some(id);
                self.active.push(id);
                out.push(Output::Nav(NavigationEvent::Started {
                    id,
                    requested_url: url,
                }));
            }
        }
        out
    }

    fn finish(
        &mut self,
        webtag: &WebTag,
        key: Option<NativeKey>,
        result: NativeNavigationResult,
    ) -> Vec<Output> {
        // Suppressed keys complete silently — expected policy tail.
        if let Some(key) = key {
            if self.suppressed_keys.remove(&key) {
                log::debug!("{webtag}: consumed completion for suppressed native key {key}");
                return Vec::new();
            }
            if self.recent_terminated.contains(&key) {
                log::debug!("{webtag}: dropped duplicate terminal for native key {key}");
                return Vec::new();
            }
        } else if self.suppressed_keyless > 0 {
            self.suppressed_keyless -= 1;
            log::debug!("{webtag}: consumed keyless completion for suppressed navigation");
            return Vec::new();
        }

        let resolved = match key {
            Some(key) => {
                let resolved = self.by_key.remove(&key);
                if resolved.is_some() {
                    self.recent_terminated.push_back(key);
                    if self.recent_terminated.len() > 8 {
                        self.recent_terminated.pop_front();
                    }
                }
                resolved
            }
            None => self.keyless_active.take(),
        };

        let mut out = Vec::new();
        let id = match resolved {
            Some(id) => {
                self.retire(id);
                id
            }
            None => match &result {
                // A real load must never be dropped: synthesize the lifecycle.
                NativeNavigationResult::Succeeded { final_url } => {
                    if self.consume_next_orphan_success {
                        self.consume_next_orphan_success = false;
                        log::debug!("{webtag}: consumed late finish after a failed attempt");
                        return out;
                    }
                    let id = NavigationId::next();
                    log::info!(
                        "{webtag}: synthesized Started for orphan success finish ({final_url})"
                    );
                    out.push(Output::Nav(NavigationEvent::Started {
                        id,
                        requested_url: final_url.clone(),
                    }));
                    id
                }
                NativeNavigationResult::Failed(error) => {
                    let id = NavigationId::next();
                    let url = error
                        .failing_url
                        .clone()
                        .unwrap_or_else(|| "about:blank".to_string());
                    log::info!("{webtag}: synthesized Started for orphan failure finish ({url})");
                    out.push(Output::Nav(NavigationEvent::Started {
                        id,
                        requested_url: url,
                    }));
                    id
                }
                // Orphan cancellations are the expected tail of suppression
                // and teardown races.
                NativeNavigationResult::Cancelled(_) => {
                    log::debug!("{webtag}: dropped orphan cancellation finish");
                    return out;
                }
            },
        };

        out.push(Output::Nav(match result {
            NativeNavigationResult::Succeeded { final_url } => {
                self.consume_next_orphan_success = false;
                NavigationEvent::Succeeded { id, final_url }
            }
            NativeNavigationResult::Failed(error) => {
                if key.is_none() {
                    self.consume_next_orphan_success = true;
                }
                NavigationEvent::Failed { id, error }
            }
            NativeNavigationResult::Cancelled(reason) => NavigationEvent::Cancelled {
                id,
                reason: reason.unwrap_or(NavigationCancellationReason::Other),
            },
        }));
        out
    }

    fn retire(&mut self, id: NavigationId) {
        self.active.retain(|active| *active != id);
        if self.keyless_active == Some(id) {
            self.keyless_active = None;
        }
    }

    fn drain_destroyed(&mut self) -> Vec<Output> {
        let drained = std::mem::take(&mut self.active);
        self.by_key.clear();
        self.keyless_active = None;
        drained
            .into_iter()
            .map(|id| {
                Output::Nav(NavigationEvent::Cancelled {
                    id,
                    reason: NavigationCancellationReason::WebViewDestroyed,
                })
            })
            .collect()
    }
}

/// Snapshot coalescing and document-generation metadata resets.
#[derive(Default)]
struct StateCoalescer {
    url: Option<String>,
    title: Option<Option<String>>,
    favicon: Option<Option<Vec<u8>>>,
    back_forward: Option<(bool, bool)>,
}

impl StateCoalescer {
    fn location(&mut self, url: String) -> Vec<Output> {
        if self.url.as_deref() == Some(url.as_str()) {
            return Vec::new();
        }
        self.url = Some(url.clone());
        vec![Output::State(WebViewStateChange::Location { url })]
    }

    fn title(&mut self, title: Option<String>) -> Vec<Output> {
        if self.title.as_ref() == Some(&title) {
            return Vec::new();
        }
        self.title = Some(title.clone());
        vec![Output::State(WebViewStateChange::Title { title })]
    }

    fn favicon(&mut self, png_bytes: Option<Vec<u8>>) -> Vec<Output> {
        if self.favicon.as_ref() == Some(&png_bytes) {
            return Vec::new();
        }
        self.favicon = Some(png_bytes.clone());
        vec![Output::State(WebViewStateChange::Favicon { png_bytes })]
    }

    fn back_forward(&mut self, can_go_back: bool, can_go_forward: bool) -> Vec<Output> {
        if self.back_forward == Some((can_go_back, can_go_forward)) {
            return Vec::new();
        }
        self.back_forward = Some((can_go_back, can_go_forward));
        vec![Output::State(WebViewStateChange::BackForwardAvailability {
            can_go_back,
            can_go_forward,
        })]
    }

    /// Commit evidence: the displayed document was replaced, so document-scoped
    /// metadata resets — coalesced, so an already-clear field emits nothing.
    fn document_committed(&mut self) -> Vec<Output> {
        let mut out = Vec::new();
        out.extend(self.title(None));
        out.extend(self.favicon(None));
        out
    }
}

struct NormalizerState {
    tracker: NavigationTracker,
    coalescer: StateCoalescer,
    queue: VecDeque<Output>,
    draining: bool,
    observers: Vec<WebViewEventObserver>,
}

/// One normalizer per WebView, keyed by webtag.
pub(crate) struct EventNormalizer {
    webtag: WebTag,
    state: Mutex<NormalizerState>,
}

static NORMALIZERS: OnceLock<Mutex<HashMap<String, Arc<EventNormalizer>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, Arc<EventNormalizer>>> {
    NORMALIZERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn normalizer_for(webtag: &WebTag) -> Arc<EventNormalizer> {
    let mut map = registry().lock().unwrap_or_else(|e| e.into_inner());
    map.entry(webtag.key().to_string())
        .or_insert_with(|| {
            Arc::new(EventNormalizer {
                webtag: webtag.clone(),
                state: Mutex::new(NormalizerState {
                    tracker: NavigationTracker::default(),
                    coalescer: StateCoalescer::default(),
                    queue: VecDeque::new(),
                    draining: false,
                    observers: Vec::new(),
                }),
            })
        })
        .clone()
}

/// Submit a native signal for `webtag`. Payloads must already be captured on
/// the native callback thread — the normalizer never queries native objects.
pub(crate) fn submit(webtag: &WebTag, signal: NativeSignal) {
    normalizer_for(webtag).submit(signal);
}

/// Register a read-only observer for `webtag`'s events.
pub fn add_observer(webtag: &WebTag, observer: WebViewEventObserver) {
    let normalizer = normalizer_for(webtag);
    let mut state = normalizer.state.lock().unwrap_or_else(|e| e.into_inner());
    state.observers.push(observer);
}

impl EventNormalizer {
    fn submit(&self, signal: NativeSignal) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let outputs = match signal {
            NativeSignal::NavigationStarted { key, url } => {
                state.tracker.start(&self.webtag, key, url)
            }
            NativeSignal::NavigationSuppressed { key } => {
                match key {
                    Some(key) => {
                        state.tracker.suppressed_keys.insert(key);
                    }
                    None => state.tracker.suppressed_keyless += 1,
                }
                Vec::new()
            }
            NativeSignal::DocumentCommitted => state.coalescer.document_committed(),
            NativeSignal::NavigationFinished { key, result } => {
                state.tracker.finish(&self.webtag, key, result)
            }
            NativeSignal::LocationChanged { url } => state.coalescer.location(url),
            NativeSignal::TitleChanged { title } => state.coalescer.title(title),
            NativeSignal::FaviconChanged { png_bytes } => state.coalescer.favicon(png_bytes),
            NativeSignal::BackForwardChanged {
                can_go_back,
                can_go_forward,
            } => state.coalescer.back_forward(can_go_back, can_go_forward),
            NativeSignal::Destroyed => state.tracker.drain_destroyed(),
        };
        state.queue.extend(outputs);

        // Flattened, non-reentrant FIFO drain: if a delegate callback causes
        // another submission (same thread) or another thread submits while we
        // drain, those events are appended and delivered by the active drain.
        if state.draining {
            return;
        }
        state.draining = true;
        loop {
            let Some(output) = state.queue.pop_front() else {
                state.draining = false;
                break;
            };
            let observers = state.observers.clone();
            drop(state);
            self.deliver(&output, &observers);
            state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        }
    }

    fn deliver(&self, output: &Output, observers: &[WebViewEventObserver]) {
        let delegate = find_webview_delegate(&self.webtag);
        match output {
            Output::Nav(event) => {
                if let Some(delegate) = &delegate {
                    delegate.on_navigation_event(event.clone());
                }
                for observer in observers {
                    observer(WebViewObservedEvent::Navigation(event));
                }
            }
            Output::State(change) => {
                if let Some(delegate) = &delegate {
                    delegate.on_webview_state_change(change.clone());
                }
                for observer in observers {
                    observer(WebViewObservedEvent::State(change));
                }
            }
        }
    }
}

/// Remove the webtag's normalizer after draining teardown cancellations.
pub(crate) fn destroy(webtag: &WebTag) {
    submit(webtag, NativeSignal::Destroyed);
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(webtag.key());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::LoadErrorKind;

    fn capture(webtag: &WebTag) -> Arc<Mutex<Vec<String>>> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        add_observer(
            webtag,
            Arc::new(move |event| {
                let line = match event {
                    WebViewObservedEvent::Navigation(nav) => match nav {
                        NavigationEvent::Started { requested_url, .. } => {
                            format!("started:{requested_url}")
                        }
                        NavigationEvent::Succeeded { final_url, .. } => {
                            format!("succeeded:{final_url}")
                        }
                        NavigationEvent::Failed { error, .. } => {
                            format!("failed:{:?}", error.kind)
                        }
                        NavigationEvent::Cancelled { reason, .. } => {
                            format!("cancelled:{reason:?}")
                        }
                    },
                    WebViewObservedEvent::State(change) => match change {
                        WebViewStateChange::Location { url } => format!("location:{url}"),
                        WebViewStateChange::Title { title } => {
                            format!("title:{}", title.as_deref().unwrap_or("<none>"))
                        }
                        WebViewStateChange::Favicon { png_bytes } => {
                            format!("favicon:{}", png_bytes.as_ref().map_or(0, |b| b.len()))
                        }
                        WebViewStateChange::BackForwardAvailability {
                            can_go_back,
                            can_go_forward,
                        } => format!("backforward:{can_go_back},{can_go_forward}"),
                    },
                };
                sink.lock().unwrap().push(line);
            }),
        );
        events
    }

    fn tag(name: &str) -> WebTag {
        WebTag::new("test-app", name, Some(1))
    }

    fn failed(url: &str) -> NativeNavigationResult {
        NativeNavigationResult::Failed(LoadError {
            failing_url: Some(url.to_string()),
            kind: LoadErrorKind::Network,
            description: "boom".into(),
        })
    }

    #[test]
    fn keyed_success_lifecycle_and_redirect_coalescing() {
        let webtag = tag("keyed-success");
        let events = capture(&webtag);
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: Some(7),
                url: "https://a/".into(),
            },
        );
        // Redirect restart with the same native id: no second Started.
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: Some(7),
                url: "https://b/".into(),
            },
        );
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: Some(7),
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://b/".into(),
                },
            },
        );
        // Duplicate terminal for the same key is dropped.
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: Some(7),
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://b/".into(),
                },
            },
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec!["started:https://a/", "succeeded:https://b/"]
        );
    }

    #[test]
    fn keyless_supersession_and_explicit_stop() {
        let webtag = tag("keyless");
        let events = capture(&webtag);
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: None,
                url: "https://one/".into(),
            },
        );
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: None,
                url: "https://two/".into(),
            },
        );
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: None,
                result: NativeNavigationResult::Cancelled(Some(
                    NavigationCancellationReason::Stopped,
                )),
            },
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "started:https://one/",
                "cancelled:Superseded",
                "started:https://two/",
                "cancelled:Stopped",
            ]
        );
    }

    #[test]
    fn failure_consumes_late_finish_and_orphans_synthesize() {
        let webtag = tag("orphans");
        let events = capture(&webtag);
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: None,
                url: "https://bad/".into(),
            },
        );
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: None,
                result: failed("https://bad/"),
            },
        );
        // Android/ArkWeb emit a bare page-finished after the failure: consumed.
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: None,
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://bad/".into(),
                },
            },
        );
        // A genuine finish-without-start still synthesizes a full lifecycle.
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: None,
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://restored/".into(),
                },
            },
        );
        // Orphan cancellations are dropped.
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: None,
                result: NativeNavigationResult::Cancelled(None),
            },
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "started:https://bad/",
                "failed:Network",
                "started:https://restored/",
                "succeeded:https://restored/",
            ]
        );
    }

    #[test]
    fn suppressed_key_completion_is_consumed() {
        let webtag = tag("suppressed");
        let events = capture(&webtag);
        submit(&webtag, NativeSignal::NavigationSuppressed { key: Some(3) });
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: Some(3),
                result: NativeNavigationResult::Cancelled(None),
            },
        );
        assert!(events.lock().unwrap().is_empty());
    }

    #[test]
    fn state_coalesces_and_commit_resets_metadata() {
        let webtag = tag("state");
        let events = capture(&webtag);
        submit(
            &webtag,
            NativeSignal::TitleChanged {
                title: Some("One".into()),
            },
        );
        submit(
            &webtag,
            NativeSignal::TitleChanged {
                title: Some("One".into()),
            },
        );
        submit(
            &webtag,
            NativeSignal::FaviconChanged {
                png_bytes: Some(vec![1, 2]),
            },
        );
        submit(&webtag, NativeSignal::DocumentCommitted);
        submit(
            &webtag,
            NativeSignal::TitleChanged {
                title: Some("Two".into()),
            },
        );
        // A second commit with already-clear metadata emits nothing for the
        // favicon (still None) but clears the new title.
        submit(&webtag, NativeSignal::DocumentCommitted);
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "title:One",
                "favicon:2",
                "title:<none>",
                "favicon:0",
                "title:Two",
                "title:<none>",
            ]
        );
    }

    #[test]
    fn destroy_drains_active_attempts() {
        let webtag = tag("destroy");
        let events = capture(&webtag);
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: Some(1),
                url: "https://a/".into(),
            },
        );
        destroy(&webtag);
        assert_eq!(
            *events.lock().unwrap(),
            vec!["started:https://a/", "cancelled:WebViewDestroyed"]
        );
    }
}
