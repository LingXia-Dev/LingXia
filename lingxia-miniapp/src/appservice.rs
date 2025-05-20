use crate::AppController;
use crate::error::MiniAppError;
use crate::log::LogLevel;
use crate::page::Page;

use rong::{JSContext, JSObject, JSRuntime, Rong, RongJS, Source, Worker, WorkerMessage};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, mpsc};

mod app;
pub mod bridge;
mod page;
use app::MiniAppSvc;
use page::PageSvc;

/// Message type for MiniApp service system
#[derive(Clone)]
enum ServiceMessage {
    // Create a new miniapp service
    CreateMiniApp {
        appid: String,
        app_path: PathBuf,
    },
    // Delete an miniapp service
    TerminateMiniApp {
        appid: String,
    },
    // Create a new page service
    CreatePage {
        appid: String,
        page: Page,
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
    // Call function of Page service
    CallPageSvc {
        appid: String,
        path: String,
        incoming: Arc<bridge::IncomingMessage>,
    },
}

#[derive(Clone)]
struct WorkerService {
    svc: ServiceMessage,
}

struct MiniAppCtx {
    ctx: JSContext,
    app_path: PathBuf, // base Path of MiniApp
    svc: Option<MiniAppSvc>,
    page_svc: HashMap<String, PageSvc>,
}

#[derive(Debug)]
struct LogMessage {
    level: LogLevel,
    message: String,
}

/// Manager for MiniApp services
pub(crate) struct MiniAppServiceManager {
    sender: Sender<ServiceMessage>,
    worker_assignments: HashMap<String, usize>, // appid -> worker_id
    free_workers: Vec<usize>,                   // available worker ids
    log_sender: Sender<LogMessage>,
}

impl MiniAppServiceManager {
    fn new(
        sender: Sender<ServiceMessage>,
        worker_count: usize,
        log_sender: Sender<LogMessage>,
    ) -> Self {
        Self {
            sender,
            worker_assignments: HashMap::new(),
            free_workers: Vec::with_capacity(worker_count),
            log_sender,
        }
    }

    // Initialize the free workers pool with worker IDs
    fn init_free_workers(&mut self, worker_ids: Vec<usize>) {
        self.free_workers = worker_ids;
    }

    /// Create a new mini app service
    pub fn create_app_svc(&mut self, appid: String, app_path: PathBuf) -> Result<(), MiniAppError> {
        // Check if app already exists
        if self.worker_assignments.contains_key(&appid) {
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

        // Send message
        self.sender
            .send(ServiceMessage::CreateMiniApp { appid, app_path })?;

        Ok(())
    }

    /// Create a new page service in an existing mini app
    pub fn create_page_svc(&self, page: Page) -> Result<(), MiniAppError> {
        self.sender.send(ServiceMessage::CreatePage {
            appid: page.appid(),
            page,
        })?;
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

    // Get a log sender that can be passed to worker threads
    fn get_log_sender(&self) -> Sender<LogMessage> {
        self.log_sender.clone()
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

    /// Call a function on a Page service
    pub fn page_svc(
        &self,
        appid: String,
        path: String,
        incoming: Arc<bridge::IncomingMessage>,
    ) -> Result<(), MiniAppError> {
        self.sender.send(ServiceMessage::CallPageSvc {
            appid,
            path,
            incoming,
        })?;

        Ok(())
    }
}

/// The core logic for a persistent worker task.
/// This function is a handler for messages received by the worker.
async fn miniapp_service_handler(
    worker_id: usize,
    manager: Arc<Mutex<MiniAppServiceManager>>,
    runtime: JSRuntime,
    message: ServiceMessage,
    current_miniapp: &mut Option<MiniAppCtx>,
    log_sender: Sender<LogMessage>,
) {
    // Helper function for logging without locking
    let log = |level: LogLevel, message: &str| {
        let _ = log_sender.send(LogMessage {
            level,
            message: message.to_string(),
        });
    };

    match message {
        ServiceMessage::CreateMiniApp { appid, app_path } => {
            let ctx = runtime.context();

            let mut miniapp = MiniAppCtx {
                ctx: ctx.clone(),
                app_path: app_path.clone(),
                svc: None,
                page_svc: HashMap::new(),
            };

            // register Page, App and getApp function
            let _ = app::init(&ctx);
            let _ = page::init(&ctx);
            let _ = rong_modules::init(&ctx);

            log(
                LogLevel::Info,
                &format!(
                    "[Worker {}] Created JS context for MiniApp '{}'",
                    worker_id, appid
                ),
            );

            let js = app_path.join("app.js");
            if js.exists() {
                if let Ok(js) = Source::from_path(&ctx, js).await {
                    if let Ok(svc) = ctx.eval::<MiniAppSvc>(js) {
                        miniapp.svc = Some(svc);
                        log(
                            LogLevel::Info,
                            &format!(
                                "[Worker {}] Successfully loaded app JS for {}",
                                worker_id, appid
                            ),
                        );
                    }
                }
            } else {
                log(
                    LogLevel::Info,
                    &format!(
                        "[Worker {}] MiniApp '{}' has no JS file: '{}'",
                        worker_id,
                        appid,
                        js.display()
                    ),
                );
            }

            *current_miniapp = Some(miniapp);

            // Only lock once to update app info
            {
                let mut manager_guard = manager.lock().unwrap();
                manager_guard.add_miniapp(&appid, worker_id);
            }
        }
        ServiceMessage::TerminateMiniApp { appid } => {
            if current_miniapp.is_some() {
                *current_miniapp = None;

                // Only lock once to update app info
                {
                    let mut manager_guard = manager.lock().unwrap();
                    manager_guard.remove_miniapp(&appid);
                }

                log(
                    LogLevel::Info,
                    &format!(
                        "[Worker {}] Removed MiniApp context for '{}'",
                        worker_id, appid
                    ),
                );
            }
        }
        ServiceMessage::CreatePage { appid: _, page } => {
            if let Some(app_ctx) = current_miniapp.as_mut() {
                // Extract app ID and path from page
                let (appid, path) = (page.appid(), page.path());

                let page_js_path = app_ctx.app_path.join(&path).with_extension("js");
                let ctx = &app_ctx.ctx;

                if page_js_path.exists() {
                    if let Ok(js) = Source::from_path(ctx, &page_js_path).await {
                        if let Ok(obj) = ctx.eval::<JSObject>(js) {
                            if let Ok(mut svc) = obj.borrow_mut::<PageSvc>() {
                                svc.attach_page(page);

                                app_ctx.page_svc.insert(path.clone(), svc.clone());
                                log(
                                    LogLevel::Info,
                                    &format!(
                                        "[Worker {}] Successfully loaded page JS for {}/{}",
                                        worker_id, appid, path
                                    ),
                                );
                            } else {
                                log(
                                    LogLevel::Error,
                                    &format!(
                                        "[Worker {}] Failed to borrow PageSvc for {}/{}",
                                        worker_id, appid, path
                                    ),
                                );
                            }
                        } else {
                            log(
                                LogLevel::Error,
                                &format!(
                                    "[Worker {}] Failed to eval page JS for {}/{}",
                                    worker_id, appid, path
                                ),
                            );
                        }
                    }
                } else {
                    log(
                        LogLevel::Info,
                        &format!(
                            "[Worker {}] MiniApp '{}' has no JS file: '{}'",
                            worker_id,
                            appid,
                            page_js_path.display()
                        ),
                    );
                }
            }
        }
        ServiceMessage::TerminatePage { appid, path } => {
            if let Some(app_ctx) = current_miniapp.as_mut() {
                // Remove page from page_svc map
                if app_ctx.page_svc.remove(&path).is_some() {
                    log(
                        LogLevel::Info,
                        &format!(
                            "[Worker {}] Removed page '{}' from MiniApp '{}'",
                            worker_id, path, appid
                        ),
                    );
                }
            }
        }
        ServiceMessage::CallAppSvc { appid, name, args } => {
            if let Some(app_ctx) = current_miniapp.as_mut() {
                if let Some(svc) = &app_ctx.svc {
                    let svc_clone = svc.clone();
                    let ctx_clone_for_task = app_ctx.ctx.clone();
                    let log_sender_for_task = log_sender.clone();

                    let task = async move {
                        if let Err(e) = svc_clone.call(&ctx_clone_for_task, &name, args).await {
                            let _ = log_sender_for_task.send(LogMessage {
                                level: LogLevel::Error,
                                message: format!(
                                    "[Worker {}] App service '{}' call '{}' failed, Error: {}",
                                    worker_id, appid, name, e
                                ),
                            });
                        }
                    };
                    tokio::task::spawn_local(task);
                } else {
                    log(
                        LogLevel::Error,
                        &format!("[Worker {}] App service '{}' not loaded", worker_id, appid),
                    );
                }
            }
        }
        ServiceMessage::CallPageSvc {
            appid: _,
            path,
            incoming,
        } => {
            if let Some(app_ctx) = current_miniapp.as_mut() {
                if let Some(page_svc_ref) = app_ctx.page_svc.get_mut(&path) {
                    let page_svc_clone = page_svc_ref.clone();
                    let ctx_clone_for_task = app_ctx.ctx.clone();
                    let log_sender_for_task = log_sender.clone();

                    if let Err(e) = page_svc_ref
                        .as_bridge()
                        .process_incoming_message(incoming, async move |_type, name, payload, callbackid| {
                            // ignore this event currently
                            if name == "LingXiaPortReady" {
                                return;
                            }

                            let name_owned = name.clone();
                            let payload_owned = payload.clone();

                            // All captures for the spawned task are now owned or 'static.
                            let task = async move {
                                if let Err(e) = page_svc_clone.call(
                                    &ctx_clone_for_task,
                                    &name_owned,
                                    payload_owned.as_deref()
                                ).await {
                                    let _ = log_sender_for_task.send(LogMessage {
                                        level: LogLevel::Error,
                                        message: format!(
                                            "[Worker {}] Exec Page {} service '{}' failed, Error: {}",
                                            worker_id, path, name_owned, e
                                        ),
                                    });
                                }
                            };
                            tokio::task::spawn_local(task);
                        })
                        .await
                    {
                        log(
                            LogLevel::Error,
                            &format!(
                                "[Worker {}] Handle incoming message error: {}",
                                worker_id, e
                            ),
                        );
                    }
                } else {
                    log(
                        LogLevel::Error,
                        &format!("[Worker {}] Page service '{}' not loaded", worker_id, path),
                    );
                }
            }
        }
    }
}

/// Initialize the MiniAppService system
/// Returns the MiniAppServiceManager for the caller to manage
pub(crate) fn init<T: AppController + 'static>(
    controller: Arc<T>,
    num: usize,
) -> Arc<Mutex<MiniAppServiceManager>> {
    let (service_sender, service_receiver) = mpsc::channel::<ServiceMessage>();
    let service_receiver = Arc::new(Mutex::new(service_receiver));

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let worker_barrier = barrier.clone();

    // Create a dedicated channel for logging
    let (log_sender, log_receiver) = mpsc::channel::<LogMessage>();

    // Create the manager with the controller and log sender
    let manager = Arc::new(Mutex::new(MiniAppServiceManager::new(
        service_sender.clone(),
        num,
        log_sender.clone(),
    )));

    // Clone the manager to return at the end
    let result_manager = manager.clone();

    // Clone manager for use in thread
    let manager_clone = manager.clone();

    // Start log processor thread
    let controller_for_logs = controller.clone();
    std::thread::spawn(move || {
        // Process log messages in a dedicated thread
        while let Ok(log_msg) = log_receiver.recv() {
            controller_for_logs.log(log_msg.level, &log_msg.message);
        }
    });

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

            // Get log sender for this thread
            let main_log_sender = {
                let manager = manager_clone.lock().unwrap();
                manager.get_log_sender()
            };

            // Set up tasks for each worker
            for worker in &workers {
                let worker_id = worker.id();
                let manager_c = manager_clone.clone();
                let worker_log_sender = {
                    let manager = manager_clone.lock().unwrap();
                    manager.get_log_sender()
                };

                if worker
                    .spawn_future(async move |runtime, mut receiver| {
                        // JSContext it's not Sendable, so we don't hold appid-> MiniAppCtx map in
                        // MiniAppServiceManager
                        // a worker either has a current miniapp context or it doesn't
                        let mut current_miniapp: Option<MiniAppCtx> = None;

                        while let Some(WorkerMessage::Custom(cmd)) = receiver.recv().await {
                            if let Ok(service) = cmd.downcast::<WorkerService>() {
                                miniapp_service_handler(
                                    worker_id,
                                    manager_c.clone(),
                                    runtime.clone(),
                                    service.svc,
                                    &mut current_miniapp,
                                    worker_log_sender.clone(),
                                )
                                .await;
                            }
                        }

                        // Clean up context when worker is shutting down
                        current_miniapp.take();

                        Ok(())
                    })
                    .is_err()
                {
                    let _ = main_log_sender.send(LogMessage {
                        level: LogLevel::Error,
                        message: format!("Failed to spawn worker {}", worker_id),
                    });
                }
            }

            let recv = service_receiver.clone();
            loop {
                match recv.lock().unwrap().recv() {
                    Ok(message) => {
                        match &message {
                            ServiceMessage::CreateMiniApp { appid, .. }
                            | ServiceMessage::TerminateMiniApp { appid, .. }
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
                        let _ = main_log_sender.send(LogMessage {
                            level: LogLevel::Error,
                            message: "Service message channel closed".to_string(),
                        });
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
