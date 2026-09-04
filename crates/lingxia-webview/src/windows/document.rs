//! WebView2-only correlation for host-issued trusted document loads.

use crate::TrustedLoadIntent;
use crate::events::normalizer::NativeKey;
use std::sync::Mutex;

/// Whether a WebView2 source callback is evidence of a restored document that
/// bypassed the normal navigation-start invalidation path.
///
/// `HistoryChanged` cannot make this decision: it also fires for same-document
/// history mutations. `SourceChanged.IsNewDocument` is the defensive evidence,
/// while the shared binding tells us whether `NavigationStarting` already
/// revoked the preceding document.
pub(super) fn source_change_requires_reproof(
    is_new_document: bool,
    preceding_document_is_still_bound: bool,
) -> bool {
    is_new_document && preceding_document_is_still_bound
}

enum TrustedLoadCorrelation {
    Pending {
        intent: TrustedLoadIntent,
        expected_url: String,
    },
    Attested {
        intent: TrustedLoadIntent,
        expected_url: String,
        navigation_key: NativeKey,
    },
}

impl TrustedLoadCorrelation {
    fn intent(&self) -> TrustedLoadIntent {
        match self {
            Self::Pending { intent, .. } | Self::Attested { intent, .. } => *intent,
        }
    }
}

/// Result of correlating a top-level WebView2 `NavigationStarting` callback.
#[derive(Clone, Copy)]
pub(super) enum TrustedNavigationStart {
    /// The first matching top-level callback owns this host-issued token.
    Attest {
        intent: TrustedLoadIntent,
        navigation_key: NativeKey,
    },
    /// Another top-level navigation won the linearization point.
    Revoke(TrustedLoadIntent),
    /// No trusted native load was pending.
    Untrusted,
}

/// Per-WebView state shared only by its STA command loop and event callbacks.
///
/// WebView2's `Navigate` does not return a navigation object. The host arms an
/// opaque intent immediately before the call, then consumes it from the first
/// top-level `NavigationStarting` callback whose URL matches the requested
/// internal load. Frame callbacks never enter this state machine.
#[derive(Default)]
pub(super) struct WindowsDocumentAuthority {
    correlation: Mutex<Option<TrustedLoadCorrelation>>,
}

impl WindowsDocumentAuthority {
    pub(super) fn arm(
        &self,
        intent: TrustedLoadIntent,
        expected_url: String,
    ) -> Option<TrustedLoadIntent> {
        self.correlation
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .replace(TrustedLoadCorrelation::Pending {
                intent,
                expected_url,
            })
            .map(|correlation| correlation.intent())
    }

    pub(super) fn navigation_start(
        &self,
        url: &str,
        navigation_key: NativeKey,
    ) -> TrustedNavigationStart {
        let mut correlation = self
            .correlation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match correlation.take() {
            Some(TrustedLoadCorrelation::Pending {
                intent,
                expected_url,
            }) if navigation_key != 0 && expected_url == url => {
                *correlation = Some(TrustedLoadCorrelation::Attested {
                    intent,
                    expected_url,
                    navigation_key,
                });
                TrustedNavigationStart::Attest {
                    intent,
                    navigation_key,
                }
            }
            Some(TrustedLoadCorrelation::Attested {
                intent,
                expected_url,
                navigation_key: attested_key,
            }) if attested_key == navigation_key && expected_url == url => {
                *correlation = Some(TrustedLoadCorrelation::Attested {
                    intent,
                    expected_url,
                    navigation_key,
                });
                TrustedNavigationStart::Untrusted
            }
            Some(correlation) => TrustedNavigationStart::Revoke(correlation.intent()),
            None => TrustedNavigationStart::Untrusted,
        }
    }

    pub(super) fn navigation_finished(&self, navigation_key: NativeKey) {
        let mut correlation = self
            .correlation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if matches!(
            correlation.as_ref(),
            Some(TrustedLoadCorrelation::Attested {
                navigation_key: current,
                ..
            }) if *current == navigation_key
        ) {
            *correlation = None;
        }
    }

    pub(super) fn revoke_if_matches(&self, intent: TrustedLoadIntent) -> bool {
        let mut correlation = self
            .correlation
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if correlation
            .as_ref()
            .is_some_and(|correlation| correlation.intent() == intent)
        {
            *correlation = None;
            true
        } else {
            false
        }
    }

    pub(super) fn revoke_pending(&self) -> Option<TrustedLoadIntent> {
        self.correlation
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .map(|correlation| correlation.intent())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(raw: u64) -> TrustedLoadIntent {
        TrustedLoadIntent::new(raw)
    }

    #[test]
    fn host_load_binds_only_the_first_matching_top_level_navigation() {
        let authority = WindowsDocumentAuthority::default();
        assert!(
            authority
                .arm(intent(1), "lingxia://settings".into())
                .is_none()
        );
        assert!(matches!(
            authority.navigation_start("lingxia://settings", 41),
            TrustedNavigationStart::Attest {
                intent: bound,
                navigation_key: 41,
            } if bound == intent(1)
        ));
        assert!(matches!(
            authority.navigation_start("lingxia://settings", 41),
            TrustedNavigationStart::Untrusted
        ));
        authority.navigation_finished(41);
        assert!(authority.revoke_pending().is_none());
    }

    #[test]
    fn redirect_to_an_unexpected_url_revokes_the_attested_intent() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(6), "lingxia://settings".into());
        assert!(matches!(
            authority.navigation_start("lingxia://settings", 43),
            TrustedNavigationStart::Attest { .. }
        ));
        assert!(matches!(
            authority.navigation_start("https://example.test/redirect", 43),
            TrustedNavigationStart::Revoke(revoked) if revoked == intent(6)
        ));
        assert!(authority.revoke_pending().is_none());
    }

    #[test]
    fn external_or_keyless_navigation_revokes_the_pending_intent() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(2), "lingxia://settings".into());
        assert!(matches!(
            authority.navigation_start("https://example.test/", 42),
            TrustedNavigationStart::Revoke(revoked) if revoked == intent(2)
        ));

        authority.arm(intent(3), "lingxia://settings".into());
        assert!(matches!(
            authority.navigation_start("lingxia://settings", 0),
            TrustedNavigationStart::Revoke(revoked) if revoked == intent(3)
        ));
    }

    #[test]
    fn replacement_and_crash_revoke_only_the_current_pending_load() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(4), "lingxia://settings".into());
        assert!(authority.arm(intent(5), "lingxia://downloads".into()) == Some(intent(4)));
        assert!(!authority.revoke_if_matches(intent(4)));
        assert!(authority.revoke_pending() == Some(intent(5)));
        assert!(authority.revoke_pending().is_none());
    }

    #[test]
    fn new_document_without_navigation_evidence_requires_reproof() {
        assert!(source_change_requires_reproof(true, true));
    }

    #[test]
    fn navigation_start_invalidates_before_new_document_source_change() {
        assert!(!source_change_requires_reproof(true, false));
    }

    #[test]
    fn same_document_history_changes_never_revoke_authority() {
        // pushState/replaceState and fragment navigation may emit source and
        // history callbacks without creating a new document.
        assert!(!source_change_requires_reproof(false, true));
        assert!(!source_change_requires_reproof(false, false));
    }
}
