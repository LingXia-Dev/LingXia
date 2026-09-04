//! Native-host terminal automation facade.

use std::sync::OnceLock;

use lxapp::terminal_automation::{NativeHostRuntimeToken, TerminalAutomationAuthority};

static AUTHORITY: OnceLock<TerminalAutomationAuthority> = OnceLock::new();

pub(crate) fn install(proof: &NativeHostRuntimeToken) -> bool {
    AUTHORITY
        .set(TerminalAutomationAuthority::for_native_runtime(proof))
        .is_ok()
}

fn with_authority<T>(fallback: T, action: impl FnOnce(&TerminalAutomationAuthority) -> T) -> T {
    AUTHORITY.get().map(action).unwrap_or(fallback)
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
