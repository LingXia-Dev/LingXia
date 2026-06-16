mod cache;
mod parser;
mod source_picker;
mod types;

#[cfg(not(target_os = "macos"))]
use crate::i18n::js_invalid_parameter_error;
use crate::i18n::{js_error_from_business_code, js_error_from_platform_error, js_internal_error};
use cache::ensure_temp_media_path;
use lingxia_platform::traits::app_runtime::AppRuntime;
#[cfg(not(target_os = "macos"))]
use lingxia_service::media::ChooseMediaMode;
use lingxia_service::media::{ChooseMediaRequest, MediaKind, MediaSource};
use lxapp::{LxApp, lx};
use parser::{parse_camera, parse_choose_mode, parse_sources};
use rong::{JSContext, JSFunc, JSResult, JSValue, JsonToJSValue, function::Optional};
use serde_json::Value;
use source_picker::present_source_picker;
use std::fs;
use std::path::{Path, PathBuf};
use types::{ChosenMediaEntry, JSChooseMediaOptions, MediaKey};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let choose_media_func = JSFunc::new(ctx, |ctx, options| async move {
        choose_media(ctx, options).await
    })?;
    lx::register_js_api(ctx, "chooseMedia", choose_media_func)?;
    Ok(())
}

async fn choose_media(
    ctx: JSContext,
    options: Optional<JSChooseMediaOptions>,
) -> JSResult<JSValue> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    let opts = options.as_ref().cloned().unwrap_or(JSChooseMediaOptions {
        count: None,
        media_type: None,
        source_type: None,
        camera: None,
        max_duration: None,
    });

    let mode = parse_choose_mode(opts.media_type)?;
    let sources = parse_sources(opts.source_type)?;
    // Windows has no camera capture pipeline; album+camera requests
    // degrade to the album picker instead of asking for a source that
    // would only fail. Camera-only requests still surface NotSupported.
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
            None => return Err(js_error_from_business_code(2000)),
        }
    } else {
        sources.first().copied().unwrap_or(MediaSource::Album)
    };

    #[cfg(not(target_os = "macos"))]
    if matches!(selected_source, MediaSource::Camera) && matches!(mode, ChooseMediaMode::Mix) {
        return Err(js_invalid_parameter_error(
            "camera source does not support selecting both image and video; specify a single mediaType entry",
        ));
    }

    let max_duration_seconds = opts
        .max_duration
        .filter(|v| !v.is_sign_negative())
        .map(|v| v.min(u32::MAX as f64).round() as u32);
    let source_types = vec![selected_source];

    let request = ChooseMediaRequest {
        max_count: opts.count.unwrap_or(9),
        mode,
        source_types,
        max_duration_seconds,
        camera_facing: parse_camera(opts.camera),
    };

    let data = lingxia_service::media::choose_media(&*lxapp.runtime, request)
        .await
        .map_err(|e| js_error_from_platform_error(&e))?;

    let parsed: Value = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("chooseMedia invalid payload: {}", e)))?;

    if parsed.is_null() {
        return Err(js_internal_error(
            "chooseMedia invalid payload: expected array",
        ));
    }

    if parsed.as_object().is_some() {
        return Err(js_internal_error(
            "chooseMedia invalid payload: expected array",
        ));
    }

    if !parsed.is_array() {
        return Err(js_internal_error(
            "chooseMedia invalid payload: expected array",
        ));
    }

    let arr: Vec<MediaKey> = serde_json::from_str(&data).map_err(|e| {
        js_internal_error(format!("chooseMedia invalid media array payload: {}", e))
    })?;

    let mut out: Vec<ChosenMediaEntry> = Vec::new();
    for key in arr.into_iter() {
        let uri = key.uri.trim();
        if uri.is_empty() {
            continue;
        }
        if uri.eq_ignore_ascii_case("[object Object]") {
            return Err(js_internal_error(
                "chooseMedia invalid payload: media uri must be string path, got [object Object]",
            ));
        }
        let kind = key.kind.as_str();
        let ext = cache_extension_for_media(kind, key.file_ext.as_deref(), uri);
        let is_original = key.is_original;

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

        let final_uri = if let Some(source_path) = local_path {
            match lxapp.resolve_accessible_path(source_path.to_string_lossy().as_ref()) {
                Ok(path) if lxapp.to_uri(&path).is_some() => {
                    response_path_for_media(&lxapp, &path)?
                }
                _ if should_copy_local_media_file_to_cache() => {
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
                                    "chooseMedia failed to copy temp file into cache (src={}, dest={}): {}",
                                    source_path.display(),
                                    dest_path.display(),
                                    e
                                ))
                            })
                        },
                    )?;
                    response_path_for_media(&lxapp, &cached_path)?
                }
                _ => lxapp
                    .grant_transient_file_access(&source_path)
                    .map_err(|e| {
                        js_internal_error(format!(
                            "chooseMedia failed to register temporary media file {}: {}",
                            source_path.display(),
                            e
                        ))
                    })?
                    .into_string(),
            }
        } else if let Ok(path) = lxapp.resolve_accessible_path(uri) {
            response_path_for_media(&lxapp, &path)?
        } else {
            let media_kind = match kind {
                "video" => MediaKind::Video,
                "image" => MediaKind::Image,
                _ => MediaKind::Image,
            };
            ensure_temp_media_path(lxapp.as_ref(), &key, &ext, None, |dest_path| {
                AppRuntime::copy_album_media_to_file(&*lxapp.runtime, uri, dest_path, media_kind)
                    .map_err(|err| js_error_from_platform_error(&err))
            })
            .and_then(|path| response_path_for_media(&lxapp, &path))?
        };

        out.push(ChosenMediaEntry {
            path: final_uri,
            kind: key.kind,
            is_original,
        });
    }
    let json = serde_json::to_string(&out)
        .map_err(|e| js_internal_error(format!("chooseMedia failed to serialize result: {}", e)))?;

    json.as_str().json_to_js_value(&ctx).map_err(|e| {
        js_internal_error(format!(
            "chooseMedia failed to materialize JS result: {}",
            e
        ))
    })
}

fn response_path_for_media(lxapp: &LxApp, path: &Path) -> JSResult<String> {
    if let Some(uri) = lxapp.to_uri(path) {
        return Ok(uri.into_string());
    }

    Err(js_internal_error(
        "chooseMedia failed to convert output path to lx:// uri",
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
