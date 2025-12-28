use crate::{I18nKey, i18n::err_code_message, ui::present_action_sheet};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::{
    AppRuntime, CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind,
    MediaSource, ToastIcon, ToastOptions, ToastPosition, UserFeedback,
};
use lxapp::{LxApp, lx};
use rong::{
    FromJSObj, JSContext, JSEngineValue, JSFunc, JSObject, JSResult, RongJSError,
    function::Optional,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hash::Hash;
use std::path::Path;
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

#[derive(Debug, Clone)]
struct ChosenMediaEntry {
    path: String,
    kind: String,
    is_original: bool,
}

impl rong::IntoJSValue<JSEngineValue> for ChosenMediaEntry {
    fn into_js_value(self, ctx: &JSContext) -> JSEngineValue {
        let obj = JSObject::new(ctx);
        obj.set("tempFilePath", self.path).ok();
        obj.set("fileType", self.kind).ok();
        obj.set("isOriginal", self.is_original).ok();
        obj.into_value()
    }
}

impl rong::function::JSParameterType for ChosenMediaEntry {}

#[derive(Deserialize, Serialize, Hash, Clone)]
struct MediaKey {
    uri: String,
    #[serde(rename = "fileType", default = "default_kind")]
    kind: String,
    #[serde(rename = "isOriginal", default = "default_is_original")]
    is_original: bool,
}

fn permission_toast_key(code: u32) -> Option<I18nKey> {
    match code {
        3001 => Some(I18nKey::PermissionCameraDenied),
        3003 => Some(I18nKey::PermissionMicrophoneDenied),
        3004 => Some(I18nKey::PermissionPhotoDenied),
        _ => None,
    }
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let choose_media_func = JSFunc::new(ctx, choose_media)?;
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
        return Err(RongJSError::Error(
            "camera source does not support selecting both image and video; specify a single mediaType entry".into(),
        ));
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

    lxapp
        .runtime
        .choose_media(request)
        .map_err(|e| RongJSError::Error(format!("chooseMedia failed to start: {}", e)))?;

    let result = receiver
        .await
        .map_err(|_| RongJSError::Error("chooseMedia cancelled or failed".to_string()))?;

    let data = match result {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => {
            // 2000 = user cancelled, return empty result
            if code == 2000 {
                return Ok(Vec::new());
            }

            if let Some(key) = permission_toast_key(code) {
                let _ = lxapp.runtime.show_toast(ToastOptions {
                    title: crate::i18n::t(key),
                    icon: ToastIcon::Error,
                    image: None,
                    duration: 2.0,
                    mask: false,
                    position: ToastPosition::Center,
                });
            }

            let message =
                err_code_message(code).unwrap_or_else(|| format!("chooseMedia error: {}", code));
            return Err(RongJSError::Error(message));
        }
    };

    let parsed: Value = serde_json::from_str(&data)
        .map_err(|_| RongJSError::Error("chooseMedia invalid payload".to_string()))?;

    if parsed.is_null() {
        return Ok(Vec::new());
    }

    if parsed.as_object().is_some() {
        return Err(RongJSError::Error(
            "chooseMedia invalid payload".to_string(),
        ));
    }

    if !parsed.is_array() {
        return Err(RongJSError::Error(
            "chooseMedia invalid payload".to_string(),
        ));
    }

    let arr: Vec<MediaKey> = serde_json::from_str(&data)
        .map_err(|_| RongJSError::Error("chooseMedia invalid payload".to_string()))?;

    let mut out: Vec<ChosenMediaEntry> = Vec::new();
    for key in arr.into_iter() {
        let uri = key.uri.clone();
        if uri.is_empty() {
            continue;
        }
        let kind = key.kind.clone();
        let is_original = key.is_original;
        let final_path = if Path::new(&uri).is_absolute() {
            uri.to_string()
        } else {
            let ext = match kind.as_str() {
                "video" => "mp4",
                _ => "jpg",
            };
            let cache = lxapp
                .cache()
                .map_err(|e| RongJSError::Error(format!("cache unavailable: {}", e)))?;

            match cache.resolve_path_with_ext(&key, ext) {
                lxapp::ResolveResult::Exists(path) => path.to_string_lossy().to_string(),
                lxapp::ResolveResult::NonExists(path) => {
                    let media_kind = match kind.as_str() {
                        "video" => MediaKind::Video,
                        "image" => MediaKind::Image,
                        _ => MediaKind::Image,
                    };
                    match AppRuntime::copy_album_media_to_file(
                        &*lxapp.runtime,
                        &uri,
                        &path,
                        media_kind,
                    ) {
                        Ok(()) => path.to_string_lossy().to_string(),
                        Err(err) => {
                            return Err(RongJSError::Error(format!("copyMedia failed: {}", err)));
                        }
                    }
                }
            }
        };

        out.push(ChosenMediaEntry {
            path: final_path,
            kind: kind.to_string(),
            is_original,
        });
    }
    Ok(out)
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
                return Err(RongJSError::Error(format!(
                    "chooseMedia invalid mediaType token \"{}\"",
                    other
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
                return Err(RongJSError::Error(format!(
                    "chooseMedia invalid sourceType token \"{}\"",
                    other
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
                RongJSError::Error("chooseMedia source picker returned invalid index".to_string())
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
