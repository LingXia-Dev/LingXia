use crate::{i18n::err_code_message, ui::present_action_sheet};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::media_interaction::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaSource,
};
use lxapp::{LxApp, lx};
use rong::{
    FromJSObj, IntoJSObj, JSContext, JSFunc, JSResult, RongJSError, error::HostError,
    function::Optional,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(FromJSObj, Clone)]
struct JSChooseMediaOptions {
    #[rename = "count"]
    count: Option<u32>,
    #[rename = "mediaType"]
    media_type: Option<Vec<String>>,
    #[rename = "sourceType"]
    source_type: Option<Vec<String>>,
    camera: Option<String>,
    #[rename = "maxDuration"]
    max_duration: Option<f64>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct ChosenMediaEntry {
    #[rename = "tempFilePath"]
    path: String,
    #[rename = "fileType"]
    kind: String,
    #[rename = "isOriginal"]
    is_original: bool,
}

#[derive(Deserialize, Serialize, Hash, Clone)]
struct MediaKey {
    uri: String,
    #[serde(rename = "fileType", default = "default_kind")]
    kind: String,
    #[serde(rename = "isOriginal", default = "default_is_original")]
    is_original: bool,
}

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
) -> JSResult<Vec<ChosenMediaEntry>> {
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
    let selected_source = if sources.len() > 1 {
        match present_source_picker(&lxapp, &sources).await? {
            Some(source) => source,
            None => return Ok(Vec::new()),
        }
    } else {
        sources.first().copied().unwrap_or(MediaSource::Album)
    };

    if matches!(selected_source, MediaSource::Camera) && matches!(mode, ChooseMediaMode::Mix) {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "camera source does not support selecting both image and video; specify a single mediaType entry",
        )));
    }

    let (callback_id, receiver) = get_callback();
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
        callback_id,
    };

    lxapp.runtime.choose_media(request).map_err(|e| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("chooseMedia failed to start: {}", e),
        ))
    })?;

    let result = receiver.await.map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia cancelled or failed",
        ))
    })?;

    let data = match result {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => {
            // 2000 = user cancelled, return empty result
            if code == 2000 {
                return Ok(Vec::new());
            }

            let message =
                err_code_message(code).unwrap_or_else(|| format!("chooseMedia error: {}", code));
            return Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                message,
            )));
        }
    };

    let parsed: Value = serde_json::from_str(&data).map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia invalid payload",
        ))
    })?;

    if parsed.is_null() {
        return Ok(Vec::new());
    }

    if parsed.as_object().is_some() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia invalid payload",
        )));
    }

    if !parsed.is_array() {
        return Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia invalid payload",
        )));
    }

    let arr: Vec<MediaKey> = serde_json::from_str(&data).map_err(|_| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            "chooseMedia invalid payload",
        ))
    })?;

    let mut out: Vec<ChosenMediaEntry> = Vec::new();
    for key in arr.into_iter() {
        let uri = key.uri.trim();
        if uri.is_empty() {
            continue;
        }
        let kind = key.kind.as_str();
        let ext = match kind {
            "video" => "mp4",
            _ => "jpg",
        };
        let is_original = key.is_original;

        let local_path = uri
            .strip_prefix("file://")
            .map(PathBuf::from)
            .or_else(|| Path::new(uri).is_absolute().then(|| PathBuf::from(uri)));

        let final_path: PathBuf = if let Some(source_path) = local_path {
            match lxapp.resolve_accessible_path(source_path.to_string_lossy().as_ref()) {
                Ok(path) => path,
                Err(_) => ensure_cached_media_path(lxapp.as_ref(), &key, ext, |dest_path| {
                    fs::copy(&source_path, dest_path).map(|_| ()).map_err(|e| {
                        RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            format!("chooseMedia failed to copy temp file into cache: {}", e),
                        ))
                    })
                })?,
            }
        } else if let Ok(path) = lxapp.resolve_accessible_path(uri) {
            path
        } else {
            let media_kind = match kind {
                "video" => MediaKind::Video,
                "image" => MediaKind::Image,
                _ => MediaKind::Image,
            };
            ensure_cached_media_path(lxapp.as_ref(), &key, ext, |dest_path| {
                AppRuntime::copy_album_media_to_file(&*lxapp.runtime, uri, dest_path, media_kind)
                    .map_err(|err| {
                        RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            format!("copyMedia failed: {}", err),
                        ))
                    })
            })?
        };

        let final_uri = lxapp
            .to_uri(&final_path)
            .ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    "chooseMedia failed to convert output path to lx:// uri",
                ))
            })?
            .into_string();

        out.push(ChosenMediaEntry {
            path: final_uri,
            kind: key.kind,
            is_original,
        });
    }
    Ok(out)
}

fn ensure_cached_media_path<F>(
    lxapp: &LxApp,
    key: &MediaKey,
    ext: &str,
    write: F,
) -> Result<PathBuf, RongJSError>
where
    F: FnOnce(&Path) -> Result<(), RongJSError>,
{
    let cache = lxapp.cache().map_err(|e| {
        RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("cache unavailable: {}", e),
        ))
    })?;

    match cache.resolve_path_with_ext(key, ext) {
        lxapp::ResolveResult::Exists(path) => Ok(path),
        lxapp::ResolveResult::NonExists(dest_path) => {
            write(&dest_path)?;
            Ok(dest_path)
        }
    }
}

fn parse_choose_mode(values: Option<Vec<String>>) -> JSResult<ChooseMediaMode> {
    let raw = values.unwrap_or_else(|| vec!["image".to_string(), "video".to_string()]);
    let mut has_image = false;
    let mut has_video = false;

    for token in raw {
        match token.to_lowercase().as_str() {
            "image" => has_image = true,
            "video" => has_video = true,
            other => {
                return Err(RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    format!("chooseMedia invalid mediaType token \"{}\"", other),
                )));
            }
        }
    }

    if !has_image && !has_video {
        has_image = true;
        has_video = true;
    }

    Ok(match (has_image, has_video) {
        (true, true) => ChooseMediaMode::Mix,
        (true, false) => ChooseMediaMode::Images,
        (false, true) => ChooseMediaMode::Videos,
        _ => ChooseMediaMode::Images,
    })
}

fn parse_sources(values: Option<Vec<String>>) -> JSResult<Vec<MediaSource>> {
    let raw = values.unwrap_or_else(|| vec!["album".to_string()]);
    let mut out: Vec<MediaSource> = Vec::new();

    for token in raw {
        let source = match token.to_lowercase().as_str() {
            "album" => MediaSource::Album,
            "camera" => MediaSource::Camera,
            other => {
                return Err(RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    format!("chooseMedia invalid sourceType token \"{}\"", other),
                )));
            }
        };

        if !out.contains(&source) {
            out.push(source);
        }
    }

    if out.is_empty() {
        out.push(MediaSource::Album);
    }

    Ok(out)
}

async fn present_source_picker(
    lxapp: &Arc<LxApp>,
    sources: &[MediaSource],
) -> JSResult<Option<MediaSource>> {
    let item_list: Vec<String> = sources
        .iter()
        .map(|source| label_for_media_source(*source).to_string())
        .collect();

    let selection = present_action_sheet(lxapp, item_list, None, None).await?;

    match selection {
        Some(idx) => sources
            .get(idx)
            .copied()
            .ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_INTERNAL,
                    "chooseMedia source picker returned invalid index",
                ))
            })
            .map(Some),
        None => Ok(None),
    }
}

fn label_for_media_source(source: MediaSource) -> &'static str {
    match source {
        MediaSource::Album => "Album",
        MediaSource::Camera => "Camera",
    }
}

fn parse_camera(s: Option<String>) -> Option<CameraFacing> {
    s.map(|v| match v.to_lowercase().as_str() {
        "front" => CameraFacing::Front,
        _ => CameraFacing::Back,
    })
}

fn default_kind() -> String {
    "image".to_string()
}

fn default_is_original() -> bool {
    true
}
