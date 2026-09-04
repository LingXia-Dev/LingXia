//! Browser-internal trusted control-document session state.
//!
//! Native evidence reaches `BootstrapPending` only. A later V3 hello must
//! authenticate its native-generated binding before the document is `Active`.

use crate::inbound::BrowserInboundRejectReason;
use crate::internal_pages::InternalPageTarget;
use lingxia_webview::{
    DocumentBinding, DocumentGeneration, DocumentOutboundGate, NativeWebViewId, NavigationId,
    TrustedDocumentAdmission, TrustedLoadIntent, WebMessageContext, WebMessageFrame,
    WebMessageTransport,
};
use lxapp::{ControlDocumentAuthority, ControlDocumentBootstrap, issue_control_document_bootstrap};
use ring::rand::SystemRandom;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, OnceLock, Weak};

type BrowserSessionRegistry = DocumentSessionRegistry<
    NativeWebViewId,
    TrustedLoadIntent,
    NavigationId,
    DocumentGeneration,
    InternalPageTarget,
    ControlDocumentAuthority,
>;
type SharedBrowserSessionRegistry = Arc<Mutex<BrowserSessionRegistry>>;
type WeakBrowserSessionRegistry = Weak<Mutex<BrowserSessionRegistry>>;

/// Read-only proof that one browser control document completed V3 hello.
/// It intentionally has no secret or route-authority field.
#[allow(dead_code)] // Retained for exact-lease registry tests and introspection.
pub(crate) struct ActiveDocumentLease {
    registry: WeakBrowserSessionRegistry,
    native_view: NativeWebViewId,
    tab_session_id: u64,
    create_token: u64,
    target: InternalPageTarget,
    intent: TrustedLoadIntent,
    navigation_id: NavigationId,
    generation: DocumentGeneration,
    public_session_id: String,
    lease_token: u64,
    authority: ControlDocumentAuthority,
}

/// Immutable BootstrapPending binding. It proves the exact native admission
/// is still current for RequiredV3 installation, but is not an outbound gate.
#[allow(dead_code)] // Consumed by RequiredV3 PageBridge binding.
#[derive(Clone)]
pub(crate) struct PendingDocumentLease {
    registry: WeakBrowserSessionRegistry,
    native_view: NativeWebViewId,
    tab_session_id: u64,
    create_token: u64,
    target: InternalPageTarget,
    intent: TrustedLoadIntent,
    navigation_id: NavigationId,
    generation: DocumentGeneration,
    authority: ControlDocumentAuthority,
}

#[allow(dead_code)] // Kept opaque outside the browser lifecycle TCB.
impl ActiveDocumentLease {
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

    pub(crate) fn authority(&self) -> ControlDocumentAuthority {
        self.authority.clone()
    }
}

#[allow(dead_code)] // Bound by BrowserTabDelegate once bridge facade lands.
impl PendingDocumentLease {
    pub(crate) fn authority(&self) -> ControlDocumentAuthority {
        self.authority.clone()
    }

    pub(crate) const fn native_view(&self) -> NativeWebViewId {
        self.native_view
    }

    pub(crate) const fn generation(&self) -> DocumentGeneration {
        self.generation
    }

    /// Runs only while the exact BootstrapPending admission remains current.
    /// The closure receives the native-held binding authority and a separate
    /// active-only outbound gate for this same immutable session identity.
    pub(crate) fn with_bootstrap_pending_current(
        &self,
        context: &WebMessageContext,
        action: &mut dyn FnMut(ControlDocumentAuthority, Arc<dyn DocumentOutboundGate>),
    ) -> bool {
        let Some(registry) = self.registry.upgrade() else {
            return false;
        };
        let Ok(registry_guard) = registry.lock() else {
            return false;
        };
        let Some(DocumentSessionState::BootstrapPending {
            tab_session_id,
            create_token,
            intent,
            navigation_id,
            generation,
            ..
        }) = registry_guard.sessions.get(&self.native_view)
        else {
            return false;
        };
        if *tab_session_id != self.tab_session_id
            || *create_token != self.create_token
            || *intent != self.intent
            || *navigation_id != self.navigation_id
            || *generation != self.generation
            || BrowserDocumentSessions::validate_context(context, self.native_view, self.generation)
                .is_err()
        {
            return false;
        }
        let active_gate: Arc<dyn DocumentOutboundGate> = Arc::new(self.clone());
        action(self.authority.clone(), active_gate);
        true
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ControlDocumentBootstrapPrepareError {
    #[error("secure random generation for control document bootstrap failed")]
    EntropyUnavailable,
}

trait SessionMaterial {
    fn matches(&self, session_id: &str, secret: &str) -> bool;
    fn revoke_execution(&self);
}

impl SessionMaterial for ControlDocumentAuthority {
    fn matches(&self, session_id: &str, secret: &str) -> bool {
        ControlDocumentAuthority::matches(self, session_id, secret)
    }

    fn revoke_execution(&self) {
        ControlDocumentAuthority::revoke_execution(self);
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
        // Preserve the exact core-issued intent for V3 frame admission.
        intent: Intent,
        navigation_id: Navigation,
        generation: Generation,
        material: Material,
        public_session_id: String,
        lease_token: u64,
    },
}

struct ActiveSessionLease<Intent, Navigation, Generation, Target, Material> {
    tab_session_id: u64,
    create_token: u64,
    target: Target,
    navigation_id: Navigation,
    generation: Generation,
    intent: Intent,
    public_session_id: String,
    lease_token: u64,
    material: Material,
}

struct PendingSessionLease<Intent, Navigation, Generation, Target, Material> {
    tab_session_id: u64,
    create_token: u64,
    target: Target,
    intent: Intent,
    navigation_id: Navigation,
    generation: Generation,
    material: Material,
}

struct DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target, Material> {
    sessions:
        HashMap<NativeView, DocumentSessionState<Intent, Navigation, Generation, Target, Material>>,
    next_lease_token: u64,
}

impl<NativeView, Intent, Navigation, Generation, Target, Material> Default
    for DocumentSessionRegistry<NativeView, Intent, Navigation, Generation, Target, Material>
{
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            next_lease_token: 1,
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
    Material: SessionMaterial + Clone,
{
    fn prepare(
        &mut self,
        native_view: NativeView,
        tab_session_id: u64,
        create_token: u64,
        target: Target,
        intent: Intent,
        material: Material,
    ) -> Option<Material> {
        if let Some(previous) = self.sessions.insert(
            native_view,
            DocumentSessionState::Reserved {
                tab_session_id,
                create_token,
                target,
                intent,
                navigation_id: None,
                material,
            },
        ) {
            let replaced = Self::material_from_state(&previous);
            Self::revoke_state_execution(&previous);
            replaced
        } else {
            None
        }
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

    fn revoke_navigation_if_matches(
        &mut self,
        native_view: NativeView,
        navigation_id: Navigation,
    ) -> bool {
        let matches_navigation = match self.sessions.get(&native_view) {
            Some(DocumentSessionState::Reserved {
                navigation_id: Some(expected),
                ..
            }) => *expected == navigation_id,
            Some(DocumentSessionState::BootstrapPending {
                navigation_id: expected,
                ..
            })
            | Some(DocumentSessionState::Active {
                navigation_id: expected,
                ..
            }) => *expected == navigation_id,
            _ => false,
        };
        if matches_navigation {
            self.force_revoke(native_view);
        }
        matches_navigation
    }

    fn force_revoke(&mut self, native_view: NativeView) {
        if let Some(previous) = self
            .sessions
            .insert(native_view, DocumentSessionState::Revoked)
        {
            Self::revoke_state_execution(&previous);
        }
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
        if let Some(previous) = self.sessions.remove(&native_view) {
            Self::revoke_state_execution(&previous);
        }
    }

    fn revoke_state_execution(
        state: &DocumentSessionState<Intent, Navigation, Generation, Target, Material>,
    ) {
        match state {
            DocumentSessionState::Reserved { material, .. }
            | DocumentSessionState::BootstrapPending { material, .. }
            | DocumentSessionState::Active { material, .. } => material.revoke_execution(),
            DocumentSessionState::Revoked => {}
        }
    }

    fn material_for(&self, native_view: NativeView) -> Option<Material> {
        self.sessions
            .get(&native_view)
            .and_then(Self::material_from_state)
    }

    fn material_from_state(
        state: &DocumentSessionState<Intent, Navigation, Generation, Target, Material>,
    ) -> Option<Material> {
        match state {
            DocumentSessionState::Reserved { material, .. }
            | DocumentSessionState::BootstrapPending { material, .. }
            | DocumentSessionState::Active { material, .. } => Some(material.clone()),
            DocumentSessionState::Revoked => None,
        }
    }

    fn admit(
        &mut self,
        native_view: NativeView,
        tab_session_id: u64,
        create_token: u64,
        navigation_id: Navigation,
        generation: Generation,
        intent: Intent,
    ) -> Option<PendingSessionLease<Intent, Navigation, Generation, Target, Material>> {
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
        let pending = PendingSessionLease {
            tab_session_id,
            create_token,
            target: target.clone(),
            intent,
            navigation_id,
            generation,
            material: material.clone(),
        };
        *state = DocumentSessionState::BootstrapPending {
            tab_session_id,
            create_token,
            target,
            intent,
            navigation_id,
            generation,
            material,
        };
        Some(pending)
    }

    fn activate_hello(
        &mut self,
        native_view: NativeView,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveSessionLease<Intent, Navigation, Generation, Target, Material>> {
        // Retried hello for the exact same binding is idempotent. It must not
        // mint a second lease or alter the current session.
        if let Some(DocumentSessionState::Active {
            tab_session_id,
            create_token,
            target,
            intent,
            navigation_id,
            generation,
            material,
            public_session_id,
            lease_token,
        }) = self.sessions.get(&native_view)
        {
            return (public_session_id == session_id && material.matches(session_id, secret)).then(
                || ActiveSessionLease {
                    tab_session_id: *tab_session_id,
                    create_token: *create_token,
                    target: target.clone(),
                    intent: *intent,
                    navigation_id: *navigation_id,
                    generation: *generation,
                    public_session_id: public_session_id.clone(),
                    lease_token: *lease_token,
                    material: material.clone(),
                },
            );
        }
        // A skipped token after an invalid hello is harmless; reusing one is
        // not. Allocate before borrowing the state entry.
        let issued_lease_token = self.next_lease_token;
        self.next_lease_token = self.next_lease_token.checked_add(1)?;
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
            intent,
            navigation_id,
            generation,
            public_session_id: session_id.to_owned(),
            lease_token: issued_lease_token,
            material: material.clone(),
        };
        *state = DocumentSessionState::Active {
            tab_session_id,
            create_token,
            target,
            intent,
            navigation_id,
            generation,
            material,
            public_session_id: session_id.to_owned(),
            lease_token: issued_lease_token,
        };
        Some(lease)
    }

    fn authenticate_frame(
        &self,
        native_view: NativeView,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveSessionLease<Intent, Navigation, Generation, Target, Material>> {
        let DocumentSessionState::Active {
            tab_session_id,
            create_token,
            target,
            intent,
            navigation_id,
            generation,
            material,
            public_session_id,
            lease_token,
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
                intent: *intent,
                navigation_id: *navigation_id,
                generation: *generation,
                public_session_id: public_session_id.clone(),
                lease_token: *lease_token,
                material: material.clone(),
            })
    }

    #[allow(dead_code)] // Retained for exact-lease registry tests.
    fn with_active_lease(
        &self,
        native_view: NativeView,
        lease: &ActiveSessionLease<Intent, Navigation, Generation, Target, Material>,
        action: &mut dyn FnMut(),
    ) -> bool {
        let Some(DocumentSessionState::Active {
            tab_session_id,
            create_token,
            intent,
            navigation_id,
            generation,
            public_session_id,
            lease_token,
            ..
        }) = self.sessions.get(&native_view)
        else {
            return false;
        };
        if *tab_session_id != lease.tab_session_id
            || *create_token != lease.create_token
            || *intent != lease.intent
            || *navigation_id != lease.navigation_id
            || *generation != lease.generation
            || *public_session_id != lease.public_session_id
            || *lease_token != lease.lease_token
        {
            return false;
        }
        action();
        true
    }

    #[cfg(test)]
    fn state(
        &self,
        native_view: NativeView,
    ) -> Option<&DocumentSessionState<Intent, Navigation, Generation, Target, Material>> {
        self.sessions.get(&native_view)
    }
}

/// Browser wrapper over opaque core identities and the registry linearization
/// point shared by bootstrap, ingress authorization, revoke, and outbound.
pub(crate) struct BrowserDocumentSessions {
    inner: SharedBrowserSessionRegistry,
}

impl Default for BrowserDocumentSessions {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(DocumentSessionRegistry::default())),
        }
    }
}

impl BrowserDocumentSessions {
    fn validate_context(
        context: &WebMessageContext,
        native_view: NativeWebViewId,
        generation: DocumentGeneration,
    ) -> Result<(), BrowserInboundRejectReason> {
        Self::validate_context_evidence(
            context.native_view(),
            context.document(),
            context.frame(),
            context.transport(),
            native_view,
            generation,
        )
    }

    fn validate_context_evidence(
        actual_native_view: NativeWebViewId,
        document: DocumentBinding,
        frame: WebMessageFrame,
        transport: WebMessageTransport,
        expected_native_view: NativeWebViewId,
        expected_generation: DocumentGeneration,
    ) -> Result<(), BrowserInboundRejectReason> {
        if actual_native_view != expected_native_view {
            return Err(BrowserInboundRejectReason::WrongNativeView);
        }
        match transport {
            WebMessageTransport::AndroidJavascriptInterface => {
                return Err(BrowserInboundRejectReason::AndroidLegacyDegraded);
            }
            WebMessageTransport::AppleScriptMessage
            | WebMessageTransport::AndroidMessagePort
            | WebMessageTransport::WindowsWebMessage => {}
            WebMessageTransport::HarmonyMessagePort | WebMessageTransport::Other => {
                return Err(BrowserInboundRejectReason::UnsupportedTransport);
            }
        }
        match frame {
            WebMessageFrame::TopLevel => {}
            WebMessageFrame::Subframe => return Err(BrowserInboundRejectReason::ChildFrame),
            WebMessageFrame::Unproven => {
                return Err(BrowserInboundRejectReason::UnprovenFrame);
            }
        }
        if document != DocumentBinding::Bound(expected_generation) {
            return Err(BrowserInboundRejectReason::StaleGeneration);
        }
        Ok(())
    }

    fn validate_binding(
        material: &impl SessionMaterial,
        session_id: &str,
        secret: &str,
    ) -> Result<(), BrowserInboundRejectReason> {
        material
            .matches(session_id, secret)
            .then_some(())
            .ok_or(BrowserInboundRejectReason::WrongBinding)
    }
    pub(crate) fn prepare(
        &self,
        native_view: NativeWebViewId,
        tab_session_id: u64,
        create_token: u64,
        target: InternalPageTarget,
        intent: TrustedLoadIntent,
    ) -> Result<
        (ControlDocumentBootstrap, Option<ControlDocumentAuthority>),
        ControlDocumentBootstrapPrepareError,
    > {
        let (bootstrap, material) = issue_control_document_bootstrap(&SystemRandom::new())
            .map_err(|_| ControlDocumentBootstrapPrepareError::EntropyUnavailable)?;
        let mut sessions = self
            .inner
            .lock()
            .map_err(|_| ControlDocumentBootstrapPrepareError::EntropyUnavailable)?;
        let replaced = sessions.prepare(
            native_view,
            tab_session_id,
            create_token,
            target,
            intent,
            material,
        );
        Ok((bootstrap, replaced))
    }

    /// Returns the trusted-start result and, on a competing start, the exact
    /// authority whose gate was revoked under this registry lock. Callers may
    /// cancel its PageBridge work after releasing the lock.
    pub(crate) fn navigation_started_with_revocation(
        &self,
        native_view: NativeWebViewId,
        navigation_id: NavigationId,
    ) -> (bool, Option<ControlDocumentAuthority>) {
        let Ok(mut sessions) = self.inner.lock() else {
            return (false, None);
        };
        let authority = sessions.material_for(native_view);
        let trusted = sessions.navigation_started(native_view, navigation_id);
        (trusted, (!trusted).then_some(authority).flatten())
    }

    /// Revoke only the exact failed/cancelled navigation, never a successor.
    /// PageBridge cancellation remains deliberately outside the registry lock.
    pub(crate) fn revoke_navigation_if_matches(
        &self,
        native_view: NativeWebViewId,
        navigation_id: NavigationId,
    ) -> Option<ControlDocumentAuthority> {
        let Ok(mut sessions) = self.inner.lock() else {
            return None;
        };
        let authority = sessions.material_for(native_view)?;
        sessions
            .revoke_navigation_if_matches(native_view, navigation_id)
            .then_some(authority)
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

    pub(crate) fn destroy_native_view(
        &self,
        native_view: NativeWebViewId,
    ) -> Option<ControlDocumentAuthority> {
        if let Ok(mut sessions) = self.inner.lock() {
            let authority = sessions.material_for(native_view);
            sessions.remove(native_view);
            authority
        } else {
            None
        }
    }

    pub(crate) fn admit(
        &self,
        tab_session_id: u64,
        create_token: u64,
        admission: TrustedDocumentAdmission,
    ) -> Option<PendingDocumentLease> {
        let pending = self.inner.lock().ok()?.admit(
            admission.native_view(),
            tab_session_id,
            create_token,
            admission.navigation_id(),
            admission.generation(),
            admission.intent(),
        )?;
        Some(PendingDocumentLease {
            registry: Arc::downgrade(&self.inner),
            native_view: admission.native_view(),
            tab_session_id: pending.tab_session_id,
            create_token: pending.create_token,
            target: pending.target,
            intent: pending.intent,
            navigation_id: pending.navigation_id,
            generation: pending.generation,
            authority: pending.material,
        })
    }

    /// Linearize the first V3 hello with the document registry. `bind` runs
    /// only for the exact current BootstrapPending session; `handle` runs
    /// only after that same session has atomically become Active.  Keeping
    /// this mutex held across both callbacks prevents a navigation revoke
    /// from landing between browser admission and PageBridge dispatch.
    pub(crate) fn with_activated_hello_for_context<T>(
        &self,
        context: &WebMessageContext,
        session_id: &str,
        secret: &str,
        mut bind: impl FnMut(ControlDocumentAuthority, Arc<dyn DocumentOutboundGate>) -> bool,
        prepare: impl FnOnce(ControlDocumentAuthority) -> Option<T>,
    ) -> Result<Option<T>, BrowserInboundRejectReason> {
        let native_view = context.native_view();
        let mut sessions = self
            .inner
            .lock()
            .map_err(|_| BrowserInboundRejectReason::SessionNotReady)?;

        let pending = match sessions.sessions.get(&native_view) {
            Some(DocumentSessionState::BootstrapPending {
                tab_session_id,
                create_token,
                target,
                intent,
                navigation_id,
                generation,
                material,
            }) => {
                Self::validate_context(context, native_view, *generation)?;
                Some(PendingDocumentLease {
                    registry: Arc::downgrade(&self.inner),
                    native_view,
                    tab_session_id: *tab_session_id,
                    create_token: *create_token,
                    target: target.clone(),
                    intent: *intent,
                    navigation_id: *navigation_id,
                    generation: *generation,
                    authority: material.clone(),
                })
            }
            Some(DocumentSessionState::Active {
                generation,
                material,
                ..
            }) => {
                Self::validate_context(context, native_view, *generation)?;
                Self::validate_binding(material, session_id, secret)?;
                None
            }
            _ => return Err(BrowserInboundRejectReason::SessionNotReady),
        };

        if let Some(pending) = pending {
            // Authenticate before installing a RequiredV3 protocol. An
            // attacker cannot use a wrong hello to replace a live bridge
            // connection or poison BootstrapPending with a foreign binding.
            Self::validate_binding(&pending.authority, session_id, secret)?;
            let gate: Arc<dyn DocumentOutboundGate> = Arc::new(pending.clone());
            if !bind(pending.authority(), gate) {
                return Err(BrowserInboundRejectReason::SessionNotReady);
            }
        }

        let Some(active) = sessions.activate_hello(native_view, session_id, secret) else {
            return Err(BrowserInboundRejectReason::SessionNotReady);
        };
        Ok(prepare(active.material))
    }

    /// Runs a non-hello V3 frame while its exact document remains Active.
    /// The frame cannot be decoded, allocated, or dispatched after a revoke
    /// slips in because the registry lock spans the supplied callback.
    pub(crate) fn with_active_frame_for_context<T>(
        &self,
        context: &WebMessageContext,
        session_id: &str,
        secret: &str,
        prepare: impl FnOnce(ControlDocumentAuthority) -> Option<T>,
    ) -> Result<Option<T>, BrowserInboundRejectReason> {
        self.with_active_bound_context(context, session_id, secret, prepare)
    }

    pub(crate) fn with_active_bound_context<T>(
        &self,
        context: &WebMessageContext,
        session_id: &str,
        secret: &str,
        action: impl FnOnce(ControlDocumentAuthority) -> Option<T>,
    ) -> Result<Option<T>, BrowserInboundRejectReason> {
        let native_view = context.native_view();
        let sessions = self
            .inner
            .lock()
            .map_err(|_| BrowserInboundRejectReason::SessionNotReady)?;
        let Some(DocumentSessionState::Active {
            generation,
            material,
            ..
        }) = sessions.sessions.get(&native_view)
        else {
            return Err(BrowserInboundRejectReason::SessionNotReady);
        };
        Self::validate_context(context, native_view, *generation)?;
        Self::validate_binding(material, session_id, secret)?;
        Ok(action(material.clone()))
    }

    /// Exact-lease helper retained for registry tests and diagnostics.
    #[allow(dead_code)]
    pub(crate) fn activate_hello(
        &self,
        native_view: NativeWebViewId,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveDocumentLease> {
        let lease = self
            .inner
            .lock()
            .ok()?
            .activate_hello(native_view, session_id, secret)?;
        Some(ActiveDocumentLease {
            registry: Arc::downgrade(&self.inner),
            native_view,
            tab_session_id: lease.tab_session_id,
            create_token: lease.create_token,
            target: lease.target,
            intent: lease.intent,
            navigation_id: lease.navigation_id,
            generation: lease.generation,
            public_session_id: lease.public_session_id,
            lease_token: lease.lease_token,
            authority: lease.material,
        })
    }

    /// Exact-lease helper retained for registry tests and diagnostics.
    #[allow(dead_code)]
    pub(crate) fn authenticate_frame(
        &self,
        native_view: NativeWebViewId,
        session_id: &str,
        secret: &str,
    ) -> Option<ActiveDocumentLease> {
        let lease = self
            .inner
            .lock()
            .ok()?
            .authenticate_frame(native_view, session_id, secret)?;
        Some(ActiveDocumentLease {
            registry: Arc::downgrade(&self.inner),
            native_view,
            tab_session_id: lease.tab_session_id,
            create_token: lease.create_token,
            target: lease.target,
            intent: lease.intent,
            navigation_id: lease.navigation_id,
            generation: lease.generation,
            public_session_id: lease.public_session_id,
            lease_token: lease.lease_token,
            authority: lease.material,
        })
    }
}

impl DocumentOutboundGate for PendingDocumentLease {
    fn with_active(&self, action: &mut dyn FnMut()) -> bool {
        let Some(registry) = self.registry.upgrade() else {
            return false;
        };
        let Ok(registry) = registry.lock() else {
            return false;
        };
        let Some(DocumentSessionState::Active {
            tab_session_id,
            create_token,
            intent,
            navigation_id,
            generation,
            ..
        }) = registry.sessions.get(&self.native_view)
        else {
            return false;
        };
        if *tab_session_id != self.tab_session_id
            || *create_token != self.create_token
            || *intent != self.intent
            || *navigation_id != self.navigation_id
            || *generation != self.generation
        {
            return false;
        }
        action();
        true
    }
}

impl lxapp::RequiredV3DocumentGate for PendingDocumentLease {
    fn with_bootstrap_pending_current(
        &self,
        context: &WebMessageContext,
        action: &mut dyn FnMut(ControlDocumentAuthority, Arc<dyn DocumentOutboundGate>),
    ) -> bool {
        PendingDocumentLease::with_bootstrap_pending_current(self, context, action)
    }
}

impl DocumentOutboundGate for ActiveDocumentLease {
    fn with_active(&self, action: &mut dyn FnMut()) -> bool {
        let Some(registry) = self.registry.upgrade() else {
            return false;
        };
        let Ok(registry) = registry.lock() else {
            return false;
        };
        registry.with_active_lease(
            self.native_view,
            &ActiveSessionLease {
                tab_session_id: self.tab_session_id,
                create_token: self.create_token,
                target: self.target.clone(),
                intent: self.intent,
                navigation_id: self.navigation_id,
                generation: self.generation,
                public_session_id: self.public_session_id.clone(),
                lease_token: self.lease_token,
                material: self.authority.clone(),
            },
            action,
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct TestMaterial(&'static str, &'static str);
    impl SessionMaterial for TestMaterial {
        fn matches(&self, session_id: &str, secret: &str) -> bool {
            self.0 == session_id && self.1 == secret
        }

        fn revoke_execution(&self) {}
    }

    #[derive(Clone)]
    struct RevocationMaterial(Arc<AtomicUsize>);
    impl SessionMaterial for RevocationMaterial {
        fn matches(&self, _: &str, _: &str) -> bool {
            false
        }

        fn revoke_execution(&self) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }
    }
    type TestRegistry = DocumentSessionRegistry<u64, u64, u64, u64, &'static str, TestMaterial>;

    fn prepare(sessions: &mut TestRegistry, native: u64, intent: u64) {
        let _ = sessions.prepare(
            native,
            1,
            2,
            "settings",
            intent,
            TestMaterial("session-id-012345", "secret"),
        );
    }

    #[test]
    fn required_v3_rejection_reasons_distinguish_stale_frame_and_android_degradation() {
        let native = NativeWebViewId::for_test(10);
        let generation = DocumentGeneration::for_test(3);
        let validate = |document, frame, transport| {
            BrowserDocumentSessions::validate_context_evidence(
                native, document, frame, transport, native, generation,
            )
        };
        assert!(
            validate(
                DocumentBinding::Bound(generation),
                WebMessageFrame::TopLevel,
                WebMessageTransport::WindowsWebMessage,
            )
            .is_ok()
        );
        assert!(
            validate(
                DocumentBinding::Bound(generation),
                WebMessageFrame::TopLevel,
                WebMessageTransport::AndroidMessagePort,
            )
            .is_ok()
        );
        assert_eq!(
            validate(
                DocumentBinding::Bound(DocumentGeneration::for_test(2)),
                WebMessageFrame::TopLevel,
                WebMessageTransport::WindowsWebMessage,
            ),
            Err(BrowserInboundRejectReason::StaleGeneration)
        );
        assert_eq!(
            validate(
                DocumentBinding::Bound(generation),
                WebMessageFrame::Subframe,
                WebMessageTransport::WindowsWebMessage,
            ),
            Err(BrowserInboundRejectReason::ChildFrame)
        );
        assert_eq!(
            validate(
                DocumentBinding::Unbound,
                WebMessageFrame::Unproven,
                WebMessageTransport::AndroidJavascriptInterface,
            ),
            Err(BrowserInboundRejectReason::AndroidLegacyDegraded)
        );
        assert_eq!(
            BrowserInboundRejectReason::AndroidLegacyDegraded.as_str(),
            "android_21_22_unproven_transport"
        );
    }

    #[test]
    fn required_v3_binding_mismatch_has_a_stable_reason() {
        let material = TestMaterial("expected-session", "expected-secret");
        assert!(
            BrowserDocumentSessions::validate_binding(
                &material,
                "expected-session",
                "expected-secret"
            )
            .is_ok()
        );
        assert_eq!(
            BrowserDocumentSessions::validate_binding(
                &material,
                "expected-session",
                "wrong-secret"
            ),
            Err(BrowserInboundRejectReason::WrongBinding)
        );
    }

    #[test]
    fn replacement_revokes_the_old_execution_domain_before_reserving_successor() {
        type Registry =
            DocumentSessionRegistry<u64, u64, u64, u64, &'static str, RevocationMaterial>;
        let mut sessions = Registry::default();
        let old_revocations = Arc::new(AtomicUsize::new(0));
        let new_revocations = Arc::new(AtomicUsize::new(0));
        assert!(
            sessions
                .prepare(
                    10,
                    1,
                    2,
                    "settings",
                    7,
                    RevocationMaterial(Arc::clone(&old_revocations)),
                )
                .is_none()
        );
        assert!(
            sessions
                .prepare(
                    10,
                    1,
                    3,
                    "settings",
                    8,
                    RevocationMaterial(Arc::clone(&new_revocations)),
                )
                .is_some()
        );
        assert_eq!(old_revocations.load(Ordering::Acquire), 1);
        assert_eq!(new_revocations.load(Ordering::Acquire), 0);
    }

    #[test]
    fn admission_stops_at_bootstrap_pending_until_hello_activates_once() {
        let mut sessions = TestRegistry::default();
        prepare(&mut sessions, 10, 7);
        sessions.navigation_started(10, 4);
        assert_eq!(
            sessions.admit(10, 1, 2, 4, 5, 7).map(|lease| lease.target),
            Some("settings")
        );
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
                .is_some()
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
        assert_eq!(
            sessions.admit(10, 1, 2, 4, 5, 8).map(|lease| lease.target),
            Some("settings")
        );

        sessions.remove(10);
        prepare(&mut sessions, 11, 1);
        assert!(sessions.admit(10, 1, 2, 4, 5, 8).is_none());
        assert!(sessions.navigation_started(11, 6));
        assert_eq!(
            sessions.admit(11, 1, 2, 6, 7, 1).map(|lease| lease.target),
            Some("settings")
        );
    }

    #[test]
    fn active_lease_executes_once_and_revocation_drops_a_queued_post() {
        let mut sessions = TestRegistry::default();
        prepare(&mut sessions, 10, 7);
        assert!(sessions.navigation_started(10, 4));
        assert!(sessions.admit(10, 1, 2, 4, 5, 7).is_some());
        let lease = sessions
            .activate_hello(10, "session-id-012345", "secret")
            .expect("valid hello activates one lease");

        let mut delivered = 0;
        assert!(sessions.with_active_lease(10, &lease, &mut || delivered += 1));
        assert_eq!(delivered, 1);

        // This models a post that was queued for a UI thread, then revoked by
        // navigation before the UI action enters the registry gate.
        sessions.force_revoke(10);
        assert!(!sessions.with_active_lease(10, &lease, &mut || delivered += 1));
        assert_eq!(delivered, 1);
    }

    #[test]
    fn recreated_or_generation_mismatched_record_cannot_use_old_lease() {
        let mut sessions = TestRegistry::default();
        prepare(&mut sessions, 10, 7);
        assert!(sessions.navigation_started(10, 4));
        assert!(sessions.admit(10, 1, 2, 4, 5, 7).is_some());
        let old_lease = sessions
            .activate_hello(10, "session-id-012345", "secret")
            .expect("valid hello activates one lease");

        // A replacement under the same logical slot gets a new intent and
        // generation. Neither a tag-style lookup nor a stale lease may cross
        // that boundary.
        prepare(&mut sessions, 10, 8);
        assert!(sessions.navigation_started(10, 6));
        assert!(sessions.admit(10, 1, 2, 6, 9, 8).is_some());
        assert!(
            sessions
                .activate_hello(10, "session-id-012345", "secret")
                .is_some()
        );

        let mut delivered = 0;
        assert!(!sessions.with_active_lease(10, &old_lease, &mut || delivered += 1));
        assert_eq!(delivered, 0);
    }
}
