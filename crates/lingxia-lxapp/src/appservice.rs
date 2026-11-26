#[cfg(feature = "js-appservice")]
#[path = "appservice/js_runtime.rs"]
mod js_runtime;
#[cfg(feature = "js-appservice")]
#[path = "appservice/js_tree.rs"]
mod js_tree;
#[cfg(feature = "js-appservice")]
#[path = "appservice/js_worker_pool.rs"]
mod js_worker_pool;

#[cfg(feature = "js-appservice")]
pub use js_runtime::PageSvc;

#[cfg(all(test, feature = "js-appservice"))]
pub(crate) use js_runtime::ServiceMessage;
#[cfg(feature = "js-appservice")]
pub(crate) use js_runtime::event_bus;
#[cfg(all(feature = "test-utils", feature = "js-appservice"))]
pub(crate) use js_runtime::prepare_test_context;
#[cfg(feature = "js-appservice")]
pub(crate) use js_worker_pool::LxAppWorkers;

/// Completes after Logic teardown has returned its dedicated worker to the pool.
#[derive(Clone, Debug)]
pub(crate) struct WorkerTermination {
    result: tokio::sync::watch::Receiver<Option<Result<(), lingxia_error::Error>>>,
}

impl WorkerTermination {
    pub(crate) fn is_complete(&self) -> bool {
        self.result.borrow().is_some()
    }

    pub(crate) fn same_completion(&self, other: &Self) -> bool {
        self.result.same_channel(&other.result)
    }
    #[cfg(feature = "js-appservice")]
    fn pending() -> (
        tokio::sync::watch::Sender<Option<Result<(), lingxia_error::Error>>>,
        Self,
    ) {
        let (tx, result) = tokio::sync::watch::channel(None);
        (tx, Self { result })
    }

    fn completed() -> Self {
        Self {
            result: tokio::sync::watch::channel(Some(Ok(()))).1,
        }
    }

    pub(crate) async fn wait(mut self) -> Result<(), lingxia_error::Error> {
        match self.result.wait_for(Option::is_some).await {
            Ok(result) => result.clone().unwrap_or(Ok(())),
            Err(_) => Err(lingxia_error::Error::internal(
                "Worker termination completion channel closed",
            )),
        }
    }
}

#[cfg(not(feature = "js-appservice"))]
pub(crate) mod event_bus {
    pub const BROWSER_TAB_CLOSED_EVENT: &str = "__lingxiaBrowserTabClosed";
    pub const APPEARANCE_CHANGE_EVENT: &str = "AppearanceChange";
    pub const NAV_CURRENT_CHANGE_EVENT: &str = "NavCurrentChange";
    pub const HOST_APPEARANCE_CHANGE_EVENT: &str = "HostAppearanceChange";
    pub const HOST_CONTRAST_CHANGE_EVENT: &str = "HostContrastChange";
    pub const DISPLAY_LANGUAGE_CHANGE_EVENT: &str = "DisplayLanguageChange";
    pub const DISPLAY_LANGUAGE_STATE_CHANGE_EVENT: &str = "DisplayLanguageStateChange";

    #[derive(Clone, Debug)]
    pub(crate) enum Scope {
        App,
        PageInstance(String),
    }

    #[derive(Clone, Debug)]
    pub(crate) struct AppBusEvent {
        pub scope: Scope,
        pub event_name: String,
        pub payload_json: Option<String>,
    }

    pub fn publish_app_event(
        _appid: &str,
        _event_name: &str,
        _payload_json: Option<String>,
    ) -> bool {
        false
    }

    pub fn publish_page_event(
        _appid: &str,
        _page_path: &str,
        _event_name: &str,
        _payload_json: Option<String>,
    ) -> bool {
        false
    }
}

#[cfg(not(feature = "js-appservice"))]
mod no_js_runtime {
    use crate::appservice::event_bus::AppBusEvent;
    use crate::info;
    use crate::lifecycle::{AppServiceEvent, PageLifecycleEvent};
    use std::sync::Arc;
    use tokio::sync::oneshot::Sender;

    /// Keeps LxApp surfaces usable when JS AppService is not compiled in.
    pub(crate) struct LxAppWorkers;

    impl LxAppWorkers {
        pub fn init(_num_workers: usize) -> Arc<Self> {
            Arc::new(Self)
        }

        pub fn create_app_svc(
            &self,
            lxapp: Arc<crate::lxapp::LxApp>,
        ) -> Result<(), lingxia_error::Error> {
            if lxapp.logic_enabled() {
                return Err(unsupported_js_runtime());
            }
            info!("Logic disabled; JS AppService is not compiled in")
                .with_appid(lxapp.appid.clone());
            Ok(())
        }

        pub fn terminate_app_svc(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
        ) -> Result<super::WorkerTermination, lingxia_error::Error> {
            Ok(super::WorkerTermination::completed())
        }

        pub fn restart_app_svc(
            &self,
            lxapp: Arc<crate::lxapp::LxApp>,
        ) -> Result<(), lingxia_error::Error> {
            if lxapp.logic_enabled() {
                return Err(unsupported_js_runtime());
            }
            Ok(())
        }

        pub fn create_page_svc_with_ack(
            &self,
            lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            _page_instance_id: Option<String>,
            ack_tx: Sender<Result<(), String>>,
        ) -> Result<(), lingxia_error::Error> {
            if lxapp.logic_enabled() {
                return Err(unsupported_js_runtime());
            }
            let _ = ack_tx.send(Ok(()));
            Ok(())
        }

        pub(crate) fn terminate_page_svc(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            _page_instance_id: Option<String>,
        ) -> Result<(), lingxia_error::Error> {
            Ok(())
        }

        pub fn call_app_service_event(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _event: AppServiceEvent,
            _args: Option<String>,
        ) -> Result<(), lingxia_error::Error> {
            Ok(())
        }

        pub fn call_page_service(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            _page_instance_id: Option<String>,
            _name: String,
            _args: Option<String>,
        ) -> Result<(), lingxia_error::Error> {
            Err(unsupported_js_runtime())
        }

        pub fn call_page_service_event(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            _page_instance_id: Option<String>,
            _event: PageLifecycleEvent,
            _args: Option<String>,
        ) -> Result<(), lingxia_error::Error> {
            Ok(())
        }

        pub(crate) fn dispatch_app_bus_event(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            event: AppBusEvent,
        ) -> Result<(), lingxia_error::Error> {
            let _ = (event.scope, event.event_name, event.payload_json);
            Ok(())
        }

        pub async fn eval_app_service(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _script: String,
            _capture_calls: bool,
        ) -> Result<String, crate::EvalError> {
            Err(crate::EvalError::Unavailable(unsupported_js_runtime()))
        }
    }

    impl crate::bridge::AppServiceBackend for LxAppWorkers {
        fn forward(
            &self,
            lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            _page_instance_id: Option<String>,
            message: crate::bridge::AppServiceCommand,
        ) -> Result<(), lingxia_error::Error> {
            if lxapp.logic_enabled() {
                return Err(unsupported_js_runtime());
            }

            match message {
                // Keep bridge handshake/state plumbing alive for native-only pages.
                crate::bridge::AppServiceCommand::BeginSessionWork { .. }
                | crate::bridge::AppServiceCommand::CancelSessionWork { .. }
                | crate::bridge::AppServiceCommand::Ready { .. }
                | crate::bridge::AppServiceCommand::StateSnapshot { .. }
                | crate::bridge::AppServiceCommand::StateAck { .. }
                | crate::bridge::AppServiceCommand::StateResync { .. } => Ok(()),
                // Real AppService traffic is unavailable in native-only mode.
                _ => Err(lingxia_error::Error::not_supported(
                    "AppService is unavailable for logic:false page",
                )),
            }
        }
    }

    fn unsupported_js_runtime() -> lingxia_error::Error {
        lingxia_error::Error::not_supported("host was built without JS AppService runtime")
    }
}

#[cfg(not(feature = "js-appservice"))]
pub(crate) use no_js_runtime::LxAppWorkers;
