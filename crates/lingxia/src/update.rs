//! Host app update API.
//!
//! Three public functions, two enums, zero ceremony. See [`host_app`].

/// Host app update flow.
///
/// # Quick start
///
/// ```ignore
/// // Once at startup. Optional; without this, the platform default installer is used.
/// lingxia::update::host_app::set_installer(|apk| my_root_install(apk))?;
///
/// // From a UI "check for updates" button.
/// match lingxia::update::host_app::check().await? {
///     Outcome::UpToDate => log::info!("already up to date"),
///     Outcome::Installed { version } => log::info!("installed {version}"),
/// }
/// ```
///
/// LingXia also fires [`check`] once per process on its own, as soon as the
/// platform reports network connectivity.
pub mod host_app {
    use lingxia_platform::traits::network::Network;
    use lingxia_service::update::{
        AppUpdateEvent, AppUpdateStage, HostAppUpdateService, UpdateError, UpdatePackageInfo,
    };
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, OnceLock, RwLock};
    use tokio::sync::{Mutex as AsyncMutex, broadcast};

    /// Final result of a check + install attempt.
    #[derive(Debug, Clone)]
    pub enum Outcome {
        /// No update was available.
        UpToDate,
        /// An update was downloaded and installation was handed off to the platform.
        Installed { version: String },
    }

    /// Progress events emitted while a [`check`] is in progress.
    #[derive(Debug, Clone)]
    pub enum Progress {
        /// Querying the provider for an update.
        Checking,
        /// Download has started (or restarted) for `version`.
        DownloadStarted { version: String },
        /// Periodic download progress.
        Downloading {
            version: String,
            downloaded_bytes: u64,
            total_bytes: Option<u64>,
            percent: Option<u8>,
        },
        /// Download finished and the package was verified.
        Downloaded { version: String },
        /// Install hand-off has been requested (installer hook or platform installer).
        Installing { version: String },
    }

    /// Registers a custom host app installer. Replaces any previously registered
    /// installer.
    ///
    /// The installer receives the downloaded and verified package path and must
    /// complete the installation. Return `Ok(())` to mark the install as handled.
    /// When no installer is registered the platform default installer is used.
    pub fn set_installer(installer: impl Fn(&Path) -> crate::Result<()> + Send + Sync + 'static) {
        lingxia_service::update::set_host_app_installer(move |path| {
            installer(path).map_err(|error| UpdateError::runtime(error.to_string()))
        });
    }

    /// Registers (or replaces) the progress handler. Single slot — re-registering
    /// overwrites. Pass `None` to clear.
    pub fn on_progress<F>(handler: F)
    where
        F: Fn(Progress) + Send + Sync + 'static,
    {
        match progress_slot().write() {
            Ok(mut guard) => {
                *guard = Some(Arc::new(handler));
            }
            Err(error) => {
                log::warn!("failed to register host app progress handler: {error}");
            }
        }
    }

    /// Runs a complete check + install cycle: queries the provider, downloads
    /// when there is an update, and hands off install via the registered
    /// installer (or the platform default).
    ///
    /// Concurrent callers (UI button while the auto-trigger is still running,
    /// double-tap, etc.) join the in-flight attempt rather than starting a new
    /// one.
    pub async fn check() -> crate::Result<Outcome> {
        let action = {
            let mut guard = inflight().lock().await;
            match guard.as_ref() {
                Some(sender) => Action::Wait(sender.subscribe()),
                None => {
                    let (tx, _) = broadcast::channel(1);
                    *guard = Some(tx.clone());
                    Action::Run(tx)
                }
            }
        };

        match action {
            Action::Run(tx) => {
                let result = run_flow().await;
                let shared: SharedResult = match &result {
                    Ok(outcome) => Ok(outcome.clone()),
                    Err(error) => Err(error.to_string()),
                };
                let _ = tx.send(Arc::new(shared));
                *inflight().lock().await = None;
                result
            }
            Action::Wait(mut rx) => match rx.recv().await {
                Ok(arc) => match (*arc).clone() {
                    Ok(outcome) => Ok(outcome),
                    Err(detail) => Err(crate::Error::Internal(detail)),
                },
                Err(_) => Err(crate::Error::internal(
                    "host app update check coordinator dropped",
                )),
            },
        }
    }

    pub(crate) fn install_auto_trigger(runtime: Arc<lingxia_platform::Platform>) {
        if AUTO_FIRED.swap(true, Ordering::SeqCst) {
            return;
        }

        let runtime_for_handler = runtime.clone();
        let callback_id = lingxia_messaging::register_handler(move |result| {
            let connected = matches!(
                &result,
                lingxia_messaging::CallbackResult::Success(json) if json_is_connected(json)
            );
            if !connected {
                return;
            }
            if AUTO_TRIGGERED.swap(true, Ordering::SeqCst) {
                return;
            }
            let listener_id = AUTO_LISTENER_ID.load(Ordering::SeqCst);
            if listener_id != 0 {
                let _ = runtime_for_handler.remove_network_change_listener(listener_id);
                let _ = lingxia_messaging::remove_callback(listener_id);
            }
            let _ = lingxia::task::spawn(async {
                match check().await {
                    Ok(Outcome::UpToDate) => {
                        log::info!("[lingxia] host app auto update: up to date");
                    }
                    Ok(Outcome::Installed { version }) => {
                        log::info!("[lingxia] host app auto update: installed version {version}");
                    }
                    Err(error) => {
                        log::warn!("[lingxia] host app auto update failed: {error}");
                    }
                }
            });
        });

        AUTO_LISTENER_ID.store(callback_id, Ordering::SeqCst);

        if let Err(error) = runtime.add_network_change_listener(callback_id) {
            log::warn!(
                "[lingxia] host app auto update: add_network_change_listener failed: {error}"
            );
            let _ = lingxia_messaging::remove_callback(callback_id);
            AUTO_LISTENER_ID.store(0, Ordering::SeqCst);
            AUTO_FIRED.store(false, Ordering::SeqCst);
        }
    }

    enum Action {
        Run(broadcast::Sender<Arc<SharedResult>>),
        Wait(broadcast::Receiver<Arc<SharedResult>>),
    }

    type SharedResult = Result<Outcome, String>;

    async fn run_flow() -> crate::Result<Outcome> {
        emit(Progress::Checking);
        let service = service()?;
        let update = service.check().await?;
        let Some(update) = update else {
            return Ok(Outcome::UpToDate);
        };
        apply(service, update).await
    }

    async fn apply(
        service: HostAppUpdateService,
        update: UpdatePackageInfo,
    ) -> crate::Result<Outcome> {
        let target_version = update.version.clone();
        let mut stream = service.apply(update);
        while let Some(event) = stream.next().await {
            match event {
                AppUpdateEvent::Available(_) => {}
                AppUpdateEvent::DownloadStarted { version } => {
                    emit(Progress::DownloadStarted { version });
                }
                AppUpdateEvent::DownloadProgress {
                    version,
                    downloaded_bytes,
                    total_bytes,
                    progress,
                } => emit(Progress::Downloading {
                    version,
                    downloaded_bytes,
                    total_bytes,
                    percent: progress,
                }),
                AppUpdateEvent::Downloaded { version } => {
                    emit(Progress::Downloaded { version });
                }
                AppUpdateEvent::InstallRequested { version } => {
                    emit(Progress::Installing {
                        version: version.clone(),
                    });
                    return Ok(Outcome::Installed { version });
                }
                AppUpdateEvent::Failed { stage, error } => {
                    return Err(crate::Error::Internal(format!(
                        "host app update failed at {}: {error}",
                        stage_name(stage)
                    )));
                }
            }
        }
        Err(crate::Error::internal(format!(
            "host app update for {target_version} ended without a terminal event"
        )))
    }

    fn stage_name(stage: AppUpdateStage) -> &'static str {
        match stage {
            AppUpdateStage::Check => "check",
            AppUpdateStage::Download => "download",
            AppUpdateStage::Install => "install",
        }
    }

    fn emit(event: Progress) {
        let handler = progress_slot()
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().cloned());
        if let Some(handler) = handler {
            handler(event);
        }
    }

    fn service() -> crate::Result<HostAppUpdateService> {
        let runtime = lxapp::get_platform()
            .ok_or_else(|| crate::Error::internal("platform is not initialized"))?;
        Ok(HostAppUpdateService::new(
            runtime,
            lxapp::provider::update_provider(),
        ))
    }

    fn inflight() -> &'static AsyncMutex<Option<broadcast::Sender<Arc<SharedResult>>>> {
        static CELL: OnceLock<AsyncMutex<Option<broadcast::Sender<Arc<SharedResult>>>>> =
            OnceLock::new();
        CELL.get_or_init(|| AsyncMutex::new(None))
    }

    type ProgressHandler = dyn Fn(Progress) + Send + Sync + 'static;

    fn progress_slot() -> &'static RwLock<Option<Arc<ProgressHandler>>> {
        static CELL: OnceLock<RwLock<Option<Arc<ProgressHandler>>>> = OnceLock::new();
        CELL.get_or_init(|| RwLock::new(None))
    }

    fn json_is_connected(json: &str) -> bool {
        serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|value| value.get("isConnected").and_then(|v| v.as_bool()))
            .unwrap_or(false)
    }

    static AUTO_FIRED: AtomicBool = AtomicBool::new(false);
    static AUTO_TRIGGERED: AtomicBool = AtomicBool::new(false);
    static AUTO_LISTENER_ID: AtomicU64 = AtomicU64::new(0);
}

pub(crate) fn install_auto_trigger(runtime: std::sync::Arc<lingxia_platform::Platform>) {
    host_app::install_auto_trigger(runtime);
}
