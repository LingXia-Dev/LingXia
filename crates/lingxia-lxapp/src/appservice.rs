#[cfg(feature = "js-appservice")]
#[path = "appservice/js_runtime.rs"]
mod js_runtime;
#[cfg(feature = "js-appservice")]
#[path = "appservice/js_worker_pool.rs"]
mod js_worker_pool;

#[cfg(feature = "js-appservice")]
pub use js_runtime::PageSvc;

#[cfg(feature = "js-appservice")]
pub(crate) use js_runtime::event_bus;
#[cfg(feature = "js-appservice")]
pub(crate) use js_worker_pool::LxAppWorkers;

#[cfg(not(feature = "js-appservice"))]
pub(crate) mod event_bus {
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
    use crate::lifecycle::{AppServiceEvent, PageServiceEvent};
    use crate::{LxAppError, info};
    use std::sync::Arc;
    use tokio::sync::oneshot::Sender;

    /// Keeps LxApp surfaces usable when JS AppService is not compiled in.
    pub(crate) struct LxAppWorkers;

    impl LxAppWorkers {
        pub fn init(_num_workers: usize) -> Arc<Self> {
            Arc::new(Self)
        }

        pub fn create_app_svc(&self, lxapp: Arc<crate::lxapp::LxApp>) -> Result<(), LxAppError> {
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
        ) -> Result<(), LxAppError> {
            Ok(())
        }

        pub fn create_page_svc_with_ack(
            &self,
            lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            ack_tx: Sender<()>,
        ) -> Result<(), LxAppError> {
            if lxapp.logic_enabled() {
                return Err(unsupported_js_runtime());
            }
            let _ = ack_tx.send(());
            Ok(())
        }

        pub(crate) fn terminate_page_svc(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
        ) -> Result<(), LxAppError> {
            Ok(())
        }

        pub fn call_app_service_event(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _event: AppServiceEvent,
            _args: Option<String>,
        ) -> Result<(), LxAppError> {
            Ok(())
        }

        pub fn call_page_service(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            _name: String,
            _args: Option<String>,
        ) -> Result<(), LxAppError> {
            Err(unsupported_js_runtime())
        }

        pub fn call_page_service_event(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            _event: PageServiceEvent,
            _args: Option<String>,
        ) -> Result<(), LxAppError> {
            Ok(())
        }

        pub(crate) fn dispatch_app_bus_event(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            event: AppBusEvent,
        ) -> Result<(), LxAppError> {
            let _ = (event.scope, event.event_name, event.payload_json);
            Ok(())
        }

        pub async fn eval_app_service(
            &self,
            _lxapp: Arc<crate::lxapp::LxApp>,
            _script: String,
        ) -> Result<String, LxAppError> {
            Err(unsupported_js_runtime())
        }
    }

    impl crate::bridge::AppServiceBackend for LxAppWorkers {
        fn forward(
            &self,
            lxapp: Arc<crate::lxapp::LxApp>,
            _path: String,
            message: crate::bridge::AppServiceCommand,
        ) -> Result<(), LxAppError> {
            if lxapp.logic_enabled() {
                return Err(unsupported_js_runtime());
            }

            match message {
                // Keep bridge handshake/state plumbing alive for native-only pages.
                crate::bridge::AppServiceCommand::Ready
                | crate::bridge::AppServiceCommand::StateSnapshot { .. }
                | crate::bridge::AppServiceCommand::StateAck { .. } => Ok(()),
                // Real AppService traffic is unavailable in native-only mode.
                _ => Err(LxAppError::UnsupportedOperation(
                    "AppService is unavailable for logic:false page".to_string(),
                )),
            }
        }
    }

    fn unsupported_js_runtime() -> LxAppError {
        LxAppError::UnsupportedOperation("host was built without JS AppService runtime".to_string())
    }
}

#[cfg(not(feature = "js-appservice"))]
pub(crate) use no_js_runtime::LxAppWorkers;
