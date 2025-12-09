use crate::appservice::bridge_events::BridgeEvent;
use crate::appservice::{ServiceMessage, WorkerService, lxapp_service_handler};
use crate::event::AppServiceEvent;
use crate::event::PageServiceEvent;
use crate::{LxAppError, error, info};

use rong::{JSContext, Rong, RongJS, Worker, WorkerMessage};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, mpsc};
use tokio::sync::oneshot::Sender;

/// LxApp Async Executor
///
/// This is the central async task executor for LxApp, managing:
/// - Tokio runtime
/// - Rong JS workers
/// - Worker assignment and lifecycle
/// - Message dispatching between different subsystems
/// - Enforces core principle: one appid corresponds to one JS runtime (worker/thread)
pub struct LxAppExecutor {
    /// Message sender for communicating with workers
    sender: mpsc::Sender<ServiceMessage>,
    /// Mapping: LxApp instance pointer -> worker_id (object-identity routing)
    instance_assignments: Arc<Mutex<HashMap<usize, usize>>>,
    /// Available worker IDs for new mini-apps (FIFO)
    free_workers: Arc<Mutex<VecDeque<usize>>>,
}

impl LxAppExecutor {
    /// Initialize the LxApp executor system
    /// This is the main entry point for the entire LxApp system
    ///
    /// # Arguments
    /// * `num_workers` - Number of JS workers to create
    ///
    /// # Returns
    /// * `Arc<LxAppExecutor>` - The executor instance
    pub fn init(num_workers: usize) -> Arc<Self> {
        // Create message channel for worker communication
        let (sender, receiver) = mpsc::channel::<ServiceMessage>();
        let receiver = Arc::new(Mutex::new(receiver));

        // Initialize worker assignment tracking
        // Instance mapping only (no appid mapping)
        let instance_assignments = Arc::new(Mutex::new(HashMap::new()));
        let free_workers = Arc::new(Mutex::new(VecDeque::with_capacity(num_workers)));
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let executor_barrier = barrier.clone();

        // Clone references for the executor thread
        let receiver_clone = receiver.clone();
        let instance_assignments_clone = instance_assignments.clone();
        let free_workers_clone = free_workers.clone();

        std::thread::spawn(move || {
            Self::run_executor(
                num_workers,
                receiver_clone,
                instance_assignments_clone,
                free_workers_clone,
                executor_barrier,
            );
        });

        // Wait for executor to be ready
        barrier.wait();

        Arc::new(Self {
            sender,
            instance_assignments,
            free_workers,
        })
    }

    /// Main executor loop - runs in dedicated thread
    fn run_executor(
        num_workers: usize,
        receiver: Arc<Mutex<mpsc::Receiver<ServiceMessage>>>,
        instance_assignments: Arc<Mutex<HashMap<usize, usize>>>,
        free_workers: Arc<Mutex<VecDeque<usize>>>,
        barrier: Arc<std::sync::Barrier>,
    ) {
        // Create tokio runtime
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create executor runtime");

        // Run the main executor logic
        rt.block_on(async {
            Self::run_async_executor(
                num_workers,
                receiver,
                instance_assignments,
                free_workers,
                barrier,
            )
            .await;
        });
    }

    /// Async executor main logic
    async fn run_async_executor(
        num_workers: usize,
        receiver: Arc<Mutex<mpsc::Receiver<ServiceMessage>>>,
        instance_assignments: Arc<Mutex<HashMap<usize, usize>>>,
        free_workers: Arc<Mutex<VecDeque<usize>>>,
        barrier: Arc<std::sync::Barrier>,
    ) {
        info!("Starting LxApp async executor with {} workers", num_workers);

        // Initialize Rong JS engine with workers
        let rong = Rong::<RongJS>::builder()
            .with_service_threads(3)
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

        // Initialize free workers with worker IDs
        {
            let mut free_workers_guard = free_workers.lock().unwrap();
            let mut ids: Vec<usize> = workers_map.keys().copied().collect();
            ids.sort_unstable();
            *free_workers_guard = VecDeque::from(ids);

            // Signal that executor is ready
            barrier.wait();
        }

        // Set up JS worker tasks
        Self::setup_js_workers(&workers).await;

        // Start message dispatch loop
        Self::run_message_dispatch_loop(receiver, instance_assignments, workers_map).await;

        // Clean up
        let _ = rong.join_all().await;
        info!("LxApp async executor shutdown complete");
    }

    /// Set up individual JS worker tasks
    async fn setup_js_workers(workers: &[Worker<RongJS>]) {
        for worker in workers {
            let worker_id = worker.id();

            if worker
                .spawn_future(async move |runtime, mut receiver| {
                    // JSContext is not Send, so each worker maintains its own context
                    let mut current_ctx: Option<JSContext> = None;

                    while let Some(WorkerMessage::Custom(cmd)) = receiver.recv().await {
                        if let Ok(service) = cmd.downcast::<WorkerService>() {
                            lxapp_service_handler(
                                worker_id,
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
        receiver: Arc<Mutex<mpsc::Receiver<ServiceMessage>>>,
        instance_assignments: Arc<Mutex<HashMap<usize, usize>>>,
        workers_map: HashMap<usize, Worker<RongJS>>,
    ) {
        loop {
            let msg = receiver.lock().unwrap().recv();
            match msg {
                Ok(message) => {
                    Self::dispatch_message(message, &instance_assignments, &workers_map).await;
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
        instance_assignments: &Arc<Mutex<HashMap<usize, usize>>>,
        workers_map: &HashMap<usize, Worker<RongJS>>,
    ) {
        let appid = match &message {
            ServiceMessage::CreateAppSvc { lxapp, .. } => &lxapp.appid,
            ServiceMessage::TerminateAppSvc { lxapp, .. } => &lxapp.appid,
            ServiceMessage::TerminatePage { lxapp, .. } => &lxapp.appid,
            ServiceMessage::CallAppSvcEvent { lxapp, .. } => &lxapp.appid,
            ServiceMessage::CallPageSvc { lxapp, .. } => &lxapp.appid,
            ServiceMessage::CreatePage { lxapp, .. } => &lxapp.appid,
            ServiceMessage::CallPageSvcEvent { lxapp, .. } => &lxapp.appid,
            ServiceMessage::DispatchBridgeEvent { lxapp, .. } => &lxapp.appid,
        };

        // Resolve target worker strictly from instance mapping (object identity)
        let instance_key: Option<usize> = match &message {
            ServiceMessage::CreateAppSvc { lxapp, .. }
            | ServiceMessage::TerminateAppSvc { lxapp, .. }
            | ServiceMessage::TerminatePage { lxapp, .. }
            | ServiceMessage::CallAppSvcEvent { lxapp, .. }
            | ServiceMessage::CallPageSvc { lxapp, .. }
            | ServiceMessage::CreatePage { lxapp, .. }
            | ServiceMessage::CallPageSvcEvent { lxapp, .. }
            | ServiceMessage::DispatchBridgeEvent { lxapp, .. } => {
                Some(lxapp.as_ref() as *const _ as usize)
            }
        };

        let target_worker_id =
            instance_key.and_then(|k| instance_assignments.lock().unwrap().get(&k).copied());

        if let Some(worker_id) = target_worker_id {
            if let Some(worker) = workers_map.get(&worker_id) {
                let _ = worker.post_message(WorkerMessage::Custom(Box::new(WorkerService {
                    svc: message,
                })));
            }
        } else {
            // No instance mapping found; drop message to avoid misrouting.
            error!(
                "No worker mapping for LxApp instance (appid: {}) while dispatching message",
                appid
            );
        }
    }

    /// Create a new lxapp service (worker reads session id from LxApp state).
    pub fn create_app_svc(&self, lxapp: Arc<crate::lxapp::LxApp>) -> Result<(), LxAppError> {
        crate::appservice::create_app_svc(
            lxapp,
            &self.sender,
            &self.instance_assignments,
            &self.free_workers,
        )
    }

    /// Terminate a lxapp service for a specific instance.
    pub fn terminate_app_svc(&self, lxapp: Arc<crate::lxapp::LxApp>) -> Result<(), LxAppError> {
        crate::appservice::terminate_app_svc(
            lxapp,
            &self.sender,
            &self.instance_assignments,
            &self.free_workers,
        )
    }

    // ACK-based helpers removed; use LxApp state subscriptions instead.

    /// Create a new page service in an existing lxapp and notify when ready
    pub fn create_page_svc_with_ack(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        ack_tx: Sender<()>,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::CreatePage {
            lxapp,
            path,
            ack_tx,
        })?;
        Ok(())
    }

    /// Terminate a page service by object identity.
    pub(crate) fn terminate_page_svc(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
    ) -> Result<(), LxAppError> {
        self.sender
            .send(ServiceMessage::TerminatePage { lxapp, path })?;
        Ok(())
    }

    /// Call an AppService event (typed)
    pub fn call_app_service_event(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        event: AppServiceEvent,
        args: Option<String>,
    ) -> Result<(), LxAppError> {
        self.sender
            .send(ServiceMessage::CallAppSvcEvent { lxapp, event, args })?;
        Ok(())
    }

    /// Handle a message from the view layer to a Page service
    pub fn handle_view_message(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        incoming: Arc<crate::appservice::bridge::IncomingMessage>,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::CallPageSvc {
            lxapp,
            path,
            source: crate::appservice::PageSvcSource::View { incoming },
        })?;
        Ok(())
    }

    /// Call a function on a Page service from native code
    pub fn call_page_service(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        name: String,
        args: Option<String>,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::CallPageSvc {
            lxapp,
            path,
            source: crate::appservice::PageSvcSource::Native { name, args },
        })?;
        Ok(())
    }

    /// Call a typed page event on a Page service
    pub fn call_page_service_event(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        event: PageServiceEvent,
        args: Option<String>,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::CallPageSvcEvent {
            lxapp,
            path,
            event,
            args,
        })?;
        Ok(())
    }

    /// Dispatch a native -> JS event through the app worker.
    pub(crate) fn dispatch_bridge_event(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        event: BridgeEvent,
    ) -> Result<(), LxAppError> {
        self.sender
            .send(ServiceMessage::DispatchBridgeEvent { lxapp, event })?;
        Ok(())
    }
}
