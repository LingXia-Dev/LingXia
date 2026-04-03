use crate::bridge::{self, AppServiceCommand};
use crate::error::LxAppError;
use crate::lx;
use crate::lxapp::LxApp;
use crate::{error, info};

use rong::{JSContext, JSResult, JSRuntime, RongJSError, error::HostError};
use rong_console as console;
use rong_fs as fs;
use rong_http as http;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use tokio::sync::oneshot;

mod app;
use crate::lifecycle::AppServiceEvent;

pub(crate) mod event_bus;

pub(crate) mod native_component;

pub(crate) mod view_call;

mod page;
use crate::lifecycle::PageServiceEvent;
pub use page::PageSvc;

mod plugin;

mod runtime_ctx;
use runtime_ctx::{
    register_app_ctx, remove_app_ctx, set_app_svc_for_ctx, with_app_svc, with_page_svc_map,
};

/// Message type for LxApp service system
pub(crate) enum ServiceMessage {
    // Create a new AppService (JS runtime) for this LxApp instance
    CreateAppSvc {
        lxapp: Arc<LxApp>,
    },
    // Terminate AppService for this LxApp instance. ACK returned when cleanup completes.
    TerminateAppSvc {
        lxapp: Arc<LxApp>,
        ack_tx: mpsc::Sender<()>,
    },
    // Create a new page service
    CreatePage {
        lxapp: Arc<LxApp>,
        path: String,
        ack_tx: oneshot::Sender<()>,
    },
    // Delete a page service (object-identity safe)
    TerminatePage {
        lxapp: Arc<LxApp>,
        path: String,
    },
    // Call predefined AppService event (typed)
    CallAppSvcEvent {
        lxapp: Arc<LxApp>,
        event: AppServiceEvent,
        args: Option<String>,
    },
    // Call function of Page service with different sources
    CallPageSvc {
        lxapp: Arc<LxApp>,
        path: String,
        source: PageSvcSource,
    },
    // Call typed page event
    CallPageSvcEvent {
        lxapp: Arc<LxApp>,
        path: String,
        event: PageServiceEvent,
        args: Option<String>,
    },
    // Native -> JS event dispatch via event bus (e.g., video context)
    DispatchAppBusEvent {
        lxapp: Arc<LxApp>,
        event: event_bus::AppBusEvent,
    },
}

/// Enum representing different sources of Page service calls
pub enum PageSvcSource {
    /// Call from view layer after the top-level bridge has parsed and routed it.
    Bridge {
        message: crate::bridge::AppServiceCommand,
    },
    /// Call from native layer with explicit function name and args
    Native {
        name: String,
        args: Option<String>, // JSON string of arguments
    },
}

pub(crate) struct WorkerService {
    pub(crate) svc: ServiceMessage,
}

// Handles a typed AppService event
async fn handle_app_service_event(
    worker_id: usize,
    ctx: &JSContext,
    appid: String,
    event: AppServiceEvent,
    args: Option<String>,
) {
    // Resolve AppSvc from registry via JSContext and clone it for use in this async handler.
    let svc = match with_app_svc(ctx, |svc| Ok(svc.clone())) {
        Ok(svc) => svc,
        Err(e) => {
            error!("[Worker {}] App service not loaded: {}", worker_id, e).with_appid(appid);
            return;
        }
    };

    if matches!(
        event,
        AppServiceEvent::OnLaunch
            | AppServiceEvent::OnShow
            | AppServiceEvent::OnHide
            | AppServiceEvent::OnUserCaptureScreen
    ) {
        if let Err(e) = svc.call_event(ctx, event, args.clone()).await {
            error!(
                "[Worker {}] App service event '{}' failed, Error: {}",
                worker_id, event, e
            )
            .with_appid(appid);
        }
    }
}

// Handles a bridge-routed message that must enter the JS runtime worker.
async fn handle_bridge_source(
    page_svc: &PageSvc,
    message: AppServiceCommand,
) -> Result<(), LxAppError> {
    match message {
        AppServiceCommand::Ready => {
            page_svc.handle_bridge_ready().await;
            Ok(())
        }
        AppServiceCommand::StateSnapshot { id, scope } => {
            let bridge = page_svc.bridge();
            match page_svc.get_state_snapshot(scope.as_deref()).await {
                Ok(snapshot) => bridge.send_res_ok(page_svc, id, snapshot)?,
                Err(err) => bridge.send_res_err(
                    page_svc,
                    id,
                    bridge::BRIDGE_INTERNAL_ERROR,
                    Some(err.to_string()),
                    None,
                )?,
            }
            Ok(())
        }
        AppServiceCommand::Req {
            id,
            method,
            params_json,
            cancel_rx,
        } => {
            let bridge = page_svc.bridge();
            match page_svc
                .handle_req(&id, &method, params_json.as_deref(), cancel_rx)
                .await
            {
                Ok(json) => bridge.send_res_ok(page_svc, id, json)?,
                Err(err) => bridge.send_res_err(page_svc, id, &err.code, err.message, err.data)?,
            }
            Ok(())
        }
        AppServiceCommand::Notify {
            method,
            params_json,
        } => {
            page_svc
                .handle_notify(&method, params_json.as_deref())
                .await;
            Ok(())
        }
        AppServiceCommand::ChOpen {
            id,
            topic,
            params_json,
        } => {
            let bridge = page_svc.bridge();
            match page_svc
                .handle_ch_open(&id, &topic, params_json.as_deref())
                .await
            {
                Ok(()) => bridge.send_ch_ack_ok(page_svc, id)?,
                Err(err) => {
                    bridge.send_ch_ack_err(page_svc, id, &err.code, err.message, err.data)?
                }
            }
            Ok(())
        }
        AppServiceCommand::ChData { id, payload_json } => {
            if let Err(err) = page_svc.handle_ch_data(&id, &payload_json).await {
                error!("channel '{}' data handler failed: {}", id, err.code)
                    .with_appid(page_svc.page.appid())
                    .with_path(page_svc.page.path());
            }
            Ok(())
        }
        AppServiceCommand::ChClose { id, code, reason } => {
            page_svc
                .handle_ch_close(&id, code.as_deref(), reason.as_deref())
                .await;
            Ok(())
        }
        AppServiceCommand::StateAck { scope, rev } => {
            page_svc.handle_state_ack(scope, rev).await;
            Ok(())
        }
    }
}

// Handles a call from native code to a Page service function
async fn handle_native_source(
    page_svc: &PageSvc,
    appid: String,
    name: String,
    args: Option<String>,
) {
    let ctx = page_svc.get_ctx();
    let page_svc_clone = page_svc.clone();
    let name_clone = name.clone();

    let task = async move {
        if let Err(e) = page_svc_clone
            .call_or_event_from_native(&ctx, &name, args.as_deref())
            .await
        {
            crate::error!("Page service call '{}' failed: {}", name_clone, e)
                .with_appid(appid)
                .with_path(page_svc_clone.page.path());
        }
    };
    rong::spawn_local(task);
}

/// The core logic for a persistent worker task.
/// This function is a handler for messages received by the worker.
pub(crate) async fn lxapp_service_handler(
    worker_id: usize,
    runtime: JSRuntime,
    message: ServiceMessage,
    current_ctx: &mut Option<JSContext>,
) {
    match message {
        ServiceMessage::CreateAppSvc { lxapp } => {
            let ctx = runtime.context();

            // Register LxApp runtime context and bind identity to JSContext
            register_app_ctx(&runtime, &ctx, &lxapp);

            // register Page, App and getApp function
            if let Err(e) = app::init(&ctx) {
                error!(
                    "[Worker {}] Failed to initialize App runtime: {}",
                    worker_id, e
                )
                .with_appid(lxapp.appid.clone());
                return;
            }
            if let Err(e) = page::init(&ctx) {
                error!(
                    "[Worker {}] Failed to initialize Page runtime: {}",
                    worker_id, e
                )
                .with_appid(lxapp.appid.clone());
                return;
            }
            if let Err(e) = plugin::init(&ctx) {
                error!(
                    "[Worker {}] Failed to initialize Plugin runtime: {}",
                    worker_id, e
                )
                .with_appid(lxapp.appid.clone());
                return;
            }
            event_bus::init(&ctx);

            let app_ctx = LxAppCtx::new(lxapp.clone());

            console::set_trace_context(
                &ctx,
                console::ConsoleTraceContext {
                    namespace: Some(lxapp.appid.clone()),
                    scope: Some("appservice".to_string()),
                },
            );

            // Set file access guard to prevent cross-app file access (Context-scoped)
            fs::set_file_access_guard(Box::new(app_ctx.clone()));

            // Set network access guard to prevent unauthorized domain access
            http::set_network_access_guard(Box::new(app_ctx));

            let _ = rong_modules::init(&ctx);
            let _ = lx::init(&ctx);

            // Execute a closure with access to the list of registered extensions.
            crate::lx::extension::with_registered_extensions(|user_extensions| {
                info!(
                    "[Worker {}] Initializing {} user-registered extensions",
                    worker_id,
                    user_extensions.len()
                )
                .with_appid(lxapp.appid.clone());

                // Iterate through the list and initialize each extension.
                for (index, user_extension) in user_extensions.iter().enumerate() {
                    if let Err(e) = user_extension.init(&ctx) {
                        error!(
                            "[Worker {}] Failed to initialize user extension #{}: {}",
                            worker_id, index, e
                        )
                        .with_appid(lxapp.appid.clone());
                    }
                }
            });

            info!("[Worker {}] Created JS context", worker_id).with_appid(lxapp.appid.clone());

            match lxapp.logic_entry_source(&ctx).await {
                Ok(Some(js)) => match ctx.eval::<()>(js) {
                    Ok(_) => {
                        info!("[Worker {}] Successfully loaded logic JS", worker_id)
                            .with_appid(lxapp.appid.clone());
                    }
                    Err(e) => {
                        info!("[Worker {}] eval logic JS  failed: {}", worker_id, e)
                            .with_appid(lxapp.appid.clone());
                    }
                },
                Ok(None) => {
                    info!(
                        "[Worker {}] Logic disabled; skipping JS bootstrap",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone());
                }
                Err(e) => {
                    error!("[Worker {}] Failed to load logic source: {}", worker_id, e)
                        .with_appid(lxapp.appid.clone());
                }
            }

            *current_ctx = Some(ctx.clone());
        }
        ServiceMessage::TerminateAppSvc { lxapp, ack_tx } => {
            // Drop the JSContext directly to release all JS/PageSvc resources.
            if current_ctx.is_some() {
                if let Some(ctx) = current_ctx.as_ref() {
                    console::clear_trace_context(ctx);
                }
                *current_ctx = None;
                info!("[Worker {}] Removed LxApp context ", worker_id)
                    .with_appid(lxapp.appid.clone());
            }
            // Clear guards on app terminate so the previous LxAppCtx is dropped
            // immediately (this shuts down its per-app cache capacity worker).
            fs::set_file_access_guard(Box::new(DenyAllFileAccessGuard));
            http::set_network_access_guard(Box::new(DenyAllNetworkAccessGuard));
            // Remove runtime context for this app so that all associated resources can be dropped.
            remove_app_ctx(&runtime, &lxapp.appid);
            // ACK back to the caller that cleanup is complete
            let _ = ack_tx.send(());
        }
        ServiceMessage::CreatePage {
            lxapp,
            path,
            ack_tx,
        } => {
            if let Some(ctx) = current_ctx.as_ref() {
                match PageSvc::create_in_ctx(ctx, &path).await {
                    Ok(()) => {
                        let _ = ack_tx.send(());
                    }
                    Err(e) => {
                        error!("[Worker {}] create_in_ctx failed: {}", worker_id, e)
                            .with_appid(lxapp.appid.clone())
                            .with_path(path);
                    }
                }
            }
        }
        ServiceMessage::TerminatePage { lxapp, path } => {
            if let Some(ctx) = current_ctx.as_ref() {
                // Ensure this TerminatePage belongs to the same LxApp
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if !same_app {
                    info!(
                        "[Worker {}] Ignored TerminatePage for different LxApp instance",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone())
                    .with_path(path.clone());
                    return;
                }

                // Remove page from page_svc map stored in registry
                let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                    Ok(page_svc_map.borrow_mut().remove(&path))
                })
                .unwrap_or(None);

                if page_svc.is_some() {
                    event_bus::clear_page(ctx, &path);

                    info!("[Worker {}] Removed page", worker_id)
                        .with_appid(lxapp.appid.clone())
                        .with_path(path);
                }
            }
        }
        ServiceMessage::CallAppSvcEvent { lxapp, event, args } => {
            if let Some(ctx) = current_ctx.as_ref() {
                // Ensure this event targets the same LxApp bound to ctx
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if same_app {
                    // Don't block the worker message pump on user JS lifecycle handlers.
                    // If an app handler awaits network/IO, blocking here can starve bridge handshake
                    // and other view messages, causing "Handshake timeout" even when transport is OK.
                    let ctx = ctx.clone();
                    let appid = lxapp.appid.clone();
                    rong::spawn_local(async move {
                        handle_app_service_event(worker_id, &ctx, appid, event, args).await;
                    });
                }
            }
        }
        ServiceMessage::CallPageSvc {
            lxapp,
            path,
            source,
        } => {
            if let Some(ctx) = current_ctx.as_ref() {
                match source {
                    PageSvcSource::Bridge { message } => {
                        let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                            Ok(page_svc_map.borrow().get(&path).cloned())
                        })
                        .unwrap_or(None);

                        if let Some(page_svc) = page_svc {
                            if let Err(e) = handle_bridge_source(&page_svc, message).await {
                                error!("[Worker {}] Handle bridge message error: {}", worker_id, e)
                                    .with_appid(lxapp.appid.clone())
                                    .with_path(path.clone());
                            }
                        } else {
                            info!(
                                "[Worker {}] Dropping bridge message: page service not loaded",
                                worker_id
                            )
                            .with_appid(lxapp.appid.clone())
                            .with_path(path);
                        }
                    }
                    PageSvcSource::Native { name, args } => {
                        let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                            Ok(page_svc_map.borrow().get(&path).cloned())
                        })
                        .unwrap_or(None);

                        if let Some(page_svc) = page_svc {
                            handle_native_source(&page_svc, lxapp.appid.clone(), name, args).await;
                        } else {
                            info!(
                                "[Worker {}] Dropping native call: page service not loaded",
                                worker_id
                            )
                            .with_appid(lxapp.appid.clone())
                            .with_path(path);
                        }
                    }
                }
            }
        }
        ServiceMessage::CallPageSvcEvent {
            lxapp,
            path,
            event,
            args,
        } => {
            if let Some(ctx) = current_ctx.as_ref() {
                // Resolve PageSvc from registry
                let page_svc = with_page_svc_map(ctx, |page_svc_map| {
                    Ok(page_svc_map.borrow().get(&path).cloned())
                })
                .unwrap_or(None);

                if let Some(page_svc) = page_svc {
                    // Enqueue page event via PageSvc API (non-blocking)
                    if let Err(e) = page_svc.call_page_event(ctx, event, args.as_deref()).await {
                        error!(
                            "[Worker {}] Page event '{}' failed: {}",
                            worker_id, event, e
                        )
                        .with_appid(lxapp.appid.clone())
                        .with_path(path);
                    }
                } else {
                    info!(
                        "[Worker {}] Dropping page event: page service not loaded",
                        worker_id
                    )
                    .with_appid(lxapp.appid.clone())
                    .with_path(path);
                }
            }
        }
        ServiceMessage::DispatchAppBusEvent { lxapp, event } => {
            if let Some(ctx) = current_ctx.as_ref() {
                let same_app = LxApp::from_ctx(ctx)
                    .map(|ctx_app| ctx_app.session.id == lxapp.session.id)
                    .unwrap_or(false);
                if same_app {
                    // Don't block the worker message pump on user JS event handlers. Like app/page
                    // lifecycle events, event bus handlers can await network/IO and would
                    // otherwise starve view messages (including handshake retries).
                    let ctx = ctx.clone();
                    let appid = lxapp.appid.clone();
                    rong::spawn_local(async move {
                        if let Err(e) = event_bus::dispatch_app_bus_event(&ctx, &event).await {
                            error!(
                                "[Worker {}] Dispatch app bus event failed: {}",
                                worker_id, e
                            )
                            .with_appid(appid);
                        }
                    });
                }
            }
        }
    }
}

/// Create a new mini-app service - enforces 1:1 appid->worker mapping
pub(crate) fn create_app_svc(
    lxapp: Arc<crate::lxapp::LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    instance_assignments: &Arc<Mutex<HashMap<usize, usize>>>,
    free_workers: &Arc<Mutex<VecDeque<usize>>>,
) -> Result<(), LxAppError> {
    let appid = lxapp.appid.clone();

    // Establish instance mapping only once; if a mapping exists, reuse it (idempotent)
    let key = lxapp.as_ref() as *const _ as usize;
    {
        let assignments = instance_assignments.lock().unwrap();
        if assignments.contains_key(&key) {
            info!("Reusing existing worker for app {}", appid);
            return Ok(());
        }
    }

    // Check if we have free workers available
    let worker_id = {
        let mut free_workers_guard = free_workers.lock().unwrap();
        if free_workers_guard.is_empty() {
            return Err(LxAppError::ResourceExhausted(
                "No available workers for new mini-app".to_string(),
            ));
        }
        free_workers_guard.pop_front().unwrap()
    };

    // Establish instance mapping: LxApp ptr -> worker_id
    instance_assignments.lock().unwrap().insert(key, worker_id);

    // Send message to create the runtime in the dedicated worker
    if let Err(e) = sender.send(ServiceMessage::CreateAppSvc { lxapp }) {
        instance_assignments.lock().unwrap().remove(&key);
        free_workers.lock().unwrap().push_front(worker_id);
        return Err(e.into());
    }

    info!("Assigned dedicated worker {} to app {}", worker_id, appid);
    Ok(())
}

/// Terminate a mini-app service - breaks 1:1 mapping and returns worker to pool
pub(crate) fn terminate_app_svc(
    lxapp_arc: Arc<LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    instance_assignments: &Arc<Mutex<HashMap<usize, usize>>>,
    free_workers: &Arc<Mutex<VecDeque<usize>>>,
) -> Result<(), LxAppError> {
    let appid = lxapp_arc.appid.clone();
    // Ensure mapping remains during terminate; get current worker_id via instance mapping
    let key = lxapp_arc.as_ref() as *const _ as usize;
    let worker_id_opt = instance_assignments.lock().unwrap().get(&key).copied();
    if worker_id_opt.is_none() {
        info!(
            "No active worker mapping for app {}; skipping terminate",
            appid
        );
        return Ok(());
    }

    // Set up ACK channel and send terminate to current worker
    let (tx, rx) = mpsc::channel();
    sender.send(ServiceMessage::TerminateAppSvc {
        lxapp: lxapp_arc,
        ack_tx: tx,
    })?;

    // Wait for ACK with timeout
    let acked = rx.recv_timeout(Duration::from_secs(3)).is_ok();
    if acked {
        info!("Terminate ACK received").with_appid(appid.clone());
    } else {
        error!("Terminate ACK timeout; forcing release").with_appid(appid.clone());
    }

    // Remove instance mapping and release the dedicated worker
    let worker_id_opt = instance_assignments.lock().unwrap().remove(&key);
    if let Some(worker_id) = worker_id_opt {
        free_workers.lock().unwrap().push_back(worker_id);
        info!("Released dedicated worker {} from app {}", worker_id, appid);
    }

    Ok(())
}

/// Wrapper for LxApp to implement external traits
#[derive(Clone)]
struct LxAppCtx {
    lxapp: Arc<LxApp>,
    cache_capacity_manager: Arc<crate::cache::CacheCapacityManager>,
}

#[derive(Debug)]
struct DenyAllFileAccessGuard;

#[derive(Debug)]
struct DenyAllNetworkAccessGuard;

impl fs::FileAccessGuard for DenyAllFileAccessGuard {
    fn resolve_access(&self, _path: &str) -> JSResult<std::path::PathBuf> {
        Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "Access denied",
        )))
    }
}

impl http::NetworkAccessGuard for DenyAllNetworkAccessGuard {
    fn check_access(&self, _domain: &str) -> JSResult<()> {
        Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "Access denied",
        )))
    }
}

impl LxAppCtx {
    pub fn new(lxapp: Arc<LxApp>) -> Self {
        let cache_capacity_manager = Arc::new(crate::cache::CacheCapacityManager::new(
            lxapp.user_cache_dir.clone(),
            crate::app::cache_max_size_bytes(),
            crate::app::cache_max_age(),
            Duration::from_secs(30),
        ));
        Self {
            lxapp,
            cache_capacity_manager,
        }
    }
}

impl std::fmt::Debug for LxAppCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LxAppCtx")
            .field("appid", &self.lxapp.appid)
            .finish()
    }
}

impl fs::FileAccessGuard for LxAppCtx {
    /// Check if the mini app has access to the specified path and resolve it to a safe absolute path.
    ///
    /// This prevents one mini app from accessing another mini app's files.
    /// Only allows access to absolute paths within:
    /// - The app's own user data directory
    /// - The app's own user cache directory
    ///
    /// Relative paths are also resolved relative to the allowed roots.
    ///
    /// For files in the user cache directory, this also updates access time and
    /// enqueues an async capacity-based LRU eviction check.
    fn resolve_access(&self, path: &str) -> JSResult<std::path::PathBuf> {
        let resolved = self
            .lxapp
            .resolve_accessible_path(path)
            // Mask absolute path details for security
            .map_err(|_| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, "Access denied"))
            })?;

        if resolved.starts_with(&self.lxapp.user_cache_dir) {
            self.cache_capacity_manager.on_cache_access(&resolved);
        }

        Ok(resolved)
    }
}

impl http::NetworkAccessGuard for LxAppCtx {
    /// Check if the mini app has access to the specified domain
    /// Returns Ok(()) if access is granted, Err with error message if denied
    fn check_access(&self, domain: &str) -> JSResult<()> {
        if self.lxapp.is_domain_allowed(domain) {
            Ok(())
        } else {
            Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Access denied: domain '{}' is not allowed", domain),
            )))
        }
    }
}
