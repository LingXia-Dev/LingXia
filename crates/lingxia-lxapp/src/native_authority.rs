//! Opaque native-host proof for control-plane bootstrap operations.

use crate::terminal_automation::NativeHostRuntimeToken;
use lingxia_platform::Platform;
use std::sync::Weak;

/// Proof that a control-plane operation originates from the native host that
/// bootstrapped the currently active runtime.
///
/// The type has no public constructor, clone implementation, serializer, or
/// process-global getter. Runtime internals receive it explicitly from the
/// host bootstrap path.
///
/// ```compile_fail
/// use lxapp::NativeControlPlaneAuthority;
/// let _ = NativeControlPlaneAuthority {};
/// ```
pub struct NativeControlPlaneAuthority {
    runtime: Weak<Platform>,
    #[cfg(any(test, feature = "test-utils"))]
    test_authority: bool,
}

impl NativeControlPlaneAuthority {
    pub(crate) fn for_native_runtime(proof: &NativeHostRuntimeToken) -> Self {
        Self {
            runtime: proof.runtime().clone(),
            #[cfg(any(test, feature = "test-utils"))]
            test_authority: false,
        }
    }

    pub(crate) fn validate(&self) -> bool {
        #[cfg(any(test, feature = "test-utils"))]
        if self.test_authority {
            return true;
        }

        let Some(runtime) = self.runtime.upgrade() else {
            return false;
        };
        crate::get_platform().is_some_and(|current| std::sync::Arc::ptr_eq(&current, &runtime))
    }

    /// Whether this opaque proof still belongs to the live platform runtime.
    /// This reveals no credential and cannot mint or promote authority.
    #[doc(hidden)]
    pub fn is_live(&self) -> bool {
        self.validate()
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::for_test_harness()
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) fn for_test_harness() -> Self {
        Self {
            runtime: Weak::new(),
            test_authority: true,
        }
    }
}
