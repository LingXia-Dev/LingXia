//! Native-only bootstrap material for one trusted browser control document.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{hmac, rand::SecureRandom};

/// One-shot V3 bootstrap embedded into a native-owned control document.
/// It is opaque, non-cloneable, and consumed by the content generator.
pub struct ControlDocumentBootstrap {
    session_id: String,
    secret: String,
}

/// Native registry material for one [`ControlDocumentBootstrap`]. It can
/// authenticate candidates without exposing the stored secret.
pub struct ControlDocumentSessionMaterial {
    session_id: String,
    secret: String,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum ControlDocumentBootstrapError {
    #[error("secure random generation for control document bootstrap failed")]
    EntropyUnavailable,
}

/// Issue a bootstrap plus matching native-only authentication material.
/// Browser code supplies `SystemRandom`; failure leaves no material to retain.
pub fn issue_control_document_bootstrap(
    rng: &dyn SecureRandom,
) -> Result<(ControlDocumentBootstrap, ControlDocumentSessionMaterial), ControlDocumentBootstrapError>
{
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
        ControlDocumentSessionMaterial { session_id, secret },
    ))
}

impl ControlDocumentBootstrap {
    pub(crate) fn take_binding(self) -> (String, String) {
        (self.session_id, self.secret)
    }
}

impl ControlDocumentSessionMaterial {
    /// Compare both fields without exposing either stored credential.
    pub fn matches(&self, session_id: &str, secret: &str) -> bool {
        // HMAC verification is ring's maintained constant-time equality
        // primitive. The domain key is not a credential; it only prevents an
        // optimizing compiler from reducing this comparison to early-exit
        // byte equality.
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"lingxia-control-document-binding-v3");
        let session_tag = hmac::sign(&key, self.session_id.as_bytes());
        let secret_tag = hmac::sign(&key, self.secret.as_bytes());
        let session_matches =
            hmac::verify(&key, session_id.as_bytes(), session_tag.as_ref()).is_ok();
        let secret_matches = hmac::verify(&key, secret.as_bytes(), secret_tag.as_ref()).is_ok();
        session_matches & secret_matches
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
        assert_ne!(first.session_id, second.session_id);
        assert_ne!(first.secret, second.secret);
        let (session_id, secret) = first_bootstrap.take_binding();
        assert!(first.matches(&session_id, &secret));
        assert!(!first.matches(&session_id, "wrong"));
        assert!(!first.matches("wrong", &secret));
    }
}
