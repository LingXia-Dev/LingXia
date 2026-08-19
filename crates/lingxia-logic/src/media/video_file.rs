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
use lxapp::LxApp;
use rong::{FromJSObject, HostError, IntoJSObject, JSContext, JSFunc, JSObject, JSResult, Promise};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, mpsc};

static THUMBNAIL_NAME_COUNTER: AtomicU64 = AtomicU64::new(0);
static COMPRESS_VIDEO_NAME_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VideoOutputStorage {
    Temp,
    UserData,
    UserCache,
}

struct VideoOutputRoots<'a> {
    temp: &'a Path,
    user_data: &'a Path,
    user_cache: &'a Path,
}

struct ValidatedVideoOutput {
    storage: VideoOutputStorage,
    size: u64,
    path: PathBuf,
    root: PathBuf,
}

#[derive(FromJSObject)]
#[ts_skip]
struct JSGetVideoInfoOptions {
    path: String,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSVideoInfoResult {
    width: u32,
    height: u32,
    #[js_name = "durationMs"]
    duration_ms: u64,
    size: u64,
    rotation: Option<u16>,
    bitrate: Option<u64>,
    fps: Option<f64>,
    #[js_name = "type"]
    video_type: Option<String>,
    #[js_name = "videoCodec"]
    video_codec: Option<String>,
    #[js_name = "hasAudio"]
    has_audio: Option<bool>,
    #[js_name = "audioCodec"]
    audio_codec: Option<String>,
    path: String,
}

#[derive(FromJSObject)]
#[ts_skip]
struct JSVideoThumbnailOptions {
    path: String,
    #[js_name = "outputPath"]
    output_path: Option<String>,
    #[js_name = "maxWidth"]
    max_width: Option<u32>,
    #[js_name = "maxHeight"]
    max_height: Option<u32>,
    #[js_name = "timeMs"]
    time_ms: Option<i64>,
    quality: Option<i32>,
}

#[derive(FromJSObject)]
#[ts_skip]
struct JSCompressVideoOptions {
    path: String,
    #[js_name = "outputPath"]
    output_path: Option<String>,
    quality: Option<String>,
    bitrate: Option<u32>,
    fps: Option<u32>,
    resolution: Option<f64>,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSVideoThumbnailResult {
    #[js_name = "tempFilePath"]
    temp_file_path: String,
    width: u32,
    height: u32,
    #[js_name = "type"]
    image_type: String,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSCompressVideoResult {
    #[js_name = "tempFilePath"]
    temp_file_path: String,
    width: u32,
    height: u32,
    #[js_name = "durationMs"]
    duration_ms: u64,
    size: u64,
    #[js_name = "type"]
    video_type: String,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSCompressProgressEvent {
    progress: u8,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
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

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getVideoInfo(
            ts_params = "options: GetVideoInfoOptions",
            ts_return = "Promise<VideoInfo>"
        ) = get_video_info_api;
        fn extractVideoThumbnail(
            ts_params = "options: ExtractVideoThumbnailOptions",
            ts_return = "Promise<ExtractVideoThumbnailResult>"
        ) = extract_video_thumbnail_api;
        fn compressVideo(
            ts_params = "options: CompressVideoOptions",
            ts_return = "CompressVideoTask"
        ) = compress_video_api;
    }
}

/// Reads local video metadata for upload preflight and presentation.
///
/// Size, dimensions, duration, and path form the portable core. Container type,
/// rotation, and track-level codec/audio fields are best-effort and may be
/// omitted when the platform cannot determine them. The receiving service must
/// still validate the uploaded bytes.
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

/// Write one frame of a video out as an image file.
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

    let explicit_output_uri = explicit_lx_output_uri(options.output_path.as_deref());
    let output_path = resolve_thumbnail_output_path(&lxapp, options.output_path.as_deref())?;
    ensure_distinct_video_paths(&resolved_source, &output_path, "extractVideoThumbnail")?;
    let request = ExtractVideoThumbnailRequest {
        source_uri,
        output_path: output_path.clone(),
        max_width: sanitize_optional_u32(options.max_width),
        max_height: sanitize_optional_u32(options.max_height),
        time_ms: sanitize_time_ms(options.time_ms),
        quality: clamp_quality(options.quality),
    };

    let thumbnail = runtime
        .extract_video_thumbnail(&request)
        .map_err(|e| js_error_from_platform_error(&e))?;
    ensure_output_quota(&lxapp, &output_path, &thumbnail.path)?;

    let uri = match explicit_output_uri {
        Some(uri) => uri,
        None => lxapp
            .to_uri(&thumbnail.path)
            .ok_or_else(|| {
                js_internal_error(
                    "extractVideoThumbnail failed to convert output path to lx:// uri",
                )
            })?
            .into_string(),
    };

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

/// Transcode a video to a smaller file.
///
/// Returns a task handle synchronously, so progress and cancellation can be
/// wired up before transcoding starts.
fn compress_video_api(ctx: JSContext, options: JSCompressVideoOptions) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = lxapp.runtime.clone();

    let resolved_source = lxapp
        .resolve_accessible_path(options.path.trim())
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let explicit_output_uri = explicit_lx_output_uri(options.output_path.as_deref());
    let output_path = resolve_compress_video_output_path(&lxapp, options.output_path.as_deref())?;
    ensure_distinct_video_paths(&resolved_source, &output_path, "compressVideo")?;
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
    let final_output_path = output_path.clone();
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
                ensure_output_quota(&final_lxapp, &final_output_path, &path)?;
                let temp_file_path = match explicit_output_uri {
                    Some(uri) => uri,
                    None => final_lxapp
                        .to_uri(&path)
                        .ok_or_else(|| {
                            js_internal_error(
                                "compressVideo failed to convert output path to lx:// uri",
                            )
                        })?
                        .into_string(),
                };
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
            Err(_) => Err(
                HostError::new(rong::error::E_ABORT, "compressVideo canceled")
                    .with_name("AbortError")
                    .into(),
            ),
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
    resolve_output_path(lxapp, raw_output_path, "extractVideoThumbnail", || {
        generate_thumbnail_output_path(&lxapp.temp_dir)
    })
}

fn resolve_compress_video_output_path(
    lxapp: &LxApp,
    raw_output_path: Option<&str>,
) -> JSResult<PathBuf> {
    resolve_output_path(lxapp, raw_output_path, "compressVideo", || {
        generate_compress_video_output_path(&lxapp.temp_dir)
    })
}

fn resolve_output_path<F>(
    lxapp: &LxApp,
    raw_output_path: Option<&str>,
    api_name: &'static str,
    default: F,
) -> JSResult<PathBuf>
where
    F: FnOnce() -> JSResult<PathBuf>,
{
    match raw_output_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(path) => {
            crate::fs::resolve_writable_file_path(lxapp, path, api_name, "outputPath", true)
        }
        None => default(),
    }
}

fn explicit_lx_output_uri(raw_output_path: Option<&str>) -> Option<String> {
    raw_output_path
        .map(str::trim)
        .filter(|path| path.starts_with("lx://"))
        .map(str::to_string)
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

fn ensure_distinct_video_paths(source: &Path, output: &Path, api_name: &str) -> JSResult<()> {
    if paths_refer_to_same_file(source, output) {
        return Err(js_invalid_parameter_error(format!(
            "{api_name} outputPath must be different from source path"
        )));
    }
    Ok(())
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
        size: info.size,
        rotation: info.rotation,
        bitrate: info.bitrate,
        fps: info.fps.map(|v| v as f64),
        video_type: info.mime_type,
        video_codec: normalize_video_codec_mime(info.video_codec),
        has_audio: info.has_audio,
        audio_codec: info.audio_codec,
        path,
    }
}

fn normalize_video_codec_mime(codec: Option<String>) -> Option<String> {
    let codec = codec?.trim().to_ascii_lowercase();
    let normalized = match codec.as_str() {
        "avc" | "avc1" | "avc3" | "h264" | "h.264" | "video/avc" | "video/avc1" | "video/avc3"
        | "video/h264" | "video/x-h264" => "video/avc",
        "hevc" | "hev1" | "hvc1" | "h265" | "h.265" | "video/hevc" | "video/hev1"
        | "video/hvc1" | "video/h265" | "video/x-h265" => "video/hevc",
        "vp8" | "vp08" | "video/vp8" | "video/x-vnd.on2.vp8" => "video/x-vnd.on2.vp8",
        "vp9" | "vp09" | "video/vp9" | "video/x-vnd.on2.vp9" => "video/x-vnd.on2.vp9",
        "av1" | "av01" | "video/av1" | "video/av01" => "video/av01",
        "mp4v" | "video/mp4v" | "video/mp4v-es" => "video/mp4v-es",
        "mpeg2" | "mpeg-2" | "video/mpeg2" => "video/mpeg2",
        "jpeg" | "mjpeg" | "video/jpeg" | "video/mjpeg" => "video/mjpeg",
        _ if codec.starts_with("video/")
            && codec.len() > "video/".len()
            && !codec.chars().any(char::is_whitespace) =>
        {
            return Some(codec);
        }
        _ => return None,
    };
    Some(normalized.to_string())
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

fn validate_video_output(
    expected_path: &Path,
    actual_path: &Path,
    roots: VideoOutputRoots<'_>,
) -> Result<ValidatedVideoOutput, String> {
    if actual_path != expected_path {
        return Err(format!(
            "video runtime returned unexpected output path: expected {}, got {}",
            expected_path.display(),
            actual_path.display()
        ));
    }

    let metadata = fs::symlink_metadata(actual_path).map_err(|err| {
        format!(
            "video runtime output is missing or unreadable at {}: {err}",
            actual_path.display()
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(format!(
            "video runtime output is not a regular file: {}",
            actual_path.display()
        ));
    }

    let canonical_path = fs::canonicalize(actual_path)
        .map_err(|err| format!("failed to resolve video output path: {err}"))?;
    let candidates = [
        (VideoOutputStorage::Temp, roots.temp),
        (VideoOutputStorage::UserData, roots.user_data),
        (VideoOutputStorage::UserCache, roots.user_cache),
    ];
    let mut matched = None;
    for (storage, root) in candidates {
        if root.as_os_str().is_empty() {
            continue;
        }
        let Ok(canonical_root) = fs::canonicalize(root) else {
            continue;
        };
        if canonical_path.starts_with(&canonical_root) {
            matched = Some((storage, root, canonical_root));
            break;
        }
    }
    let (storage, root, canonical_root) = matched
        .ok_or_else(|| "video output path is outside LingXia-managed storage".to_string())?;

    let (inspection_root, inspection_path) = if actual_path.starts_with(root) {
        (root, actual_path)
    } else {
        (canonical_root.as_path(), canonical_path.as_path())
    };
    let relative = inspection_path
        .strip_prefix(inspection_root)
        .map_err(|_| "video output path is outside its storage root".to_string())?;
    let mut current = inspection_root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_none() {
            break;
        }
        current.push(component.as_os_str());
        let ancestor = fs::symlink_metadata(&current).map_err(|err| {
            format!(
                "failed to inspect video output ancestor {}: {err}",
                current.display()
            )
        })?;
        if ancestor.file_type().is_symlink() {
            return Err(format!(
                "video output path must not pass through a symlink: {}",
                current.display()
            ));
        }
    }

    Ok(ValidatedVideoOutput {
        storage,
        size: metadata.len(),
        path: canonical_path,
        root: canonical_root,
    })
}

fn ensure_output_quota(lxapp: &LxApp, expected_path: &Path, actual_path: &Path) -> JSResult<()> {
    let validated = validate_video_output(
        expected_path,
        actual_path,
        VideoOutputRoots {
            temp: &lxapp.temp_dir,
            user_data: &lxapp.user_data_dir,
            user_cache: &lxapp.user_cache_dir,
        },
    )
    .map_err(js_internal_error)?;
    let result = match validated.storage {
        VideoOutputStorage::Temp => {
            storage::ensure_temp_quota(&validated.root, &validated.path, validated.size)
        }
        VideoOutputStorage::UserData => {
            storage::ensure_userdata_quota(&validated.root, &validated.path, validated.size)
                .and_then(|()| {
                    storage::ensure_app_storage_quota(
                        &validated.root,
                        &lxapp.user_cache_dir,
                        &validated.path,
                        validated.size,
                    )
                })
        }
        VideoOutputStorage::UserCache => {
            storage::ensure_usercache_quota(&validated.root, &validated.path, validated.size, None)
                .and_then(|()| {
                    storage::ensure_app_storage_quota(
                        &lxapp.user_data_dir,
                        &validated.root,
                        &validated.path,
                        validated.size,
                    )
                })
        }
    };

    match result {
        Ok(()) => {
            if validated.storage == VideoOutputStorage::UserCache {
                lxapp::touch_access_time(&validated.path);
            }
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(&validated.path);
            Err(js_error_from_business_code_with_detail(1002, err.detail()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PlatformVideoInfo, VideoOutputRoots, VideoOutputStorage, ensure_distinct_video_paths,
        explicit_lx_output_uri, normalize_video_codec_mime, platform_video_info_to_js,
        validate_video_output,
    };
    use std::fs;

    fn create_output_roots(base: &std::path::Path) -> [std::path::PathBuf; 3] {
        let roots = [
            base.join("temp"),
            base.join("userdata"),
            base.join("usercache"),
        ];
        for root in &roots {
            fs::create_dir_all(root).unwrap();
        }
        roots
    }

    #[test]
    fn video_output_validation_accepts_expected_regular_managed_file() {
        let base = tempfile::tempdir().unwrap();
        let [temp, user_data, user_cache] = create_output_roots(base.path());
        let output = user_data.join("videos/output.mp4");
        fs::create_dir_all(output.parent().unwrap()).unwrap();
        fs::write(&output, b"video").unwrap();

        let validated = validate_video_output(
            &output,
            &output,
            VideoOutputRoots {
                temp: &temp,
                user_data: &user_data,
                user_cache: &user_cache,
            },
        )
        .unwrap();

        assert_eq!(validated.storage, VideoOutputStorage::UserData);
        assert_eq!(validated.size, 5);
    }

    #[test]
    fn video_outputs_must_not_replace_the_source() {
        let base = tempfile::tempdir().unwrap();
        let source = base.path().join("clip.mp4");
        let other = base.path().join("thumbnail.jpg");
        fs::write(&source, b"video").unwrap();

        assert!(ensure_distinct_video_paths(&source, &source, "extractVideoThumbnail").is_err());
        assert!(ensure_distinct_video_paths(&source, &other, "extractVideoThumbnail").is_ok());
    }

    #[test]
    fn video_output_validation_accepts_explicit_temp_file() {
        let base = tempfile::tempdir().unwrap();
        let [temp, user_data, user_cache] = create_output_roots(base.path());
        let output = temp.join("explicit-output.mp4");
        fs::write(&output, b"video").unwrap();

        let validated = validate_video_output(
            &output,
            &output,
            VideoOutputRoots {
                temp: &temp,
                user_data: &user_data,
                user_cache: &user_cache,
            },
        )
        .unwrap();

        assert_eq!(validated.storage, VideoOutputStorage::Temp);
    }

    #[test]
    fn video_output_validation_accepts_canonical_temp_root_alias() {
        let base = tempfile::tempdir().unwrap();
        let alias_parent = base.path().join("alias-parent");
        fs::create_dir(&alias_parent).unwrap();
        let temp = base.path().join("temp");
        let user_data = base.path().join("userdata");
        let user_cache = base.path().join("usercache");
        for root in [&temp, &user_data, &user_cache] {
            fs::create_dir(root).unwrap();
        }
        let aliased_temp = alias_parent.join("..").join("temp");
        let output = temp.join("explicit-output.mp4");
        fs::write(&output, b"video").unwrap();

        let validated = validate_video_output(
            &output,
            &output,
            VideoOutputRoots {
                temp: &aliased_temp,
                user_data: &user_data,
                user_cache: &user_cache,
            },
        )
        .unwrap();

        assert_eq!(validated.storage, VideoOutputStorage::Temp);
        assert_eq!(validated.root, fs::canonicalize(temp).unwrap());
        assert_eq!(
            explicit_lx_output_uri(Some(" lx://temp/existing-token ")).as_deref(),
            Some("lx://temp/existing-token")
        );
    }

    #[test]
    fn video_output_validation_rejects_mismatch_missing_and_directory() {
        let base = tempfile::tempdir().unwrap();
        let [temp, user_data, user_cache] = create_output_roots(base.path());
        let expected = user_data.join("expected.mp4");
        let other = user_data.join("other.mp4");
        fs::write(&other, b"video").unwrap();
        let roots = || VideoOutputRoots {
            temp: &temp,
            user_data: &user_data,
            user_cache: &user_cache,
        };

        assert!(validate_video_output(&expected, &other, roots()).is_err());
        assert!(validate_video_output(&expected, &expected, roots()).is_err());
        fs::create_dir(&expected).unwrap();
        assert!(validate_video_output(&expected, &expected, roots()).is_err());
    }

    #[test]
    fn video_output_validation_rejects_lexical_storage_escape() {
        let base = tempfile::tempdir().unwrap();
        let [temp, user_data, user_cache] = create_output_roots(base.path());
        let escaped = user_data.join("..").join("outside.mp4");
        fs::write(base.path().join("outside.mp4"), b"video").unwrap();

        assert!(
            validate_video_output(
                &escaped,
                &escaped,
                VideoOutputRoots {
                    temp: &temp,
                    user_data: &user_data,
                    user_cache: &user_cache,
                },
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn video_output_validation_rejects_symlinked_ancestor_inside_root() {
        use std::os::unix::fs::symlink;

        let base = tempfile::tempdir().unwrap();
        let [temp, user_data, user_cache] = create_output_roots(base.path());
        let real_dir = user_data.join("real");
        fs::create_dir(&real_dir).unwrap();
        fs::write(real_dir.join("output.mp4"), b"video").unwrap();
        symlink(&real_dir, user_data.join("link")).unwrap();
        let output = user_data.join("link/output.mp4");

        assert!(
            validate_video_output(
                &output,
                &output,
                VideoOutputRoots {
                    temp: &temp,
                    user_data: &user_data,
                    user_cache: &user_cache,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn video_info_conversion_keeps_upload_preflight_metadata() {
        let result = platform_video_info_to_js(
            PlatformVideoInfo {
                width: 1920,
                height: 1080,
                duration_ms: 12_345,
                size: 4_096,
                rotation: Some(90),
                bitrate: Some(2_000_000),
                fps: Some(29.97),
                mime_type: Some("video/mp4".to_string()),
                video_codec: Some("H264".to_string()),
                has_audio: Some(true),
                audio_codec: Some("audio/mp4a-latm".to_string()),
            },
            "lx://temp/upload.mp4".to_string(),
        );

        assert_eq!(result.size, 4_096);
        assert_eq!(result.video_type.as_deref(), Some("video/mp4"));
        assert_eq!(result.video_codec.as_deref(), Some("video/avc"));
        assert_eq!(result.has_audio, Some(true));
        assert_eq!(result.audio_codec.as_deref(), Some("audio/mp4a-latm"));
        assert_eq!(result.path, "lx://temp/upload.mp4");
    }

    #[test]
    fn video_codec_mime_normalizes_known_platform_values() {
        let cases = [
            ("avc1", "video/avc"),
            ("video/h264", "video/avc"),
            ("hvc1", "video/hevc"),
            ("video/h265", "video/hevc"),
            ("vp08", "video/x-vnd.on2.vp8"),
            ("video/vp9", "video/x-vnd.on2.vp9"),
            ("av1", "video/av01"),
            ("video/mp4v", "video/mp4v-es"),
            ("mpeg-2", "video/mpeg2"),
            ("video/jpeg", "video/mjpeg"),
        ];

        for (input, expected) in cases {
            assert_eq!(
                normalize_video_codec_mime(Some(input.to_string())).as_deref(),
                Some(expected),
                "input: {input}"
            );
        }
    }

    #[test]
    fn video_codec_mime_preserves_future_video_mime_values() {
        assert_eq!(
            normalize_video_codec_mime(Some(" Video/Dolby-Vision ".to_string())).as_deref(),
            Some("video/dolby-vision")
        );
    }

    #[test]
    fn video_codec_mime_omits_empty_and_non_video_values() {
        for input in [None, Some(""), Some("audio/opus"), Some("h266")] {
            assert_eq!(normalize_video_codec_mime(input.map(str::to_string)), None);
        }
    }
}
