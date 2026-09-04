//! Native-host terminal automation facade.

use std::sync::OnceLock;

use lxapp::terminal_automation::{NativeHostRuntimeToken, TerminalAutomationAuthority};

struct NativeAuthoritySlot(OnceLock<TerminalAutomationAuthority>);

impl NativeAuthoritySlot {
    const fn new() -> Self {
        Self(OnceLock::new())
    }

    fn install(&self, proof: &NativeHostRuntimeToken) -> bool {
        self.0
            .set(TerminalAutomationAuthority::for_native_runtime(proof))
            .is_ok()
    }

    fn with<T>(&self, fallback: T, action: impl FnOnce(&TerminalAutomationAuthority) -> T) -> T {
        self.0.get().map(action).unwrap_or(fallback)
    }

    #[cfg(test)]
    fn authority(&self) -> Option<&TerminalAutomationAuthority> {
        self.0.get()
    }
}

static AUTHORITY: NativeAuthoritySlot = NativeAuthoritySlot::new();

pub(crate) fn install(proof: &NativeHostRuntimeToken) -> bool {
    AUTHORITY.install(proof)
}

fn with_authority<T>(fallback: T, action: impl FnOnce(&TerminalAutomationAuthority) -> T) -> T {
    AUTHORITY.with(fallback, action)
}

pub(crate) fn publish_snapshot(surface_id: &str, snapshot_json: &str) -> bool {
    with_authority(false, |authority| {
        lxapp::terminal_automation::publish_snapshot(authority, surface_id, snapshot_json).is_ok()
    })
}

pub(crate) fn remove_workspace(surface_id: &str) {
    with_authority((), |authority| {
        lxapp::terminal_automation::remove_workspace(authority, surface_id)
    });
}

pub(crate) fn take_command(surface_id: &str) -> String {
    with_authority(String::new(), |authority| {
        lxapp::terminal_automation::take_command(authority, surface_id)
    })
}

pub(crate) fn complete_command(id: u64, ok: bool, payload: &str) -> bool {
    with_authority(false, |authority| {
        lxapp::terminal_automation::complete_command(authority, id, ok, payload)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_platform::Platform;
    use std::sync::Arc;

    fn runtime(root: &std::path::Path) -> Arc<Platform> {
        Arc::new(
            Platform::new(
                root.join("data").display().to_string(),
                root.join("cache").display().to_string(),
                "en-US".to_string(),
            )
            .expect("test platform"),
        )
    }

    #[test]
    fn native_token_installation_is_single_owner_and_expires_with_its_runtime() {
        let first_root = tempfile::tempdir().expect("first runtime root");
        let successor_root = tempfile::tempdir().expect("successor runtime root");
        let first = runtime(first_root.path());
        let successor = runtime(successor_root.path());
        let first_token = NativeHostRuntimeToken::for_test(&first);
        let successor_token = NativeHostRuntimeToken::for_test(&successor);
        let slot = NativeAuthoritySlot::new();

        assert!(slot.install(&first_token));
        assert!(!slot.install(&successor_token));
        let installed = slot.authority().expect("installed authority");
        assert!(
            installed
                .validate_native_runtime_for_test(Some(&first))
                .is_ok()
        );
        assert!(
            installed
                .validate_native_runtime_for_test(Some(&successor))
                .expect_err("successor cannot reuse the first runtime token")
                .contains("does not match")
        );

        drop(first);
        assert!(
            installed
                .validate_native_runtime_for_test(None)
                .expect_err("dropped runtime revokes its token")
                .contains("no longer live")
        );
    }
}
