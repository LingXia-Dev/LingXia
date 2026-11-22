mod cache;
mod parser;
mod types;

use crate::I18nKey;
use crate::dismissal::{USER_DISMISSED, canceled};
use crate::i18n::{js_error_from_platform_error, js_internal_error, t};
use cache::ensure_temp_media_path;
use lingxia_platform::error::PlatformError;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::media_interaction::PickMediaMode;
use lingxia_service::media::{MediaKind, MediaSource, PickMediaRequest};
use lxapp::LxApp;
use parser::{CameraSettings, parse_capture_kind, parse_pick_mode, parse_sources};
use rong::{JSContext, JSObject, JSResult, function::Optional};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use types::{JSCaptureOptions, JSPickMediaOptions, MediaKey};

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
        fn capture(
            ts_params = "options: CaptureOptions",
            ts_return = "Promise<PickFileResult>"
        ) = pick_capture;
    }
}

/// Lets the person get pictures and videos from where they choose: the library, the camera, or
/// either, asked first in a host-drawn sheet. Library items are copied to temp on mobile; desktop
/// returns the user's file. The camera route returns one item; to set how the camera records
/// without offering the library, `lx.pick.capture` does the same with one result. Compression is
/// `lx.media.compressImage`.
///
/// Resolves `{ status: 'canceled' }` when the person dismisses the sheet, picker or camera,
/// otherwise `{ status: 'ok', files }` with at least one file. Only the camera route needs
/// `pick.camera`.
///
/// @supports pick.camera
/// @throws {E_INVALID_ARG} A `sources` entry other than `'library'` or `'camera'`, an empty
/// `sources`, or a `types` entry other than `'image'` or `'video'`.
/// @throws {E_NOT_SUPPORTED} `sources` is only `['camera']` on a host that cannot capture, such
/// as Windows; with the library also offered, such a host opens the library without asking.
/// @throws {E_INTERNAL} The picker failed or returned an invalid payload.
async fn pick_media(ctx: JSContext, options: Optional<JSPickMediaOptions>) -> JSResult<JSObject> {
    let opts = options.0.unwrap_or_default();
    let sources = parse_sources(opts.sources)?;
    let mode = parse_pick_mode(opts.types)?;
    let camera = CameraSettings::from_media_option(opts.camera)?;
    let sources = without_unsupported_camera(sources)?;
    let source = if sources.len() > 1 {
        match choose_source(&ctx, &sources).await? {
            Some(source) => source,
            None => return canceled(&ctx),
        }
    } else {
        sources[0]
    };
    let picked = match source {
        MediaSource::Album => {
            run_pick(
                &ctx,
                MediaSource::Album,
                mode,
                opts.count.unwrap_or(9),
                CameraSettings::default(),
            )
            .await?
        }
        MediaSource::Camera => {
            // One camera records one kind: a photo unless only video was asked for.
            let mode = match mode {
                PickMediaMode::Videos => PickMediaMode::Videos,
                _ => PickMediaMode::Images,
            };
            run_pick(&ctx, MediaSource::Camera, mode, 1, camera).await?
        }
    };
    match picked {
        Some(files) => crate::pick::files_result(&ctx, &files),
        None => canceled(&ctx),
    }
}

/// A host without a camera drops it from a pair and refuses it alone.
fn without_unsupported_camera(sources: Vec<MediaSource>) -> JSResult<Vec<MediaSource>> {
    if cfg!(not(target_os = "windows")) {
        return Ok(sources);
    }
    let library: Vec<MediaSource> = sources
        .into_iter()
        .filter(|source| *source != MediaSource::Camera)
        .collect();
    if library.is_empty() {
        return Err(rong::HostError::new(
            rong::error::E_NOT_SUPPORTED,
            "pick.media: this host has no camera; offer 'library' or check lx.supports('pick.camera')",
        )
        .into());
    }
    Ok(library)
}

/// The host-drawn sheet; its labels and Cancel come from the SDK catalogue. `None` on dismiss.
async fn choose_source(ctx: &JSContext, sources: &[MediaSource]) -> JSResult<Option<MediaSource>> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let labels = sources
        .iter()
        .map(|source| match source {
            MediaSource::Album => t(I18nKey::PickSourceLibrary),
            MediaSource::Camera => t(I18nKey::PickSourceCamera),
        })
        .collect();
    let chosen = crate::ui::present_action_sheet(&lxapp, labels, None, None).await?;
    Ok(chosen.and_then(|index| sources.get(index).copied()))
}

/// Takes one photo or records one video with the camera, with these settings. Resolves
/// `{ status: 'canceled' }` only when the person dismisses the camera, otherwise
/// `{ status: 'ok', file }`. To let the person choose the library instead, use `lx.pick.media`
/// with `sources: ['library', 'camera']`.
///
/// @supports pick.camera
/// @throws {E_INVALID_ARG} A `type` other than `'image'` or `'video'`.
/// @throws {E_NOT_SUPPORTED} The host cannot capture, such as on Windows.
/// @throws {E_INTERNAL} The camera failed or returned an invalid payload.
async fn pick_capture(ctx: JSContext, options: JSCaptureOptions) -> JSResult<JSObject> {
    let mode = parse_capture_kind(&options.kind)?;
    let camera = CameraSettings::new(options.facing, options.max_duration_ms);
    let picked = run_pick(&ctx, MediaSource::Camera, mode, 1, camera).await?;
    match picked.and_then(|files| files.into_iter().next()) {
        Some(file) => crate::pick::file_result(&ctx, &file),
        None => canceled(&ctx),
    }
}

/// `None` when the person dismissed the picker or camera.
async fn run_pick(
    ctx: &JSContext,
    source: MediaSource,
    mode: PickMediaMode,
    max_count: u32,
    camera: CameraSettings,
) -> JSResult<Option<Vec<crate::pick::PickedFile>>> {
    let lxapp = LxApp::from_ctx(ctx)?;
    let request = PickMediaRequest {
        max_count,
        mode,
        source_types: vec![source],
        max_duration_seconds: camera.max_duration_seconds,
        camera_facing: camera.facing,
        owner: lxapp.dialog_owner(),
    };

    let data = match lingxia_service::media::pick_media(&*lxapp.runtime, request).await {
        Ok(data) => data,
        // Business code 2000 is the platform's "user dismissed the picker";
        // every other platform error stays a rejection.
        Err(PlatformError::BusinessError(USER_DISMISSED)) => return Ok(None),
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
        return Ok(None);
    }

    Ok(Some(out))
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
