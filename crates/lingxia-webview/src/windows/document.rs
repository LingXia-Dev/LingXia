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
    /// Redirect restart of the already-attested host load (same native key).
    Coalesced(TrustedLoadIntent),
    /// A different start must pass policy before it may retire this load.
    Competing(TrustedLoadIntent),
    /// Another top-level navigation won the linearization point.
    Revoke(TrustedLoadIntent),
    /// No trusted native load was pending.
    Untrusted,
}

/// WebView2 canonicalizes `lingxia://settings` to `lingxia://settings/` when
/// the scheme is registered with an authority component. Memory pages already
/// ignore that trailing slash; attestation must too, or the host-issued token
/// is revoked before hello and the Settings bridge never becomes ready.
fn trusted_navigation_urls_match(expected: &str, actual: &str) -> bool {
    fn canonical(url: &str) -> String {
        let without_fragment = url.split_once('#').map(|(head, _)| head).unwrap_or(url);
        without_fragment
            .trim()
            .trim_end_matches('/')
            .to_ascii_lowercase()
    }
    canonical(expected) == canonical(actual)
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
            }) if navigation_key != 0 && trusted_navigation_urls_match(&expected_url, url) => {
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
            }) if attested_key == navigation_key
                && trusted_navigation_urls_match(&expected_url, url) =>
            {
                *correlation = Some(TrustedLoadCorrelation::Attested {
                    intent,
                    expected_url,
                    navigation_key,
                });
                TrustedNavigationStart::Coalesced(intent)
            }
            Some(TrustedLoadCorrelation::Pending {
                intent,
                expected_url,
            }) if navigation_key != 0 => {
                // In-document clicks (Settings ↔ Downloads) start an unrelated
                // navigation. Stealing the armed token made the host Navigate
                // land untrusted; the page then sat on its skeleton until
                // "Bridge handshake failed".
                *correlation = Some(TrustedLoadCorrelation::Pending {
                    intent,
                    expected_url,
                });
                TrustedNavigationStart::Competing(intent)
            }
            Some(TrustedLoadCorrelation::Attested {
                intent,
                expected_url,
                navigation_key: attested_key,
            }) if navigation_key != 0 && attested_key != navigation_key => {
                *correlation = Some(TrustedLoadCorrelation::Attested {
                    intent,
                    expected_url,
                    navigation_key: attested_key,
                });
                TrustedNavigationStart::Competing(intent)
            }
            Some(correlation) => TrustedNavigationStart::Revoke(correlation.intent()),
            None => TrustedNavigationStart::Untrusted,
        }
    }

    /// Resolve against the same intent observed before policy called host code.
    /// A cancelled competing click leaves the loader armed; an accepted one
    /// supersedes it in both this correlation and the shared normalizer.
    pub(super) fn resolve_policy(
        &self,
        start: TrustedNavigationStart,
        allowed: bool,
    ) -> Option<TrustedLoadIntent> {
        let intent = match start {
            TrustedNavigationStart::Revoke(intent) => intent,
            TrustedNavigationStart::Competing(intent) if allowed => intent,
            TrustedNavigationStart::Attest { intent, .. }
            | TrustedNavigationStart::Coalesced(intent)
                if !allowed =>
            {
                intent
            }
            _ => return None,
        };
        self.revoke_if_matches(intent);
        Some(intent)
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
    fn trailing_slash_and_scheme_case_still_attest_the_host_issued_load() {
        let authority = WindowsDocumentAuthority::default();
        assert!(
            authority
                .arm(intent(1), "lingxia://settings".into())
                .is_none()
        );
        assert!(matches!(
            authority.navigation_start("lingxia://settings/", 41),
            TrustedNavigationStart::Attest {
                intent: bound,
                navigation_key: 41,
            } if bound == intent(1)
        ));

        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(2), "lingxia://settings/".into());
        assert!(matches!(
            authority.navigation_start("Lingxia://Settings", 42),
            TrustedNavigationStart::Attest { intent: bound, .. } if bound == intent(2)
        ));
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
            TrustedNavigationStart::Coalesced(_)
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
    fn competing_top_level_start_does_not_steal_a_pending_trusted_load() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(2), "lingxia://settings".into());
        assert!(matches!(
            authority.navigation_start("lingxia://downloads", 42),
            TrustedNavigationStart::Competing(_)
        ));
        assert!(matches!(
            authority.navigation_start("lingxia://settings/", 41),
            TrustedNavigationStart::Attest {
                intent: bound,
                navigation_key: 41,
            } if bound == intent(2)
        ));
    }

    #[test]
    fn competing_top_level_start_does_not_steal_an_attested_trusted_load() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(7), "lingxia://downloads".into());
        assert!(matches!(
            authority.navigation_start("lingxia://downloads", 51),
            TrustedNavigationStart::Attest { .. }
        ));
        assert!(matches!(
            authority.navigation_start("lingxia://settings#downloads", 52),
            TrustedNavigationStart::Competing(_)
        ));
        assert!(matches!(
            authority.navigation_start("lingxia://downloads", 51),
            TrustedNavigationStart::Coalesced(_)
        ));
    }

    #[test]
    fn keyless_navigation_still_revokes_the_pending_intent() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(3), "lingxia://settings".into());
        assert!(matches!(
            authority.navigation_start("lingxia://settings", 0),
            TrustedNavigationStart::Revoke(revoked) if revoked == intent(3)
        ));
    }

    #[test]
    fn keyless_start_revokes_an_already_attested_load() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(8), "lingxia://settings".into());
        authority.navigation_start("lingxia://settings", 81);
        let start = authority.navigation_start("lingxia://settings", 0);
        assert!(matches!(start, TrustedNavigationStart::Revoke(value) if value == intent(8)));
        assert!(authority.resolve_policy(start, false) == Some(intent(8)));
        assert!(matches!(
            authority.navigation_start("lingxia://settings", 81),
            TrustedNavigationStart::Untrusted
        ));
    }

    #[test]
    fn cancelling_a_coalesced_start_retires_its_attestation() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(9), "lingxia://settings".into());
        authority.navigation_start("lingxia://settings", 91);
        let start = authority.navigation_start("lingxia://settings", 91);
        assert!(authority.resolve_policy(start, false) == Some(intent(9)));
        assert!(authority.revoke_pending().is_none());
    }

    #[test]
    fn policy_resolution_cannot_revoke_a_reentrant_replacement() {
        let authority = WindowsDocumentAuthority::default();
        authority.arm(intent(10), "lingxia://settings".into());
        let start = authority.navigation_start("https://example.test/", 101);
        authority.arm(intent(11), "lingxia://downloads".into());
        assert!(authority.resolve_policy(start, true) == Some(intent(10)));
        assert!(
            matches!(authority.navigation_start("lingxia://downloads", 102),
            TrustedNavigationStart::Attest { intent: value, .. } if value == intent(11))
        );
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
