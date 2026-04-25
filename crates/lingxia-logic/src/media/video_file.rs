use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_internal_error, js_invalid_parameter_error,
};
use lingxia_platform::traits::media_runtime::{
    CompressVideoRequest, CompressedVideo as PlatformCompressedVideo, ExtractVideoThumbnailRequest,
    MediaRuntime, VideoCompressQuality, VideoInfo as PlatformVideoInfo,
};
use lingxia_service::storage;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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

async fn compress_video_api(
    ctx: JSContext,
    options: JSCompressVideoOptions,
) -> JSResult<JSCompressVideoResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let resolved_source = lxapp
        .resolve_accessible_path(options.path.trim())
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let output_path = resolve_compress_video_output_path(&lxapp, options.output_path.as_deref())?;
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

    let request = CompressVideoRequest {
        source_uri,
        quality,
        bitrate_kbps,
        fps,
        resolution_ratio,
        output_path,
    };

    let compressed = runtime
        .compress_video(&request)
        .map_err(|e| js_error_from_platform_error(&e))?;
    ensure_output_quota(&lxapp, &compressed.path)?;

    compressed_video_to_js(&lxapp, compressed)
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

fn compressed_video_to_js(
    lxapp: &LxApp,
    compressed: PlatformCompressedVideo,
) -> JSResult<JSCompressVideoResult> {
    let temp_file_path = lxapp
        .to_uri(&compressed.path)
        .ok_or_else(|| {
            js_internal_error("compressVideo failed to convert output path to lx:// uri")
        })?
        .into_string();

    Ok(JSCompressVideoResult {
        temp_file_path,
        width: compressed.width,
        height: compressed.height,
        duration_ms: compressed.duration_ms,
        size: compressed.size,
        video_type: "video/mp4".to_string(),
    })
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
            if path.starts_with(&lxapp.user_cache_dir)
                && let Ok(cache) = lxapp.cache()
            {
                cache.on_access(path);
            }
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::remove_file(path);
            Err(js_error_from_business_code_with_detail(1002, err.detail()))
        }
    }
}
