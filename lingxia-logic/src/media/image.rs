use lingxia_platform::traits::media_runtime::{CompressImageRequest, MediaRuntime};
use lxapp::{LxApp, lx};
use rong::{FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(FromJSObj)]
struct JSGetImageInfoOptions {
    path: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSImageInfoResult {
    width: u32,
    height: u32,
    #[rename = "type"]
    image_type: String,
    path: String,
}

#[derive(FromJSObj)]
struct JSCompressImageOptions {
    path: String,
    quality: Option<i32>,
    #[rename = "compressedWidth"]
    compressed_width: Option<u32>,
    #[rename = "compressedHeight"]
    compressed_height: Option<u32>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSCompressImageResult {
    #[rename = "tempFilePath"]
    temp_file_path: String,
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let get_image_info_func = JSFunc::new(ctx, get_image_info_api)?;
    lx::register_js_api(ctx, "getImageInfo", get_image_info_func)?;

    let compress_image_func = JSFunc::new(ctx, compress_image_api)?;
    lx::register_js_api(ctx, "compressImage", compress_image_func)?;
    Ok(())
}

async fn get_image_info_api(
    ctx: JSContext,
    options: JSGetImageInfoOptions,
) -> JSResult<JSImageInfoResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let original_path = options.path;
    let trimmed_path = original_path.trim();
    let resolved = lxapp
        .resolve_accessible_path(trimmed_path)
        .map_err(|err| RongJSError::Error(format!("getImageInfo path error: {}", err)))?;
    let normalized_path = resolved.to_string_lossy().into_owned();

    let response_path = if trimmed_path.starts_with("lx://")
        || is_bundle_relative_path(trimmed_path)
    {
        // Keep relative bundle paths unchanged (e.g. `images/1.png`) so WebView-relative usage works.
        trimmed_path.to_string()
    } else {
        lxapp
            .to_uri(&resolved)
            .ok_or_else(|| {
                RongJSError::Error("getImageInfo failed to convert path to lx:// uri".to_string())
            })?
            .into_string()
    };

    runtime
        .get_image_info(&normalized_path)
        .map(|info| {
            let image_type = info
                .mime_type
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| infer_mime_from_path(&normalized_path).to_string());

            JSImageInfoResult {
                width: info.width,
                height: info.height,
                image_type,
                path: response_path,
            }
        })
        .map_err(|e| RongJSError::Error(format!("getImageInfo failed: {}", e)))
}

async fn compress_image_api(
    ctx: JSContext,
    options: JSCompressImageOptions,
) -> JSResult<JSCompressImageResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let resolved_source = lxapp
        .resolve_accessible_path(options.path.trim())
        .map_err(|err| RongJSError::Error(format!("compressImage path error: {}", err)))?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let output_path = generate_compress_output_path(&lxapp.user_cache_dir)?;

    let request = CompressImageRequest {
        source_uri,
        quality: clamp_quality(options.quality),
        max_width: sanitize_dimension(options.compressed_width),
        max_height: sanitize_dimension(options.compressed_height),
        output_path,
    };

    let path = runtime
        .compress_image(&request)
        .map_err(|e| RongJSError::Error(format!("compressImage failed: {}", e)))?;

    let uri = lxapp
        .to_uri(&path)
        .ok_or_else(|| {
            RongJSError::Error(
                "compressImage failed to convert output path to lx:// uri".to_string(),
            )
        })?
        .into_string();

    Ok(JSCompressImageResult {
        temp_file_path: uri,
    })
}

fn sanitize_dimension(value: Option<u32>) -> Option<u32> {
    match value {
        Some(v) if v > 0 => Some(v),
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
        return Err(RongJSError::Error(format!(
            "Failed to prepare directory {}: {}",
            path.display(),
            err
        )));
    }
    Ok(())
}

fn generate_compress_output_path(cache_root: &Path) -> JSResult<PathBuf> {
    let base_dir = cache_root.join("image-compress");
    ensure_dir(&base_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let filename = format!("lx_{}.jpg", timestamp);

    Ok(base_dir.join(filename))
}

fn infer_mime_from_path(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".heic") || lower.ends_with(".heif") {
        "image/heic"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}
