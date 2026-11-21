mod cache;
mod parser;
mod source_picker;
mod types;

use crate::dismissal::{USER_DISMISSED, canceled};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::i18n::js_invalid_parameter_error;
use crate::i18n::{js_error_from_platform_error, js_internal_error};
use cache::ensure_temp_media_path;
use lingxia_platform::error::PlatformError;
use lingxia_platform::traits::app_runtime::AppRuntime;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use lingxia_service::media::PickMediaMode;
use lingxia_service::media::{MediaKind, MediaSource, PickMediaRequest};
use lxapp::LxApp;
use parser::{parse_camera, parse_pick_mode, parse_sources};
use rong::{JSContext, JSObject, JSResult, function::Optional};
use serde_json::Value;
use source_picker::present_source_picker;
use std::fs;
use std::path::{Path, PathBuf};
use types::{JSPickMediaOptions, MediaKey};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace PickApi = crate::pick::pick_namespace(ctx)?;
        fn media(
            ts_params = "options?: PickMediaOptions",
            ts_return = "Promise<PickFilesResult>"
        ) = pick_media;
    }
}

/// Picks pictures and videos from the library, or captures one with the camera.
/// Library items are copied to temp on mobile; desktop returns the user's file.
///
/// Resolves `{ status: 'canceled' }` only when the user dismisses the picker. A
/// completed selection resolves `{ status: 'ok', files }` with at least one
/// file. Rejects when capture or selection fails, or the host returns an invalid
/// payload.
///
/// @supports pick.camera
/// @throws {E_NOT_SUPPORTED} A camera-only request where the host cannot capture, such as on Windows.
async fn pick_media(ctx: JSContext, options: Optional<JSPickMediaOptions>) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    let opts = options.as_ref().cloned().unwrap_or(JSPickMediaOptions {
        count: None,
        media_type: None,
        source_type: None,
        camera: None,
        max_duration_ms: None,
    });

    let mode = parse_pick_mode(opts.media_type)?;
    let sources = parse_sources(opts.source_type)?;
    // Windows has no camera capture pipeline; album+camera requests
    // degrade to the album picker instead of asking for a source that
    // would only fail. A camera-only request stays as it is, for the
    // platform to reject with NotSupported.
    #[cfg(target_os = "windows")]
    let sources: Vec<MediaSource> = {
        let filtered: Vec<MediaSource> = sources
            .iter()
            .copied()
            .filter(|source| !matches!(source, MediaSource::Camera))
            .collect();
        if filtered.is_empty() {
            sources
        } else {
            filtered
        }
    };
    let selected_source = if sources.len() > 1 {
        match present_source_picker(&lxapp, &sources).await? {
            Some(source) => source,
            None => return canceled(&ctx),
        }
    } else {
        sources.first().copied().unwrap_or(MediaSource::Album)
    };

    // Windows is excluded so a camera-only request reads as NotSupported,
    // not as a bad mediaType.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    if matches!(selected_source, MediaSource::Camera) && matches!(mode, PickMediaMode::Mix) {
        return Err(js_invalid_parameter_error(
            "camera source does not support selecting both image and video; specify a single mediaType entry",
        ));
    }

    // The platform pickers take whole seconds; round up so a limit never shrinks.
    let max_duration_seconds = opts
        .max_duration_ms
        .filter(|v| !v.is_sign_negative())
        .map(|v| (v / 1000.0).ceil().min(u32::MAX as f64) as u32);
    let source_types = vec![selected_source];

    let request = PickMediaRequest {
        max_count: opts.count.unwrap_or(9),
        mode,
        source_types,
        max_duration_seconds,
        camera_facing: parse_camera(opts.camera),
        owner: lxapp.dialog_owner(),
    };

    let data = match lingxia_service::media::pick_media(&*lxapp.runtime, request).await {
        Ok(data) => data,
        // Business code 2000 is the platform's "user dismissed the picker";
        // every other platform error stays a rejection.
        Err(PlatformError::BusinessError(USER_DISMISSED)) => return canceled(&ctx),
        Err(err) => return Err(js_error_from_platform_error(&err)),
    };

    let parsed: Value = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("pick.media invalid payload: {}", e)))?;

    if parsed.is_null() {
        return Err(js_internal_error(
            "pick.media invalid payload: expected array",
        ));
    }

    if parsed.as_object().is_some() {
        return Err(js_internal_error(
            "pick.media invalid payload: expected array",
        ));
    }

    if !parsed.is_array() {
        return Err(js_internal_error(
            "pick.media invalid payload: expected array",
        ));
    }

    let arr: Vec<MediaKey> = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("pick.media invalid media array payload: {}", e)))?;

    let mut out: Vec<crate::pick::PickedFile> = Vec::new();
    let offered = arr.len();
    let picked_at = std::time::SystemTime::now();
    for key in arr.into_iter() {
        let uri = key.uri.trim();
        if uri.is_empty() {
            continue;
        }
        if uri.eq_ignore_ascii_case("[object Object]") {
            return Err(js_internal_error(
                "pick.media invalid payload: media uri must be string path, got [object Object]",
            ));
        }
        let kind = key.kind.as_str();
        let ext = cache_extension_for_media(kind, key.file_ext.as_deref(), uri);

        // Only treat `file://...` URIs as local filesystem paths when the remainder is an absolute
        // path. Pickers may also return non-filesystem URIs (e.g. Android `content://...`, iOS
        // Photos identifiers, or Harmony `file://media/...`), which must be copied via the
        // platform runtime (copy_album_media_to_file).
        let local_path = if let Some(path_str) = uri.strip_prefix("file://") {
            Path::new(path_str)
                .is_absolute()
                .then(|| PathBuf::from(path_str))
        } else {
            Path::new(uri).is_absolute().then(|| PathBuf::from(uri))
        };

        // The user's own file keeps its name; a capture or a library item the
        // platform copied out gets a generated one.
        let mut original_name: Option<String> = key.file_name.clone();
        let (final_uri, final_path) = if let Some(source_path) = local_path {
            match lxapp.resolve_accessible_path(source_path.to_string_lossy().as_ref()) {
                Ok(path) if lxapp.to_uri(&path).is_some() => {
                    (response_path_for_media(&lxapp, &path)?, path)
                }
                _ if should_copy_local_media_file_to_cache() => {
                    original_name = original_name.or_else(|| file_name_of(&source_path));
                    let incoming_bytes = fs::metadata(&source_path).ok().map(|m| m.len());
                    let cached_path = ensure_temp_media_path(
                        lxapp.as_ref(),
                        &key,
                        &ext,
                        incoming_bytes,
                        |dest_path| {
                            let result = fs::copy(&source_path, dest_path);
                            result.map(|_| ()).map_err(|e| {
                                js_internal_error(format!(
                                    "pick.media failed to copy temp file into cache (src={}, dest={}): {}",
                                    source_path.display(),
                                    dest_path.display(),
                                    e
                                ))
                            })
                        },
                    )?;
                    (response_path_for_media(&lxapp, &cached_path)?, cached_path)
                }
                _ => {
                    original_name = original_name.or_else(|| file_name_of(&source_path));
                    let uri = lxapp
                        .grant_transient_file_access(&source_path)
                        .map_err(|err| crate::i18n::js_error_from_lxapp_error(&err))?
                        .into_string();
                    (uri, source_path)
                }
            }
        } else if let Ok(path) = lxapp.resolve_accessible_path(uri) {
            (response_path_for_media(&lxapp, &path)?, path)
        } else {
            let media_kind = match kind {
                "video" => MediaKind::Video,
                "image" => MediaKind::Image,
                _ => MediaKind::Image,
            };
            let path = ensure_temp_media_path(lxapp.as_ref(), &key, &ext, None, |dest_path| {
                AppRuntime::copy_album_media_to_file(&*lxapp.runtime, uri, dest_path, media_kind)
                    .map_err(|err| js_error_from_platform_error(&err))
            })?;
            (response_path_for_media(&lxapp, &path)?, path)
        };

        let name = original_name.unwrap_or_else(|| {
            crate::pick::generated_media_name(kind == "video", &ext, out.len(), picked_at)
        });
        out.push(crate::pick::PickedFile::new(
            final_uri,
            Some(&final_path),
            Some(name),
            None,
        ));
    }
    if out.is_empty() {
        // An empty selection is how several hosts report a dismissal. But if
        // the picker offered entries and every one was unusable, the user did
        // choose — say so instead of laundering an adapter bug into a cancel.
        if offered > 0 {
            return Err(js_internal_error(
                "pick.media invalid payload: every selected entry was unusable",
            ));
        }
        return canceled(&ctx);
    }

    crate::pick::files_result(&ctx, &out)
}

fn file_name_of(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn response_path_for_media(lxapp: &LxApp, path: &Path) -> JSResult<String> {
    if let Some(uri) = lxapp.to_uri(path) {
        return Ok(uri.into_string());
    }

    Err(js_internal_error(
        "pick.media failed to convert output path to lx:// uri",
    ))
}

fn should_copy_local_media_file_to_cache() -> bool {
    cfg!(any(
        target_os = "android",
        target_os = "ios",
        all(target_os = "linux", target_env = "ohos")
    ))
}

fn cache_extension_for_media(kind: &str, raw_ext: Option<&str>, uri: &str) -> String {
    match kind {
        "video" => normalize_video_extension(raw_ext).unwrap_or_else(|| {
            infer_extension_from_uri(uri).unwrap_or_else(|| {
                #[cfg(target_os = "ios")]
                {
                    "mov".to_string()
                }
                #[cfg(not(target_os = "ios"))]
                {
                    "mp4".to_string()
                }
            })
        }),
        _ => "jpg".to_string(),
    }
}

fn normalize_video_extension(raw_ext: Option<&str>) -> Option<String> {
    let ext = raw_ext?.trim().trim_start_matches('.').to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "3gp" | "3gpp" => Some(ext),
        _ => None,
    }
}

fn infer_extension_from_uri(uri: &str) -> Option<String> {
    let path = uri.split(['?', '#']).next().unwrap_or(uri);
    let ext = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())?;
    if ext.is_empty() { None } else { Some(ext) }
}
