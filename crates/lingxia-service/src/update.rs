// Update work runs as detached background tasks on the executor; the join
// handle is intentionally dropped (fire-and-forget).
#![allow(clippy::let_underscore_future)]

use bytes::Bytes;
use http::Request;
use http_body_util::{BodyExt, Empty};
use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::update::UpdateService;
use lingxia_provider::BoxFuture;
use lingxia_update::AppUpdateHost;
use rong_rt::download::{self as service_executor, BodySink};
use rong_rt::http as host_http;
use std::fs;
use std::io::{Error as IoError, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

pub use lingxia_update::{
    AppUpdateApply, AppUpdateEvent, AppUpdateEventReceiver, AppUpdateProgressReporter,
    AppUpdateStage, UpdateConfig, UpdateError, UpdatePackageInfo, UpdateProvider, UpdateTarget,
    Version, VersionError, configure_update, subscribe_app_update_events, update_config,
};

pub type HostAppInstaller = dyn Fn(&Path) -> Result<(), UpdateError> + Send + Sync + 'static;

static HOST_APP_INSTALLER: OnceLock<RwLock<Option<Arc<HostAppInstaller>>>> = OnceLock::new();

fn host_app_installer_slot() -> &'static RwLock<Option<Arc<HostAppInstaller>>> {
    HOST_APP_INSTALLER.get_or_init(|| RwLock::new(None))
}

/// Registers a custom host app installer. Replaces any previously registered
/// installer. The installer receives the downloaded and verified package path
/// and is responsible for completing installation; returning `Ok(())` marks the
/// install as handled and skips the platform default installer.
pub fn set_host_app_installer(
    installer: impl Fn(&Path) -> Result<(), UpdateError> + Send + Sync + 'static,
) {
    match host_app_installer_slot().write() {
        Ok(mut guard) => {
            *guard = Some(Arc::new(installer));
        }
        Err(error) => {
            log::warn!("failed to register host app installer: {error}");
        }
    }
}

/// Returns whether a custom installer has been registered.
pub fn has_host_app_installer() -> bool {
    host_app_installer_slot()
        .read()
        .ok()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

#[derive(Clone)]
pub struct HostAppUpdateService {
    runtime: Arc<Platform>,
    provider: &'static dyn UpdateProvider,
}

impl HostAppUpdateService {
    pub fn new(runtime: Arc<Platform>, provider: &'static dyn UpdateProvider) -> Self {
        Self { runtime, provider }
    }

    pub async fn check(&self) -> Result<Option<UpdatePackageInfo>, UpdateError> {
        lingxia_update::check_app_update(self).await
    }

    pub fn apply(&self, update: UpdatePackageInfo) -> AppUpdateApply {
        let (apply, sender) = AppUpdateApply::channel();
        let runner = self.clone();
        let _ = rong_rt::RongExecutor::global().spawn(async move {
            let result = async {
                let current_version = runner
                    .current_app_version()
                    .map_err(|error| (AppUpdateStage::Download, error))?;
                lingxia_update::ensure_app_update_candidate_version(
                    &current_version,
                    &update.version,
                )
                .map_err(|error| (AppUpdateStage::Download, error))?;
                lingxia_update::send_app_update_event(
                    &sender,
                    AppUpdateEvent::DownloadStarted {
                        version: update.version.clone(),
                    },
                );

                let path = runner
                    .download_app_update(
                        &update,
                        AppUpdateProgressReporter::scoped(&update.version, sender.clone()),
                    )
                    .await
                    .map_err(|error| (AppUpdateStage::Download, error))?;
                lingxia_update::send_app_update_event(
                    &sender,
                    AppUpdateEvent::Downloaded {
                        version: update.version.clone(),
                    },
                );

                let current_version = runner
                    .current_app_version()
                    .map_err(|error| (AppUpdateStage::Install, error))?;
                lingxia_update::ensure_app_update_candidate_version(
                    &current_version,
                    &update.version,
                )
                .map_err(|error| (AppUpdateStage::Install, error))?;
                runner
                    .install_app_update(&path)
                    .map_err(|error| (AppUpdateStage::Install, error))?;
                lingxia_update::send_app_update_event(
                    &sender,
                    AppUpdateEvent::InstallRequested {
                        version: update.version.clone(),
                    },
                );
                Ok::<(), (AppUpdateStage, UpdateError)>(())
            }
            .await;
            if let Err((stage, error)) = result {
                log::warn!("Host app update apply failed: {error}");
                lingxia_update::send_app_update_failed(&sender, stage, &error);
            }
        });
        apply
    }
}

impl lingxia_update::AppUpdateHost for HostAppUpdateService {
    fn spawn_detached(&self, task: BoxFuture<'static, ()>) {
        let _ = rong_rt::RongExecutor::global().spawn(task);
    }

    fn current_app_version(&self) -> Result<String, UpdateError> {
        resolve_required_app_version()
            .map(str::to_string)
            .map_err(|error| UpdateError::runtime(error.to_string()))
    }

    fn check_app_update<'a>(
        &'a self,
        current_version: &'a str,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
        Box::pin(async move {
            let target = UpdateTarget::app(Some(current_version));
            self.provider
                .check_update(target)
                .await
                .map_err(UpdateError::from)
        })
    }

    fn download_app_update<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
        progress: AppUpdateProgressReporter,
    ) -> BoxFuture<'a, Result<PathBuf, UpdateError>> {
        Box::pin(async move {
            download_host_app_update_package(
                self.runtime.clone(),
                &update.url,
                &update.checksum_sha256,
                &update.version,
                update.size,
                progress,
            )
            .await
        })
    }

    fn install_app_update(&self, package_path: &Path) -> Result<(), UpdateError> {
        if let Some(installer) = host_app_installer_slot()
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().cloned())
        {
            return installer(package_path);
        }

        self.runtime.install_update(package_path).map_err(|error| {
            UpdateError::runtime(format!("failed to request app update install: {error}"))
        })
    }

    fn log_app_update_warning(&self, detail: &str) {
        log::warn!("{detail}");
    }
}

struct ProgressSink {
    total_bytes: Option<u64>,
    downloaded_bytes: u64,
    reporter: AppUpdateProgressReporter,
}

impl ProgressSink {
    fn new(total_bytes: Option<u64>, reporter: AppUpdateProgressReporter) -> Self {
        Self {
            total_bytes,
            downloaded_bytes: 0,
            reporter,
        }
    }
}

impl BodySink for ProgressSink {
    fn write(&mut self, chunk: &[u8]) -> Result<(), String> {
        self.downloaded_bytes += chunk.len() as u64;
        self.reporter
            .report(self.downloaded_bytes, self.total_bytes);
        Ok(())
    }

    fn close(&mut self, result: &Result<(), String>) {
        if result.is_ok() {
            self.reporter.report(
                self.downloaded_bytes,
                self.total_bytes.or(Some(self.downloaded_bytes)),
            );
        }
    }
}

async fn download_host_app_update_package(
    runtime: Arc<Platform>,
    url: &str,
    checksum_sha256: &str,
    version: &str,
    expected_size: Option<u64>,
    progress: AppUpdateProgressReporter,
) -> Result<PathBuf, UpdateError> {
    log::info!("App update download start: url={url} version={version}");
    let dest_dir = runtime.app_cache_dir().join("lingxia").join("app_updates");
    let _ = fs::create_dir_all(&dest_dir);

    let dest = dest_dir.join(app_update_filename(url, version));
    log::info!("App update download dest: {}", dest.display());

    if dest.exists() {
        if checksum_sha256.is_empty() {
            let existing_size = dest.metadata().map(|m| m.len()).unwrap_or(0);
            if existing_size > 0 {
                log::info!("App update package already downloaded: {}", dest.display());
                progress.report(existing_size, expected_size.or(Some(existing_size)));
                return Ok(dest);
            }
            let _ = fs::remove_file(&dest);
        }
        if verify_sha256(&dest, checksum_sha256).is_ok() {
            log::info!(
                "App update package already downloaded and verified: {}",
                dest.display()
            );
            let existing_size = dest.metadata().map(|m| m.len()).unwrap_or(0);
            progress.report(existing_size, expected_size.or(Some(existing_size)));
            return Ok(dest);
        }
        let _ = fs::remove_file(&dest);
    }

    let file_size = match expected_size.filter(|size| *size > 0) {
        Some(size) => Some(size),
        None => match get_content_length(url).await {
            Ok(size) => Some(size),
            Err(error) => {
                log::debug!("App update content length unavailable: {error}");
                None
            }
        },
    };

    progress.report(0, file_size);
    let sink: Box<dyn BodySink> = Box::new(ProgressSink::new(file_size, progress));

    // Resume on the existing `.part` file when possible. The 10s connect timeout
    // is a sensible cap for weak networks — without it rong_rt falls back to the
    // OS default which can hang far longer.
    let options = service_executor::DownloadOptions::new(url.to_string(), dest.clone())
        .with_sink(sink)
        .with_resume()
        .with_connect_timeout(Duration::from_secs(10));
    let receiver = match service_executor::spawn_download(options, None) {
        Ok(receiver) => receiver,
        Err(error) => {
            return Err(UpdateError::runtime(format!(
                "failed to start download: {error}"
            )));
        }
    };

    match receiver
        .await
        .map_err(|_| UpdateError::runtime("download task cancelled"))?
    {
        Ok(()) => {
            if !checksum_sha256.is_empty() {
                if let Err(error) = verify_sha256(&dest, checksum_sha256) {
                    let _ = fs::remove_file(&dest);
                    Err(error)
                } else {
                    Ok(dest)
                }
            } else {
                Ok(dest)
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&dest);
            Err(UpdateError::runtime(format!("download failed: {error}")))
        }
    }
}

fn app_update_filename(url: &str, version: &str) -> String {
    let safe_version = version.replace(['/', '\\'], "_");
    let main = url.split(&['?', '#'][..]).next().unwrap_or(url);
    let seg = main.rsplit('/').next().unwrap_or(main);
    if !seg.is_empty() && seg.contains('.') {
        format!("app_{safe_version}_{seg}")
    } else {
        format!("app_{}_{}.apk", safe_version, hash_url(url))
    }
}

async fn get_content_length(url: &str) -> Result<u64, String> {
    let request = Request::builder()
        .method("HEAD")
        .uri(url)
        .body(
            Empty::<Bytes>::new()
                .map_err(|_| IoError::other("body error"))
                .boxed(),
        )
        .map_err(|error| format!("failed to build HEAD request: {error}"))?;

    // Add a connect timeout so weak-network HEAD probes don't hang on the
    // system default (which can be a minute or more on some networks).
    let response = host_http::send_with_small_body_limit(
        request,
        1024,
        host_http::RequestOptions::new()
            .with_connect_timeout(Duration::from_secs(10))
            .with_request_timeout(Duration::from_secs(15)),
    )
    .await
    .map_err(|error| format!("HEAD request failed: {error}"))?;

    if !response.status.is_success() {
        return Err(format!("HEAD request returned HTTP {}", response.status));
    }

    response
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| "No Content-Length header".to_string())
}

fn resolve_required_app_version() -> Result<&'static str, UpdateError> {
    let product_version = lingxia_app_context::product_version()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            UpdateError::runtime(
                "app check-update requires productVersion, but app context is missing it",
            )
        })?;

    Version::parse(product_version).map_err(|_| {
        UpdateError::runtime(format!(
            "app productVersion is not semantic version: {product_version}"
        ))
    })?;

    Ok(product_version)
}

fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), UpdateError> {
    if expected_hex.is_empty() {
        return Ok(());
    }
    let actual = compute_sha256_hex(path)?;
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(UpdateError::runtime(format!(
            "checksum mismatch: expected {expected_hex}, got {actual}"
        )))
    }
}

fn compute_sha256_hex(path: &Path) -> Result<String, UpdateError> {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

fn hash_url(url: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
