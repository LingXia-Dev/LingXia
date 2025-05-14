use crate::AppController;
use crate::error::MiniAppError;
use crate::log::LogLevel;

use rong::{JSContext, JSRuntime, Rong, RongJS, Source, Worker, WorkerMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, mpsc};

mod app;
mod page;
use app::MiniAppSvc;
use page::PageSvc;

/// Message type for MiniApp service system
#[derive(Debug, Clone, Serialize, Deserialize)]
enum ServiceMessage {
    // Create a new miniapp service
    CreateMiniApp { appid: String, app_path: PathBuf },
    // Delete an miniapp service
    TerminateMiniApp { appid: String },
    // Create a new page service
    CreatePage { appid: String, path: String },
    // Delete a page service
    TerminatePage { appid: String, path: String },
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
    pub fn create_app_svc(&mut self, appid: &str, app_path: PathBuf) -> Result<(), MiniAppError> {
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
        self.worker_assignments.insert(appid.to_string(), worker_id);

        // Send message
        self.sender.send(ServiceMessage::CreateMiniApp {
            appid: appid.to_string(),
            app_path,
        })?;

        Ok(())
    }

    /// Create a new page service in an existing mini app
    pub fn create_page_svc(&self, appid: &str, path: &str) -> Result<(), MiniAppError> {
        self.sender.send(ServiceMessage::CreatePage {
            appid: appid.to_string(),
            path: path.to_string(),
        })?;

        Ok(())
    }

    /// Terminate a page service in a mini app
    pub fn terminate_page_svc(&self, appid: &str, path: &str) -> Result<(), MiniAppError> {
        self.sender.send(ServiceMessage::TerminatePage {
            appid: appid.to_string(),
            path: path.to_string(),
        })?;

        Ok(())
    }

    /// Terminate a mini app service
    pub fn terminate_app_svc(&mut self, appid: &str) -> Result<(), MiniAppError> {
        // If we have this app, get its worker ID
        if let Some(worker_id) = self.worker_assignments.remove(appid) {
            // Return worker to free pool
            self.free_workers.push(worker_id);
        }

        self.sender.send(ServiceMessage::TerminateMiniApp {
            appid: appid.to_string(),
        })?;

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
}

/// The core logic for a persistent worker task.
/// This function is a handler for messages received by the worker.
async fn miniapp_service_handler(
    worker_id: usize,
    manager: Arc<Mutex<MiniAppServiceManager>>,
    runtime: JSRuntime,
    cmd_str: String,
    miniapp_ctx: &mut HashMap<String, MiniAppCtx>,
    log_sender: Sender<LogMessage>,
) {
    // Helper function for logging without locking
    let log = |level: LogLevel, message: &str| {
        let _ = log_sender.send(LogMessage {
            level,
            message: message.to_string(),
        });
    };

    if let Ok(message) = serde_json::from_str::<ServiceMessage>(&cmd_str) {
        match message {
            ServiceMessage::CreateMiniApp { appid, app_path } => {
                let ctx = runtime.context();

                // Create and store MiniAppCtx containing both JSContext and app_path
                let mut local_ctx = MiniAppCtx {
                    ctx: ctx.clone(),
                    app_path: app_path.clone(),
                    svc: None,
                    page_svc: HashMap::new(),
                };

                // register Page, App and getApp function
                let _ = app::init(&ctx);
                let _ = page::init(&ctx);

                let js = app_path.join("app.js");
                if js.exists() {
                    if let Ok(js) = Source::from_path(&ctx, js).await {
                        if let Ok(svc) = ctx.eval::<MiniAppSvc>(js) {
                            local_ctx.svc = Some(svc);
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

                miniapp_ctx.insert(appid.clone(), local_ctx);

                // Only lock once to update app info
                {
                    let mut manager_guard = manager.lock().unwrap();
                    manager_guard.add_miniapp(&appid, worker_id);
                }

                log(
                    LogLevel::Info,
                    &format!(
                        "[Worker {}] Created JS context for MiniApp '{}'",
                        worker_id, appid
                    ),
                );
            }
            ServiceMessage::TerminateMiniApp { appid } => {
                if miniapp_ctx.remove(&appid).is_some() {
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
            ServiceMessage::CreatePage { appid, path } => {
                if let Some(app_ctx) = miniapp_ctx.get_mut(&appid) {
                    let page_js_path = app_ctx.app_path.join(&path).with_extension("js");
                    let ctx = &app_ctx.ctx;

                    if page_js_path.exists() {
                        if let Ok(js) = Source::from_path(ctx, &page_js_path).await {
                            match ctx.eval::<PageSvc>(js) {
                                Ok(page_svc) => {
                                    // Add the page service to the map
                                    app_ctx.page_svc.insert(path.clone(), page_svc);
                                    log(
                                        LogLevel::Info,
                                        &format!(
                                            "[Worker {}] Successfully loaded page JS for {}/{}",
                                            worker_id, appid, path
                                        ),
                                    );
                                }
                                Err(e) => {
                                    log(
                                        LogLevel::Error,
                                        &format!(
                                            "[Worker {}] Failed to eval page JS: {}",
                                            worker_id, e
                                        ),
                                    );
                                }
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
                if let Some(app_ctx) = miniapp_ctx.get_mut(&appid) {
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
        }
    } else {
        log(
            LogLevel::Warn,
            &format!("[Worker {}] Invalid message format: {}", worker_id, cmd_str),
        );
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
                        // Contexts are stored in worker thread, since it's not Sendable
                        let mut miniapp_ctxs = HashMap::<String, MiniAppCtx>::new();

                        while let Some(WorkerMessage::String(cmd_str)) = receiver.recv().await {
                            miniapp_service_handler(
                                worker_id,
                                manager_c.clone(),
                                runtime.clone(),
                                cmd_str,
                                &mut miniapp_ctxs,
                                worker_log_sender.clone(),
                            )
                            .await;
                        }

                        // Clean up contexts when worker is shutting down
                        if !miniapp_ctxs.is_empty() {
                            miniapp_ctxs.clear();
                        }

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
                            ServiceMessage::CreateMiniApp { appid, .. } => {
                                // Get worker ID from manager record
                                if let Some(worker_id) =
                                    manager_clone.lock().unwrap().get_worker_id(appid)
                                {
                                    // Send message to worker
                                    if let Some(worker) = workers_map.get(&worker_id) {
                                        if let Ok(cmd_str) = serde_json::to_string(&message) {
                                            let _ =
                                                worker.post_message(WorkerMessage::String(cmd_str));
                                        }
                                    }
                                }
                            }
                            ServiceMessage::TerminateMiniApp { appid } => {
                                // Get worker ID and send message
                                if let Some(worker_id) =
                                    manager_clone.lock().unwrap().get_worker_id(appid)
                                {
                                    if let Some(worker) = workers_map.get(&worker_id) {
                                        if let Ok(cmd_str) = serde_json::to_string(&message) {
                                            let _ =
                                                worker.post_message(WorkerMessage::String(cmd_str));
                                        }
                                    }
                                }
                            }
                            ServiceMessage::CreatePage { appid, .. }
                            | ServiceMessage::TerminatePage { appid, .. } => {
                                // Find worker for appid and send message
                                if let Some(worker_id) =
                                    manager_clone.lock().unwrap().get_worker_id(appid)
                                {
                                    if let Some(worker) = workers_map.get(&worker_id) {
                                        if let Ok(cmd_str) = serde_json::to_string(&message) {
                                            let _ =
                                                worker.post_message(WorkerMessage::String(cmd_str));
                                        }
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
