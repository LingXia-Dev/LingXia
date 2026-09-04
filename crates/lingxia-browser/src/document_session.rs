//! Browser-internal trusted control-document session state.
//!
//! Native evidence reaches `BootstrapPending` only. A later V3 hello must
//! authenticate its native-generated binding before the document is `Active`.

use crate::internal_pages::InternalPageTarget;
use lingxia_webview::{
    DocumentGeneration, NativeWebViewId, NavigationId, TrustedDocumentAdmission, TrustedLoadIntent,
};
use lxapp::{
    ControlDocumentBootstrap, ControlDocumentSessionMaterial, issue_control_document_bootstrap,
};
use ring::rand::SystemRandom;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, OnceLock};

/// Read-only proof that one browser control document completed V3 hello.
/// It intentionally has no secret or route-authority field.
#[allow(dead_code)] // Future V3 bridge admission consumes this lease in Commit B.
pub(crate) struct ActiveControlDocumentLease {
    native_view: NativeWebViewId,
    tab_session_id: u64,
    create_token: u64,
    target: InternalPageTarget,
    navigation_id: NavigationId,
    generation: DocumentGeneration,
}

#[allow(dead_code)] // Kept opaque until the future bridge consumer is wired.
impl ActiveControlDocumentLease {
    pub(crate) const fn native_view(&self) -> NativeWebViewId {
        self.native_view
    }
    pub(crate) const fn tab_session_id(&self) -> u64 {
        self.tab_session_id
    }
    pub(crate) const fn create_token(&self) -> u64 {
        self.create_token
    }
    pub(crate) fn target(&self) -> &InternalPageTarget {
        &self.target
    }
    pub(crate) const fn navigation_id(&self) -> NavigationId {
        self.navigation_id
    }
    pub(crate) const fn generation(&self) -> DocumentGeneration {
        self.generation
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ControlDocumentBootstrapPrepareError {
    #[error("secure random generation for control document bootstrap failed")]
    EntropyUnavailable,
}

trait SessionMaterial {
    fn matches(&self, session_id: &str, secret: &str) -> bool;
}

impl SessionMaterial for ControlDocumentSessionMaterial {
    fn matches(&self, session_id: &str, secret: &str) -> bool {
        ControlDocumentSessionMaterial::matches(self, session_id, secret)
    }
}

pub(crate) enum DocumentSessionState<Intent, Navigation, Generation, Target, Material> {
    Revoked,
    Reserved {
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        intent: Intent,
        navigation_id: Option<Navigation>,
        material: Material,
    },
    BootstrapPending {
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        intent: Intent,
        navigation_id: Navigation,
        generation: Generation,
        material: Material,
    },
    Active {
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        // Commit A preserves the exact core-issued intent for the later V3
        // frame admission step, even though no production consumer reads it.
        #[allow(dead_code)]
        intent: Intent,
        navigation_id: Navigation,
        generation: Generation,
        material: Material,
    },
}

struct ActiveSessionLease<Target, Navigation, Generation> {
    tab_session_id: u64,
    create_token: u64,
    target: Target,
    navigation_id: Navigation,
    generation: Generation,
}

struct DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target, Material> {
    sessions:
        HashMap<NativeView, DocumentSessionState<Intent, Navigation, Generation, Target, Material>>,
}

impl<NativeView, Intent, Navigation, Generation, Target, Material> Default
    for DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target, Material>
{
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

impl<NativeView, Intent, Navigation, Generation, Target, Material>
    DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target, Material>
where
    NativeView: Copy + Eq + Hash,
    Intent: Copy + Eq,
    Navigation: Copy + Eq,
    Generation: Copy + Eq,
    Target: Clone,
    Material: SessionMaterial,
{
    fn prepare(
        &mut self,
        native_view: NativeView,
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        intent: Intent,
        material: Material,
    ) {
        self.sessions.insert(
            native_view,
            DocumentSessionState::Reserved {
                tab_session_id,
                create_token,
                target,
                intent,
                navigation_id: None,
                material,
            },
        );
    }

    fn navigation_started(&mut self, native_view: NativeView, navigation_id: Navigation) -> bool {
        match self.sessions.get_mut(&native_view) {
            Some(DocumentSessionState::Reserved {
                navigation_id: expected @ None,
                ..
            }) => {
                *expected = Some(navigation_id);
                true
            }
            Some(DocumentSessionState::Reserved {
                navigation_id: Some(expected),
                ..
            }) if *expected == navigation_id => true,
            _ => {
                self.force_revoke(native_view);
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
        let Some(DocumentSessionState::Reserved {
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
        let previous = std::mem::replace(state, DocumentSessionState::Revoked);
        let DocumentSessionState::Reserved {
            tab_session_id: expected_session,
            create_token: expected_token,
            target,
            intent: expected_intent,
            navigation_id: expected_navigation,
            material,
        } = previous
        else {
            *state = previous;
            return None;
        };
        if expected_session != tab_session_id
            || expected_token != create_token
            || expected_intent != intent
            || expected_navigation != Some(navigation_id)
        {
            *state = DocumentSessionState::Reserved {
                tab_session_id: expected_session,
                create_token: expected_token,
                target,
                intent: expected_intent,
                navigation_id: expected_navigation,
                material,
            };
            return None;
        }
        let admitted_target = target.clone();
        *state = DocumentSessionState::BootstrapPending {
            tab_session_id,
            create_token,
            target,
            intent,
            navigation_id,
            generation,
            material,
        };
        Some(admitted_target)
    }

    fn activate_hello(
        &mut self,
        native_view: NativeView,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveSessionLease<Target, Navigation, Generation>> {
        let state = self.sessions.get_mut(&native_view)?;
        let previous = std::mem::replace(state, DocumentSessionState::Revoked);
        let DocumentSessionState::BootstrapPending {
            tab_session_id,
            create_token,
            target,
            intent,
            navigation_id,
            generation,
            material,
        } = previous
        else {
            *state = previous;
            return None;
        };
        if !material.matches(session_id, secret) {
            *state = DocumentSessionState::BootstrapPending {
                tab_session_id,
                create_token,
                target,
                intent,
                navigation_id,
                generation,
                material,
            };
            return None;
        }
        let lease = ActiveSessionLease {
            tab_session_id,
            create_token,
            target: target.clone(),
            navigation_id,
            generation,
        };
        *state = DocumentSessionState::Active {
            tab_session_id,
            create_token,
            target,
            intent,
            navigation_id,
            generation,
            material,
        };
        Some(lease)
    }

    fn authenticate_frame(
        &self,
        native_view: NativeView,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveSessionLease<Target, Navigation, Generation>> {
        let DocumentSessionState::Active {
            tab_session_id,
            create_token,
            target,
            navigation_id,
            generation,
            material,
            ..
        } = self.sessions.get(&native_view)?
        else {
            return None;
        };
        material
            .matches(session_id, secret)
            .then(|| ActiveSessionLease {
                tab_session_id: *tab_session_id,
                create_token: *create_token,
                target: target.clone(),
                navigation_id: *navigation_id,
                generation: *generation,
            })
    }

    #[cfg(test)]
    fn state(
        &self,
        native_view: NativeView,
    ) -> Option<&DocumentSessionState<Intent, Navigation, Generation, Target, Material>> {
        self.sessions.get(&native_view)
    }
}

/// Browser wrapper over opaque core identities. This commit records native
/// evidence only; bridge and route authorization remain disabled.
pub(crate) struct BrowserDocumentSessions {
    inner: Mutex<
        DocumentSessionRegistry<
            NativeWebViewId,
            TrustedLoadIntent,
            NavigationId,
            DocumentGeneration,
            InternalPageTarget,
            ControlDocumentSessionMaterial,
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
    ) -> Result<ControlDocumentBootstrap, ControlDocumentBootstrapPrepareError> {
        let (bootstrap, material) = issue_control_document_bootstrap(&SystemRandom::new())
            .map_err(|_| ControlDocumentBootstrapPrepareError::EntropyUnavailable)?;
        let mut sessions = self
            .inner
            .lock()
            .map_err(|_| ControlDocumentBootstrapPrepareError::EntropyUnavailable)?;
        sessions.prepare(
            native_view,
            tab_session_id,
            create_token,
            target,
            intent,
            material,
        );
        Ok(bootstrap)
    }

    pub(crate) fn navigation_started(
        &self,
        native_view: NativeWebViewId,
        navigation_id: NavigationId,
    ) -> bool {
        self.inner
            .lock()
            .is_ok_and(|mut sessions| sessions.navigation_started(native_view, navigation_id))
    }

    pub(crate) fn revoke_if_matches(
        &self,
        native_view: NativeWebViewId,
        tab_session_id: u64,
        create_token: u64,
        intent: TrustedLoadIntent,
    ) -> bool {
        self.inner.lock().is_ok_and(|mut sessions| {
            sessions.revoke_if_matches(native_view, tab_session_id, create_token, intent)
        })
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

    /// Future V3 hello seam. No production path calls this in Commit A.
    #[allow(dead_code)] // Reserved for Commit B's V3 hello handler.
    pub(crate) fn activate_hello(
        &self,
        native_view: NativeWebViewId,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveControlDocumentLease> {
        let lease = self
            .inner
            .lock()
            .ok()?
            .activate_hello(native_view, session_id, secret)?;
        Some(ActiveControlDocumentLease {
            native_view,
            tab_session_id: lease.tab_session_id,
            create_token: lease.create_token,
            target: lease.target,
            navigation_id: lease.navigation_id,
            generation: lease.generation,
        })
    }

    /// Future V3 per-frame seam. No production path calls this in Commit A.
    #[allow(dead_code)] // Reserved for Commit B's per-frame V3 authentication.
    pub(crate) fn authenticate_frame(
        &self,
        native_view: NativeWebViewId,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveControlDocumentLease> {
        let lease = self
            .inner
            .lock()
            .ok()?
            .authenticate_frame(native_view, session_id, secret)?;
        Some(ActiveControlDocumentLease {
            native_view,
            tab_session_id: lease.tab_session_id,
            create_token: lease.create_token,
            target: lease.target,
            navigation_id: lease.navigation_id,
            generation: lease.generation,
        })
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

    struct TestMaterial(&'static str, &'static str);
    impl SessionMaterial for TestMaterial {
        fn matches(&self, session_id: &str, secret: &str) -> bool {
            self.0 == session_id && self.1 == secret
        }
    }
    type TestRegistry = DocumentSessionRegistry<u64, u64, u64, u64, &'static str, TestMaterial>;

    fn prepare(sessions: &mut TestRegistry, native: u64, intent: u64) {
        sessions.prepare(
            native,
            1,
            2,
            "settings",
            intent,
            TestMaterial("session-id-012345", "secret"),
        );
    }

    #[test]
    fn admission_stops_at_bootstrap_pending_until_hello_activates_once() {
        let mut sessions = TestRegistry::default();
        prepare(&mut sessions, 10, 7);
        sessions.navigation_started(10, 4);
        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 7), Some("settings"));
        assert!(matches!(
            sessions.state(10),
            Some(DocumentSessionState::BootstrapPending { .. })
        ));
        assert!(
            sessions
                .authenticate_frame(10, "session-id-012345", "secret")
                .is_none()
        );
        assert!(sessions.activate_hello(10, "wrong", "secret").is_none());
        assert!(
            sessions
                .activate_hello(10, "session-id-012345", "wrong")
                .is_none()
        );
        assert!(
            sessions
                .activate_hello(10, "session-id-012345", "secret")
                .is_some()
        );
        assert!(matches!(
            sessions.state(10),
            Some(DocumentSessionState::Active { .. })
        ));
        assert!(
            sessions
                .activate_hello(10, "session-id-012345", "secret")
                .is_none()
        );
        assert!(
            sessions
                .authenticate_frame(10, "session-id-012345", "secret")
                .is_some()
        );
    }

    #[test]
    fn wrong_tuple_and_revoke_never_leave_an_active_lease() {
        let mut sessions = TestRegistry::default();
        prepare(&mut sessions, 10, 7);
        sessions.navigation_started(10, 4);
        assert!(sessions.admit(10, 1, 99, 4, 5, 7).is_none());
        assert!(sessions.admit(10, 1, 2, 4, 5, 7).is_some());
        sessions.force_revoke(10);
        assert!(
            sessions
                .activate_hello(10, "session-id-012345", "secret")
                .is_none()
        );
        assert!(
            sessions
                .authenticate_frame(10, "session-id-012345", "secret")
                .is_none()
        );
        assert!(matches!(
            sessions.state(10),
            Some(DocumentSessionState::Revoked)
        ));
    }

    #[test]
    fn stale_rollback_and_recreated_native_view_cannot_inherit_a_reservation() {
        let mut sessions = TestRegistry::default();
        prepare(&mut sessions, 10, 7);
        prepare(&mut sessions, 10, 8);
        assert!(!sessions.revoke_if_matches(10, 1, 2, 7));
        assert!(sessions.navigation_started(10, 4));
        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 8), Some("settings"));

        sessions.remove(10);
        prepare(&mut sessions, 11, 1);
        assert_eq!(sessions.admit(10, 1, 2, 4, 5, 8), None);
        assert!(sessions.navigation_started(11, 6));
        assert_eq!(sessions.admit(11, 1, 2, 6, 7, 1), Some("settings"));
    }
}
