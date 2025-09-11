use crate::error::LxAppError;
use crate::log::{LogBuilder, LogLevel, LogTag};
use crate::lx;
use crate::lxapp::LxApp;
use crate::{error, info};

use rong::{JSContext, JSFunc, JSObject, JSResult, JSRuntime, RongJSError, Source};
use rong_modules::{console, fs, http, storage};

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};

mod app;
use app::LxAppSvc;

pub mod bridge;

mod page;
use page::PageSvc;

/// Message type for LxApp service system
#[derive(Clone)]
pub(crate) enum ServiceMessage {
    // Create a new lxapp service
    CreateLxApp {
        lxapp: Arc<LxApp>,
    },
    // Delete an lxapp service
    TerminateLxApp {
        appid: String,
    },
    // Create a new page service
    CreatePage {
        appid: String,
        path: String,
    },
    // Delete a page service
    TerminatePage {
        appid: String,
        path: String,
    },
    // Call function of App service
    CallAppSvc {
        appid: String,
        name: String,
        args: Option<String>, // JSON string of arguments
    },
    // Call function of Page service with different sources
    CallPageSvc {
        appid: String,
        path: String,
        source: PageSvcSource,
    },
}

/// Enum representing different sources of Page service calls
#[derive(Clone)]
pub enum PageSvcSource {
    /// Call from view layer via bridge
    View {
        incoming: Arc<bridge::IncomingMessage>,
    },
    /// Call from native layer with explicit function name and args
    Native {
        name: String,
        args: Option<String>, // JSON string of arguments
    },
}

#[derive(Clone)]
pub(crate) struct WorkerService {
    pub(crate) svc: ServiceMessage,
}

// Handles a call to an App service function
async fn handle_app_service_call(
    worker_id: usize,
    ctx: &JSContext,
    appid: String,
    name: String,
    args: Option<String>,
) {
    if let Some(svc) = ctx.get_user_data::<LxAppSvc>() {
        let svc_clone = svc.clone();
        let ctx_clone_for_task = ctx.clone();

        let task = async move {
            if let Err(e) = svc_clone.call(&ctx_clone_for_task, &name, args).await {
                error!(
                    "[Worker {}] App service call '{}' failed, Error: {}",
                    worker_id, name, e
                )
                .with_appid(appid);
            }
        };
        rong::spawn(task);
    } else {
        error!("[Worker {}] App service not loaded", worker_id).with_appid(appid);
    }
}

// Handles a message from the view layer to a Page service
async fn handle_view_source(
    page_svc_ref: &PageSvc,
    incoming: Arc<bridge::IncomingMessage>,
) -> Result<(), LxAppError> {
    page_svc_ref
        .as_bridge()
        .process_incoming_message(page_svc_ref, page_svc_ref, incoming)
        .await
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
    rong::spawn(task);
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
        ServiceMessage::CreateLxApp { lxapp } => {
            let ctx = runtime.context();

            // Store the LxApp reference directly in JSContext user data
            ctx.set_user_data(lxapp.clone());

            // Create a HashMap for PageSvc instances and store it in JSContext
            let page_svc_map: Rc<RefCell<HashMap<String, PageSvc>>> =
                Rc::new(RefCell::new(HashMap::new()));
            ctx.set_user_data(page_svc_map.clone());

            // register Page, App and getApp function
            let _ = app::init(&ctx);
            let _ = page::init(&ctx);

            // Set console writer
            console::set_writer(Box::new(LxAppCtx::new(lxapp.clone())));

            // Set file access guard to prevent cross-app file access
            fs::set_file_access_guard(Box::new(LxAppCtx::new(lxapp.clone())));

            // Set network access guard to prevent unauthorized domain access
            http::set_network_access_guard(Box::new(LxAppCtx::new(lxapp.clone())));

            let localstorage = lxapp.storage_dir.join(format!("{}.redb", lxapp.appid));
            if let Err(e) = storage::set_storage_path(localstorage) {
                info!("[Worker {}] failed to open localstorage: {}", worker_id, e)
                    .with_appid(lxapp.appid.clone());
            }

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

            let js = lxapp.lxapp_dir.join("logic.js");
            if js.exists() {
                if let Ok(js) = Source::from_path(&ctx, js).await {
                    match ctx.eval::<()>(js) {
                        Ok(_) => {
                            info!("[Worker {}] Successfully loaded logic JS", worker_id)
                                .with_appid(lxapp.appid.clone());
                        }
                        Err(e) => {
                            info!("[Worker {}] eval logic JS  failed: {}", worker_id, e)
                                .with_appid(lxapp.appid.clone());
                        }
                    }
                }
            } else {
                error!(
                    "[Worker {}] Not found JS file: '{}'",
                    worker_id,
                    js.display()
                )
                .with_appid(lxapp.appid.clone());
            }

            *current_ctx = Some(ctx.clone());
        }
        ServiceMessage::TerminateLxApp { appid } => {
            if current_ctx.is_some() {
                *current_ctx = None;
                info!("[Worker {}] Removed LxApp context ", worker_id).with_appid(appid.clone());
            }
        }
        ServiceMessage::CreatePage { appid, path } => {
            if let Some(ctx) = current_ctx.as_ref() {
                if let Ok(page_jsfunc) = ctx.global().get::<_, JSFunc>("__CREATE_PAGE__") {
                    if let Err(e) = page_jsfunc.call::<_, ()>(None, (path.clone(),)) {
                        error!("[Worker {}] __CREATE_PAGE__ call failed: {}", worker_id, e)
                            .with_appid(appid)
                            .with_path(path);
                    }
                }
            }
        }
        ServiceMessage::TerminatePage { appid, path } => {
            if let Some(ctx) = current_ctx.as_ref() {
                // Remove page from page_svc map stored in JSContext
                let page_svc = if let Some(page_svc_map) =
                    ctx.get_user_data::<Rc<RefCell<HashMap<String, PageSvc>>>>()
                {
                    page_svc_map.borrow_mut().remove(&path)
                } else {
                    None
                };

                if let Some(page_svc) = page_svc {
                    if let Ok(registry) = ctx.global().get::<_, JSObject>("__PAGE_REGISTRY__") {
                        registry.del(page_svc.page.path().as_str());
                    }

                    info!("[Worker {}] Removed page", worker_id)
                        .with_appid(appid)
                        .with_path(path);
                }
            }
        }
        ServiceMessage::CallAppSvc { appid, name, args } => {
            if let Some(ctx) = current_ctx.as_ref() {
                handle_app_service_call(worker_id, ctx, appid, name, args).await;
            }
        }
        ServiceMessage::CallPageSvc {
            appid,
            path,
            source,
        } => {
            if let Some(ctx) = current_ctx.as_ref() {
                match source {
                    PageSvcSource::View { incoming } => {
                        let page_svc = if let Some(page_svc_map) =
                            ctx.get_user_data::<Rc<RefCell<HashMap<String, PageSvc>>>>()
                        {
                            page_svc_map.borrow().get(&path).cloned()
                        } else {
                            None
                        };

                        if let Some(page_svc) = page_svc {
                            if let Err(e) = handle_view_source(&page_svc, incoming).await {
                                error!(
                                    "[Worker {}] Handle incoming message error: {}",
                                    worker_id, e
                                );
                            }
                        } else {
                            error!("[Worker {}] Page service not loaded", worker_id)
                                .with_path(path)
                                .with_appid(appid);
                        }
                    }
                    PageSvcSource::Native { name, args } => {
                        let page_svc = if let Some(page_svc_map) =
                            ctx.get_user_data::<Rc<RefCell<HashMap<String, PageSvc>>>>()
                        {
                            page_svc_map.borrow().get(&path).cloned()
                        } else {
                            None
                        };

                        if let Some(page_svc) = page_svc {
                            handle_native_source(&page_svc, appid, name, args).await;
                        } else {
                            error!("[Worker {}] Page service not loaded", worker_id)
                                .with_appid(appid)
                                .with_path(path);
                        }
                    }
                }
            }
        }
    }
}

/// Create a new mini-app service - enforces 1:1 appid->worker mapping
pub(crate) fn create_app_svc(
    lxapp: Arc<crate::lxapp::LxApp>,
    sender: &mpsc::Sender<ServiceMessage>,
    worker_assignments: &Arc<Mutex<HashMap<String, usize>>>,
    free_workers: &Arc<Mutex<Vec<usize>>>,
) -> Result<(), LxAppError> {
    let appid = lxapp.appid.clone();

    // Check if app already has a dedicated worker (enforce 1:1 mapping)
    if worker_assignments.lock().unwrap().contains_key(&appid) {
        info!("App {} already has a dedicated worker", appid);
        return Ok(());
    }

    // Check if we have free workers available
    let worker_id = {
        let mut free_workers_guard = free_workers.lock().unwrap();
        if free_workers_guard.is_empty() {
            return Err(LxAppError::ResourceExhausted(
                "No available workers for new mini-app".to_string(),
            ));
        }
        free_workers_guard.pop().unwrap()
    };

    // Establish the 1:1 mapping: appid -> worker_id
    worker_assignments
        .lock()
        .unwrap()
        .insert(appid.clone(), worker_id);

    // Send message to create the runtime in the dedicated worker
    sender.send(ServiceMessage::CreateLxApp { lxapp })?;

    info!("Assigned dedicated worker {} to app {}", worker_id, appid);
    Ok(())
}

/// Terminate a mini-app service - breaks 1:1 mapping and returns worker to pool
pub(crate) fn terminate_app_svc(
    appid: String,
    sender: &mpsc::Sender<ServiceMessage>,
    worker_assignments: &Arc<Mutex<HashMap<String, usize>>>,
    free_workers: &Arc<Mutex<Vec<usize>>>,
) -> Result<(), LxAppError> {
    // Break the 1:1 mapping and release the dedicated worker
    if let Some(worker_id) = worker_assignments.lock().unwrap().remove(&appid) {
        // Return the worker to free pool for reuse by other mini-apps
        free_workers.lock().unwrap().push(worker_id);
        info!("Released dedicated worker {} from app {}", worker_id, appid);
    }

    sender.send(ServiceMessage::TerminateLxApp { appid })?;
    Ok(())
}

/// Wrapper for LxApp to implement external traits
struct LxAppCtx {
    lxapp: Arc<LxApp>,
}

impl LxAppCtx {
    pub fn new(lxapp: Arc<LxApp>) -> Self {
        Self { lxapp }
    }
}

impl std::fmt::Debug for LxAppCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LxAppCtx")
            .field("appid", &self.lxapp.appid)
            .finish()
    }
}

impl console::ConsoleWriter for LxAppCtx {
    fn write(&self, level: console::LogLevel, message: String) {
        let log = LogBuilder::new(LogTag::LxAppServiceConsole, message);
        match level {
            console::LogLevel::Verbose => log
                .with_level(LogLevel::Verbose)
                .with_appid(self.lxapp.appid.clone()),
            console::LogLevel::Info => log
                .with_level(LogLevel::Info)
                .with_appid(self.lxapp.appid.clone()),
            console::LogLevel::Debug => log
                .with_level(LogLevel::Debug)
                .with_appid(self.lxapp.appid.clone()),
            console::LogLevel::Error => log
                .with_level(LogLevel::Error)
                .with_appid(self.lxapp.appid.clone()),
            console::LogLevel::Warn => log
                .with_level(LogLevel::Warn)
                .with_appid(self.lxapp.appid.clone()),
        };
    }

    fn is_tty(&self) -> bool {
        false
    }
}

impl fs::FileAccessGuard for LxAppCtx {
    /// Check if the mini app has access to the specified path
    ///
    /// This prevents one mini app from accessing another mini app's files.
    /// Only allows access to absolute paths within:
    /// - The app's own user data directory
    /// - The app's own user cache directory
    ///
    /// Relative paths are rejected
    fn check_access(&self, path: &str) -> JSResult<()> {
        let path = Path::new(path);

        // Reject relative paths
        if !path.is_absolute() {
            return Err(RongJSError::Error(format!(
                "Access denied: relative paths not allowed"
            )));
        }

        // Helper function to canonicalize paths
        let canonicalize = |p: &Path| -> Result<std::path::PathBuf, String> {
            p.canonicalize().or_else(|_| {
                // If path doesn't exist, try parent + filename
                p.parent()
                    .and_then(|parent| parent.canonicalize().ok())
                    .map(|parent| parent.join(p.file_name().unwrap_or_default()))
                    .ok_or_else(|| format!("Invalid path: {}", p.display()))
            })
        };

        // Get canonical paths
        let canonical_path = canonicalize(path).map_err(|e| RongJSError::Error(e))?;
        let user_data_canonical =
            canonicalize(&self.lxapp.user_data_dir).map_err(|e| RongJSError::Error(e))?;
        let user_cache_canonical =
            canonicalize(&self.lxapp.user_cache_dir).map_err(|e| RongJSError::Error(e))?;

        // Check if path is within allowed directories
        if canonical_path.starts_with(&user_data_canonical)
            || canonical_path.starts_with(&user_cache_canonical)
        {
            Ok(())
        } else {
            Err(RongJSError::Error(
                "Access denied: path outside allowed".to_string(),
            ))
        }
    }
}

impl http::NetworkAccessGuard for LxAppCtx {
    /// Check if the mini app has access to the specified domain
    /// Returns Ok(()) if access is granted, Err with error message if denied
    fn check_access(&self, domain: &str) -> JSResult<()> {
        if self.lxapp.is_domain_allowed(domain) {
            Ok(())
        } else {
            Err(RongJSError::Error(format!(
                "Access denied: domain '{}' is not allowed ",
                domain
            )))
        }
    }
}
