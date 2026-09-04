//! Private cross-crate test ABI for authority consumers.
//!
//! `test-utils` intentionally exports no safe Rust constructor. Workspace
//! crates bind these symbols only from their own `cfg(test)` modules, so Cargo
//! feature unification cannot turn a production dependency into an authority
//! minting API.

use crate::host::AuthenticatedCaller;
use crate::terminal_automation::TerminalAutomationAuthority;
use crate::{AppSessionClass, NativeControlPlaneAuthority};
use lingxia_platform::Platform;
use std::sync::Arc;

#[unsafe(export_name = "lingxia_lxapp_test_control_authority_v1")]
pub(crate) extern "Rust" fn control_authority() -> NativeControlPlaneAuthority {
    NativeControlPlaneAuthority::for_test_harness()
}

#[unsafe(export_name = "lingxia_lxapp_test_authenticated_caller_v1")]
pub(crate) extern "Rust" fn authenticated_caller(
    app_id: &str,
    session_id: u64,
    class: AppSessionClass,
) -> AuthenticatedCaller {
    AuthenticatedCaller::lxapp_session_for_test(app_id, session_id, class)
}

#[unsafe(export_name = "lingxia_lxapp_test_browser_caller_v1")]
pub(crate) extern "Rust" fn browser_caller() -> AuthenticatedCaller {
    AuthenticatedCaller::browser_document_for_test()
}

#[unsafe(export_name = "lingxia_lxapp_test_terminal_authority_v1")]
pub(crate) extern "Rust" fn terminal_authority(
    runtime: &Arc<Platform>,
) -> TerminalAutomationAuthority {
    TerminalAutomationAuthority::native_runtime_for_test(runtime)
}

#[unsafe(export_name = "lingxia_lxapp_test_validate_terminal_authority_v1")]
pub(crate) extern "Rust" fn validate_terminal_authority(
    authority: &TerminalAutomationAuthority,
    runtime: Option<&Arc<Platform>>,
) -> Result<(), String> {
    authority.validate_native_runtime_for_test(runtime)
}
