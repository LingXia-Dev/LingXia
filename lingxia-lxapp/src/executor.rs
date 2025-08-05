use crate::appservice::{
    LxAppServiceManager, ServiceMessage, WorkerService, lxapp_service_handler,
};
use crate::{error, info};

use rong::{JSContext, Rong, RongJS, Worker, WorkerMessage};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// LxApp Async Executor
///
/// This is the central async task executor for LxApp, managing:
/// - Tokio runtime
/// - Rong JS workers
/// - Message dispatching between different subsystems
pub struct LxAppExecutor {
    /// Handle to the background executor thread
    _executor_handle: std::thread::JoinHandle<()>,
}

impl LxAppExecutor {
    /// Initialize the LxApp executor system
    ///
    /// # Arguments
    /// * `num_workers` - Number of JS workers to create
    /// * `service_manager` - The service manager to integrate with
    ///
    /// # Returns
    /// * `LxAppExecutor` - The executor instance
    pub fn init(num_workers: usize, service_manager: Arc<Mutex<LxAppServiceManager>>) -> Self {
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let executor_barrier = barrier.clone();

        let executor_handle = std::thread::spawn(move || {
            Self::run_executor(num_workers, service_manager, executor_barrier);
        });

        // Wait for executor to be ready
        barrier.wait();

        Self {
            _executor_handle: executor_handle,
        }
    }

    /// Main executor loop - runs in dedicated thread
    fn run_executor(
        num_workers: usize,
        service_manager: Arc<Mutex<LxAppServiceManager>>,
        barrier: Arc<std::sync::Barrier>,
    ) {
        // Create tokio runtime
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create executor runtime");

        // Run the main executor logic
        rt.block_on(async {
            Self::run_async_executor(num_workers, service_manager, barrier).await;
        });
    }

    /// Async executor main logic
    async fn run_async_executor(
        num_workers: usize,
        service_manager: Arc<Mutex<LxAppServiceManager>>,
        barrier: Arc<std::sync::Barrier>,
    ) {
        info!("Starting LxApp async executor with {} workers", num_workers);

        // Initialize Rong JS engine with workers
        let rong = Rong::<RongJS>::builder()
            .with_num_workers(num_workers)
            .build();

        // Acquire all workers upfront
        let mut workers = Vec::with_capacity(num_workers);
        for _i in 0..num_workers {
            if let Ok(worker) = rong.get_worker().await {
                workers.push(worker);
            }
        }

        // Create worker map for ID-based lookups
        let workers_map: HashMap<usize, Worker<RongJS>> =
            workers.iter().map(|w| (w.id(), w.clone())).collect();

        // Initialize service manager with worker IDs
        {
            let mut manager = service_manager.lock().unwrap();
            manager.init_free_workers(workers_map.keys().copied().collect());

            // Signal that executor is ready
            barrier.wait();
        }

        // Set up JS worker tasks
        Self::setup_js_workers(&workers, service_manager.clone()).await;

        // Start message dispatch loop
        Self::run_message_dispatch_loop(service_manager, workers_map).await;

        // Clean up
        let _ = rong.join_all().await;
        info!("LxApp async executor shutdown complete");
    }

    /// Set up individual JS worker tasks
    async fn setup_js_workers(
        workers: &[Worker<RongJS>],
        service_manager: Arc<Mutex<LxAppServiceManager>>,
    ) {
        for worker in workers {
            let worker_id = worker.id();
            let manager_clone = service_manager.clone();

            if worker
                .spawn_future(async move |runtime, mut receiver| {
                    // JSContext is not Send, so each worker maintains its own context
                    let mut current_ctx: Option<JSContext> = None;

                    while let Some(WorkerMessage::Custom(cmd)) = receiver.recv().await {
                        if let Ok(service) = cmd.downcast::<WorkerService>() {
                            lxapp_service_handler(
                                worker_id,
                                manager_clone.clone(),
                                runtime.clone(),
                                service.svc,
                                &mut current_ctx,
                            )
                            .await;
                        }
                    }

                    // Clean up context when worker shuts down
                    current_ctx.take();
                    Ok(())
                })
                .is_err()
            {
                error!("Failed to spawn JS worker {}", worker_id);
            }
        }
    }

    /// Main message dispatch loop
    async fn run_message_dispatch_loop(
        service_manager: Arc<Mutex<LxAppServiceManager>>,
        workers_map: HashMap<usize, Worker<RongJS>>,
    ) {
        // Get the service receiver from the manager
        let service_receiver = {
            let manager = service_manager.lock().unwrap();
            manager.get_service_receiver()
        };

        loop {
            match service_receiver.lock().unwrap().recv() {
                Ok(message) => {
                    Self::dispatch_message(message, &service_manager, &workers_map).await;
                }
                Err(_) => {
                    error!("Service message channel closed");
                    break;
                }
            }
        }
    }

    /// Dispatch a single message to appropriate worker
    async fn dispatch_message(
        message: ServiceMessage,
        service_manager: &Arc<Mutex<LxAppServiceManager>>,
        workers_map: &HashMap<usize, Worker<RongJS>>,
    ) {
        match &message {
            ServiceMessage::CreateLxApp { lxapp } => {
                let appid = &lxapp.appid;
                if let Some(worker_id) = service_manager.lock().unwrap().get_worker_id(appid) {
                    if let Some(worker) = workers_map.get(&worker_id) {
                        let _ =
                            worker.post_message(WorkerMessage::Custom(Box::new(WorkerService {
                                svc: message,
                            })));
                    }
                }
            }
            ServiceMessage::TerminateLxApp { appid }
            | ServiceMessage::CallAppSvc { appid, .. }
            | ServiceMessage::CallPageSvc { appid, .. }
            | ServiceMessage::TerminatePage { appid, .. }
            | ServiceMessage::CreatePage { appid, .. } => {
                if let Some(worker_id) = service_manager.lock().unwrap().get_worker_id(appid) {
                    if let Some(worker) = workers_map.get(&worker_id) {
                        let _ =
                            worker.post_message(WorkerMessage::Custom(Box::new(WorkerService {
                                svc: message,
                            })));
                    }
                }
            }
        }
    }
}
