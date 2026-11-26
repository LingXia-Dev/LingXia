use crate::i18n::{js_error, js_error_from_business_code_with_detail, js_internal_error};
use lingxia_platform::traits::media_runtime::{CompressImageRequest, MediaRuntime};
use lingxia_service::storage;
use lxapp::LxApp;
use rong::{FromJSObject, IntoJSObject, JSContext, JSResult, function::Optional};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// An image's size and type, and a path to read it from.
#[derive(Debug, Clone, IntoJSObject)]
struct ImageInfo {
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// MIME type, e.g. `image/jpeg`.
    #[js_name = "type"]
    image_type: String,
    /// A path the image can be read from.
    path: String,
}

#[derive(FromJSObject, Default)]
#[ts_skip]
struct JSCompressImageOptions {
    quality: Option<i32>,
    #[js_name = "maxWidth"]
    max_width: Option<u32>,
    #[js_name = "maxHeight"]
    max_height: Option<u32>,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSCompressImageResult {
    path: String,
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace MediaApi = crate::media::media_namespace(ctx)?;
        fn imageInfo(
            ts_params = "path: string",
            ts_return = "Promise<ImageInfo>"
        ) = get_image_info_api;
        fn compressImage(
            ts_params = "path: string, options?: CompressImageOptions",
            ts_return = "Promise<CompressImageResult>"
        ) = compress_image_api;
    }
}

/// Read an image's dimensions and type without decoding it into a view.
///
/// @example
/// ```ts
/// const { width, height } = await lx.media.imageInfo('images/cover.png');
/// ```
///
/// @throws {E_NOT_FOUND} No file at the path.
/// @throws {E_INVALID_ARG} A path the lxapp may not read.
/// @throws {E_INTERNAL} The file is not an image the platform can read.
async fn get_image_info_api(ctx: JSContext, path: String) -> JSResult<ImageInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;

    let trimmed_path = path.trim();
    let resolved = lxapp
        .resolve_accessible_path(trimmed_path)
        .map_err(js_error)?;
    let normalized_path = resolved.to_string_lossy().into_owned();

    let response_path =
        if trimmed_path.starts_with("lx://") || is_bundle_relative_path(trimmed_path) {
            // Keep relative bundle paths unchanged (e.g. `images/1.png`) so WebView-relative usage works.
            trimmed_path.to_string()
        } else {
            lxapp
                .to_uri(&resolved)
                .ok_or_else(|| {
                    js_internal_error("media.imageInfo failed to convert path to lx:// uri")
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

            ImageInfo {
                width: info.width,
                height: info.height,
                image_type,
                path: response_path,
            }
        })
        .map_err(js_error)
}

/// Re-encode an image as JPEG at a lower quality or size, writing a new temporary file.
///
/// @example
/// ```ts
/// const { path } = await lx.media.compressImage(photo, { quality: 60, maxWidth: 1280 });
/// ```
///
/// @throws {E_NOT_FOUND} No file at the path.
/// @throws {E_INVALID_ARG} A path the lxapp may not read, or output over the temporary storage quota.
/// @throws {E_INTERNAL} The image could not be re-encoded.
async fn compress_image_api(
    ctx: JSContext,
    path: String,
    options: Optional<JSCompressImageOptions>,
) -> JSResult<JSCompressImageResult> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let runtime = &lxapp.runtime;
    let options = options.0.unwrap_or_default();

    let resolved_source = lxapp
        .resolve_accessible_path(path.trim())
        .map_err(js_error)?;
    let source_uri = resolved_source.to_string_lossy().into_owned();

    let output_path = generate_compress_output_path(&lxapp.temp_dir)?;

    let request = CompressImageRequest {
        source_uri,
        quality: clamp_quality(options.quality),
        max_width: sanitize_dimension(options.max_width),
        max_height: sanitize_dimension(options.max_height),
        output_path,
    };

    let output = runtime.compress_image(&request).map_err(js_error)?;
    ensure_temp_output_quota(&lxapp, &output)?;

    let path = lxapp
        .to_uri(&output)
        .ok_or_else(|| {
            js_internal_error("compressImage failed to convert output path to lx:// uri")
        })?
        .into_string();

    Ok(JSCompressImageResult { path })
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
