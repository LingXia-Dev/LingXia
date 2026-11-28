//! A real lxapp session for Rust route tests; `lingxia::testing` drives it.

use super::{AuthenticatedCaller, HostInvocationContext};
use crate::appservice::LxAppWorkers;
use crate::lxapp::{AppSessionClass, LxApp, LxAppSessionStatus};
use lingxia_platform::Platform;
use std::any::Any;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// One lxapp session of `class` under its own data directory, removed on drop.
#[doc(hidden)]
pub struct TestSession {
    lxapp: Arc<LxApp>,
    root: PathBuf,
}

impl TestSession {
    pub fn new(class: AppSessionClass) -> lingxia_error::Result<Self> {
        static WORKERS: OnceLock<Arc<LxAppWorkers>> = OnceLock::new();
        let id = uuid::Uuid::new_v4();
        let root = std::env::temp_dir().join(format!("lingxia-test-host-{id}"));
        let dir = |name: &str| root.join(name).display().to_string();
        let runtime = Platform::new(dir("data"), dir("cache"), "en-US".to_string())
            .map_err(|err| lingxia_error::Error::internal(format!("test platform: {err}")))?;
        let app_id = format!("app.lingxia.test-host.{id}");
        crate::lxapp::register_synthetic_lxapp(app_id.clone());
        let workers = Arc::clone(WORKERS.get_or_init(|| LxAppWorkers::init(1)));
        let mut lxapp =
            LxApp::new_with_session_class_for_test(app_id, Arc::new(runtime), workers, class)?;
        lxapp.approve_unrestricted_permissions_for_test();
        let lxapp = Arc::new(lxapp);
        lxapp.bind_arc();
        lxapp.set_status(LxAppSessionStatus::Opened);
        Ok(Self { lxapp, root })
    }

    /// What a route sees when this session calls it.
    pub fn invocation(&self) -> HostInvocationContext {
        let caller = AuthenticatedCaller::for_lxapp(&self.lxapp);
        HostInvocationContext::for_dispatch(Arc::clone(&self.lxapp), &caller, None)
            .expect("a session's own caller belongs to it")
    }

    pub fn lxapp(&self) -> Arc<LxApp> {
        Arc::clone(&self.lxapp)
    }

    /// A value this session's routes get in place of the host-wide one of its type.
    pub fn provide(&self, value: Arc<dyn Any + Send + Sync>) {
        self.lxapp
            .test_values
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(value);
    }
}

impl Drop for TestSession {
    fn drop(&mut self) {
        self.lxapp.lxapp_states.clear();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
