//! Dev builds: one point every native call passes (Logic, GPU View, WebView bridge) where
//! the lxapp's mocks may answer before the host does.

use super::{
    HostCancel, HostFuture, HostHandler, HostInvocationContext, HostOutput, SyncHostHandler,
};
use std::sync::{Arc, OnceLock};

/// Answers native calls from the lxapp's mocks; the automation runtime registers it.
pub trait NativeMockHook: Send + Sync + 'static {
    /// Answers `route`, or hands the call to `real`. `real` is `None` when no host
    /// registered the route.
    fn call(
        &self,
        route: &str,
        invocation: HostInvocationContext,
        input: Option<String>,
        cancel: HostCancel,
        real: Option<Arc<dyn HostHandler>>,
    ) -> HostFuture<'static>;

    /// A sync route, on the Logic thread whose context issued it.
    fn call_sync(
        &self,
        logic: &rong::JSContext,
        route: &str,
        invocation: HostInvocationContext,
        input: Option<String>,
        real: Option<Arc<dyn SyncHostHandler>>,
    ) -> lingxia_error::Result<HostOutput>;
}

static HOOK: OnceLock<Box<dyn NativeMockHook>> = OnceLock::new();

/// Installs the hook once per process; a second registration is ignored.
#[doc(hidden)]
pub fn register_native_mock_hook(hook: Box<dyn NativeMockHook>) {
    let _ = HOOK.set(hook);
}

pub(super) fn hook() -> Option<&'static dyn NativeMockHook> {
    HOOK.get().map(|hook| hook.as_ref())
}

/// A route as the hook sees it: mocks first, then `real`.
pub(super) struct Mocked {
    pub route: String,
    pub real: Option<Arc<dyn HostHandler>>,
}

impl HostHandler for Mocked {
    fn call<'a>(
        &'a self,
        invocation: HostInvocationContext,
        input: Option<String>,
        cancel: HostCancel,
    ) -> HostFuture<'a> {
        match hook() {
            Some(hook) => hook.call(&self.route, invocation, input, cancel, self.real.clone()),
            None => unreachable!("a mocked route exists only with a hook"),
        }
    }
}
