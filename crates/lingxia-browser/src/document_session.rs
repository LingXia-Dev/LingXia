//! Browser-internal document admission state.
//!
//! This is deliberately separate from bridge/route admission. It records only
//! native evidence for one browser-owned direct HTML load, keyed by the exact
//! native WebView identity.

use crate::internal_pages::InternalPageTarget;
use lingxia_webview::{
    DocumentGeneration, NativeWebViewId, NavigationId, TrustedDocumentAdmission, TrustedLoadIntent,
};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DocumentSessionState<Intent, Navigation, Generation, Target> {
    Revoked,
    Pending {
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        intent: Intent,
        navigation_id: Option<Navigation>,
    },
    Active {
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        intent: Intent,
        navigation_id: Navigation,
        generation: Generation,
    },
}

struct DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target> {
    sessions: HashMap<NativeView, DocumentSessionState<Intent, Navigation, Generation, Target>>,
}

impl<NativeView, Intent, Navigation, Generation, Target> Default
    for DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target>
{
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

impl<NativeView, Intent, Navigation, Generation, Target>
    DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target>
where
    NativeView: Copy + Eq + Hash,
    Intent: Copy + Eq,
    Navigation: Copy + Eq,
    Generation: Copy + Eq,
    Target: Clone,
{
    fn prepare(
        &mut self,
        native_view: NativeView,
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        intent: Intent,
    ) {
        self.sessions.insert(
            native_view,
            DocumentSessionState::Pending {
                tab_session_id,
                create_token,
                target,
                intent,
                navigation_id: None,
            },
        );
    }

    fn navigation_started(&mut self, native_view: NativeView, navigation_id: Navigation) -> bool {
        // A host reservation is already the successor to any old document.
        // The following native start belongs to that one-shot load, while an
        // Active record always belongs to the document being replaced.
        match self.sessions.get_mut(&native_view) {
            Some(DocumentSessionState::Pending {
                navigation_id: expected @ None,
                ..
            }) => {
                *expected = Some(navigation_id);
                true
            }
            Some(DocumentSessionState::Pending {
                navigation_id: Some(expected),
                ..
            }) if *expected == navigation_id => true,
            _ => {
                self.sessions
                    .insert(native_view, DocumentSessionState::Revoked);
                false
            }
        }
    }

    fn force_revoke(&mut self, native_view: NativeView) {
        self.sessions
            .insert(native_view, DocumentSessionState::Revoked);
    }

    fn revoke_if_matches(
        &mut self,
        native_view: NativeView,
        tab_session_id: u64,
        create_token: u64,
        intent: Intent,
    ) -> bool {
        let Some(DocumentSessionState::Pending {
            tab_session_id: expected_session,
            create_token: expected_token,
            intent: expected_intent,
            ..
        }) = self.sessions.get(&native_view)
        else {
            return false;
        };
        if *expected_session != tab_session_id
            || *expected_token != create_token
            || *expected_intent != intent
        {
            return false;
        }
        self.force_revoke(native_view);
        true
    }

    fn remove(&mut self, native_view: NativeView) {
        self.sessions.remove(&native_view);
    }

    fn admit(
        &mut self,
        native_view: NativeView,
        tab_session_id: u64,
        create_token: u64,
        navigation_id: Navigation,
        generation: Generation,
        intent: Intent,
    ) -> Option<Target> {
        let state = self.sessions.get_mut(&native_view)?;
        let DocumentSessionState::Pending {
            tab_session_id: expected_session,
            create_token: expected_token,
            target,
            intent: expected_intent,
            navigation_id: expected_navigation,
        } = state
        else {
            return None;
        };
        if *expected_session != tab_session_id
            || *expected_token != create_token
            || *expected_intent != intent
            || *expected_navigation != Some(navigation_id)
        {
            return None;
        }
        let target = target.clone();
        *state = DocumentSessionState::Active {
            tab_session_id,
            create_token,
            target: target.clone(),
            intent,
            navigation_id,
            generation,
        };
        Some(target)
    }

    #[cfg(test)]
    fn state(
        &self,
        native_view: NativeView,
    ) -> Option<&DocumentSessionState<Intent, Navigation, Generation, Target>> {
        self.sessions.get(&native_view)
    }
}

/// Browser wrapper over opaque core identities. It exposes no URL-derived
/// mutation and does not authorize routes in this commit.
pub(crate) struct BrowserDocumentSessions {
    inner: Mutex<
        DocumentSessionRegistry<
            NativeWebViewId,
            TrustedLoadIntent,
            NavigationId,
            DocumentGeneration,
            InternalPageTarget,
        >,
    >,
}

impl Default for BrowserDocumentSessions {
    fn default() -> Self {
        Self {
            inner: Mutex::new(DocumentSessionRegistry::default()),
        }
    }
}

impl BrowserDocumentSessions {
    pub(crate) fn prepare(
        &self,
        native_view: NativeWebViewId,
        tab_session_id: u64,
        create_token: u64,
        target: InternalPageTarget,
        intent: TrustedLoadIntent,
    ) {
        if let Ok(mut sessions) = self.inner.lock() {
            sessions.prepare(native_view, tab_session_id, create_token, target, intent);
        }
    }

    pub(crate) fn navigation_started(
        &self,
        native_view: NativeWebViewId,
        navigation_id: NavigationId,
    ) -> bool {
        if let Ok(mut sessions) = self.inner.lock() {
            return sessions.navigation_started(native_view, navigation_id);
        }
        false
    }

    pub(crate) fn revoke_if_matches(
        &self,
        native_view: NativeWebViewId,
        tab_session_id: u64,
        create_token: u64,
        intent: TrustedLoadIntent,
    ) -> bool {
        if let Ok(mut sessions) = self.inner.lock() {
            return sessions.revoke_if_matches(native_view, tab_session_id, create_token, intent);
        }
        false
    }

    pub(crate) fn destroy_native_view(&self, native_view: NativeWebViewId) {
        if let Ok(mut sessions) = self.inner.lock() {
            sessions.remove(native_view);
        }
    }

    pub(crate) fn admit(
        &self,
        tab_session_id: u64,
        create_token: u64,
        admission: TrustedDocumentAdmission,
    ) -> Option<InternalPageTarget> {
        self.inner.lock().ok()?.admit(
            admission.native_view(),
            tab_session_id,
            create_token,
            admission.navigation_id(),
            admission.generation(),
            admission.intent(),
        )
    }
}

static BROWSER_DOCUMENT_SESSIONS: OnceLock<Arc<BrowserDocumentSessions>> = OnceLock::new();

pub(crate) fn browser_document_sessions() -> Arc<BrowserDocumentSessions> {
    BROWSER_DOCUMENT_SESSIONS
        .get_or_init(|| Arc::new(BrowserDocumentSessions::default()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestRegistry = DocumentSessionRegistry<u64, u64, u64, u64, &'static str>;

    #[test]
    fn exact_admission_activates_only_the_matching_pending_load() {
        let mut sessions = TestRegistry::default();
        sessions.prepare(10, 1, 2, "settings", 7);

        sessions.navigation_started(10, 4);
        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 7), Some("settings"));
        assert!(matches!(
            sessions.state(10),
            Some(DocumentSessionState::Active {
                tab_session_id: 1,
                create_token: 2,
                target: "settings",
                intent: 7,
                navigation_id: 4,
                generation: 5,
            })
        ));
    }

    #[test]
    fn stale_or_reordered_admission_cannot_replace_pending_target() {
        let mut sessions = TestRegistry::default();
        sessions.prepare(10, 1, 2, "settings", 7);
        sessions.prepare(10, 1, 2, "downloads", 8);

        sessions.navigation_started(10, 4);
        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 7), None);
        assert_eq!(sessions.admit(10, 1, 3, 4, 5, 8), None);
        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 8), Some("downloads"));
    }

    #[test]
    fn navigation_start_revokes_active_but_keeps_the_host_reservation() {
        let mut sessions = TestRegistry::default();
        sessions.prepare(10, 1, 2, "settings", 7);
        sessions.navigation_started(10, 4);
        sessions.navigation_started(10, 4);

        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 7), Some("settings"));
        sessions.navigation_started(10, 5);
        assert!(matches!(
            sessions.state(10),
            Some(DocumentSessionState::Revoked)
        ));
    }

    #[test]
    fn recreated_native_view_cannot_inherit_an_old_tab_session() {
        let mut sessions = TestRegistry::default();
        sessions.prepare(10, 1, 2, "settings", 7);
        sessions.remove(10);
        sessions.prepare(11, 1, 3, "settings", 8);

        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 7), None);
        sessions.navigation_started(11, 6);
        assert_eq!(sessions.admit(11, 1, 3, 6, 7, 8), Some("settings"));
    }

    #[test]
    fn unsupported_load_without_intent_never_creates_a_session() {
        let mut sessions = TestRegistry::default();

        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 7), None);
        assert!(sessions.state(10).is_none());
    }

    #[test]
    fn dropped_or_failed_reservation_force_revokes_its_pending_session() {
        let mut sessions = TestRegistry::default();
        sessions.prepare(10, 1, 2, "settings", 7);
        assert!(sessions.revoke_if_matches(10, 1, 2, 7));

        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 7), None);
        assert!(matches!(
            sessions.state(10),
            Some(DocumentSessionState::Revoked)
        ));
    }

    #[test]
    fn stale_reservation_rollback_cannot_revoke_a_successor() {
        let mut sessions = TestRegistry::default();
        sessions.prepare(10, 1, 2, "settings", 7);
        sessions.prepare(10, 1, 2, "downloads", 8);

        assert!(!sessions.revoke_if_matches(10, 1, 2, 7));
        sessions.navigation_started(10, 9);
        assert_eq!(sessions.admit(10, 1, 2, 9, 10, 8), Some("downloads"));
    }
}
