//! Run the real setup callback before the creating thread resumes, modeling
//! immediate native readiness and a worker that outruns instance registration.
//! Logic creation, registry lookup, cleanup, and snapshots use production code.

use super::*;
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{OnceLock, Weak};

static HOSTS: OnceLock<Mutex<HashMap<String, Weak<TestHost>>>> = OnceLock::new();

#[derive(Default)]
pub(crate) struct TestHost {
    pub(crate) outcomes: Mutex<Vec<Result<(), String>>>,
    pub(crate) html_loads: AtomicUsize,
    pub(crate) allocation_barrier: Mutex<Option<Arc<std::sync::Barrier>>>,
}

pub(crate) struct Registration(String);

impl Drop for Registration {
    fn drop(&mut self) {
        HOSTS.get().unwrap().lock().unwrap().remove(&self.0);
    }
}

impl TestHost {
    pub(crate) fn allocated(&self) {
        let barrier = self.allocation_barrier.lock().unwrap().clone();
        if let Some(barrier) = barrier {
            barrier.wait();
        }
    }
    pub(crate) fn install(appid: &str) -> (Arc<Self>, Registration) {
        let host = Arc::new(Self::default());
        HOSTS
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .insert(appid.to_string(), Arc::downgrade(&host));
        (host, Registration(appid.to_string()))
    }

    pub(crate) fn run_setup(
        &self,
        page: &PageInstance,
        setup: impl Future<Output = Result<(), String>>,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), setup)
                .await
                .expect("page setup did not settle; possible registration-lock deadlock")
        });
        self.outcomes.lock().unwrap().push(result.clone());
        page.finish_webview_setup(result);
    }

    pub(crate) fn load_html(&self) -> Result<(), LxAppError> {
        self.html_loads.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

pub(crate) fn for_page(page: &PageInstance) -> Option<Arc<TestHost>> {
    HOSTS
        .get()?
        .lock()
        .unwrap()
        .get(&page.appid())
        .and_then(Weak::upgrade)
}
