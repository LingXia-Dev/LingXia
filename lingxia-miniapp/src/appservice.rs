use crate::error::MiniAppError;
use crate::log::{LogBuilder, LogLevel, LogTag};
use crate::miniapp::MiniApp;
use crate::{error, info};

use rong::{
    JSContext, JSFunc, JSResult, JSRuntime, Rong, RongJS, RongJSError, Source, Worker,
    WorkerMessage,
};
use rong_modules::{console, fs, http, storage};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, mpsc};

mod app;
use app::MiniAppSvc;

pub mod bridge;

mod page;
use page::PageSvc;

mod lx;

/// Message type for MiniApp service system
#[derive(Clone)]
enum ServiceMessage {
    // Create a new miniapp service
    CreateMiniApp {
        miniapp: Arc<MiniApp>,
    },
    // Delete an miniapp service
    TerminateMiniApp {
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
struct WorkerService {
    svc: ServiceMessage,
}

/// Manager for MiniApp services
pub(crate) struct MiniAppServiceManager {
    sender: Sender<ServiceMessage>,
    worker_assignments: HashMap<String, usize>, // appid -> worker_id
    free_workers: Vec<usize>,                   // available worker ids
}

impl MiniAppServiceManager {
    fn new(sender: Sender<ServiceMessage>, worker_count: usize) -> Self {
        Self {
            sender,
            worker_assignments: HashMap::new(),
            free_workers: Vec::with_capacity(worker_count),
        }
    }

    // Initialize the free workers pool with worker IDs
    fn init_free_workers(&mut self, worker_ids: Vec<usize>) {
        self.free_workers = worker_ids;
    }

    /// Create a new mini app service
    pub fn create_app_svc(
        &mut self,
        miniapp: Arc<crate::miniapp::MiniApp>,
    ) -> Result<(), MiniAppError> {
        let appid = &miniapp.appid;

        // Check if app already exists
        if self.worker_assignments.contains_key(appid) {
            return Ok(());
        }

        // Check if we have free workers available
        if self.free_workers.is_empty() {
            return Err(MiniAppError::ResourceExhausted(
                "No available workers".to_string(),
            ));
        }

        // Get a free worker
        let worker_id = self.free_workers.pop().unwrap();

        // Update assignment
        self.worker_assignments.insert(appid.clone(), worker_id);

        // Send message with MiniApp reference
        self.sender
            .send(ServiceMessage::CreateMiniApp { miniapp })?;

        Ok(())
    }

    /// Create a new page service in an existing mini app
    pub fn create_page_svc(&self, appid: String, path: String) -> Result<(), MiniAppError> {
        self.sender
            .send(ServiceMessage::CreatePage { appid, path })?;
        Ok(())
    }

    /// Terminate a page service in a mini app
    pub fn terminate_page_svc(&self, appid: String, path: String) -> Result<(), MiniAppError> {
        self.sender
            .send(ServiceMessage::TerminatePage { appid, path })?;

        Ok(())
    }

    /// Terminate a mini app service
    pub fn terminate_app_svc(&mut self, appid: String) -> Result<(), MiniAppError> {
        // If we have this app, get its worker ID
        if let Some(worker_id) = self.worker_assignments.remove(&appid) {
            // Return worker to free pool
            self.free_workers.push(worker_id);
        }

        self.sender
            .send(ServiceMessage::TerminateMiniApp { appid })?;

        Ok(())
    }

    fn get_worker_id(&self, appid: &str) -> Option<usize> {
        self.worker_assignments.get(appid).copied()
    }

    fn add_miniapp(&mut self, appid: &str, worker_id: usize) {
        // Update assignment map
        self.worker_assignments.insert(appid.to_string(), worker_id);

        // Remove from free workers if present
        if let Some(pos) = self.free_workers.iter().position(|&id| id == worker_id) {
            self.free_workers.remove(pos);
        }
    }

    fn remove_miniapp(&mut self, appid: &str) -> Option<usize> {
        // If we have this app, get its worker ID and add back to free pool
        if let Some(worker_id) = self.worker_assignments.remove(appid) {
            self.free_workers.push(worker_id);
            return Some(worker_id);
        }
        None
    }

    /// Call a function on an App service
    pub fn app_svc(
        &self,
        appid: String,
        name: String,
        args: Option<String>,
    ) -> Result<(), MiniAppError> {
        self.sender
            .send(ServiceMessage::CallAppSvc { appid, name, args })?;

        Ok(())
    }

    /// Call a function on a Page service from the view layer
    pub fn handle_view_message(
        &self,
        appid: String,
        path: String,
        incoming: Arc<bridge::IncomingMessage>,
    ) -> Result<(), MiniAppError> {
        self.sender.send(ServiceMessage::CallPageSvc {
            appid,
            path,
            source: PageSvcSource::View { incoming },
        })?;

        Ok(())
    }

    /// Call a function on a Page service from native code
    pub fn invoke_page_function(
        &self,
        appid: String,
        path: String,
        name: String,
        args: Option<String>,
    ) -> Result<(), MiniAppError> {
        self.sender.send(ServiceMessage::CallPageSvc {
            appid,
            path,
            source: PageSvcSource::Native { name, args },
        })?;

        Ok(())
    }
}

// Handles a call to an App service function
async fn handle_app_service_call(
    worker_id: usize,
    ctx: &JSContext,
    appid: String,
    name: String,
    args: Option<String>,
) {
    if let Some(svc) = ctx.get_user_data::<MiniAppSvc>() {
        let svc_clone = svc.clone();
        let ctx_clone_for_task = ctx.clone();

        let task = async move {
            if let Err(e) = svc_clone.call(&ctx_clone_for_task, &name, args).await {
                error!(
                    "[Worker {}] App service '{}' call '{}' failed, Error: {}",
                    worker_id, appid, name, e
                );
            }
        };
        rong::spawn(task);
    } else {
        error!("[Worker {}] App service '{}' not loaded", worker_id, appid);
    }
}

// Handles a message from the view layer to a Page service
async fn handle_view_source(
    page_svc_ref: &PageSvc,
    incoming: Arc<bridge::IncomingMessage>,
) -> Result<(), MiniAppError> {
    page_svc_ref
        .as_bridge()
        .process_incoming_message(page_svc_ref, page_svc_ref, incoming)
        .await
}

// Handles a call from native code to a Page service function
async fn handle_native_source(page_svc: &PageSvc, name: String, args: Option<String>) {
    let ctx = page_svc.get_ctx();
    let page_svc_clone = page_svc.clone();
    let name_clone = name.clone();

    let task = async move {
        if let Err(e) = page_svc_clone
            .call_or_event_from_native(&ctx, &name, args.as_deref())
            .await
        {
            crate::error!("Page service call '{}' failed: {}", name_clone, e);
        }
    };
    rong::spawn(task);
}

/// The core logic for a persistent worker task.
/// This function is a handler for messages received by the worker.
async fn miniapp_service_handler(
    worker_id: usize,
    manager: Arc<Mutex<MiniAppServiceManager>>,
    runtime: JSRuntime,
    message: ServiceMessage,
    current_ctx: &mut Option<JSContext>,
) {
    match message {
        ServiceMessage::CreateMiniApp { miniapp } => {
            let ctx = runtime.context();

            // Store the MiniApp reference directly in JSContext user data
            ctx.set_user_data(miniapp.clone());

            // Create a HashMap for PageSvc instances and store it in JSContext
            let page_svc_map: Rc<RefCell<HashMap<String, PageSvc>>> =
                Rc::new(RefCell::new(HashMap::new()));
            ctx.set_user_data(page_svc_map.clone());

            // register Page, App and getApp function
            let _ = app::init(&ctx);
            let _ = page::init(&ctx);

            // Set console writer
            console::set_writer(Box::new(MiniAppCtx::new(miniapp.clone())));

            // Set file access guard to prevent cross-app file access
            fs::set_file_access_guard(Box::new(MiniAppCtx::new(miniapp.clone())));

            // Set network access guard to prevent unauthorized domain access
            http::set_network_access_guard(Box::new(MiniAppCtx::new(miniapp.clone())));

            let localstorage = miniapp.storage_dir.join(format!("{}.redb", miniapp.appid));
            if let Err(e) = storage::set_storage_path(localstorage) {
                info!("[Worker {}] failed to open localstorage: {}", worker_id, e)
                    .with_appid(miniapp.appid.clone());
            }

            let _ = rong_modules::init(&ctx);
            let _ = lx::init(&ctx);

            info!("[Worker {}] Created JS context", worker_id).with_appid(miniapp.appid.clone());

            let js = miniapp.lxapp_dir.join("logic.js");
            if js.exists() {
                if let Ok(js) = Source::from_path(&ctx, js).await {
                    match ctx.eval::<()>(js) {
                        Ok(_) => {
                            info!("[Worker {}] Successfully loaded logic JS", worker_id)
                                .with_appid(miniapp.appid.clone());
                        }
                        Err(e) => {
                            info!("[Worker {}] eval logic JS  failed: {}", worker_id, e)
                                .with_appid(miniapp.appid.clone());
                        }
                    }
                }
            } else {
                error!(
                    "[Worker {}] Not found JS file: '{}'",
                    worker_id,
                    js.display()
                )
                .with_appid(miniapp.appid.clone());
            }

            *current_ctx = Some(ctx.clone());

            // Only lock once to update app info
            {
                let mut manager_guard = manager.lock().unwrap();
                manager_guard.add_miniapp(&miniapp.appid, worker_id);
            }
        }
        ServiceMessage::TerminateMiniApp { appid } => {
            if current_ctx.is_some() {
                *current_ctx = None;

                // Only lock once to update app info
                {
                    let mut manager_guard = manager.lock().unwrap();
                    manager_guard.remove_miniapp(&appid);
                }

                info!("[Worker {}] Removed MiniApp context ", worker_id).with_appid(appid.clone());
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
                    let _ = page_svc
                        .call_or_event_from_native(ctx, "onUnload", None)
                        .await;

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
                            handle_native_source(&page_svc, name, args).await;
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

/// Initialize the MiniAppService system
/// Returns the MiniAppServiceManager for the caller to manage
pub(crate) fn init(num: usize) -> Arc<Mutex<MiniAppServiceManager>> {
    let (service_sender, service_receiver) = mpsc::channel::<ServiceMessage>();
    let service_receiver = Arc::new(Mutex::new(service_receiver));

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let worker_barrier = barrier.clone();

    // Create the manager with the controller and log sender
    let manager = Arc::new(Mutex::new(MiniAppServiceManager::new(
        service_sender.clone(),
        num,
    )));

    // Clone the manager to return at the end
    let result_manager = manager.clone();

    // Clone manager for use in thread
    let manager_clone = manager.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create worker runtime");

        // Run the worker setup and central message loop
        rt.block_on(async {
            let rong = Rong::<RongJS>::builder().with_num_workers(num).build();

            // Acquire all workers upfront
            let mut workers = Vec::with_capacity(num);
            for _i in 0..num {
                if let Ok(worker) = rong.get_worker().await {
                    workers.push(worker);
                }
            }

            // Create worker map for ID-based lookups
            let workers_map: HashMap<usize, Worker<RongJS>> =
                workers.iter().map(|w| (w.id(), w.clone())).collect();

            // Initialize free workers with all worker IDs
            {
                let mut manager = manager_clone.lock().unwrap();
                manager.init_free_workers(workers_map.keys().copied().collect());

                worker_barrier.wait();
            }

            // Set up tasks for each worker
            for worker in &workers {
                let worker_id = worker.id();
                let manager_c = manager_clone.clone();

                if worker
                    .spawn_future(async move |runtime, mut receiver| {
                        // JSContext it's not Sendable, so we don't hold appid-> JSContext map in
                        // MiniAppServiceManager
                        // a worker either has a current context or it doesn't
                        let mut current_ctx: Option<JSContext> = None;

                        while let Some(WorkerMessage::Custom(cmd)) = receiver.recv().await {
                            if let Ok(service) = cmd.downcast::<WorkerService>() {
                                miniapp_service_handler(
                                    worker_id,
                                    manager_c.clone(),
                                    runtime.clone(),
                                    service.svc,
                                    &mut current_ctx,
                                )
                                .await;
                            }
                        }

                        // Clean up context when worker is shutting down
                        current_ctx.take();

                        Ok(())
                    })
                    .is_err()
                {
                    error!("Failed to spawn worker {}", worker_id);
                }
            }

            let recv = service_receiver.clone();
            loop {
                match recv.lock().unwrap().recv() {
                    Ok(message) => {
                        match &message {
                            ServiceMessage::CreateMiniApp { miniapp } => {
                                let appid = &miniapp.appid;
                                // Find worker for appid and send message
                                if let Some(worker_id) =
                                    manager_clone.lock().unwrap().get_worker_id(appid)
                                {
                                    if let Some(worker) = workers_map.get(&worker_id) {
                                        let _ = worker.post_message(WorkerMessage::Custom(
                                            Box::new(WorkerService {
                                                svc: message.clone(),
                                            }),
                                        ));
                                    }
                                }
                            }
                            ServiceMessage::TerminateMiniApp { appid }
                            | ServiceMessage::CallAppSvc { appid, .. }
                            | ServiceMessage::CallPageSvc { appid, .. }
                            | ServiceMessage::TerminatePage { appid, .. }
                            | ServiceMessage::CreatePage { appid, .. } => {
                                // Find worker for appid and send message
                                if let Some(worker_id) =
                                    manager_clone.lock().unwrap().get_worker_id(appid)
                                {
                                    if let Some(worker) = workers_map.get(&worker_id) {
                                        let _ = worker.post_message(WorkerMessage::Custom(
                                            Box::new(WorkerService {
                                                svc: message.clone(),
                                            }),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        error!("Service message channel closed");
                        break;
                    }
                }
            }

            let _ = rong.join_all().await;
        });
    });

    barrier.wait();
    result_manager
}

/// Wrapper for MiniApp to implement external traits
struct MiniAppCtx {
    miniapp: Arc<MiniApp>,
}

impl MiniAppCtx {
    pub fn new(miniapp: Arc<MiniApp>) -> Self {
        Self { miniapp }
    }
}

impl std::fmt::Debug for MiniAppCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiniAppCtx")
            .field("appid", &self.miniapp.appid)
            .finish()
    }
}

impl console::ConsoleWriter for MiniAppCtx {
    fn write(&self, level: console::LogLevel, message: String) {
        let log = LogBuilder::new(LogTag::MiniAppServiceConsole, message);
        match level {
            console::LogLevel::Verbose => log
                .with_level(LogLevel::Verbose)
                .with_appid(self.miniapp.appid.clone()),
            console::LogLevel::Info => log
                .with_level(LogLevel::Info)
                .with_appid(self.miniapp.appid.clone()),
            console::LogLevel::Debug => log
                .with_level(LogLevel::Debug)
                .with_appid(self.miniapp.appid.clone()),
            console::LogLevel::Error => log
                .with_level(LogLevel::Error)
                .with_appid(self.miniapp.appid.clone()),
            console::LogLevel::Warn => log
                .with_level(LogLevel::Warn)
                .with_appid(self.miniapp.appid.clone()),
        };
    }

    fn is_tty(&self) -> bool {
        false
    }
}

impl fs::FileAccessGuard for MiniAppCtx {
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
            canonicalize(&self.miniapp.user_data_dir).map_err(|e| RongJSError::Error(e))?;
        let user_cache_canonical =
            canonicalize(&self.miniapp.user_cache_dir).map_err(|e| RongJSError::Error(e))?;

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

impl http::NetworkAccessGuard for MiniAppCtx {
    /// Check if the mini app has access to the specified domain
    /// Returns Ok(()) if access is granted, Err with error message if denied
    fn check_access(&self, domain: &str) -> JSResult<()> {
        if self.miniapp.is_domain_allowed(domain) {
            Ok(())
        } else {
            Err(RongJSError::Error(format!(
                "Access denied: domain '{}' is not allowed ",
                domain
            )))
        }
    }
}
