//! Harmony top-level document correlation independent of ArkWeb FFI.
#![cfg_attr(not(all(target_os = "linux", target_env = "ohos")), allow(dead_code))]

use crate::TrustedLoadIntent;
use crate::events::normalizer::NativeKey;
use crate::traits::DocumentGeneration;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

const LOAD_MARKER: &str = "__lingxia_native_load";
static NEXT_HARMONY_NAVIGATION_KEY: AtomicU64 = AtomicU64::new(1);

fn next_navigation_key() -> NativeKey {
    NEXT_HARMONY_NAVIGATION_KEY
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("Harmony navigation key space exhausted")
}

fn marked_url(url: &str, native_generation: &str, key: NativeKey) -> String {
    let marker = format!("{LOAD_MARKER}={native_generation}-{key}");
    let (head, fragment) = url.split_once('#').unwrap_or((url, ""));
    let separator = if head.contains('?') { '&' } else { '?' };
    let mut marked = format!("{head}{separator}{marker}");
    if !fragment.is_empty() {
        marked.push('#');
        marked.push_str(fragment);
    }
    marked
}

#[derive(Clone)]
struct TrustedCorrelation {
    intent: TrustedLoadIntent,
    key: NativeKey,
    public_url: String,
    platform_url: String,
    page_epoch: Option<u64>,
}

#[derive(Clone)]
struct ActiveNavigation {
    key: NativeKey,
    public_url: String,
    platform_url: String,
    page_epoch: u64,
    committed: bool,
    finished: bool,
    generation: Option<DocumentGeneration>,
}

#[derive(Default)]
struct State {
    trusted: Option<TrustedCorrelation>,
    active: Option<ActiveNavigation>,
    committed_urls: Option<(String, String)>,
}

pub(crate) struct ArmedTrustedLoad {
    pub(crate) key: NativeKey,
    pub(crate) platform_url: String,
    pub(crate) replaced: Option<TrustedLoadIntent>,
}

pub(crate) enum PageBegin {
    Attest {
        intent: TrustedLoadIntent,
        key: NativeKey,
        public_url: String,
    },
    Untrusted {
        key: NativeKey,
        public_url: String,
        revoked: Option<TrustedLoadIntent>,
    },
    Invalid,
}

pub(crate) struct PageTerminal {
    pub(crate) key: NativeKey,
    pub(crate) public_url: String,
}

pub(crate) enum DocumentCommit {
    Committed(PageTerminal),
    Restored { public_url: String },
    Invalid,
}

/// One native WebView's trusted-load and ArkTS callback correlation.
///
/// URLs are consistency inputs only. Authority comes from the Rust-issued
/// intent and non-reused navigation key, bound to a current native generation
/// by the caller before any method here is reached.
#[derive(Default)]
pub(crate) struct HarmonyDocumentAuthority {
    state: Mutex<State>,
}

impl HarmonyDocumentAuthority {
    pub(crate) fn arm(
        &self,
        intent: TrustedLoadIntent,
        public_url: &str,
        native_generation: &str,
    ) -> ArmedTrustedLoad {
        let key = next_navigation_key();
        let platform_url = marked_url(public_url, native_generation, key);
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let replaced = state.trusted.replace(TrustedCorrelation {
            intent,
            key,
            public_url: public_url.to_owned(),
            platform_url: platform_url.clone(),
            page_epoch: None,
        });
        state.active = None;
        state.committed_urls = None;
        ArmedTrustedLoad {
            key,
            platform_url,
            replaced: replaced.map(|correlation| correlation.intent),
        }
    }

    pub(crate) fn page_begin(&self, observed_url: &str, page_epoch: u64) -> PageBegin {
        if page_epoch == 0 || observed_url.is_empty() {
            return PageBegin::Invalid;
        }
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.committed_urls = None;
        match state.trusted.take() {
            Some(mut trusted)
                if trusted.page_epoch.is_none() && trusted.platform_url == observed_url =>
            {
                trusted.page_epoch = Some(page_epoch);
                let key = trusted.key;
                let public_url = trusted.public_url.clone();
                state.active = Some(ActiveNavigation {
                    key,
                    public_url: public_url.clone(),
                    platform_url: trusted.platform_url.clone(),
                    page_epoch,
                    committed: false,
                    finished: false,
                    generation: None,
                });
                let intent = trusted.intent;
                state.trusted = Some(trusted);
                PageBegin::Attest {
                    intent,
                    key,
                    public_url,
                }
            }
            trusted => {
                // A second, external, redirected, stale, or callback-reused
                // top-level begin makes the pending proof ambiguous. Even an
                // identical public URL cannot inherit the previous token.
                let revoked = trusted.map(|correlation| correlation.intent);
                let key = next_navigation_key();
                state.active = Some(ActiveNavigation {
                    key,
                    public_url: observed_url.to_owned(),
                    platform_url: observed_url.to_owned(),
                    page_epoch,
                    committed: false,
                    finished: false,
                    generation: None,
                });
                PageBegin::Untrusted {
                    key,
                    public_url: observed_url.to_owned(),
                    revoked,
                }
            }
        }
    }

    pub(crate) fn document_commit(&self, observed_url: &str, page_epoch: u64) -> DocumentCommit {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let Some(active) = state.active.as_ref().cloned() else {
            return DocumentCommit::Invalid;
        };
        if active.page_epoch != page_epoch || active.platform_url != observed_url {
            state.active = None;
            state.trusted = None;
            state.committed_urls = None;
            return DocumentCommit::Invalid;
        }
        if active.committed {
            state.active = None;
            state.trusted = None;
            state.committed_urls = None;
            return DocumentCommit::Restored {
                public_url: active.public_url,
            };
        }
        if let Some(trusted) = state.trusted.as_ref()
            && (trusted.key != active.key || trusted.page_epoch != Some(page_epoch))
        {
            state.active = None;
            state.trusted = None;
            state.committed_urls = None;
            return DocumentCommit::Invalid;
        }
        state
            .active
            .as_mut()
            .expect("validated active navigation remains present")
            .committed = true;
        let terminal = PageTerminal {
            key: active.key,
            public_url: active.public_url.clone(),
        };
        state.trusted = None;
        state.committed_urls = Some((active.platform_url.clone(), active.public_url.clone()));
        DocumentCommit::Committed(terminal)
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
        if active.page_epoch != page_epoch || active.platform_url != observed_url || active.finished
        {
            state.active = None;
            state.trusted = None;
            return None;
        }
        active.finished = true;
        Some(PageTerminal {
            key: active.key,
            public_url: active.public_url.clone(),
        })
    }

    pub(crate) fn page_failed(&self, observed_url: &str, page_epoch: u64) -> Option<PageTerminal> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let active = state.active.take()?;
        state.trusted = None;
        state.committed_urls = None;
        (active.page_epoch == page_epoch && active.platform_url == observed_url).then_some(
            PageTerminal {
                key: active.key,
                public_url: active.public_url,
            },
        )
    }

    pub(crate) fn public_url(&self, observed_url: &str) -> String {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state
            .committed_urls
            .as_ref()
            .filter(|(platform, _)| platform == observed_url)
            .map(|(_, public)| public.clone())
            .or_else(|| {
                state
                    .active
                    .as_ref()
                    .filter(|active| active.platform_url == observed_url)
                    .map(|active| active.public_url.clone())
            })
            .or_else(|| {
                state
                    .trusted
                    .as_ref()
                    .filter(|trusted| trusted.platform_url == observed_url)
                    .map(|trusted| trusted.public_url.clone())
            })
            .unwrap_or_else(|| observed_url.to_owned())
    }

    pub(crate) fn invalidate(&self) -> Option<TrustedLoadIntent> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.active = None;
        state.committed_urls = None;
        state.trusted.take().map(|correlation| correlation.intent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(raw: u64) -> TrustedLoadIntent {
        TrustedLoadIntent::new(raw)
    }

    fn committed(result: DocumentCommit) -> PageTerminal {
        match result {
            DocumentCommit::Committed(committed) => committed,
            _ => panic!("expected a fresh document commit"),
        }
    }

    #[test]
    fn trusted_load_requires_its_marked_top_level_begin_and_commit() {
        let authority = HarmonyDocumentAuthority::default();
        let armed = authority.arm(intent(1), "lingxia://settings", "native-7");
        assert!(matches!(
            authority.page_begin(&armed.platform_url, 11),
            PageBegin::Attest { intent: value, key, .. }
                if value == intent(1) && key == armed.key
        ));
        let terminal = committed(authority.document_commit(&armed.platform_url, 11));
        assert_eq!(terminal.key, armed.key);
        assert_eq!(terminal.public_url, "lingxia://settings");
        assert_eq!(
            authority.public_url(&armed.platform_url),
            "lingxia://settings"
        );
    }

    #[test]
    fn stale_same_url_attempt_cannot_reuse_a_replaced_load() {
        let authority = HarmonyDocumentAuthority::default();
        let stale = authority.arm(intent(2), "lingxia://settings", "native-8");
        let current = authority.arm(intent(3), "lingxia://settings", "native-8");
        assert!(current.replaced == Some(intent(2)));
        assert!(matches!(
            authority.page_begin(&stale.platform_url, 12),
            PageBegin::Untrusted { revoked: Some(value), .. } if value == intent(3)
        ));
        assert!(matches!(
            authority.document_commit(&current.platform_url, 13),
            DocumentCommit::Invalid
        ));
    }

    #[test]
    fn callback_epoch_and_port_document_cannot_be_reused_after_reload() {
        let authority = HarmonyDocumentAuthority::default();
        let first = authority.arm(intent(4), "lingxia://downloads", "native-9");
        assert!(matches!(
            authority.page_begin(&first.platform_url, 20),
            PageBegin::Attest { .. }
        ));
        assert!(matches!(
            authority.document_commit(&first.platform_url, 19),
            DocumentCommit::Invalid
        ));

        let reload = authority.arm(intent(5), "lingxia://downloads", "native-9");
        assert!(matches!(
            authority.page_begin(&reload.platform_url, 21),
            PageBegin::Attest { .. }
        ));
        assert!(matches!(
            authority.document_commit(&first.platform_url, 20),
            DocumentCommit::Invalid
        ));
    }

    #[test]
    fn delivery_gate_closes_as_soon_as_a_successor_begin_is_observed() {
        let authority = HarmonyDocumentAuthority::default();
        let first = authority.arm(intent(11), "lingxia://settings", "native-13");
        assert!(matches!(
            authority.page_begin(&first.platform_url, 60),
            PageBegin::Attest { .. }
        ));
        let committed = committed(authority.document_commit(&first.platform_url, 60));
        let generation = DocumentGeneration::new(4);
        assert!(authority.bind_generation(committed.key, generation));
        let mut delivered = false;
        assert!(authority.with_current_generation(generation, &mut || delivered = true));
        assert!(delivered);

        assert!(matches!(
            authority.page_begin("https://external.example", 61),
            PageBegin::Untrusted { .. }
        ));
        assert!(!authority.with_current_generation(generation, &mut || {
            panic!("stale document action must not run")
        }));
    }

    #[test]
    fn repeated_visible_commit_detects_same_url_bfcache_without_new_callbacks() {
        let authority = HarmonyDocumentAuthority::default();
        let load = authority.arm(intent(12), "lingxia://settings", "native-14");
        assert!(matches!(
            authority.page_begin(&load.platform_url, 70),
            PageBegin::Attest { .. }
        ));
        let first = committed(authority.document_commit(&load.platform_url, 70));
        let generation = DocumentGeneration::new(5);
        assert!(authority.bind_generation(first.key, generation));
        assert!(authority.page_end(&load.platform_url, 70).is_some());

        // ArkWeb exposes a new page-visible callback but no new begin/commit
        // identity for a history/BFCache restoration.
        assert!(matches!(
            authority.document_commit(&load.platform_url, 70),
            DocumentCommit::Restored { ref public_url }
                if public_url == "lingxia://settings"
        ));
        assert!(!authority.with_current_generation(generation, &mut || {
            panic!("restored document must lose outbound authority")
        }));
        assert!(matches!(
            authority.document_commit(&load.platform_url, 70),
            DocumentCommit::Invalid
        ));
    }

    #[test]
    fn external_or_second_begin_revokes_trusted_correlation() {
        let authority = HarmonyDocumentAuthority::default();
        let armed = authority.arm(intent(6), "lingxia://history", "native-10");
        assert!(matches!(
            authority.page_begin("https://example.test", 30),
            PageBegin::Untrusted { revoked: Some(value), .. } if value == intent(6)
        ));
        assert!(matches!(
            authority.document_commit(&armed.platform_url, 31),
            DocumentCommit::Invalid
        ));

        let armed = authority.arm(intent(7), "lingxia://history", "native-10");
        assert!(matches!(
            authority.page_begin(&armed.platform_url, 32),
            PageBegin::Attest { .. }
        ));
        assert!(matches!(
            authority.page_begin(&armed.platform_url, 32),
            PageBegin::Untrusted { revoked: Some(value), .. } if value == intent(7)
        ));
    }

    #[test]
    fn crash_revokes_pending_and_committed_url_state() {
        let authority = HarmonyDocumentAuthority::default();
        let armed = authority.arm(intent(8), "lingxia://settings", "native-11");
        assert!(matches!(
            authority.page_begin(&armed.platform_url, 40),
            PageBegin::Attest { .. }
        ));
        assert!(authority.invalidate() == Some(intent(8)));
        assert!(matches!(
            authority.document_commit(&armed.platform_url, 40),
            DocumentCommit::Invalid
        ));
        assert_eq!(
            authority.public_url(&armed.platform_url),
            armed.platform_url
        );
    }

    #[test]
    fn stale_terminal_and_failure_cannot_finish_a_successor() {
        let authority = HarmonyDocumentAuthority::default();
        let first = authority.arm(intent(9), "lingxia://settings", "native-12");
        assert!(matches!(
            authority.page_begin(&first.platform_url, 50),
            PageBegin::Attest { .. }
        ));
        let second = authority.arm(intent(10), "lingxia://settings", "native-12");
        assert!(matches!(
            authority.page_begin(&second.platform_url, 51),
            PageBegin::Attest { .. }
        ));
        assert!(authority.page_end(&first.platform_url, 50).is_none());
        assert!(authority.page_failed(&first.platform_url, 50).is_none());
    }
}
