//! Native-only bootstrap material for one trusted browser control document.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{hmac, rand::SecureRandom};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::watch;

/// One-shot V3 bootstrap embedded into a native-owned control document.
/// It is opaque, non-cloneable, and consumed by the content generator.
pub struct ControlDocumentBootstrap {
    session_id: String,
    secret: String,
}

struct ControlDocumentAuthorityMaterial {
    session_id: String,
    secret: String,
    execution_gate: RequiredV3ExecutionGate,
}

/// Opaque liveness gate captured by a prepared RequiredV3 frame.
///
/// Only the matching native browser session can revoke it. It has no public
/// constructor, identity accessor, or way to reactivate a revoked gate.
#[doc(hidden)]
#[derive(Clone)]
pub struct RequiredV3ExecutionGate(Arc<Mutex<RequiredV3ExecutionGateState>>);

struct RequiredV3ExecutionGateState {
    revoked: bool,
    permits: Vec<Weak<RequiredV3ExecutionPermitState>>,
}

struct RequiredV3ExecutionPermitState {
    active: AtomicBool,
    effect_committed: AtomicBool,
    cancelled_tx: watch::Sender<bool>,
}

/// A registered execution slot for exactly one synchronous ingress dispatch.
/// Revoke invalidates every outstanding permit in the same gate domain.
#[doc(hidden)]
#[derive(Clone)]
pub struct RequiredV3ExecutionPermit(Arc<RequiredV3ExecutionPermitState>);

/// Host-TCB proof for exactly one control-document bootstrap binding.
///
/// It deliberately offers no credential getter, serialization, display, or
/// external constructor. Clones retain the same native-held binding so the
/// browser lifecycle registry and PageBridge can prove one session without
/// copying the secret into browser-owned values.
#[doc(hidden)]
#[derive(Clone)]
pub struct ControlDocumentAuthority(Arc<ControlDocumentAuthorityMaterial>);

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum ControlDocumentBootstrapError {
    #[error("secure random generation for control document bootstrap failed")]
    EntropyUnavailable,
}

/// Issue a bootstrap plus matching native-only authentication material.
/// Browser code supplies `SystemRandom`; failure leaves no material to retain.
pub fn issue_control_document_bootstrap(
    rng: &dyn SecureRandom,
) -> Result<(ControlDocumentBootstrap, ControlDocumentAuthority), ControlDocumentBootstrapError> {
    let mut public_session_id = [0_u8; 16];
    let mut secret = [0_u8; 32];
    rng.fill(&mut public_session_id)
        .map_err(|_| ControlDocumentBootstrapError::EntropyUnavailable)?;
    rng.fill(&mut secret)
        .map_err(|_| ControlDocumentBootstrapError::EntropyUnavailable)?;
    let session_id = URL_SAFE_NO_PAD.encode(public_session_id);
    let secret = URL_SAFE_NO_PAD.encode(secret);
    Ok((
        ControlDocumentBootstrap {
            session_id: session_id.clone(),
            secret: secret.clone(),
        },
        ControlDocumentAuthority(Arc::new(ControlDocumentAuthorityMaterial {
            session_id,
            secret,
            execution_gate: RequiredV3ExecutionGate(Arc::new(Mutex::new(
                RequiredV3ExecutionGateState {
                    revoked: false,
                    permits: Vec::new(),
                },
            ))),
        })),
    ))
}

impl ControlDocumentBootstrap {
    pub(crate) fn take_binding(self) -> (String, String) {
        (self.session_id, self.secret)
    }
}

impl ControlDocumentAuthority {
    /// Compare both fields without exposing either stored credential.
    pub fn matches(&self, session_id: &str, secret: &str) -> bool {
        // HMAC verification is ring's maintained constant-time equality
        // primitive. The domain key is not a credential; it only prevents an
        // optimizing compiler from reducing this comparison to early-exit
        // byte equality.
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"lingxia-control-document-binding-v3");
        let session_tag = hmac::sign(&key, self.0.session_id.as_bytes());
        let secret_tag = hmac::sign(&key, self.0.secret.as_bytes());
        let session_matches =
            hmac::verify(&key, session_id.as_bytes(), session_tag.as_ref()).is_ok();
        let secret_matches = hmac::verify(&key, secret.as_bytes(), secret_tag.as_ref()).is_ok();
        session_matches & secret_matches
    }

    /// Capture the session's revocable execution liveness without exposing
    /// its credential or mutable state.
    #[doc(hidden)]
    pub fn execution_gate(&self) -> RequiredV3ExecutionGate {
        self.0.execution_gate.clone()
    }

    /// Browser lifecycle invalidates every prepared frame for this exact
    /// authority while it still holds the document-session registry lock.
    #[doc(hidden)]
    pub fn revoke_execution(&self) {
        self.0.execution_gate.revoke();
    }

    /// Bridge-only construction of the binding verifier. Browser code can
    /// authenticate candidates but cannot manufacture a V3 protocol.
    #[allow(dead_code)] // Consumed by PageBridge RequiredV3 binding.
    pub(crate) fn v3_inbound_binding(&self) -> crate::bridge::V3InboundBinding {
        crate::bridge::V3InboundBinding::new(self.0.session_id.clone(), self.0.secret.clone())
            .expect("native-generated control document binding must be valid")
    }
}

impl RequiredV3ExecutionGate {
    /// Register a dispatch permit in the same domain that lifecycle revocation
    /// closes. If revocation wins first, no permit is issued; if this wins,
    /// revocation invalidates the returned permit before its next effect.
    #[doc(hidden)]
    pub fn try_begin(&self) -> Option<RequiredV3ExecutionPermit> {
        let Ok(mut state) = self.0.lock() else {
            return None;
        };
        if state.revoked {
            return None;
        }
        state.permits.retain(|permit| permit.strong_count() != 0);
        let permit = Arc::new(RequiredV3ExecutionPermitState {
            active: AtomicBool::new(true),
            effect_committed: AtomicBool::new(false),
            cancelled_tx: watch::channel(false).0,
        });
        state.permits.push(Arc::downgrade(&permit));
        Some(RequiredV3ExecutionPermit(permit))
    }

    fn revoke(&self) {
        let Ok(mut state) = self.0.lock() else {
            return;
        };
        state.revoked = true;
        for permit in state
            .permits
            .drain(..)
            .filter_map(|permit| permit.upgrade())
        {
            permit.active.store(false, Ordering::Release);
            permit.cancelled_tx.send_replace(true);
        }
    }
}

impl RequiredV3ExecutionPermit {
    /// Call before the first poll and every effect that can outlive ingress.
    #[doc(hidden)]
    pub fn is_active(&self) -> bool {
        self.0.active.load(Ordering::Acquire)
    }

    /// Linearize the first route effect/handler start against revoke. A
    /// permit commits at most once; a caller that loses either the revoke or
    /// another effect-start race must drop its prepared ingress without work.
    #[doc(hidden)]
    pub fn try_commit_effect(&self) -> bool {
        self.is_active()
            && self
                .0
                .effect_committed
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            && self.is_active()
    }

    /// Subscribe before spawning work derived from this ingress. Revocation
    /// wakes the receiver even when that work is awaiting a host future.
    #[doc(hidden)]
    pub fn cancellation_receiver(&self) -> watch::Receiver<bool> {
        self.0.cancelled_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;

    #[test]
    fn issued_material_is_fresh_and_authenticates_only_its_tuple() {
        let (first_bootstrap, first) =
            issue_control_document_bootstrap(&SystemRandom::new()).unwrap();
        let (_second_bootstrap, second) =
            issue_control_document_bootstrap(&SystemRandom::new()).unwrap();
        let (session_id, secret) = first_bootstrap.take_binding();
        assert!(first.matches(&session_id, &secret));
        assert!(!second.matches(&session_id, &secret));
        assert!(!first.matches(&session_id, "wrong"));
        assert!(!first.matches("wrong", &secret));
        let gate = first.execution_gate();
        let permit = gate.try_begin().expect("fresh gate should issue permit");
        assert!(permit.is_active());
        assert!(permit.try_commit_effect());
        assert!(!permit.try_commit_effect());
        first.revoke_execution();
        assert!(!permit.is_active());
        assert!(gate.try_begin().is_none());
    }
}
