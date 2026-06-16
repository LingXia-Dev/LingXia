use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_internal_error, js_invalid_parameter_error,
};
use lingxia_messaging::{CallbackResult, get_callback, get_stream_callback, remove_callback};
use lingxia_platform::traits::media_runtime::{
    CompressVideoRequest, ExtractVideoThumbnailRequest, MediaRuntime, VideoCompressQuality,
    VideoInfo as PlatformVideoInfo,
};
use lingxia_service::storage;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, HostError, IntoJSObj, JSContext, JSFunc, JSObject, JSResult, Promise};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, mpsc};

static THUMBNAIL_NAME_COUNTER: AtomicU64 = AtomicU64::new(0);
static COMPRESS_VIDEO_NAME_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(FromJSObj)]
struct JSGetVideoInfoOptions {
    path: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSVideoInfoResult {
    width: u32,
    height: u32,
    #[rename = "durationMs"]
    duration_ms: u64,
    rotation: Option<u16>,
    bitrate: Option<u64>,
    fps: Option<f64>,
    #[rename = "type"]
    video_type: Option<String>,
    path: String,
}

#[derive(FromJSObj)]
struct JSVideoThumbnailOptions {
    path: String,
    #[rename = "outputPath"]
    output_path: Option<String>,
    #[rename = "maxWidth"]
    max_width: Option<u32>,
    #[rename = "maxHeight"]
    max_height: Option<u32>,
    #[rename = "timeMs"]
    time_ms: Option<i64>,
    quality: Option<i32>,
}

#[derive(FromJSObj)]
struct JSCompressVideoOptions {
    path: String,
    #[rename = "outputPath"]
    output_path: Option<String>,
    quality: Option<String>,
    bitrate: Option<u32>,
    fps: Option<u32>,
    resolution: Option<f64>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSVideoThumbnailResult {
    #[rename = "tempFilePath"]
    temp_file_path: String,
    width: u32,
    height: u32,
    #[rename = "type"]
    image_type: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSCompressVideoResult {
    #[rename = "tempFilePath"]
    temp_file_path: String,
    width: u32,
    height: u32,
    #[rename = "durationMs"]
    duration_ms: u64,
    size: u64,
    #[rename = "type"]
    video_type: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSCompressProgressEvent {
    progress: u8,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSCompressIteratorStep {
    done: bool,
    value: Option<JSCompressProgressEvent>,
}

/// Completion payload sent by the platform natives.
#[derive(Deserialize)]
struct NativeCompressVideoResult {
    success: bool,
    error: Option<String>,
    path: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
    size: Option<u64>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
}

#[derive(Deserialize)]
struct NativeCompressProgressEvent {
    progress: u8,
}

struct CompressProgressState {
    receiver: Option<mpsc::Receiver<CallbackResult>>,
    last_progress: Option<u8>,
    closed: bool,
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_video_info_func = JSFunc::new(ctx, get_video_info_api)?;
    lx::register_js_api(ctx, "getVideoInfo", get_video_info_func)?;

    let extract_video_thumbnail_func = JSFunc::new(ctx, extract_video_thumbnail_api)?;
    lx::register_js_api(ctx, "extractVideoThumbnail", extract_video_thumbnail_func)?;

    let compress_video_func = JSFunc::new(ctx, compress_video_api)?;
    lx::register_js_api(ctx, "compressVideo", compress_video_func)?;
    Ok(())
}

async fn get_video_info_api(
    ctx: JSContext,
    options: JSGetVideoInfoOptions,
) -> JSResult<JSVideoInfoResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let original_path = options.path;
    let trimmed_path = original_path.trim();
    let resolved = lxapp
        .resolve_accessible_path(trimmed_path)
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let normalized_path = resolved.to_string_lossy().into_owned();

    let response_path = if trimmed_path.starts_with("lx://")
        || is_bundle_relative_path(trimmed_path)
    {
        trimmed_path.to_string()
    } else {
        lxapp
            .to_uri(&resolved)
            .ok_or_else(|| js_internal_error("getVideoInfo failed to convert path to lx:// uri"))?
            .into_string()
    };

    runtime
        .get_video_info(&normalized_path)
        .map(|info| platform_video_info_to_js(info, response_path))
        .map_err(|e| js_error_from_platform_error(&e))
}

async fn extract_video_thumbnail_api(
    ctx: JSContext,
    options: JSVideoThumbnailOptions,
) -> JSResult<JSVideoThumbnailResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let resolved_source = lxapp
        .resolve_accessible_path(options.path.trim())
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let output_path = resolve_thumbnail_output_path(&lxapp, options.output_path.as_deref())?;
    let request = ExtractVideoThumbnailRequest {
        source_uri,
        output_path,
        max_width: sanitize_optional_u32(options.max_width),
        max_height: sanitize_optional_u32(options.max_height),
        time_ms: sanitize_time_ms(options.time_ms),
        quality: clamp_quality(options.quality),
    };

    let thumbnail = runtime
        .extract_video_thumbnail(&request)
        .map_err(|e| js_error_from_platform_error(&e))?;
    ensure_output_quota(&lxapp, &thumbnail.path)?;

    let uri = lxapp
        .to_uri(&thumbnail.path)
        .ok_or_else(|| {
            js_internal_error("extractVideoThumbnail failed to convert output path to lx:// uri")
        })?
        .into_string();

    Ok(JSVideoThumbnailResult {
        temp_file_path: uri,
        width: thumbnail.width,
        height: thumbnail.height,
        image_type: thumbnail
            .mime_type
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "image/jpeg".to_string()),
    })
}

fn compress_video_api(ctx: JSContext, options: JSCompressVideoOptions) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = lxapp.runtime.clone();

    let resolved_source = lxapp
        .resolve_accessible_path(options.path.trim())
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let output_path = resolve_compress_video_output_path(&lxapp, options.output_path.as_deref())?;
    if paths_refer_to_same_file(&resolved_source, &output_path) {
        return Err(js_invalid_parameter_error(
            "compressVideo outputPath must be different from source path",
        ));
    }
    let quality = parse_video_quality(options.quality.as_deref())?;
    let (bitrate_kbps, fps, resolution_ratio) = if quality.is_some() {
        (None, None, None)
    } else {
        (
            sanitize_optional_u32(options.bitrate),
            sanitize_optional_u32(options.fps),
            sanitize_resolution(options.resolution)?,
        )
    };

    let (progress_callback_id, progress_rx) = get_stream_callback();
    let (callback_id, completion_rx) = get_callback();

    let request = CompressVideoRequest {
        source_uri,
        quality,
        bitrate_kbps,
        fps,
        resolution_ratio,
        output_path: output_path.clone(),
        progress_callback_id,
        callback_id,
    };

    if let Err(err) = runtime.compress_video(&request) {
        remove_callback(progress_callback_id);
        remove_callback(callback_id);
        return Err(js_error_from_platform_error(&err));
    }

    let cancelled = Arc::new(AtomicBool::new(false));

    let final_lxapp = lxapp.clone();
    let final_promise = Promise::from_future(&ctx, None, async move {
        let result = completion_rx.await;
        // The transcode is over (or cancelled): close the progress stream so
        // `for await` loops over the task finish.
        remove_callback(progress_callback_id);
        match result {
            Ok(CallbackResult::Success(json)) => {
                let parsed: NativeCompressVideoResult =
                    serde_json::from_str(&json).map_err(|err| {
                        js_internal_error(format!("compressVideo returned invalid payload: {err}"))
                    })?;
                if !parsed.success {
                    return Err(js_internal_error(
                        parsed
                            .error
                            .unwrap_or_else(|| "compressVideo failed".to_string()),
                    ));
                }
                let path = PathBuf::from(parsed.path.ok_or_else(|| {
                    js_internal_error("compressVideo result is missing the output path")
                })?);
                ensure_output_quota(&final_lxapp, &path)?;
                let temp_file_path = final_lxapp
                    .to_uri(&path)
                    .ok_or_else(|| {
                        js_internal_error(
                            "compressVideo failed to convert output path to lx:// uri",
                        )
                    })?
                    .into_string();
                Ok(JSCompressVideoResult {
                    temp_file_path,
                    width: parsed.width.unwrap_or(0),
                    height: parsed.height.unwrap_or(0),
                    duration_ms: parsed.duration_ms.unwrap_or(0),
                    size: parsed.size.unwrap_or(0),
                    video_type: parsed
                        .mime_type
                        .filter(|m| !m.is_empty())
                        .unwrap_or_else(|| "video/mp4".to_string()),
                })
            }
            Ok(CallbackResult::Error(code)) => Err(js_internal_error(format!(
                "compressVideo failed with code {code}"
            ))),
            // The oneshot sender is dropped when cancel() removes the callback.
            Err(_) => Err(HostError::new(rong::error::E_ABORT, "compressVideo canceled")
                .with_name("AbortError")
                .into()),
        }
    })?;

    let state = Arc::new(Mutex::new(CompressProgressState {
        receiver: Some(progress_rx),
        last_progress: None,
        closed: false,
    }));
    let task = JSObject::new(&ctx);

    let next_state = state.clone();
    task.set(
        "next",
        JSFunc::new(&ctx, move || {
            let state = next_state.clone();
            async move { compress_progress_next_step(&state).await }
        })?,
    )?;

    let return_state = state.clone();
    task.set(
        "return",
        JSFunc::new(&ctx, move || {
            let state = return_state.clone();
            async move {
                let mut guard = state.lock().await;
                guard.closed = true;
                guard.receiver = None;
                Ok(JSCompressIteratorStep {
                    done: true,
                    value: None,
                })
            }
        })?,
    )?;

    let cancel_output_path = output_path;
    task.set(
        "cancel",
        JSFunc::new(&ctx, move || {
            if cancelled.swap(true, Ordering::SeqCst) {
                return Ok(());
            }
            let _ = runtime.cancel_compress_video(callback_id);
            remove_callback(progress_callback_id);
            remove_callback(callback_id);
            let _ = fs::remove_file(&cancel_output_path);
            Ok(())
        })?,
    )?;

    crate::task_object::install_promise_methods(&ctx, &task, final_promise)?;
    crate::task_object::install_async_iterator(&ctx, &task)?;
    Ok(task)
}

async fn compress_progress_next_step(
    state: &Arc<Mutex<CompressProgressState>>,
) -> JSResult<JSCompressIteratorStep> {
    loop {
        let (mut receiver, last_progress) = {
            let mut guard = state.lock().await;
            if guard.closed {
                return Ok(JSCompressIteratorStep {
                    done: true,
                    value: None,
                });
            }
            let Some(receiver) = guard.receiver.take() else {
                return Ok(JSCompressIteratorStep {
                    done: true,
                    value: None,
                });
            };
            (receiver, guard.last_progress)
        };

        let event = receiver.recv().await;

        let mut guard = state.lock().await;
        if guard.closed {
            return Ok(JSCompressIteratorStep {
                done: true,
                value: None,
            });
        }

        let Some(event) = event else {
            guard.receiver = None;
            return Ok(JSCompressIteratorStep {
                done: true,
                value: None,
            });
        };
        guard.receiver = Some(receiver);

        let CallbackResult::Success(json) = event else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<NativeCompressProgressEvent>(&json) else {
            continue;
        };
        let progress = parsed.progress.min(100);
        // Natives poll their encoders, so consecutive ticks often repeat.
        if last_progress == Some(progress) {
            continue;
        }
        guard.last_progress = Some(progress);
        return Ok(JSCompressIteratorStep {
            done: false,
            value: Some(JSCompressProgressEvent { progress }),
        });
    }
}

fn resolve_thumbnail_output_path(
    lxapp: &LxApp,
    raw_output_path: Option<&str>,
) -> JSResult<PathBuf> {
    resolve_output_path(lxapp, raw_output_path, || {
        generate_thumbnail_output_path(&lxapp.temp_dir)
    })
}

fn resolve_compress_video_output_path(
    lxapp: &LxApp,
    raw_output_path: Option<&str>,
) -> JSResult<PathBuf> {
    resolve_output_path(lxapp, raw_output_path, || {
        generate_compress_video_output_path(&lxapp.temp_dir)
    })
}

fn resolve_output_path<F>(
    lxapp: &LxApp,
    raw_output_path: Option<&str>,
    default: F,
) -> JSResult<PathBuf>
where
    F: FnOnce() -> JSResult<PathBuf>,
{
    match raw_output_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(path) => lxapp
            .resolve_accessible_path(path)
            .map_err(|err| js_error_from_lxapp_error(&err)),
        None => default(),
    }
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    let left = comparable_path(left);
    let right = comparable_path(right);
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn comparable_path(path: &Path) -> PathBuf {
    if let Ok(path) = fs::canonicalize(path) {
        return path;
    }
    if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name())
        && let Ok(parent) = fs::canonicalize(parent)
    {
        return parent.join(file_name);
    }
    path.to_path_buf()
}

fn platform_video_info_to_js(info: PlatformVideoInfo, path: String) -> JSVideoInfoResult {
    JSVideoInfoResult {
        width: info.width,
        height: info.height,
        duration_ms: info.duration_ms,
        rotation: info.rotation,
        bitrate: info.bitrate,
        fps: info.fps.map(|v| v as f64),
        video_type: info.mime_type,
        path,
    }
}

fn parse_video_quality(value: Option<&str>) -> JSResult<Option<VideoCompressQuality>> {
    let Some(raw) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let quality = match raw.to_ascii_lowercase().as_str() {
        "low" => VideoCompressQuality::Low,
        "medium" => VideoCompressQuality::Medium,
        "high" => VideoCompressQuality::High,
        _ => {
            return Err(js_invalid_parameter_error(
                "compressVideo quality must be one of: low, medium, high",
            ));
        }
    };
    Ok(Some(quality))
}

fn sanitize_optional_u32(value: Option<u32>) -> Option<u32> {
    value.filter(|v| *v > 0)
}

fn sanitize_resolution(value: Option<f64>) -> JSResult<Option<f32>> {
    let Some(v) = value else {
        return Ok(None);
    };
    if !v.is_finite() || v <= 0.0 || v > 1.0 {
        return Err(js_invalid_parameter_error(
            "compressVideo resolution must be in range (0, 1]",
        ));
    }
    Ok(Some(v as f32))
}

fn sanitize_time_ms(value: Option<i64>) -> Option<u64> {
    match value {
        Some(v) if v >= 0 => Some(v as u64),
        _ => None,
    }
}

fn clamp_quality(value: Option<i32>) -> u8 {
    let raw = value.unwrap_or(80);
    raw.clamp(0, 100) as u8
}

fn is_bundle_relative_path(value: &str) -> bool {
    !Path::new(value).is_absolute() && !value.contains(':')
}

fn ensure_dir(path: &Path) -> JSResult<()> {
    if let Err(err) = fs::create_dir_all(path) {
        return Err(js_internal_error(format!(
            "Failed to prepare directory {}: {}",
            path.display(),
            err
        )));
    }
    Ok(())
}

fn generate_thumbnail_output_path(cache_root: &Path) -> JSResult<PathBuf> {
    generate_timestamped_output_path(
        &cache_root.join("video-thumbnail"),
        "vx",
        "jpg",
        &THUMBNAIL_NAME_COUNTER,
    )
}

fn generate_compress_video_output_path(cache_root: &Path) -> JSResult<PathBuf> {
    generate_timestamped_output_path(
        &cache_root.join("video-compress"),
        "vx_comp",
        "mp4",
        &COMPRESS_VIDEO_NAME_COUNTER,
    )
}

fn generate_timestamped_output_path(
    base_dir: &Path,
    prefix: &str,
    ext: &str,
    counter: &AtomicU64,
) -> JSResult<PathBuf> {
    ensure_dir(base_dir)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let nonce = counter.fetch_add(1, Ordering::Relaxed);
    let filename = format!("{}_{}_{}.{}", prefix, timestamp, nonce, ext);

    Ok(base_dir.join(filename))
}

fn ensure_output_quota(lxapp: &LxApp, path: &Path) -> JSResult<()> {
    let size = storage::path_size(path);
    let result = if path.starts_with(&lxapp.temp_dir) {
        storage::ensure_temp_quota(&lxapp.temp_dir, path, size)
    } else if path.starts_with(&lxapp.user_data_dir) {
        storage::ensure_userdata_quota(&lxapp.user_data_dir, path, size).and_then(|()| {
            storage::ensure_app_storage_quota(
                &lxapp.user_data_dir,
                &lxapp.user_cache_dir,
                path,
                size,
            )
        })
    } else if path.starts_with(&lxapp.user_cache_dir) {
        storage::ensure_usercache_quota(&lxapp.user_cache_dir, path, size, None).and_then(|()| {
            storage::ensure_app_storage_quota(
                &lxapp.user_data_dir,
                &lxapp.user_cache_dir,
                path,
                size,
            )
        })
    } else {
        Ok(())
    };

    match result {
        Ok(()) => {
            if path.starts_with(&lxapp.user_cache_dir) {
                lxapp::touch_access_time(path);
            }
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::remove_file(path);
            Err(js_error_from_business_code_with_detail(1002, err.detail()))
        }
    }
}
