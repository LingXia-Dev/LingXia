//! Harmony top-level document correlation independent of ArkWeb FFI.
#![cfg_attr(not(all(target_os = "linux", target_env = "ohos")), allow(dead_code))]

use crate::events::normalizer::NativeKey;
use crate::traits::DocumentGeneration;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_HARMONY_NAVIGATION_KEY: AtomicU64 = AtomicU64::new(1);

fn next_navigation_key() -> NativeKey {
    NEXT_HARMONY_NAVIGATION_KEY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("Harmony navigation key space exhausted")
}

#[derive(Clone)]
struct ActiveNavigation {
    key: NativeKey,
    url: String,
    page_epoch: u64,
    committed: bool,
    finished: bool,
    generation: Option<DocumentGeneration>,
}

#[derive(Default)]
struct State {
    active: Option<ActiveNavigation>,
}

pub(crate) enum PageBegin {
    Started { key: NativeKey, url: String },
    Invalid,
}

pub(crate) struct PageTerminal {
    pub(crate) key: NativeKey,
    pub(crate) url: String,
}

pub(crate) enum DocumentCommit {
    Committed(PageTerminal),
    Restored,
    Invalid,
}

/// One native WebView's ArkTS callback correlation. Every top-level begin
/// mints a fresh navigation key, bound to a current native generation by the
/// caller before any method here is reached.
#[derive(Default)]
pub(crate) struct HarmonyDocumentAuthority {
    state: Mutex<State>,
}

impl HarmonyDocumentAuthority {
    pub(crate) fn page_begin(&self, observed_url: &str, page_epoch: u64) -> PageBegin {
        if page_epoch == 0 || observed_url.is_empty() {
            return PageBegin::Invalid;
        }
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let key = next_navigation_key();
        state.active = Some(ActiveNavigation {
            key,
            url: observed_url.to_owned(),
            page_epoch,
            committed: false,
            finished: false,
            generation: None,
        });
        PageBegin::Started {
            key,
            url: observed_url.to_owned(),
        }
    }

    pub(crate) fn document_commit(&self, observed_url: &str, page_epoch: u64) -> DocumentCommit {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(active) = state.active.as_ref().cloned() else {
            return DocumentCommit::Invalid;
        };
        if active.page_epoch != page_epoch || active.url != observed_url {
            // Exact-match by design: a commit that does not name the URL its
            // begin saw cannot bind that navigation. Logged because the whole
            // bridge stays silent when it happens.
            log::warn!(
                "Harmony document commit rejected: begin epoch {}, commit epoch {}, URL matched: {}",
                active.page_epoch,
                page_epoch,
                active.url == observed_url
            );
            state.active = None;
            return DocumentCommit::Invalid;
        }
        if active.committed {
            state.active = None;
            return DocumentCommit::Restored;
        }
        state
            .active
            .as_mut()
            .expect("validated active navigation remains present")
            .committed = true;
        DocumentCommit::Committed(PageTerminal {
            key: active.key,
            url: active.url,
        })
    }

    pub(crate) fn bind_generation(&self, key: NativeKey, generation: DocumentGeneration) -> bool {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(active) = state.active.as_mut() else {
            return false;
        };
        if !active.committed || active.key != key || active.generation.is_some() {
            return false;
        }
        active.generation = Some(generation);
        true
    }

    /// Non-holding form of [`Self::with_current_generation`], for callers that
    /// must not own this lock while acquiring another.
    pub(crate) fn is_current_generation(&self, generation: DocumentGeneration) -> bool {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.active.as_ref().and_then(|active| active.generation) == Some(generation)
    }

    pub(crate) fn with_current_generation(
        &self,
        generation: DocumentGeneration,
        action: &mut dyn FnMut(),
    ) -> bool {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.active.as_ref().and_then(|active| active.generation) != Some(generation) {
            return false;
        }
        action();
        true
    }

    pub(crate) fn page_end(&self, observed_url: &str, page_epoch: u64) -> Option<PageTerminal> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let active = state.active.as_mut()?;
        if active.page_epoch != page_epoch || active.url != observed_url || active.finished {
            state.active = None;
            return None;
        }
        active.finished = true;
        Some(PageTerminal {
            key: active.key,
            url: active.url.clone(),
        })
    }

    pub(crate) fn page_failed(&self, observed_url: &str, page_epoch: u64) -> Option<PageTerminal> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let active = state.active.take()?;
        (active.page_epoch == page_epoch && active.url == observed_url).then_some(PageTerminal {
            key: active.key,
            url: active.url,
        })
    }

    pub(crate) fn invalidate(&self) {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "lx://app/pages/home/index.html";

    fn started(result: PageBegin) -> NativeKey {
        match result {
            PageBegin::Started { key, .. } => key,
            PageBegin::Invalid => panic!("expected an accepted begin"),
        }
    }

    fn committed(result: DocumentCommit) -> PageTerminal {
        match result {
            DocumentCommit::Committed(committed) => committed,
            _ => panic!("expected a fresh document commit"),
        }
    }

    #[test]
    fn commit_requires_its_own_begin_epoch_and_url() {
        let authority = HarmonyDocumentAuthority::default();
        let key = started(authority.page_begin(HOME, 11));
        assert!(matches!(
            authority.document_commit(HOME, 10),
            DocumentCommit::Invalid
        ));

        let key2 = started(authority.page_begin(HOME, 12));
        assert_ne!(key, key2);
        assert!(matches!(
            authority.document_commit("lx://app/other", 12),
            DocumentCommit::Invalid
        ));

        let key3 = started(authority.page_begin(HOME, 13));
        let terminal = committed(authority.document_commit(HOME, 13));
        assert_eq!(terminal.key, key3);
        assert_eq!(terminal.url, HOME);
    }

    #[test]
    fn delivery_gate_closes_as_soon_as_a_successor_begin_is_observed() {
        let authority = HarmonyDocumentAuthority::default();
        started(authority.page_begin(HOME, 60));
        let committed = committed(authority.document_commit(HOME, 60));
        let generation = DocumentGeneration::new(4);
        assert!(authority.bind_generation(committed.key, generation));
        let mut delivered = false;
        assert!(authority.with_current_generation(generation, &mut || delivered = true));
        assert!(delivered);

        started(authority.page_begin("https://external.example", 61));
        assert!(!authority.with_current_generation(generation, &mut || {
            panic!("stale document action must not run")
        }));
    }

    #[test]
    fn repeated_visible_commit_detects_same_url_bfcache_without_new_callbacks() {
        let authority = HarmonyDocumentAuthority::default();
        started(authority.page_begin(HOME, 70));
        let first = committed(authority.document_commit(HOME, 70));
        let generation = DocumentGeneration::new(5);
        assert!(authority.bind_generation(first.key, generation));
        assert!(authority.page_end(HOME, 70).is_some());

        // ArkWeb exposes a new page-visible callback but no new begin/commit
        // identity for a history/BFCache restoration.
        assert!(matches!(
            authority.document_commit(HOME, 70),
            DocumentCommit::Restored
        ));
        assert!(!authority.with_current_generation(generation, &mut || {
            panic!("restored document must lose outbound authority")
        }));
        assert!(matches!(
            authority.document_commit(HOME, 70),
            DocumentCommit::Invalid
        ));
    }

    #[test]
    fn crash_revokes_the_active_document() {
        let authority = HarmonyDocumentAuthority::default();
        started(authority.page_begin(HOME, 40));
        authority.invalidate();
        assert!(matches!(
            authority.document_commit(HOME, 40),
            DocumentCommit::Invalid
        ));
    }

    #[test]
    fn stale_terminal_and_failure_cannot_finish_a_successor() {
        let authority = HarmonyDocumentAuthority::default();
        started(authority.page_begin(HOME, 50));
        started(authority.page_begin(HOME, 51));
        assert!(authority.page_end(HOME, 50).is_none());
        started(authority.page_begin(HOME, 52));
        assert!(authority.page_failed(HOME, 51).is_none());
    }
}
