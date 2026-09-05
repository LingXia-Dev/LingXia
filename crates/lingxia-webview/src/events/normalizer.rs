//! Per-WebView event normalizer: adapters submit [`NativeSignal`]s captured on
//! the native callback thread; the normalizer owns navigation identity,
//! exactly-once terminals, orphan synthesis, document generations, state
//! coalescing, and flattened FIFO delivery to the delegate and observers.

use super::{
    NavigationCancellationReason, NavigationEvent, NavigationId, WebViewEventObserver,
    WebViewObservedEvent, WebViewStateChange,
};
use crate::traits::{
    DocumentBinding, DocumentGeneration, LoadError, NativeWebViewId, TrustedDocumentAdmission,
    TrustedLoadIntent,
};
use crate::webview::{WebTag, find_webview_delegate_by_native_view_id};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

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
    /// Reliable top-level commit evidence for one accepted navigation.
    DocumentCommitted {
        key: Option<NativeKey>,
    },
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
    /// The renderer/document disappeared while the native WebView remains
    /// reusable (for example a WebView2 render-process failure). No later
    /// callback from that document may retain authority.
    DocumentInvalidated,
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
    DocumentCommitted {
        generation: DocumentGeneration,
        navigation_id: NavigationId,
    },
    TrustedDocumentAdmitted(TrustedDocumentAdmission),
}

static NEXT_TRUSTED_LOAD_INTENT: AtomicU64 = AtomicU64::new(1);

fn next_trusted_load_intent() -> TrustedLoadIntent {
    // Like the native WebView identity, this value must never wrap: a stale
    // token matching a future native load would be an authority escalation.
    let raw = NEXT_TRUSTED_LOAD_INTENT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("trusted load intent space exhausted");
    TrustedLoadIntent::new(raw)
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
    /// A just-finished success can still be the current committed document
    /// when WebKit re-enters through didCommit/didFinish before loadHTMLString
    /// returns its navigation object to Rust.
    recent_succeeded: VecDeque<(NativeKey, NavigationId)>,
}

impl NavigationTracker {
    /// The accepted navigation which a reliable commit signal is allowed to
    /// bind. `DocumentTracker` only mints after the corresponding start, so
    /// this is present for every valid commit.
    fn active_for_commit(&self, key: Option<NativeKey>) -> Option<NavigationId> {
        match key {
            Some(key) => self.by_key.get(&key).copied(),
            None => self.keyless_active,
        }
    }

    fn navigation_for_trusted_attestation(&self, key: NativeKey) -> Option<NavigationId> {
        self.by_key.get(&key).copied().or_else(|| {
            self.recent_succeeded
                .iter()
                .rev()
                .find_map(|(finished_key, id)| (*finished_key == key).then_some(*id))
        })
    }

    fn recently_terminated(&self, key: NativeKey) -> bool {
        self.recent_terminated.contains(&key)
    }

    fn start(&mut self, webtag: &WebTag, key: Option<NativeKey>, url: String) -> Vec<Output> {
        let mut out = Vec::new();
        match key {
            Some(key) => {
                if self.by_key.contains_key(&key) {
                    // Redirect restart with the same native id: same attempt.
                    return out;
                }
                // A terminated key that starts again is a reused native
                // identity (a freed WKNavigation's address reallocated for
                // the next load). The duplicate-terminal guard must not
                // swallow the new attempt's completion.
                self.recent_terminated
                    .retain(|terminated| *terminated != key);
                self.recent_succeeded
                    .retain(|(terminated, _)| *terminated != key);
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

        if let (Some(key), NativeNavigationResult::Succeeded { .. }) = (key, &result) {
            self.recent_succeeded.push_back((key, id));
            if self.recent_succeeded.len() > 8 {
                self.recent_succeeded.pop_front();
            }
        }

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

/// The document binding is intentionally independent from [`NavigationId`].
/// A navigation id describes an attempt; a generation describes only a
/// document which has actually committed in this native WebView.
struct DocumentTracker {
    binding: DocumentBinding,
    /// The key which produced `binding`, retained across a successful terminal
    /// callback so a reentrant direct-loader attestation can still prove the
    /// document is current.
    current_key: Option<NativeKey>,
    next_generation: u64,
    /// An overlap from a backend without native attempt keys is permanently
    /// unresolvable for this WebView instance. Subsequent keyless commits may
    /// be stale callbacks from either attempt, so they must all fail closed.
    keyless_tainted: bool,
    /// Only the most recently accepted attempt may claim the next document.
    /// A newer start supersedes an older pending commit, even on backends
    /// which report overlapping native navigation ids.
    pending: Option<PendingDocumentCommit>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingDocumentCommit {
    Keyed {
        key: NativeKey,
        committed: bool,
    },
    /// An ID-less platform cannot associate a second start or terminal signal
    /// with one of two overlapping attempts. Once that happens, fail closed:
    /// neither commit can prove which document it belongs to.
    Keyless {
        committed: bool,
        ambiguous: bool,
    },
}

impl Default for DocumentTracker {
    fn default() -> Self {
        Self {
            binding: DocumentBinding::Unbound,
            current_key: None,
            next_generation: 0,
            keyless_tainted: false,
            pending: None,
        }
    }
}

impl DocumentTracker {
    fn current(&self) -> DocumentBinding {
        self.binding
    }

    /// A submitted start is a top-level navigation which policy has accepted.
    /// The old document becomes unusable immediately, before load completion.
    fn started(&mut self, key: Option<NativeKey>) {
        match key {
            Some(key)
                if matches!(
                    self.pending,
                    Some(PendingDocumentCommit::Keyed { key: pending, .. }) if pending == key
                ) =>
            {
                // Redirect/repeated platform start for the same native attempt.
            }
            Some(key) => {
                self.binding = DocumentBinding::Unbound;
                self.current_key = None;
                self.pending = Some(PendingDocumentCommit::Keyed {
                    key,
                    committed: false,
                });
            }
            None => {
                self.binding = DocumentBinding::Unbound;
                self.current_key = None;
                let ambiguous = self.keyless_tainted
                    || matches!(self.pending, Some(PendingDocumentCommit::Keyless { .. }));
                self.keyless_tainted |= ambiguous;
                self.pending = Some(PendingDocumentCommit::Keyless {
                    committed: false,
                    ambiguous,
                });
            }
        }
    }

    /// Bind only on commit evidence. A keyed navigation and a keyless start
    /// each consume at most one commit, making duplicate callbacks harmless.
    fn committed(&mut self, key: Option<NativeKey>) -> bool {
        let pending = match (self.pending, key) {
            (
                Some(PendingDocumentCommit::Keyed {
                    key: pending,
                    committed,
                }),
                Some(key),
            ) if pending == key && !committed => PendingDocumentCommit::Keyed {
                key,
                committed: true,
            },
            (
                Some(PendingDocumentCommit::Keyless {
                    committed: false,
                    ambiguous: false,
                }),
                None,
            ) if !self.keyless_tainted => PendingDocumentCommit::Keyless {
                committed: true,
                ambiguous: false,
            },
            _ => return false,
        };
        self.pending = Some(pending);
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("document generation sequence exhausted");
        self.binding = DocumentBinding::Bound(DocumentGeneration::new(self.next_generation));
        self.current_key = key;
        true
    }

    fn finished(&mut self, key: Option<NativeKey>) {
        let clears_pending = match (self.pending, key) {
            (Some(PendingDocumentCommit::Keyed { key: pending, .. }), Some(key)) => pending == key,
            // An ID-less terminal cannot identify one of overlapping starts.
            // Clearing the pending claim is deliberately all it may do: a
            // later commit has no pending proof from which to mint a binding.
            (Some(PendingDocumentCommit::Keyless { .. }), None) => true,
            _ => false,
        };
        if clears_pending {
            self.pending = None;
        }
    }

    /// Whether an issued trusted load may still bind this native key.
    ///
    /// A missing pending record is allowed only for the key-before-start race.
    /// Once another keyed start has replaced this key, late platform attestation
    /// is permanently ineligible rather than being retained for a future
    /// callback.
    fn trusted_attestation_state(&self, key: NativeKey) -> TrustedAttestationState {
        match (self.current_key, self.binding, self.pending) {
            (Some(current), DocumentBinding::Bound(generation), _) if current == key => {
                TrustedAttestationState::CommittedCurrent(generation)
            }
            (
                _,
                _,
                Some(PendingDocumentCommit::Keyed {
                    key: pending,
                    committed: false,
                }),
            ) if pending == key => TrustedAttestationState::AwaitingCommit,
            (
                _,
                _,
                Some(PendingDocumentCommit::Keyed {
                    key: pending,
                    committed: true,
                }),
            ) if pending == key => TrustedAttestationState::Rejected,
            (_, _, None) => TrustedAttestationState::BeforeStart,
            _ => TrustedAttestationState::Rejected,
        }
    }

    fn destroyed(&mut self) {
        self.binding = DocumentBinding::Unbound;
        self.current_key = None;
        self.keyless_tainted = false;
        self.pending = None;
    }
}

#[derive(Clone, Copy)]
enum TrustedAttestationState {
    BeforeStart,
    AwaitingCommit,
    CommittedCurrent(DocumentGeneration),
    Rejected,
}

/// One direct native HTML load may be awaiting platform correlation for this
/// concrete WebView. A later explicit trusted load supersedes it before its
/// platform callback can create authority.
#[derive(Default)]
struct TrustedLoadTracker {
    pending: Option<TrustedLoadState>,
}

#[derive(Clone, Copy)]
enum TrustedLoadState {
    Issued {
        intent: TrustedLoadIntent,
    },
    Attested {
        intent: TrustedLoadIntent,
        key: NativeKey,
        navigation_id: Option<NavigationId>,
    },
}

impl TrustedLoadTracker {
    fn issue(&mut self) -> TrustedLoadIntent {
        let intent = next_trusted_load_intent();
        self.pending = Some(TrustedLoadState::Issued { intent });
        intent
    }

    fn attest(
        &mut self,
        intent: TrustedLoadIntent,
        key: NativeKey,
        navigation_id: Option<NavigationId>,
    ) -> bool {
        if key == 0
            || !matches!(self.pending, Some(TrustedLoadState::Issued { intent: issued }) if issued == intent)
        {
            return false;
        }
        self.pending = Some(TrustedLoadState::Attested {
            intent,
            key,
            navigation_id,
        });
        true
    }

    fn navigation_started(&mut self, key: Option<NativeKey>, navigation_id: NavigationId) {
        match self.pending {
            // A matching native key may arrive before or after the platform
            // returns it from loadHTMLString:. Redirect starts reuse that key
            // and therefore cannot rebind the intent to another navigation.
            Some(TrustedLoadState::Attested {
                intent,
                key: trusted_key,
                navigation_id: None,
            }) if key == Some(trusted_key) => {
                self.pending = Some(TrustedLoadState::Attested {
                    intent,
                    key: trusted_key,
                    navigation_id: Some(navigation_id),
                });
            }
            Some(TrustedLoadState::Attested {
                key: trusted_key, ..
            }) if key != Some(trusted_key) => self.pending = None,
            // An issued load has not yet disclosed a key. Keep it pending
            // until the direct platform call returns its exact key; binding a
            // different callback is impossible without that key.
            _ => {}
        }
    }

    fn navigation_finished(&mut self, key: Option<NativeKey>) {
        if matches!(self.pending, Some(TrustedLoadState::Attested { key: trusted_key, .. }) if key == Some(trusted_key))
        {
            self.pending = None;
        }
    }

    fn admit(
        &mut self,
        native_view: NativeWebViewId,
        key: Option<NativeKey>,
        navigation_id: NavigationId,
        generation: DocumentGeneration,
    ) -> Option<TrustedDocumentAdmission> {
        let TrustedLoadState::Attested {
            intent,
            key: trusted_key,
            navigation_id: Some(trusted_navigation_id),
        } = self.pending?
        else {
            return None;
        };
        if key != Some(trusted_key) || navigation_id != trusted_navigation_id {
            return None;
        }
        self.pending = None;
        Some(TrustedDocumentAdmission::new(
            native_view,
            generation,
            navigation_id,
            intent,
        ))
    }

    fn revoke(&mut self, intent: TrustedLoadIntent) {
        if matches!(self.pending, Some(TrustedLoadState::Issued { intent: current } | TrustedLoadState::Attested { intent: current, .. }) if current == intent)
        {
            self.pending = None;
        }
    }

    fn clear(&mut self) {
        self.pending = None;
    }
}

struct NormalizerState {
    tracker: NavigationTracker,
    documents: DocumentTracker,
    trusted_load: TrustedLoadTracker,
    coalescer: StateCoalescer,
    queue: VecDeque<Output>,
    draining: bool,
    observers: Vec<WebViewEventObserver>,
}

/// One normalizer per concrete native WebView, indexed by its current tag.
pub(crate) struct EventNormalizer {
    webtag: WebTag,
    native_view_id: NativeWebViewId,
    state: Mutex<NormalizerState>,
}

static NORMALIZERS: OnceLock<Mutex<HashMap<String, Arc<EventNormalizer>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, Arc<EventNormalizer>>> {
    NORMALIZERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn new_normalizer(webtag: &WebTag, native_view_id: NativeWebViewId) -> Arc<EventNormalizer> {
    Arc::new(EventNormalizer {
        webtag: webtag.clone(),
        native_view_id,
        state: Mutex::new(NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        }),
    })
}

/// Start a normalizer lifecycle before native WebView creation can emit events.
///
/// The preallocated native identity is mandatory: a reused [`WebTag`] must
/// never let late navigation callbacks alter its replacement's document.
pub(crate) fn begin(webtag: &WebTag, native_view_id: NativeWebViewId) {
    let mut map = registry().lock().unwrap_or_else(|e| e.into_inner());
    match map.get(webtag.key()) {
        Some(existing) if existing.native_view_id == native_view_id => {}
        _ => {
            if let Some(existing) = map.get(webtag.key()) {
                existing.revoke_all_trusted_loads();
            }
            map.insert(
                webtag.key().to_string(),
                new_normalizer(webtag, native_view_id),
            );
        }
    }
}

/// Issue a token before a platform direct-data-load call. The token is not
/// authority until [`attest_trusted_load`] associates it with the native
/// navigation object returned by that exact call.
pub(crate) fn issue_trusted_load(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
) -> Option<TrustedLoadIntent> {
    let normalizer = normalizer_for(webtag)?;
    if normalizer.native_view_id != native_view_id {
        return None;
    }
    let mut state = normalizer.state.lock().unwrap_or_else(|e| e.into_inner());
    Some(state.trusted_load.issue())
}

/// Bind a just-issued token to the exact platform navigation key returned by
/// the direct native load. It is valid whether the platform start callback
/// arrived immediately before or after this function.
pub(crate) fn attest_trusted_load(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
    intent: TrustedLoadIntent,
    key: NativeKey,
) -> bool {
    let Some(normalizer) = normalizer_for(webtag) else {
        return false;
    };
    if normalizer.native_view_id != native_view_id {
        return false;
    }
    let mut state = normalizer.state.lock().unwrap_or_else(|e| e.into_inner());
    let Some(admission) = attest_trusted_load_state(&mut state, native_view_id, intent, key) else {
        return false;
    };
    if let Some(admission) = admission {
        state
            .queue
            .push_back(Output::TrustedDocumentAdmitted(admission));
    }
    normalizer.drain_locked(state);
    true
}

/// Returns `None` for an ineligible key; `Some(None)` for an accepted load
/// whose start has not committed yet; and an admission for the reentrant
/// start → commit → platform-key ordering.
fn attest_trusted_load_state(
    state: &mut NormalizerState,
    native_view_id: NativeWebViewId,
    intent: TrustedLoadIntent,
    key: NativeKey,
) -> Option<Option<TrustedDocumentAdmission>> {
    let navigation_id = state.tracker.navigation_for_trusted_attestation(key);
    let document_state = state.documents.trusted_attestation_state(key);
    if matches!(document_state, TrustedAttestationState::Rejected)
        || (matches!(document_state, TrustedAttestationState::BeforeStart)
            && navigation_id.is_none()
            && state.tracker.recently_terminated(key))
        || !state.trusted_load.attest(intent, key, navigation_id)
    {
        return None;
    }
    let admission = match (document_state, navigation_id) {
        (TrustedAttestationState::CommittedCurrent(generation), Some(navigation_id)) => state
            .trusted_load
            .admit(native_view_id, Some(key), navigation_id, generation),
        _ => None,
    };
    Some(admission)
}

/// Forget an issued token after a platform direct-load failure.
pub(crate) fn revoke_trusted_load(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
    intent: TrustedLoadIntent,
) {
    let Some(normalizer) = normalizer_for(webtag) else {
        return;
    };
    if normalizer.native_view_id != native_view_id {
        return;
    }
    normalizer
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .trusted_load
        .revoke(intent);
}

/// Confirm that a ready WebView owns the normalizer lifecycle reserved for
/// its native identity. Kept separate from [`begin`] so registration makes the
/// identity hand-off explicit while early platform callbacks remain covered.
pub(crate) fn bind_native_view(webtag: &WebTag, native_view_id: NativeWebViewId) {
    begin(webtag, native_view_id);
}

fn normalizer_for(webtag: &WebTag) -> Option<Arc<EventNormalizer>> {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(webtag.key())
        .cloned()
}

/// A removed normalizer remains able to drain teardown notifications, but a
/// replacement registered under the same tag makes every queued old output
/// stale. This check is separate from delegate lookup so observers are also
/// never invoked for the replacement lifecycle.
fn normalizer_was_replaced(webtag: &WebTag, native_view_id: NativeWebViewId) -> bool {
    normalizer_for(webtag).is_some_and(|current| current.native_view_id != native_view_id)
}

/// Submit a native signal for `webtag`. Payloads must already be captured on
/// the native callback thread — the normalizer never queries native objects.
pub(crate) fn submit(webtag: &WebTag, native_view_id: NativeWebViewId, signal: NativeSignal) {
    if let Some(normalizer) = normalizer_for(webtag) {
        if normalizer.native_view_id != native_view_id {
            log::debug!(
                "Dropping stale navigation signal for {webtag}: native WebView identity changed"
            );
            return;
        }
        normalizer.submit(signal);
    }
}

/// Submit an Android-style keyed commit and return only the generation minted
/// for that exact callback key. Reading `current_document_binding` afterwards
/// would let a stale callback accidentally claim a successor generation.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn submit_document_commit(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
    key: NativeKey,
) -> Option<DocumentGeneration> {
    let normalizer = normalizer_for(webtag)?;
    if normalizer.native_view_id != native_view_id {
        return None;
    }
    let mut state = normalizer.state.lock().unwrap_or_else(|e| e.into_inner());
    let outputs = document_committed_outputs(webtag, native_view_id, &mut state, Some(key));
    let generation = outputs.iter().find_map(|output| match output {
        Output::DocumentCommitted { generation, .. } => Some(*generation),
        _ => None,
    });
    state.queue.extend(outputs);
    normalizer.drain_locked(state);
    generation
}

/// Snapshot the currently committed document for a concrete native WebView.
/// The only mutation path is the normalizer's accepted-start/commit sequence;
/// adapters cannot synthesize a bound generation.
pub(crate) fn current_document_binding(native_view_id: NativeWebViewId) -> DocumentBinding {
    let normalizer = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .find(|normalizer| normalizer.native_view_id == native_view_id)
        .cloned();
    normalizer.map_or(DocumentBinding::Unbound, |normalizer| {
        normalizer
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .documents
            .current()
    })
}

/// Revoke one currently bound document restored without fresh navigation
/// evidence. Returns false after the first revocation so platform callbacks
/// cannot schedule duplicate trusted reloads for the same stale document.
#[cfg_attr(
    not(any(target_os = "android", target_os = "windows", test)),
    allow(dead_code)
)]
pub(crate) fn invalidate_restored_document(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
) -> bool {
    if !matches!(
        current_document_binding(native_view_id),
        DocumentBinding::Bound(_)
    ) {
        return false;
    }
    submit(webtag, native_view_id, NativeSignal::DocumentInvalidated);
    true
}

/// Run one native action while an exact committed document remains current.
///
/// Keep both the registry and per-WebView state locks through `action`: a
/// replacement native view or a navigation start must linearize entirely
/// before or after the action, never between its generation check and effect.
pub(crate) fn with_current_document_binding(
    native_view_id: NativeWebViewId,
    expected_generation: DocumentGeneration,
    action: &mut dyn FnMut(),
) -> bool {
    let normalizers = registry().lock().unwrap_or_else(|e| e.into_inner());
    let Some(normalizer) = normalizers
        .values()
        .find(|normalizer| normalizer.native_view_id == native_view_id)
    else {
        return false;
    };
    let state = normalizer.state.lock().unwrap_or_else(|e| e.into_inner());
    if state.documents.current() != DocumentBinding::Bound(expected_generation) {
        return false;
    }
    action();
    true
}

/// Register a read-only observer for `webtag`'s events.
pub fn add_observer(webtag: &WebTag, observer: WebViewEventObserver) {
    let Some(normalizer) = normalizer_for(webtag) else {
        log::debug!("Ignoring observer for inactive WebView {webtag}");
        return;
    };
    let mut state = normalizer.state.lock().unwrap_or_else(|e| e.into_inner());
    state.observers.push(observer);
}

impl EventNormalizer {
    fn submit(&self, signal: NativeSignal) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let outputs = match signal {
            NativeSignal::NavigationStarted { key, url } => {
                let outputs = state.tracker.start(&self.webtag, key, url);
                if let Some(navigation_id) = outputs.iter().find_map(|output| match output {
                    Output::Nav(NavigationEvent::Started { id, .. }) => Some(*id),
                    _ => None,
                }) {
                    state.documents.started(key);
                    state.trusted_load.navigation_started(key, navigation_id);
                }
                outputs
            }
            NativeSignal::NavigationSuppressed { key } => {
                match key {
                    Some(key) => {
                        state.tracker.suppressed_keys.insert(key);
                    }
                    None => state.tracker.suppressed_keyless += 1,
                }
                // A policy-cancelled native navigation cannot later claim a
                // trusted document even if its callback tail arrives late.
                state.trusted_load.navigation_finished(key);
                Vec::new()
            }
            NativeSignal::DocumentCommitted { key } => {
                document_committed_outputs(&self.webtag, self.native_view_id, &mut state, key)
            }
            NativeSignal::NavigationFinished { key, result } => {
                state.documents.finished(key);
                state.trusted_load.navigation_finished(key);
                state.tracker.finish(&self.webtag, key, result)
            }
            NativeSignal::LocationChanged { url } => state.coalescer.location(url),
            NativeSignal::TitleChanged { title } => state.coalescer.title(title),
            NativeSignal::FaviconChanged { png_bytes } => state.coalescer.favicon(png_bytes),
            NativeSignal::BackForwardChanged {
                can_go_back,
                can_go_forward,
            } => state.coalescer.back_forward(can_go_back, can_go_forward),
            NativeSignal::DocumentInvalidated | NativeSignal::Destroyed => {
                state.documents.destroyed();
                state.trusted_load.clear();
                state.tracker.drain_destroyed()
            }
        };
        state.queue.extend(outputs);

        self.drain_locked(state);
    }

    // Flattened, non-reentrant FIFO drain: if a delegate callback causes
    // another submission (same thread) or another thread submits while we
    // drain, those events are appended and delivered by the active drain.
    fn drain_locked<'a>(&'a self, mut state: MutexGuard<'a, NormalizerState>) {
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
        if normalizer_was_replaced(&self.webtag, self.native_view_id) {
            log::debug!(
                "Dropping queued navigation output for {}: native WebView identity changed",
                self.webtag
            );
            return;
        }
        let delegate = find_webview_delegate_by_native_view_id(&self.webtag, self.native_view_id);
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
            Output::DocumentCommitted {
                generation,
                navigation_id,
            } => {
                if let Some(delegate) = &delegate {
                    notify_document_committed(
                        delegate,
                        self.native_view_id,
                        *generation,
                        *navigation_id,
                    );
                }
            }
            Output::TrustedDocumentAdmitted(admission) => {
                if let Some(delegate) = &delegate {
                    delegate.on_trusted_document_admitted(*admission);
                }
            }
        }
    }
}

fn document_committed_outputs(
    webtag: &WebTag,
    native_view_id: NativeWebViewId,
    state: &mut NormalizerState,
    key: Option<NativeKey>,
) -> Vec<Output> {
    // Metadata reset is an observable platform commit signal in its own right.
    // Generation minting is stricter and may drop a stale or duplicate attempt
    // without suppressing that state maintenance.
    let navigation_id = state.tracker.active_for_commit(key);
    let minted = state.documents.committed(key);
    let mut outputs = state.coalescer.document_committed();
    if minted {
        let DocumentBinding::Bound(generation) = state.documents.current() else {
            unreachable!("accepted document commit must bind a generation");
        };
        if let Some(navigation_id) = navigation_id {
            // Queue the hook before metadata reset state. FIFO delivery
            // preserves the successful mint as the document-binding
            // linearization point without invoking application code under the
            // state mutex.
            outputs.insert(
                0,
                Output::DocumentCommitted {
                    generation,
                    navigation_id,
                },
            );
            if let Some(admission) =
                state
                    .trusted_load
                    .admit(native_view_id, key, navigation_id, generation)
            {
                outputs.insert(1, Output::TrustedDocumentAdmitted(admission));
            }
        } else {
            log::error!("{webtag}: accepted document commit had no active navigation");
        }
    }
    outputs
}

impl EventNormalizer {
    fn revoke_all_trusted_loads(&self) {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .trusted_load
            .clear();
    }
}

fn notify_document_committed(
    delegate: &Arc<dyn crate::WebViewDelegate>,
    native_view_id: NativeWebViewId,
    generation: DocumentGeneration,
    navigation_id: NavigationId,
) {
    delegate.on_document_committed(native_view_id, generation, navigation_id);
}

/// Remove the webtag's normalizer after draining teardown cancellations.
pub(crate) fn destroy(webtag: &WebTag) {
    let normalizer = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(webtag.key());
    if let Some(normalizer) = normalizer {
        normalizer.submit(NativeSignal::Destroyed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{IncomingWebMessage, LoadErrorKind};
    use crate::{LogLevel, WebViewDelegate};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    struct CommitCapture(Arc<Mutex<Vec<(NativeWebViewId, DocumentGeneration, NavigationId)>>>);

    impl WebViewDelegate for CommitCapture {
        fn on_navigation_event(&self, _event: NavigationEvent) {}

        fn handle_post_message(&self, _message: IncomingWebMessage) {}

        fn log(&self, _level: LogLevel, _message: &str) {}

        fn on_document_committed(
            &self,
            native_view: NativeWebViewId,
            generation: DocumentGeneration,
            navigation_id: NavigationId,
        ) {
            self.0
                .lock()
                .unwrap()
                .push((native_view, generation, navigation_id));
        }
    }

    #[test]
    fn document_commit_hook_receives_exact_binding_identity() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let delegate: Arc<dyn WebViewDelegate> = Arc::new(CommitCapture(calls.clone()));
        notify_document_committed(
            &delegate,
            NativeWebViewId::new(87),
            DocumentGeneration::new(4),
            NavigationId::next(),
        );

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, NativeWebViewId::new(87));
        assert_eq!(calls[0].1, DocumentGeneration::new(4));
    }

    #[test]
    fn document_commit_output_only_exists_for_an_accepted_binding() {
        let webtag = tag("document-commit-output");
        let mut state = NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        };

        state
            .tracker
            .start(&webtag, Some(11), "https://one/".into());
        state.documents.started(Some(11));
        let accepted =
            document_committed_outputs(&webtag, NativeWebViewId::new(1), &mut state, Some(11));
        assert!(matches!(
            accepted.first(),
            Some(Output::DocumentCommitted { generation, .. }) if *generation == DocumentGeneration::new(1)
        ));

        let duplicate =
            document_committed_outputs(&webtag, NativeWebViewId::new(1), &mut state, Some(11));
        assert!(
            !duplicate
                .iter()
                .any(|output| matches!(output, Output::DocumentCommitted { .. }))
        );

        state
            .tracker
            .start(&webtag, Some(12), "https://two/".into());
        state.documents.started(Some(12));
        let stale =
            document_committed_outputs(&webtag, NativeWebViewId::new(1), &mut state, Some(11));
        assert!(
            !stale
                .iter()
                .any(|output| matches!(output, Output::DocumentCommitted { .. }))
        );

        let mut keyless = NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        };
        keyless.tracker.start(&webtag, None, "https://one/".into());
        keyless.documents.started(None);
        keyless.tracker.start(&webtag, None, "https://two/".into());
        keyless.documents.started(None);
        let tainted =
            document_committed_outputs(&webtag, NativeWebViewId::new(1), &mut keyless, None);
        assert!(
            !tainted
                .iter()
                .any(|output| matches!(output, Output::DocumentCommitted { .. }))
        );
    }

    #[test]
    fn trusted_load_admission_binds_the_returned_key_before_or_after_start() {
        let webtag = tag("trusted-load-admission");
        let native_view = NativeWebViewId::new(19);
        let mut state = NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        };

        // WebKit may report didStart before the host thread regains control
        // from loadHTMLString: and can submit the returned key for attestation.
        let started = state
            .tracker
            .start(&webtag, Some(71), "https://assets.example/".into());
        let navigation_id = match started.as_slice() {
            [Output::Nav(NavigationEvent::Started { id, .. })] => *id,
            _ => panic!("expected one accepted navigation start"),
        };
        state.documents.started(Some(71));
        let intent = state.trusted_load.issue();
        assert!(
            state
                .trusted_load
                .attest(intent, 71, state.tracker.active_for_commit(Some(71)))
        );

        // A redirect restart shares WebKit's navigation object and cannot
        // create a replacement NavigationId or rebind this intent.
        assert!(
            state
                .tracker
                .start(&webtag, Some(71), "https://assets.example/redirect".into())
                .is_empty()
        );

        let committed = document_committed_outputs(&webtag, native_view, &mut state, Some(71));
        let admission = committed.iter().find_map(|output| match output {
            Output::TrustedDocumentAdmitted(admission) => Some(*admission),
            _ => None,
        });
        let admission = admission.expect("matching trusted load must be admitted once");
        assert_eq!(admission.native_view(), native_view);
        assert_eq!(admission.navigation_id(), navigation_id);
        assert_eq!(admission.generation(), DocumentGeneration::new(1));
        // TrustedLoadIntent intentionally has no Debug implementation, so a
        // failing assertion must not expose the opaque capability in logs.
        assert!(admission.intent() == intent);

        let duplicate = document_committed_outputs(&webtag, native_view, &mut state, Some(71));
        assert!(
            !duplicate
                .iter()
                .any(|output| matches!(output, Output::TrustedDocumentAdmitted(_)))
        );
    }

    #[test]
    fn trusted_load_is_revoked_by_supersede_terminal_failure_and_drop() {
        let mut trusted = TrustedLoadTracker::default();
        let first = trusted.issue();
        assert!(trusted.attest(first, 1, Some(NavigationId::next())));
        trusted.navigation_started(Some(2), NavigationId::next());
        assert!(
            trusted
                .admit(
                    NativeWebViewId::new(1),
                    Some(1),
                    NavigationId::next(),
                    DocumentGeneration::new(1)
                )
                .is_none()
        );

        let second = trusted.issue();
        let second_navigation = NavigationId::next();
        assert!(trusted.attest(second, 2, Some(second_navigation)));
        trusted.navigation_finished(Some(2));
        assert!(
            trusted
                .admit(
                    NativeWebViewId::new(1),
                    Some(2),
                    second_navigation,
                    DocumentGeneration::new(2)
                )
                .is_none()
        );

        let third = trusted.issue();
        // Reservation Drop and an immediate platform-load error use the same
        // explicit revoke path; neither can leave a later callback eligible.
        trusted.revoke(third);
        assert!(!trusted.attest(third, 3, None));

        let fourth = trusted.issue();
        trusted.clear();
        assert!(!trusted.attest(fourth, 4, None));
    }

    #[test]
    fn trusted_load_key_can_arrive_before_navigation_start() {
        let webtag = tag("trusted-load-key-first");
        let native_view = NativeWebViewId::new(23);
        let mut state = NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        };

        let intent = state.trusted_load.issue();
        assert!(state.trusted_load.attest(intent, 83, None));
        let started = state
            .tracker
            .start(&webtag, Some(83), "https://assets.example/".into());
        let navigation_id = match started.as_slice() {
            [Output::Nav(NavigationEvent::Started { id, .. })] => *id,
            _ => panic!("expected one accepted navigation start"),
        };
        state.documents.started(Some(83));
        state
            .trusted_load
            .navigation_started(Some(83), navigation_id);

        let committed = document_committed_outputs(&webtag, native_view, &mut state, Some(83));
        assert!(committed.iter().any(|output| {
            matches!(output, Output::TrustedDocumentAdmitted(admission) if admission.intent() == intent)
        }));
    }

    #[test]
    fn late_trusted_attestation_admits_a_current_reentrant_commit_once() {
        let webtag = tag("trusted-load-commit-before-attest");
        let native_view = NativeWebViewId::new(29);
        let mut state = NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        };
        let intent = state.trusted_load.issue();
        let started = state
            .tracker
            .start(&webtag, Some(91), "https://assets.example/".into());
        let navigation_id = match started.as_slice() {
            [Output::Nav(NavigationEvent::Started { id, .. })] => *id,
            _ => panic!("expected one accepted navigation start"),
        };
        state.documents.started(Some(91));
        state
            .trusted_load
            .navigation_started(Some(91), navigation_id);
        let committed = document_committed_outputs(&webtag, native_view, &mut state, Some(91));
        assert!(
            !committed
                .iter()
                .any(|output| matches!(output, Output::TrustedDocumentAdmitted(_)))
        );

        let admission = attest_trusted_load_state(&mut state, native_view, intent, 91)
            .expect("current committed key remains eligible")
            .expect("late exact key must admit once");
        assert!(admission.intent() == intent);
        assert_eq!(admission.native_view(), native_view);
        assert_eq!(admission.navigation_id(), navigation_id);
        assert_eq!(admission.generation(), DocumentGeneration::new(1));
        assert!(attest_trusted_load_state(&mut state, native_view, intent, 91).is_none());
    }

    #[test]
    fn late_trusted_attestation_cannot_claim_a_superseded_document() {
        let webtag = tag("trusted-load-superseded-before-attest");
        let native_view = NativeWebViewId::new(31);
        let mut state = NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        };
        let intent = state.trusted_load.issue();
        let first = state
            .tracker
            .start(&webtag, Some(101), "https://assets.example/one".into());
        let first_id = match first.as_slice() {
            [Output::Nav(NavigationEvent::Started { id, .. })] => *id,
            _ => panic!("expected one accepted navigation start"),
        };
        state.documents.started(Some(101));
        state.trusted_load.navigation_started(Some(101), first_id);
        document_committed_outputs(&webtag, native_view, &mut state, Some(101));

        let second = state
            .tracker
            .start(&webtag, Some(102), "https://assets.example/two".into());
        let second_id = match second.as_slice() {
            [Output::Nav(NavigationEvent::Started { id, .. })] => *id,
            _ => panic!("expected one accepted navigation start"),
        };
        state.documents.started(Some(102));
        state.trusted_load.navigation_started(Some(102), second_id);

        assert!(attest_trusted_load_state(&mut state, native_view, intent, 101).is_none());
        assert!(matches!(
            state.documents.current(),
            DocumentBinding::Unbound
        ));
    }

    #[test]
    fn late_trusted_attestation_rejects_a_failed_navigation() {
        let webtag = tag("trusted-load-failed-before-attest");
        let native_view = NativeWebViewId::new(37);
        let mut state = NormalizerState {
            tracker: NavigationTracker::default(),
            documents: DocumentTracker::default(),
            trusted_load: TrustedLoadTracker::default(),
            coalescer: StateCoalescer::default(),
            queue: VecDeque::new(),
            draining: false,
            observers: Vec::new(),
        };
        let intent = state.trusted_load.issue();
        let started = state
            .tracker
            .start(&webtag, Some(111), "https://assets.example/".into());
        let navigation_id = match started.as_slice() {
            [Output::Nav(NavigationEvent::Started { id, .. })] => *id,
            _ => panic!("expected one accepted navigation start"),
        };
        state.documents.started(Some(111));
        state
            .trusted_load
            .navigation_started(Some(111), navigation_id);
        state.documents.finished(Some(111));
        state.tracker.finish(
            &webtag,
            Some(111),
            NativeNavigationResult::Failed(LoadError {
                failing_url: Some("https://assets.example/".into()),
                kind: LoadErrorKind::Network,
                description: "test failure".into(),
            }),
        );

        assert!(attest_trusted_load_state(&mut state, native_view, intent, 111).is_none());
    }

    #[test]
    fn stale_native_identity_cannot_late_attest_a_retired_normalizer() {
        let webtag = tag("trusted-load-stale-native-attest");
        let retired = NativeWebViewId::new(401);
        let replacement = NativeWebViewId::new(402);
        super::begin(&webtag, retired);
        let intent = issue_trusted_load(&webtag, retired).expect("active reservation");
        super::begin(&webtag, replacement);

        assert!(!attest_trusted_load(&webtag, retired, intent, 1));
        destroy(&webtag);
    }

    fn test_native_view_id(webtag: &WebTag) -> NativeWebViewId {
        let mut hasher = DefaultHasher::new();
        webtag.key().hash(&mut hasher);
        NativeWebViewId::new(hasher.finish().max(1))
    }

    fn begin(webtag: &WebTag) {
        super::begin(webtag, test_native_view_id(webtag));
    }

    fn submit(webtag: &WebTag, signal: NativeSignal) {
        super::submit(webtag, test_native_view_id(webtag), signal);
    }

    fn capture(webtag: &WebTag) -> Arc<Mutex<Vec<String>>> {
        begin(webtag);
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
    fn a_reused_native_key_terminates_its_new_attempt() {
        let webtag = tag("key-reuse");
        let events = capture(&webtag);
        // First attempt completes; its key lands in the duplicate guard.
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: Some(9),
                url: "https://first/".into(),
            },
        );
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: Some(9),
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://first/".into(),
                },
            },
        );
        // The freed native object's address is reused for the next load; the
        // new attempt's terminal must be delivered, not dropped as duplicate.
        submit(
            &webtag,
            NativeSignal::NavigationStarted {
                key: Some(9),
                url: "https://second/".into(),
            },
        );
        submit(
            &webtag,
            NativeSignal::NavigationFinished {
                key: Some(9),
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://second/".into(),
                },
            },
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "started:https://first/",
                "succeeded:https://first/",
                "started:https://second/",
                "succeeded:https://second/",
            ]
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
        submit(&webtag, NativeSignal::DocumentCommitted { key: None });
        submit(
            &webtag,
            NativeSignal::TitleChanged {
                title: Some("Two".into()),
            },
        );
        // A second commit with already-clear metadata emits nothing for the
        // favicon (still None) but clears the new title.
        submit(&webtag, NativeSignal::DocumentCommitted { key: None });
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
    fn document_binding_revokes_on_start_and_commits_once_per_key() {
        let webtag = tag("document-keyed");
        let native_view_id = NativeWebViewId::new(8101);
        super::begin(&webtag, native_view_id);

        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(51),
                url: "https://first/".into(),
            },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );

        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(51) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(1))
        );

        // A duplicate platform commit is not a second document.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(51) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(1))
        );

        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(52),
                url: "https://second/".into(),
            },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(52) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(2))
        );
        destroy(&webtag);
    }

    #[test]
    fn renderer_crash_revokes_generation_and_rejects_stale_commit_before_reload() {
        let webtag = tag("document-process-failure");
        let native_view_id = NativeWebViewId::new(8198);
        super::begin(&webtag, native_view_id);
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(61),
                url: "lingxia://settings".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(61) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(1))
        );

        super::submit(&webtag, native_view_id, NativeSignal::DocumentInvalidated);
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(61) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );

        // A host reload must establish a fresh navigation and generation.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(62),
                url: "lingxia://settings".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(62) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(2))
        );
        destroy(&webtag);
    }

    #[test]
    fn document_bound_action_linearizes_before_navigation_start() {
        use std::sync::mpsc::{self, RecvTimeoutError};
        use std::time::Duration;

        let webtag = tag("document-bound-action-linearization");
        let native_view_id = NativeWebViewId::new(8199);
        super::begin(&webtag, native_view_id);
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(61),
                url: "https://first/".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(61) },
        );

        let (post_entered_tx, post_entered_rx) = mpsc::channel();
        let (release_post_tx, release_post_rx) = mpsc::channel();
        let post = std::thread::spawn(move || {
            let mut action = || {
                post_entered_tx.send(()).unwrap();
                release_post_rx.recv().unwrap();
            };
            assert!(with_current_document_binding(
                native_view_id,
                DocumentGeneration::new(1),
                &mut action,
            ));
        });
        post_entered_rx.recv().unwrap();

        let navigation_tag = webtag.clone();
        let (navigation_attempted_tx, navigation_attempted_rx) = mpsc::channel();
        let (navigation_done_tx, navigation_done_rx) = mpsc::channel();
        let navigation = std::thread::spawn(move || {
            navigation_attempted_tx.send(()).unwrap();
            super::submit(
                &navigation_tag,
                native_view_id,
                NativeSignal::NavigationStarted {
                    key: Some(62),
                    url: "https://second/".into(),
                },
            );
            navigation_done_tx.send(()).unwrap();
        });
        navigation_attempted_rx.recv().unwrap();
        assert_eq!(
            navigation_done_rx.recv_timeout(Duration::from_millis(25)),
            Err(RecvTimeoutError::Timeout),
            "navigation start must wait until the bound action completes"
        );

        release_post_tx.send(()).unwrap();
        post.join().unwrap();
        navigation.join().unwrap();
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        destroy(&webtag);
    }

    #[test]
    fn queued_outbound_for_stale_generation_is_dropped_after_reload_start() {
        let webtag = tag("queued-stale-outbound");
        let native_view_id = NativeWebViewId::new(8200);
        super::begin(&webtag, native_view_id);
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(71),
                url: "lingxia://settings".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(71) },
        );
        let queued_generation = DocumentGeneration::new(1);

        // The UI-thread delivery has not happened yet when reload starts.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(72),
                url: "lingxia://settings".into(),
            },
        );
        let mut delivered = false;
        assert!(!with_current_document_binding(
            native_view_id,
            queued_generation,
            &mut || delivered = true,
        ));
        assert!(!delivered);
        destroy(&webtag);
    }

    #[test]
    fn restored_document_revokes_inbound_and_queued_outbound_once() {
        let webtag = tag("restored-stale-delivery");
        let native_view_id = NativeWebViewId::new(8201);
        super::begin(&webtag, native_view_id);
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(81),
                url: "lingxia://settings".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(81) },
        );
        let stale_generation = DocumentGeneration::new(1);

        assert!(super::invalidate_restored_document(&webtag, native_view_id));
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        let mut stale_outbound_delivered = false;
        assert!(!with_current_document_binding(
            native_view_id,
            stale_generation,
            &mut || stale_outbound_delivered = true,
        ));
        assert!(!stale_outbound_delivered);
        assert!(
            !super::invalidate_restored_document(&webtag, native_view_id),
            "repeat restore evidence cannot schedule another replacement"
        );

        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: Some(82),
                url: "lingxia://settings".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(82) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(2))
        );
        assert!(!with_current_document_binding(
            native_view_id,
            stale_generation,
            &mut || stale_outbound_delivered = true,
        ));
        destroy(&webtag);
    }

    #[test]
    fn overlapping_keyed_start_cannot_let_a_stale_commit_bind_the_new_document() {
        let webtag = tag("document-overlap");
        let native_view_id = NativeWebViewId::new(8105);
        super::begin(&webtag, native_view_id);
        for key in [1, 2] {
            super::submit(
                &webtag,
                native_view_id,
                NativeSignal::NavigationStarted {
                    key: Some(key),
                    url: format!("https://{key}/"),
                },
            );
        }
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(1) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: Some(2) },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(1))
        );
        destroy(&webtag);
    }

    #[test]
    fn keyless_start_consumes_only_one_commit_and_failed_attempt_stays_unbound() {
        let webtag = tag("document-keyless");
        let native_view_id = NativeWebViewId::new(8102);
        super::begin(&webtag, native_view_id);

        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: None,
                url: "https://first/".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: None },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(1))
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: None },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Bound(DocumentGeneration::new(1))
        );

        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: None,
                url: "https://failed/".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationFinished {
                key: None,
                result: failed("https://failed/"),
            },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        // A late keyless commit after a failed attempt cannot resurrect it.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: None },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        destroy(&webtag);
    }

    #[test]
    fn overlapping_keyless_attempts_taint_the_native_webview_until_recreated() {
        let webtag = tag("document-keyless-overlap");
        let native_view_id = NativeWebViewId::new(8106);
        super::begin(&webtag, native_view_id);

        // Without native attempt keys, B's start makes the source of every
        // subsequent commit ambiguous. The old document remains revoked.
        for url in ["https://a/", "https://b/"] {
            super::submit(
                &webtag,
                native_view_id,
                NativeSignal::NavigationStarted {
                    key: None,
                    url: url.into(),
                },
            );
        }
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );

        // This may be A's commit. It cannot establish a binding for B.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: None },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );

        // A keyless terminal only discards the pending claim. A later start
        // cannot make the WebView trustworthy again: B's delayed callbacks
        // are still indistinguishable from C's own document callbacks.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationFinished {
                key: None,
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://a/".into(),
                },
            },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationStarted {
                key: None,
                url: "https://c/".into(),
            },
        );
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: None },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );

        // C's own commit is no more trustworthy than B's late commit once
        // the prior overlap has tainted this keyless WebView lifetime.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::DocumentCommitted { key: None },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );

        // B's late finish is an orphan at the document layer and remains
        // unable to alter the revoked binding.
        super::submit(
            &webtag,
            native_view_id,
            NativeSignal::NavigationFinished {
                key: None,
                result: NativeNavigationResult::Succeeded {
                    final_url: "https://b/".into(),
                },
            },
        );
        assert_eq!(
            current_document_binding(native_view_id),
            DocumentBinding::Unbound
        );
        destroy(&webtag);

        // A fresh native WebView gets a fresh tracker and can establish its
        // first unambiguous keyless document normally.
        let replacement = NativeWebViewId::new(8107);
        super::begin(&webtag, replacement);
        super::submit(
            &webtag,
            replacement,
            NativeSignal::NavigationStarted {
                key: None,
                url: "https://replacement/".into(),
            },
        );
        super::submit(
            &webtag,
            replacement,
            NativeSignal::DocumentCommitted { key: None },
        );
        assert_eq!(
            current_document_binding(replacement),
            DocumentBinding::Bound(DocumentGeneration::new(1))
        );
        destroy(&webtag);
    }

    #[test]
    fn retired_normalizer_drops_queued_output_after_tag_reuse() {
        let webtag = tag("normalizer-tag-reuse");
        let retired = NativeWebViewId::new(8108);
        let replacement = NativeWebViewId::new(8109);
        super::begin(&webtag, retired);
        let retired_normalizer = normalizer_for(&webtag).expect("reserved normalizer");
        let delivery_count = Arc::new(Mutex::new(0_usize));
        let observed = delivery_count.clone();
        add_observer(&webtag, Arc::new(move |_| *observed.lock().unwrap() += 1));

        // Simulate a callback which resolved the old normalizer before the
        // replacement claimed this logical tag, then reaches its FIFO drain.
        super::begin(&webtag, replacement);
        retired_normalizer.submit(NativeSignal::NavigationStarted {
            key: Some(1),
            url: "https://retired/".into(),
        });

        assert_eq!(*delivery_count.lock().unwrap(), 0);
        destroy(&webtag);
    }

    #[test]
    fn reused_tag_drops_old_native_view_navigation_signals() {
        let webtag = tag("document-tag-reuse");
        let retired = NativeWebViewId::new(8103);
        let replacement = NativeWebViewId::new(8104);
        super::begin(&webtag, retired);
        super::submit(
            &webtag,
            retired,
            NativeSignal::NavigationStarted {
                key: Some(1),
                url: "https://retired/".into(),
            },
        );
        super::submit(
            &webtag,
            retired,
            NativeSignal::DocumentCommitted { key: Some(1) },
        );
        assert!(matches!(
            current_document_binding(retired),
            DocumentBinding::Bound(_)
        ));

        super::begin(&webtag, replacement);
        assert_eq!(
            current_document_binding(replacement),
            DocumentBinding::Unbound
        );
        super::submit(
            &webtag,
            retired,
            NativeSignal::NavigationStarted {
                key: Some(2),
                url: "https://stale/".into(),
            },
        );
        assert_eq!(
            current_document_binding(replacement),
            DocumentBinding::Unbound
        );
        destroy(&webtag);
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

    #[test]
    fn late_signals_do_not_recreate_destroyed_normalizer() {
        let webtag = tag("late-after-destroy");
        begin(&webtag);
        assert!(normalizer_for(&webtag).is_some());

        destroy(&webtag);
        submit(
            &webtag,
            NativeSignal::TitleChanged {
                title: Some("late".into()),
            },
        );

        assert!(normalizer_for(&webtag).is_none());
    }

    #[test]
    fn reused_webtag_starts_a_fresh_normalizer_lifecycle() {
        let webtag = tag("reuse-after-destroy");
        let first_events = capture(&webtag);
        submit(
            &webtag,
            NativeSignal::TitleChanged {
                title: Some("first".into()),
            },
        );
        destroy(&webtag);

        let second_events = capture(&webtag);
        submit(
            &webtag,
            NativeSignal::TitleChanged {
                title: Some("second".into()),
            },
        );

        assert_eq!(*first_events.lock().unwrap(), vec!["title:first"]);
        assert_eq!(*second_events.lock().unwrap(), vec!["title:second"]);
        destroy(&webtag);
    }
}
