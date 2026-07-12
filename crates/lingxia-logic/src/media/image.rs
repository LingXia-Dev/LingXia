use crate::i18n::{
    js_error_from_business_code_with_detail, js_error_from_lxapp_error,
    js_error_from_platform_error, js_internal_error,
};
use lingxia_platform::traits::media_runtime::{CompressImageRequest, MediaRuntime};
use lingxia_service::storage;
use lxapp::LxApp;
use rong::{FromJSObject, IntoJSObject, JSContext, JSResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(FromJSObject)]
#[ts_skip]
struct JSGetImageInfoOptions {
    path: String,
}

#[derive(Debug, Clone, IntoJSObject)]
struct ImageInfo {
    width: u32,
    height: u32,
    #[js_name = "type"]
    image_type: String,
    path: String,
}

#[derive(FromJSObject)]
#[ts_skip]
struct JSCompressImageOptions {
    path: String,
    quality: Option<i32>,
    #[js_name = "compressedWidth"]
    compressed_width: Option<u32>,
    #[js_name = "compressedHeight"]
    compressed_height: Option<u32>,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSCompressImageResult {
    #[js_name = "tempFilePath"]
    temp_file_path: String,
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getImageInfo(
            ts_params = "options: GetImageInfoOptions",
            ts_return = "Promise<ImageInfo>"
        ) = get_image_info_api;
        fn compressImage(
            ts_params = "options: CompressImageOptions",
            ts_return = "Promise<CompressImageResult>"
        ) = compress_image_api;
    }
}

async fn get_image_info_api(ctx: JSContext, options: JSGetImageInfoOptions) -> JSResult<ImageInfo> {
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
        // Keep relative bundle paths unchanged (e.g. `images/1.png`) so WebView-relative usage works.
        trimmed_path.to_string()
    } else {
        lxapp
            .to_uri(&resolved)
            .ok_or_else(|| js_internal_error("getImageInfo failed to convert path to lx:// uri"))?
            .into_string()
    };

    runtime
        .get_image_info(&normalized_path)
        .map(|info| {
            let image_type = info
                .mime_type
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| infer_mime_from_path(&normalized_path).to_string());

            ImageInfo {
                width: info.width,
                height: info.height,
                image_type,
                path: response_path,
            }
        })
        .map_err(|e| js_error_from_platform_error(&e))
}

async fn compress_image_api(
    ctx: JSContext,
    options: JSCompressImageOptions,
) -> JSResult<JSCompressImageResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let resolved_source = lxapp
        .resolve_accessible_path(options.path.trim())
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let output_path = generate_compress_output_path(&lxapp.temp_dir)?;

    let request = CompressImageRequest {
        source_uri,
        quality: clamp_quality(options.quality),
        max_width: sanitize_dimension(options.compressed_width),
        max_height: sanitize_dimension(options.compressed_height),
        output_path,
    };

    let path = runtime
        .compress_image(&request)
        .map_err(|e| js_error_from_platform_error(&e))?;
    ensure_temp_output_quota(&lxapp, &path)?;

    let uri = lxapp
        .to_uri(&path)
        .ok_or_else(|| {
            js_internal_error("compressImage failed to convert output path to lx:// uri")
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
        return Err(js_internal_error(format!(
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

fn ensure_temp_output_quota(lxapp: &LxApp, path: &Path) -> JSResult<()> {
    let size = storage::path_size(path);
    storage::ensure_temp_quota(&lxapp.temp_dir, path, size)
        .map_err(|err| js_error_from_business_code_with_detail(1002, err.detail()))
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
