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
        /// An update is available but the user chose "Later" at the prompt.
        Deferred { version: String },
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
            // Already finished successfully — nothing more to do.
            if AUTO_TRIGGERED.load(Ordering::SeqCst) {
                return;
            }
            // Another attempt is already running. Skip; if it fails it will
            // leave the listener subscribed and the next event will retry.
            if AUTO_RUNNING.swap(true, Ordering::SeqCst) {
                return;
            }

            let runtime_for_task = runtime_for_handler.clone();
            std::mem::drop(lingxia::task::spawn(async move {
                let outcome = check().await;
                AUTO_RUNNING.store(false, Ordering::SeqCst);
                match outcome {
                    Ok(Outcome::UpToDate) => {
                        log::info!("[lingxia] host app auto update: up to date");
                        unsubscribe_auto_listener(&runtime_for_task);
                    }
                    Ok(Outcome::Installed { version }) => {
                        log::info!("[lingxia] host app auto update: installed version {version}");
                        unsubscribe_auto_listener(&runtime_for_task);
                    }
                    Ok(Outcome::Deferred { version }) => {
                        // User chose "Later"; the shell shows a quiet reminder.
                        // Stop polling so we don't re-prompt on every reconnect.
                        log::info!("[lingxia] host app auto update: deferred {version}");
                        unsubscribe_auto_listener(&runtime_for_task);
                    }
                    Err(error) => {
                        // Keep the listener subscribed so the next connected
                        // event retries — typical weak-network recovery.
                        log::warn!(
                            "[lingxia] host app auto update failed (will retry on next connect): {error}"
                        );
                    }
                }
            }));
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

    /// Remove the network-change listener installed by `install_auto_trigger`.
    /// Idempotent: safe to call if the listener was already removed.
    fn unsubscribe_auto_listener(runtime: &Arc<lingxia_platform::Platform>) {
        AUTO_TRIGGERED.store(true, Ordering::SeqCst);
        let listener_id = AUTO_LISTENER_ID.swap(0, Ordering::SeqCst);
        if listener_id != 0 {
            let _ = runtime.remove_network_change_listener(listener_id);
            let _ = lingxia_messaging::remove_callback(listener_id);
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
        // Store-delivered platforms (iOS App Store, HarmonyOS AppGallery) never
        // self-download or self-install: the store owns updates. Point the user
        // at the store when possible, then stop — no background download.
        if !service.self_update_supported() {
            let info = update_info_json(&update);
            let opened = service.open_update_store(&info);
            log::info!(
                "[lingxia] host app update {} available; store-delivered platform (opened store: {opened})",
                update.version
            );
            return Ok(Outcome::Deferred {
                version: update.version,
            });
        }
        // No pre-download prompt: the package downloads silently in the
        // background. The only user-facing moment is the post-download
        // "ready to update" prompt, which each platform presents from its
        // install hand-off (a dismissible reminder, or a blocking modal when
        // the update is forced). Headless / non-desktop applies unattended.
        apply(service, update).await
    }

    fn update_info_json(update: &UpdatePackageInfo) -> String {
        serde_json::json!({
            "version": update.version,
            "size": update.size,
            "releaseNotes": update.release_notes,
            "isForceUpdate": update.is_force_update,
        })
        .to_string()
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
                } => {
                    // Download is silent — progress is surfaced only to the
                    // registered handler (logging), not to a native UI.
                    emit(Progress::Downloading {
                        version,
                        downloaded_bytes,
                        total_bytes,
                        percent: progress,
                    });
                }
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
        let runtime = crate::runtime::platform()?;
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
    /// Set only after a successful check (UpToDate or Installed). Once true,
    /// the listener has been removed and subsequent network events are no-ops.
    static AUTO_TRIGGERED: AtomicBool = AtomicBool::new(false);
    /// Guards against spawning a second check while one is already running.
    /// Important on weak networks where the listener stays subscribed across
    /// failed attempts and may receive multiple `isConnected: true` events.
    static AUTO_RUNNING: AtomicBool = AtomicBool::new(false);
    static AUTO_LISTENER_ID: AtomicU64 = AtomicU64::new(0);
}

pub(crate) fn install_auto_trigger(runtime: std::sync::Arc<lingxia_platform::Platform>) {
    host_app::install_auto_trigger(runtime);
}
