use crate::appservice::event_bus::AppBusEvent;
use crate::appservice::js_runtime::{
    PageSvcSource, ServiceMessage, WorkerService, create_app_svc, lxapp_service_handler,
    terminate_app_svc,
};
use crate::lifecycle::AppServiceEvent;
use crate::lifecycle::PageServiceEvent;
use crate::{LxAppError, error, info};

use rong::{JSContext, Rong, RongJS, TaskHandle, TaskMessage, Worker};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, mpsc};
use tokio::sync::oneshot::{self, Sender};

/// LxApp Async Executor
///
/// This is the central async task executor for LxApp, managing:
/// - Tokio runtime
/// - Rong JS workers
/// - Worker assignment and lifecycle
/// - Message dispatching between different subsystems
/// - Enforces core principle: one appid corresponds to one JS runtime (worker/thread)
pub struct LxAppWorkers {
    /// Message sender for communicating with workers
    sender: mpsc::Sender<ServiceMessage>,
    /// Mapping: LxApp instance pointer -> worker_id (object-identity routing)
    instance_assignments: Arc<Mutex<HashMap<usize, usize>>>,
    /// Available worker IDs for new mini-apps (FIFO)
    free_workers: Arc<Mutex<VecDeque<usize>>>,
}

impl LxAppWorkers {
    /// Initialize the LxApp executor system
    /// This is the main entry point for the entire LxApp system
    ///
    /// # Arguments
    /// * `num_workers` - Number of JS workers to create
    ///
    /// # Returns
    /// * `Arc<LxAppWorkers>` - The worker runtime instance
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

        // Initialize the shared Rong worker pool. LxApp keeps its own dedicated
        // app->worker assignment layer on top, so we explicitly choose `shared`
        // here instead of relying on Rong placement semantics.
        let rong = Rong::<RongJS>::builder()
            .shared()
            .workers(num_workers)
            .build()
            .expect("Failed to build shared Rong worker pool");
        let workers = rong.workers();

        // Start one long-lived service loop per Rong worker.
        let worker_tasks = Self::setup_js_workers(&workers).await;

        // Initialize free workers with worker IDs that successfully started.
        {
            let mut free_workers_guard = free_workers.lock().unwrap();
            let mut ids: Vec<usize> = worker_tasks.keys().copied().collect();
            ids.sort_unstable();
            *free_workers_guard = VecDeque::from(ids);

            // Signal that executor is ready
            barrier.wait();
        }

        // Start message dispatch loop
        Self::run_message_dispatch_loop(receiver, instance_assignments, worker_tasks).await;

        // Clean up
        let _ = rong.join().await;
        info!("LxApp async executor shutdown complete");
    }

    /// Set up individual JS worker tasks
    async fn setup_js_workers(workers: &[Worker<RongJS>]) -> HashMap<usize, TaskHandle<()>> {
        let mut worker_tasks = HashMap::with_capacity(workers.len());
        for worker in workers {
            let worker_id = worker.id();

            match worker
                .spawn(async move |runtime, mut receiver| -> rong::JSResult<()> {
                    // JSContext is not Send, so each worker maintains its own context
                    let mut current_ctx: Option<JSContext> = None;

                    while let Some(TaskMessage::Custom(cmd)) = receiver.recv().await {
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
                .await
            {
                Ok(task) => {
                    worker_tasks.insert(worker_id, task);
                }
                Err(err) => {
                    error!("Failed to spawn JS worker {}: {}", worker_id, err);
                }
            }
        }
        worker_tasks
    }

    /// Main message dispatch loop
    async fn run_message_dispatch_loop(
        receiver: Arc<Mutex<mpsc::Receiver<ServiceMessage>>>,
        instance_assignments: Arc<Mutex<HashMap<usize, usize>>>,
        worker_tasks: HashMap<usize, TaskHandle<()>>,
    ) {
        loop {
            let msg = receiver.lock().unwrap().recv();
            match msg {
                Ok(message) => {
                    Self::dispatch_message(message, &instance_assignments, &worker_tasks).await;
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
        worker_tasks: &HashMap<usize, TaskHandle<()>>,
    ) {
        let appid = match &message {
            ServiceMessage::CreateAppSvc { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::TerminateAppSvc { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::TerminatePage { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::CallAppSvcEvent { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::CallPageSvc { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::CreatePage { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::CallPageSvcEvent { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::DispatchAppBusEvent { lxapp, .. } => lxapp.appid.clone(),
            ServiceMessage::Eval { lxapp, .. } => lxapp.appid.clone(),
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
            | ServiceMessage::DispatchAppBusEvent { lxapp, .. }
            | ServiceMessage::Eval { lxapp, .. } => Some(lxapp.as_ref() as *const _ as usize),
        };

        let target_worker_id =
            instance_key.and_then(|k| instance_assignments.lock().unwrap().get(&k).copied());

        if let Some(worker_id) = target_worker_id {
            if let Some(worker_task) = worker_tasks.get(&worker_id) {
                if let Err(err) = worker_task
                    .send(TaskMessage::Custom(Box::new(WorkerService {
                        svc: message,
                    })))
                    .await
                {
                    error!(
                        "Failed to send service message to worker {} for appid {}: {}",
                        worker_id, appid, err
                    );
                }
            } else {
                error!(
                    "Worker {} not found in task map for appid {}",
                    worker_id, appid
                );
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
        if !lxapp.logic_enabled() {
            info!("Logic disabled in lxapp.json; skipping worker startup")
                .with_appid(lxapp.appid.clone());
            return Ok(());
        }
        if !crate::js_appservice_supported() {
            return Err(LxAppError::UnsupportedOperation(
                "host was built without JS AppService runtime".to_string(),
            ));
        }
        create_app_svc(
            lxapp,
            &self.sender,
            &self.instance_assignments,
            &self.free_workers,
        )
    }

    /// Terminate a lxapp service for a specific instance.
    pub fn terminate_app_svc(&self, lxapp: Arc<crate::lxapp::LxApp>) -> Result<(), LxAppError> {
        terminate_app_svc(
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
        page_instance_id: Option<String>,
        ack_tx: Sender<Result<(), String>>,
    ) -> Result<(), LxAppError> {
        if !lxapp.logic_enabled() {
            let _ = ack_tx.send(Ok(()));
            return Ok(());
        }
        if !crate::js_appservice_supported() {
            return Err(LxAppError::UnsupportedOperation(
                "host was built without JS AppService runtime".to_string(),
            ));
        }
        self.sender.send(ServiceMessage::CreatePage {
            lxapp,
            path,
            page_instance_id,
            ack_tx,
        })?;
        Ok(())
    }

    /// Terminate a page service by object identity.
    pub(crate) fn terminate_page_svc(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        page_instance_id: Option<String>,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::TerminatePage {
            lxapp,
            path,
            page_instance_id,
        })?;
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

    /// Call a function on a PageInstance service from native code
    pub fn call_page_service(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        page_instance_id: Option<String>,
        name: String,
        args: Option<String>,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::CallPageSvc {
            lxapp,
            path,
            page_instance_id,
            source: PageSvcSource::Native { name, args },
        })?;
        Ok(())
    }

    /// Call a typed page event on a PageInstance service
    pub fn call_page_service_event(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        page_instance_id: Option<String>,
        event: PageServiceEvent,
        args: Option<String>,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::CallPageSvcEvent {
            lxapp,
            path,
            page_instance_id,
            event,
            args,
        })?;
        Ok(())
    }

    /// Dispatch a native -> JS event through the app worker.
    pub(crate) fn dispatch_app_bus_event(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        event: AppBusEvent,
    ) -> Result<(), LxAppError> {
        self.sender
            .send(ServiceMessage::DispatchAppBusEvent { lxapp, event })?;
        Ok(())
    }

    pub async fn eval_app_service(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        script: String,
    ) -> Result<String, LxAppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ServiceMessage::Eval { lxapp, script, tx })?;
        rx.await
            .map_err(|err| LxAppError::ChannelError(err.to_string()))?
    }
}

impl crate::bridge::AppServiceBackend for LxAppWorkers {
    fn forward(
        &self,
        lxapp: Arc<crate::lxapp::LxApp>,
        path: String,
        page_instance_id: Option<String>,
        message: crate::bridge::AppServiceCommand,
    ) -> Result<(), LxAppError> {
        self.sender.send(ServiceMessage::CallPageSvc {
            lxapp,
            path,
            page_instance_id,
            source: PageSvcSource::Bridge { message },
        })?;
        Ok(())
    }
}
