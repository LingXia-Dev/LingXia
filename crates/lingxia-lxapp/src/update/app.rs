use super::*;
use http::Request;
use http_body_util::{BodyExt, Empty};
use std::io::Error;
use tokio::time::sleep;

pub(super) const APP_UPDATE_START_DELAY: Duration = Duration::from_secs(15);

impl UpdateManager {
    pub(super) fn spawn_app_update_flow_internal(
        runtime: Arc<Platform>,
        current_version: Option<String>,
        start_delay: Duration,
        bypass_cooldown: bool,
    ) {
        let _ = crate::executor::spawn(async move {
            if !start_delay.is_zero() {
                sleep(start_delay).await;
            }

            let app_update_target = UpdateTarget::app(None::<String>).scope_key();
            if !bypass_cooldown && !state::try_acquire_update_check_window(&app_update_target) {
                return;
            }

            let result =
                UpdateManager::check_and_install_app_update(runtime, current_version.as_deref())
                    .await;

            if let Err(err) = result {
                crate::warn!("App update flow failed: {}", err);
            }
        });
    }

    /// Check for host app updates via the registered Provider.
    /// Returns no update if no provider is registered.
    pub async fn check_app_update(
        current_version: Option<&str>,
    ) -> Result<Option<UpdatePackageInfo>, LxAppError> {
        let provider = crate::get_provider();
        let target = UpdateTarget::app(current_version);

        provider.check_update(target).await.map_err(|e| {
            crate::error!("check_app_update failed: {}", e);
            provider_error_to_lxapp_error(&e)
        })
    }

    /// Spawn async flow: check -> prompt -> download -> install for host app updates.
    pub fn spawn_app_update_flow(runtime: Arc<Platform>, current_version: Option<String>) {
        Self::spawn_app_update_flow_internal(
            runtime,
            current_version,
            APP_UPDATE_START_DELAY,
            false,
        );
    }

    /// Check for host app updates and install when user confirms.
    /// Forced updates are non-skippable from UI perspective.
    pub async fn check_and_install_app_update(
        runtime: Arc<Platform>,
        current_version: Option<&str>,
    ) -> Result<(), LxAppError> {
        crate::info!(
            "App update flow start: current_version={:?}",
            current_version
        );
        let update = UpdateManager::check_app_update(current_version).await?;
        let Some(pkg) = update else {
            crate::info!("No app update available");
            return Ok(());
        };
        crate::info!(
            "App update available: version={} url={}",
            pkg.version,
            pkg.url
        );

        let update_info_json = {
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("version".to_string(), serde_json::json!(&pkg.version));
            json_obj.insert(
                "isForceUpdate".to_string(),
                serde_json::json!(pkg.is_force_update),
            );
            if let Some(size) = pkg.size {
                json_obj.insert("size".to_string(), serde_json::json!(size));
            }
            if let Some(notes) = &pkg.release_notes {
                json_obj.insert("releaseNotes".to_string(), serde_json::json!(notes));
            }
            Some(serde_json::to_string(&json_obj).unwrap_or_default())
        };

        let (callback_id, receiver) = lingxia_messaging::get_callback();
        if let Err(e) = runtime.show_update_prompt(callback_id, update_info_json.as_deref()) {
            let _ = lingxia_messaging::remove_callback(callback_id);
            return Err(LxAppError::Runtime(format!(
                "Failed to show update prompt: {}",
                e
            )));
        }

        let confirmed = match receiver.await {
            Ok(lingxia_messaging::CallbackResult::Success(data)) => {
                serde_json::from_str::<Value>(&data)
                    .ok()
                    .and_then(|json| json.get("confirm").and_then(|v| v.as_bool()))
                    .unwrap_or(false)
            }
            Ok(lingxia_messaging::CallbackResult::Error(_)) => false,
            Err(_) => false,
        };

        if !confirmed && pkg.is_force_update {
            return Err(LxAppError::Runtime(
                "Forced app update was not confirmed".to_string(),
            ));
        }

        if !confirmed {
            crate::info!("App update cancelled or deferred");
            return Ok(());
        }
        crate::info!("App update confirmed, starting download");

        let path = UpdateManager::download_app_update_with_checksum(
            runtime.clone(),
            &pkg.url,
            &pkg.checksum_sha256,
            &pkg.version,
        )
        .await?;
        crate::info!("App update downloaded: {}", path.display());

        runtime.install_update(&path).map_err(|e| {
            LxAppError::Runtime(format!("Failed to request app update install: {}", e))
        })?;
        crate::info!("App update install requested");

        Ok(())
    }

    /// Download a host app update package and verify checksum when provided.
    pub async fn download_app_update_with_checksum(
        runtime: Arc<Platform>,
        url: &str,
        checksum_sha256: &str,
        version: &str,
    ) -> Result<PathBuf, LxAppError> {
        crate::info!("App update download start: url={} version={}", url, version);
        let dest_dir = runtime
            .app_cache_dir()
            .join(LINGXIA_DIR)
            .join("app_updates");
        let _ = fs::create_dir_all(&dest_dir);

        let dest = dest_dir.join(app_update_filename(url, version));
        crate::info!("App update download dest: {}", dest.display());

        if dest.exists() {
            if checksum_sha256.is_empty() {
                if dest.metadata().map(|m| m.len()).unwrap_or(0) > 0 {
                    crate::info!("App update package already downloaded: {}", dest.display());
                    let _ = runtime.dismiss_download_progress();
                    return Ok(dest);
                }
                let _ = fs::remove_file(&dest);
            }
            if archive::verify_sha256(&dest, checksum_sha256).is_ok() {
                crate::info!(
                    "App update package already downloaded and verified: {}",
                    dest.display()
                );
                let _ = runtime.dismiss_download_progress();
                return Ok(dest);
            }
            let _ = fs::remove_file(&dest);
        }

        let file_size = get_content_length(url).await.unwrap_or(0);

        if let Err(e) = runtime.show_download_progress() {
            crate::warn!("Failed to show download progress: {}", e);
        }

        let sink: Option<Box<dyn BodySink>> = if file_size > 0 {
            Some(Box::new(ProgressSink::new(
                file_size,
                Some(runtime.clone()),
            )))
        } else {
            None
        };

        let receiver =
            match service_executor::request_download(url.to_string(), dest.clone(), None, sink) {
                Ok(receiver) => receiver,
                Err(e) => {
                    let _ = runtime.dismiss_download_progress();
                    return Err(LxAppError::IoError(format!(
                        "failed to start download: {}",
                        e
                    )));
                }
            };

        let result = match receiver
            .await
            .map_err(|_| LxAppError::IoError("download task cancelled".to_string()))?
        {
            Ok(()) => {
                if !checksum_sha256.is_empty() {
                    if let Err(e) = archive::verify_sha256(&dest, checksum_sha256) {
                        let _ = fs::remove_file(&dest);
                        Err(e)
                    } else {
                        Ok(dest)
                    }
                } else {
                    Ok(dest)
                }
            }
            Err(err) => {
                let _ = fs::remove_file(&dest);
                Err(LxAppError::IoError(format!("download failed: {}", err)))
            }
        };

        let _ = runtime.dismiss_download_progress();
        result
    }
}

fn app_update_filename(url: &str, version: &str) -> String {
    let safe_version = version.replace(['/', '\\'], "_");
    let main = url.split(&['?', '#'][..]).next().unwrap_or(url);
    let seg = main.rsplit('/').next().unwrap_or(main);
    if !seg.is_empty() && seg.contains('.') {
        format!("app_{}_{}", safe_version, seg)
    } else {
        format!("app_{}_{}.apk", safe_version, UpdateManager::hash_url(url))
    }
}

async fn get_content_length(url: &str) -> Result<u64, String> {
    let request = Request::builder()
        .method("HEAD")
        .uri(url)
        .body(
            Empty::<bytes::Bytes>::new()
                .map_err(|_| Error::other("body error"))
                .boxed(),
        )
        .map_err(|e| format!("Failed to build HEAD request: {}", e))?;

    let response =
        host_http::send_with_small_body_limit(request, 1024, host_http::RequestOptions::new())
            .await
            .map_err(|e| format!("HEAD request failed: {}", e))?;

    if let Some(content_length) = response
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        Ok(content_length)
    } else {
        Err("No Content-Length header".to_string())
    }
}
