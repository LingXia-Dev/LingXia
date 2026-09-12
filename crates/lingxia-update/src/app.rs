use crate::{
    BoxFuture, UpdatePackageInfo, UpdateTarget, UpdateVerifyTarget, Version, check_update_enabled,
    embedded_update_public_keys, host_update_platform, verify_checked_update,
};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::sync::broadcast;

use super::error::UpdateError;

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
    fn download_app_update<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
        progress: AppUpdateProgressReporter,
    ) -> BoxFuture<'a, Result<PathBuf, UpdateError>>;
    /// Hand off the downloaded package to the platform installer. `info_json`
    /// carries the prompt metadata `{version, releaseNotes, isForceUpdate}`:
    /// the platform shows release notes in the "ready to update" prompt and,
    /// when `isForceUpdate` is true, presents a blocking "must update" prompt
    /// instead of the dismissible reminder.
    fn install_app_update(&self, package_path: &Path, info_json: &str) -> Result<(), UpdateError>;
    fn log_app_update_warning(&self, detail: &str);
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
    let target_id = lingxia_app_context::app_config()
        .and_then(|config| config.lingxia_id.clone())
        .filter(|id| !id.is_empty())
        .unwrap_or_default();
    check_app_update_for(host, &embedded_update_public_keys(), target_id).await
}

async fn check_app_update_for<H: AppUpdateHost>(
    host: &H,
    trusted_public_keys: &[String],
    target_id: String,
) -> Result<Option<UpdatePackageInfo>, UpdateError> {
    if !check_update_enabled(trusted_public_keys) {
        return Ok(None);
    }
    let current_version = host.current_app_version()?;
    let candidate = host.check_app_update(&current_version).await?;
    let Some(package) = candidate else {
        return Ok(None);
    };
    let package = verify_checked_update(
        package,
        &UpdateVerifyTarget {
            kind: "app".into(),
            target_id,
            channel: String::new(),
            platform: host_update_platform().into(),
            exact_version: None,
        },
        trusted_public_keys,
    )?;
    // Only surface a strictly-newer candidate. A provider that re-offers the
    // installed version (or the same version after a successful update) would
    // otherwise make the app re-download and re-prompt on every check — an
    // endless "update available" loop. Unparseable versions fall through to the
    // apply-time downgrade guard.
    if !app_update_candidate_is_newer(&package.version, &current_version) {
        return Ok(None);
    }
    Ok(Some(package))
}

fn app_update_candidate_is_newer(candidate: &str, current: &str) -> bool {
    match (
        Version::parse(candidate.trim()),
        Version::parse(current.trim()),
    ) {
        (Ok(candidate), Ok(current)) => candidate > current,
        // Can't compare — let the apply-time guard decide rather than hiding it.
        _ => true,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::{SignRequest, archive_sha256_hex, public_key_base64url, sign_package};
    use crate::{UpdateAuthentication, UpdatePackageInfo};
    use lingxia_app_context::{AppConfig, AppEnv};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const SEED: [u8; 32] = [7u8; 32];
    const ARCHIVE: &[u8] = b"host-update-golden-archive";
    const TARGET_ID: &str = "demo-host";

    #[derive(Clone)]
    struct FakeHost {
        current_version: String,
        response: Option<UpdatePackageInfo>,
        provider_calls: Arc<AtomicUsize>,
    }

    impl FakeHost {
        fn new(current_version: &str, response: Option<UpdatePackageInfo>) -> Self {
            Self {
                current_version: current_version.into(),
                response,
                provider_calls: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl AppUpdateHost for FakeHost {
        fn spawn_detached(&self, _task: BoxFuture<'static, ()>) {}

        fn current_app_version(&self) -> Result<String, UpdateError> {
            Ok(self.current_version.clone())
        }

        fn check_app_update<'a>(
            &'a self,
            _current_version: &'a str,
        ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
            self.provider_calls.fetch_add(1, Ordering::SeqCst);
            let response = self.response.clone();
            Box::pin(async move { Ok(response) })
        }

        fn download_app_update<'a>(
            &'a self,
            _update: &'a UpdatePackageInfo,
            _progress: AppUpdateProgressReporter,
        ) -> BoxFuture<'a, Result<PathBuf, UpdateError>> {
            Box::pin(async { Err(UpdateError::runtime("download not used")) })
        }

        fn install_app_update(
            &self,
            _package_path: &Path,
            _info_json: &str,
        ) -> Result<(), UpdateError> {
            Err(UpdateError::runtime("install not used"))
        }

        fn log_app_update_warning(&self, _detail: &str) {}
    }

    fn install_release_keys() {
        let config = AppConfig {
            product_name: "Host Verify".to_string(),
            product_version: "1.0.0".to_string(),
            lingxia_id: Some(TARGET_ID.to_string()),
            lingxia_server: None,
            env: AppEnv::Prod,
            home_app_id: String::new(),
            home_app_version: String::new(),
            cache_max_size_mb: 1024,
            storage: None,
            splash: None,
            dev_ws_url: None,
            dev_bundle_base_url: None,
            app_links: None,
            theme: None,
            settings_destination: None,
            capabilities: None,
            panels: None,
            update_trusted_public_keys: vec![public_key_base64url(&SEED)],
        };
        lingxia_app_context::set_app_config(config).expect("install host verify config");
    }

    fn signed_package(version: &str, auth: Option<UpdateAuthentication>) -> UpdatePackageInfo {
        let sha256 = archive_sha256_hex(ARCHIVE);
        UpdatePackageInfo {
            version: version.into(),
            url: "https://cdn.example.com/app".into(),
            checksum_sha256: sha256,
            size: Some(ARCHIVE.len() as u64),
            release_notes: None,
            is_force_update: true,
            required_runtime_version: None,
            authentication: auth,
        }
    }

    fn sign(version: &str) -> UpdateAuthentication {
        let sha256 = archive_sha256_hex(ARCHIVE);
        sign_package(
            &SEED,
            &SignRequest {
                kind: "app",
                target_id: TARGET_ID,
                channel: "",
                platform: host_update_platform(),
                version,
                sha256: &sha256,
                size: ARCHIVE.len() as u64,
                required_runtime_version: "",
            },
        )
        .expect("sign host package")
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future)
    }

    #[test]
    fn check_app_update_verifies_before_version_or_force_decisions() {
        install_release_keys();

        let unsigned = FakeHost::new("1.0.0", Some(signed_package("1.0.1", None)));
        let err = block_on(check_app_update(&unsigned)).expect_err("unsigned release");
        assert!(err.to_string().contains("require signed updates"), "{err}");

        let mut bad = sign("1.0.1");
        let mut sig = crate::decode_base64url(&bad.signatures[0]).unwrap();
        sig[0] ^= 0xff;
        bad.signatures[0] = crate::encode_base64url(&sig);
        let tampered = FakeHost::new("1.0.0", Some(signed_package("1.0.1", Some(bad))));
        assert!(block_on(check_app_update(&tampered)).is_err());

        let same_version =
            FakeHost::new("1.0.1", Some(signed_package("1.0.1", Some(sign("1.0.1")))));
        let none = block_on(check_app_update(&same_version)).expect("verified same version");
        assert!(none.is_none(), "version filter runs only after verify");

        let newer = FakeHost::new("1.0.0", Some(signed_package("1.0.1", Some(sign("1.0.1")))));
        let accepted = block_on(check_app_update(&newer))
            .expect("verified newer")
            .expect("update available");
        assert_eq!(accepted.version, "1.0.1");
        assert_eq!(accepted.checksum_sha256, archive_sha256_hex(ARCHIVE));
    }

    #[test]
    fn prod_without_keys_skips_check() {
        let host = FakeHost::new("1.0.0", Some(signed_package("1.0.1", None)));
        let result = block_on(check_app_update_for(&host, &[], TARGET_ID.into()))
            .expect("skip is not an error");
        assert!(result.is_none());
        assert_eq!(host.provider_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn prod_rejects_unsigned_host_update() {
        // Host updates have no channel; the prod host still requires a signature.
        let host = FakeHost::new("1.0.0", Some(signed_package("1.0.1", None)));
        let keys = [public_key_base64url(&SEED)];
        let err = block_on(check_app_update_for(&host, &keys, TARGET_ID.into()))
            .expect_err("an unsigned package on a prod build");
        assert!(err.to_string().contains("require signed updates"), "{err}");
        assert_eq!(host.provider_calls.load(Ordering::SeqCst), 1);
    }
}
