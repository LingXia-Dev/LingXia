use lingxia_platform::traits::media_runtime::{
    ExtractVideoThumbnailRequest, MediaRuntime, VideoInfo as PlatformVideoInfo,
};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, HostError, IntoJSObj, JSContext, JSFunc, JSResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static THUMBNAIL_NAME_COUNTER: AtomicU64 = AtomicU64::new(0);

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

#[derive(Debug, Clone, IntoJSObj)]
struct JSVideoThumbnailResult {
    #[rename = "tempFilePath"]
    temp_file_path: String,
    width: u32,
    height: u32,
    #[rename = "type"]
    image_type: String,
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_video_info_func = JSFunc::new(ctx, get_video_info_api)?;
    lx::register_js_api(ctx, "getVideoInfo", get_video_info_func)?;

    let extract_video_thumbnail_func = JSFunc::new(ctx, extract_video_thumbnail_api)?;
    lx::register_js_api(ctx, "extractVideoThumbnail", extract_video_thumbnail_func)?;
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
    let resolved = lxapp.resolve_accessible_path(trimmed_path).map_err(|err| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("getVideoInfo path error: {}", err),
        )
    })?;
    let normalized_path = resolved.to_string_lossy().into_owned();

    let response_path =
        if trimmed_path.starts_with("lx://") || is_bundle_relative_path(trimmed_path) {
            trimmed_path.to_string()
        } else {
            lxapp
                .to_uri(&resolved)
                .ok_or_else(|| {
                    HostError::new(
                        rong::error::E_INTERNAL,
                        "getVideoInfo failed to convert path to lx:// uri",
                    )
                })?
                .into_string()
        };

    runtime
        .get_video_info(&normalized_path)
        .map(|info| platform_video_info_to_js(info, response_path))
        .map_err(|e| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("getVideoInfo failed: {}", e),
            )
            .into()
        })
}

async fn extract_video_thumbnail_api(
    ctx: JSContext,
    options: JSVideoThumbnailOptions,
) -> JSResult<JSVideoThumbnailResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let resolved_source = lxapp
        .resolve_accessible_path(options.path.trim())
        .map_err(|err| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("extractVideoThumbnail path error: {}", err),
            )
        })?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let output_path = resolve_thumbnail_output_path(&lxapp, options.output_path.as_deref())?;
    let request = ExtractVideoThumbnailRequest {
        source_uri,
        output_path,
        max_width: sanitize_dimension(options.max_width),
        max_height: sanitize_dimension(options.max_height),
        time_ms: sanitize_time_ms(options.time_ms),
        quality: clamp_quality(options.quality),
    };

    let thumbnail = runtime.extract_video_thumbnail(&request).map_err(|e| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("extractVideoThumbnail failed: {}", e),
        )
    })?;

    let uri = lxapp
        .to_uri(&thumbnail.path)
        .ok_or_else(|| {
            HostError::new(
                rong::error::E_INTERNAL,
                "extractVideoThumbnail failed to convert output path to lx:// uri",
            )
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

fn resolve_thumbnail_output_path(
    lxapp: &LxApp,
    raw_output_path: Option<&str>,
) -> JSResult<PathBuf> {
    match raw_output_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(path) => lxapp.resolve_accessible_path(path).map_err(|err| {
            HostError::new(
                rong::error::E_INTERNAL,
                format!("extractVideoThumbnail outputPath error: {}", err),
            )
            .into()
        }),
        None => generate_thumbnail_output_path(&lxapp.user_cache_dir),
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

fn sanitize_dimension(value: Option<u32>) -> Option<u32> {
    match value {
        Some(v) if v > 0 => Some(v),
        _ => None,
    }
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
        return Err(HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to prepare directory {}: {}", path.display(), err),
        )
        .into());
    }
    Ok(())
}

fn generate_thumbnail_output_path(cache_root: &Path) -> JSResult<PathBuf> {
    let base_dir = cache_root.join("video-thumbnail");
    ensure_dir(&base_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let nonce = THUMBNAIL_NAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let filename = format!("vx_{}_{}.jpg", timestamp, nonce);

    Ok(base_dir.join(filename))
}
