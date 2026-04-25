use crate::config::{UpdateUiMode, update_config};
use crate::{BoxFuture, UpdatePackageInfo, UpdateTarget, Version};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;

use super::error::UpdateError;

pub const APP_UPDATE_START_DELAY: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub enum AppUpdateEvent {
    Available(UpdatePackageInfo),
    DownloadStarted {
        version: String,
    },
    DownloadProgress {
        version: String,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        progress: Option<u8>,
    },
    Downloaded {
        version: String,
    },
    InstallRequested {
        version: String,
    },
    Failed {
        stage: AppUpdateStage,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppUpdateStage {
    Check,
    Prompt,
    Download,
    Install,
}

pub type AppUpdateEventReceiver = broadcast::Receiver<AppUpdateEvent>;
pub type AppUpdateEventSender = broadcast::Sender<AppUpdateEvent>;

pub struct AppUpdateApply {
    receiver: AppUpdateEventReceiver,
    done: bool,
}

impl AppUpdateApply {
    pub fn new(receiver: AppUpdateEventReceiver) -> Self {
        Self {
            receiver,
            done: false,
        }
    }

    pub fn channel() -> (Self, AppUpdateEventSender) {
        let (sender, receiver) = broadcast::channel(32);
        (Self::new(receiver), sender)
    }

    pub async fn next(&mut self) -> Option<AppUpdateEvent> {
        if self.done {
            return None;
        }

        let event = loop {
            match self.receiver.recv().await {
                Ok(event) => break Some(event),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break None,
            }
        };

        let Some(event) = event else {
            self.done = true;
            return None;
        };

        if matches!(
            event,
            AppUpdateEvent::InstallRequested { .. } | AppUpdateEvent::Failed { .. }
        ) {
            self.done = true;
        }

        Some(event)
    }
}

#[derive(Debug, Clone)]
pub struct AppUpdateProgressReporter {
    version: String,
    sender: Option<AppUpdateEventSender>,
}

impl AppUpdateProgressReporter {
    fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            sender: None,
        }
    }

    pub fn scoped(version: impl Into<String>, sender: AppUpdateEventSender) -> Self {
        Self {
            version: version.into(),
            sender: Some(sender),
        }
    }

    fn emit(&self, event: AppUpdateEvent) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(event);
        } else {
            emit_app_update_event(event);
        }
    }

    pub fn report(&self, downloaded_bytes: u64, total_bytes: Option<u64>) {
        let progress = total_bytes.filter(|total| *total > 0).map(|total| {
            ((downloaded_bytes as f64 / total as f64) * 100.0)
                .round()
                .clamp(0.0, 100.0) as u8
        });
        self.emit(AppUpdateEvent::DownloadProgress {
            version: self.version.clone(),
            downloaded_bytes,
            total_bytes,
            progress,
        });
    }
}

pub fn send_app_update_event(sender: &AppUpdateEventSender, event: AppUpdateEvent) {
    let _ = sender.send(event);
}

fn emit_app_update_failed(stage: AppUpdateStage, error: &UpdateError) {
    emit_app_update_event(AppUpdateEvent::Failed {
        stage,
        error: error.to_string(),
    });
}

pub fn send_app_update_failed(
    sender: &AppUpdateEventSender,
    stage: AppUpdateStage,
    error: &UpdateError,
) {
    send_app_update_event(
        sender,
        AppUpdateEvent::Failed {
            stage,
            error: error.to_string(),
        },
    );
}

pub trait AppUpdateHost: Clone + Send + Sync + 'static {
    fn spawn_detached(&self, task: BoxFuture<'static, ()>);
    fn current_app_version(&self) -> Result<String, UpdateError>;
    fn check_app_update<'a>(
        &'a self,
        current_version: &'a str,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>>;
    fn show_builtin_update_prompt<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<bool, UpdateError>>;
    fn download_app_update<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
        progress: AppUpdateProgressReporter,
    ) -> BoxFuture<'a, Result<PathBuf, UpdateError>>;
    fn install_app_update(&self, package_path: &Path) -> Result<(), UpdateError>;
    fn log_app_update_warning(&self, detail: &str);
    fn notify_app_update_available(&self, update: &UpdatePackageInfo) -> Result<(), UpdateError>;
}

fn app_update_events() -> &'static broadcast::Sender<AppUpdateEvent> {
    static APP_UPDATE_EVENTS: OnceLock<broadcast::Sender<AppUpdateEvent>> = OnceLock::new();
    APP_UPDATE_EVENTS.get_or_init(|| {
        let (tx, _) = broadcast::channel(32);
        tx
    })
}

pub fn subscribe_app_update_events() -> AppUpdateEventReceiver {
    app_update_events().subscribe()
}

fn emit_app_update_event(event: AppUpdateEvent) {
    let _ = app_update_events().send(event);
}

pub async fn check_app_update<H: AppUpdateHost>(
    host: &H,
) -> Result<Option<UpdatePackageInfo>, UpdateError> {
    let current_version = host.current_app_version()?;
    host.check_app_update(&current_version).await
}

pub async fn download_app_update<H: AppUpdateHost>(
    host: &H,
    update: &UpdatePackageInfo,
) -> Result<PathBuf, UpdateError> {
    let current_version = host.current_app_version().map_err(|error| {
        emit_app_update_failed(AppUpdateStage::Download, &error);
        error
    })?;
    ensure_app_update_candidate_version(&current_version, &update.version).map_err(|error| {
        emit_app_update_failed(AppUpdateStage::Download, &error);
        error
    })?;

    emit_app_update_event(AppUpdateEvent::DownloadStarted {
        version: update.version.clone(),
    });
    let path = host
        .download_app_update(update, AppUpdateProgressReporter::new(&update.version))
        .await
        .map_err(|error| {
            emit_app_update_failed(AppUpdateStage::Download, &error);
            error
        })?;
    emit_app_update_event(AppUpdateEvent::Downloaded {
        version: update.version.clone(),
    });

    Ok(path)
}

pub fn install_app_update<H: AppUpdateHost>(
    host: &H,
    update: &UpdatePackageInfo,
    package_path: &Path,
) -> Result<(), UpdateError> {
    let current_version = host.current_app_version().map_err(|error| {
        emit_app_update_failed(AppUpdateStage::Install, &error);
        error
    })?;
    ensure_app_update_candidate_version(&current_version, &update.version).map_err(|error| {
        emit_app_update_failed(AppUpdateStage::Install, &error);
        error
    })?;

    host.install_app_update(package_path).map_err(|error| {
        emit_app_update_failed(AppUpdateStage::Install, &error);
        error
    })?;
    emit_app_update_event(AppUpdateEvent::InstallRequested {
        version: update.version.clone(),
    });

    Ok(())
}

pub async fn download_and_install_app_update<H: AppUpdateHost>(
    host: &H,
    update: &UpdatePackageInfo,
) -> Result<PathBuf, UpdateError> {
    let path = download_app_update(host, update).await?;
    install_app_update(host, update, &path)?;
    Ok(path)
}

pub async fn check_and_install_app_update<H: AppUpdateHost>(host: &H) -> Result<(), UpdateError> {
    let current_version = host.current_app_version().map_err(|error| {
        emit_app_update_failed(AppUpdateStage::Check, &error);
        error
    })?;
    let update = host
        .check_app_update(&current_version)
        .await
        .map_err(|error| {
            emit_app_update_failed(AppUpdateStage::Check, &error);
            error
        })?;
    let Some(update) = update else {
        return Ok(());
    };

    ensure_app_update_candidate_version(&current_version, &update.version).map_err(|error| {
        emit_app_update_failed(AppUpdateStage::Check, &error);
        error
    })?;
    emit_app_update_event(AppUpdateEvent::Available(update.clone()));

    if update_config().ui_mode == UpdateUiMode::Custom {
        host.notify_app_update_available(&update)?;
        if update.is_force_update {
            return Err(UpdateError::runtime(
                "forced app update requires explicit host handling in custom UI mode",
            ));
        }
        return Ok(());
    }

    let confirmed = host
        .show_builtin_update_prompt(&update)
        .await
        .map_err(|error| {
            emit_app_update_failed(AppUpdateStage::Prompt, &error);
            error
        })?;

    if !confirmed && update.is_force_update {
        let error = UpdateError::runtime("forced app update was not confirmed");
        emit_app_update_failed(AppUpdateStage::Prompt, &error);
        return Err(error);
    }

    if !confirmed {
        return Ok(());
    }

    download_and_install_app_update(host, &update)
        .await
        .map(|_| ())
}

pub fn spawn_app_update_flow<H: AppUpdateHost>(
    host: H,
    start_delay: Duration,
    bypass_auto_check: bool,
) {
    let runner = host.clone();
    host.spawn_detached(Box::pin(async move {
        if !start_delay.is_zero() {
            sleep(start_delay).await;
        }

        if !bypass_auto_check && !update_config().auto_check_app {
            return;
        }

        if let Err(error) = check_and_install_app_update(&runner).await {
            runner.log_app_update_warning(&format!("App update flow failed: {}", error));
        }
    }));
}

pub fn ensure_app_update_candidate_version(
    current_version: &str,
    candidate_version: &str,
) -> Result<(), UpdateError> {
    let candidate_version = candidate_version.trim();
    if candidate_version.is_empty() {
        return Err(UpdateError::invalid_parameter(
            "app update package version is empty",
        ));
    }

    let candidate = Version::parse(candidate_version).map_err(|_| {
        UpdateError::invalid_parameter(format!(
            "app update package version is not semantic version: {}",
            candidate_version
        ))
    })?;

    let current = Version::parse(current_version).map_err(|_| {
        UpdateError::runtime(format!(
            "current app version is not semantic version: {}",
            current_version
        ))
    })?;

    if candidate < current {
        return Err(UpdateError::unsupported(format!(
            "reject app downgrade: current={} candidate={}",
            current_version, candidate_version
        )));
    }

    Ok(())
}

pub fn app_update_scope_key() -> String {
    UpdateTarget::app(None::<String>).scope_key()
}
