use crate::ui::present_action_sheet;
use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::AppRuntime;
use lingxia_platform::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaSource,
    PreviewMediaItem, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest, ScanType,
};
use rong::{
    FromJSObj, IntoJSObj, JSContext, JSFunc, JSObject, JSResult, RongJSError, function::Optional,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::hash::Hash;
use std::sync::Arc;

#[derive(FromJSObj)]
struct JSPreviewMediaItem {
    path: Option<String>,
    #[rename = "type"]
    kind: Option<String>,
    #[rename = "coverPath"]
    cover_path: Option<String>,
}

#[derive(FromJSObj)]
struct JSPreviewMediaOptions {
    sources: Vec<JSPreviewMediaItem>,
}

fn parse_media_kind(value: Option<String>) -> MediaKind {
    match value
        .unwrap_or_else(|| "image".to_string())
        .to_lowercase()
        .as_str()
    {
        "video" => MediaKind::Video,
        "image" => MediaKind::Image,
        _ => MediaKind::Image,
    }
}

fn preview_media(ctx: JSContext, options: JSPreviewMediaOptions) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let runtime = &lxapp.runtime;

    if options.sources.is_empty() {
        return Err(RongJSError::Error(
            "previewMedia requires at least one item".into(),
        ));
    }

    let items: Vec<PreviewMediaItem> = options
        .sources
        .into_iter()
        .map(|item| -> Result<PreviewMediaItem, RongJSError> {
            let JSPreviewMediaItem {
                path,
                kind,
                cover_path,
            } = item;

            let raw_path =
                path.ok_or_else(|| RongJSError::Error("previewMedia item requires path".into()))?;

            let resolved_path = lxapp.resolve_accessible_path(&raw_path).map_err(|err| {
                RongJSError::Error(format!("previewMedia path not accessible: {}", err))
            })?;
            let normalized_path = resolved_path.to_string_lossy().into_owned();

            Ok(PreviewMediaItem {
                path: normalized_path,
                media_type: parse_media_kind(kind),
                cover_path,
            })
        })
        .collect::<Result<_, _>>()?;

    let request = PreviewMediaRequest { items };

    runtime
        .preview_media(request)
        .map_err(|e| RongJSError::Error(format!("previewMedia failed: {}", e)))
}

#[derive(FromJSObj)]
struct JSSaveMediaOptions {
    #[rename = "filePath"]
    file_path: String,
}

fn save_image_to_photos_album(ctx: JSContext, options: JSSaveMediaOptions) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let runtime = &lxapp.runtime;

    let request = SaveMediaRequest {
        file_uri: options.file_path,
    };

    runtime
        .save_image_to_photos_album(request)
        .map_err(|e| RongJSError::Error(format!("saveImageToPhotosAlbum failed: {}", e)))
}

fn save_video_to_photos_album(ctx: JSContext, options: JSSaveMediaOptions) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let runtime = &lxapp.runtime;

    let request = SaveMediaRequest {
        file_uri: options.file_path,
    };

    runtime
        .save_video_to_photos_album(request)
        .map_err(|e| RongJSError::Error(format!("saveVideoToPhotosAlbum failed: {}", e)))
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let preview_media_func = JSFunc::new(ctx, preview_media)?;
    lx::register_js_api(ctx, "previewMedia", preview_media_func)?;

    let save_image_func = JSFunc::new(ctx, save_image_to_photos_album)?;
    lx::register_js_api(ctx, "saveImageToPhotosAlbum", save_image_func)?;

    let save_video_func = JSFunc::new(ctx, save_video_to_photos_album)?;
    lx::register_js_api(ctx, "saveVideoToPhotosAlbum", save_video_func)?;

    let choose_media_func = JSFunc::new(ctx, choose_media)?;
    lx::register_js_api(ctx, "chooseMedia", choose_media_func)?;

    let scan_func = JSFunc::new(ctx, scan)?;
    lx::register_js_api(ctx, "scanCode", scan_func)?;

    Ok(())
}

#[derive(FromJSObj, Clone)]
struct JSChooseMediaOptions {
    #[rename = "count"]
    count: Option<u32>,
    #[rename = "mediaType"]
    media_type: Option<Vec<String>>, // ["image", "video"]
    #[rename = "sourceType"]
    source_type: Option<Vec<String>>, // ["album", "camera"]
    camera: Option<String>, // "front" | "back"
    #[rename = "maxDuration"]
    max_duration: Option<f64>,
}

#[derive(FromJSObj, Clone, Default)]
struct JSScanOptions {
    #[rename = "onlyFromCamera"]
    only_from_camera: Option<bool>,
    #[rename = "scanType"]
    scan_type: Option<Vec<String>>, // strict: if present, must be array; otherwise omit for all
}

#[derive(Debug, Clone, IntoJSObj)]
struct ScanResultObj {
    #[rename = "scanResult"]
    scan_result: String,
    #[rename = "scanType"]
    scan_type: String,
}

fn parse_scan_type_token(value: &str) -> Option<ScanType> {
    // Strict tokens only
    match value {
        "barCode" => Some(ScanType::BarCode),
        "qrCode" => Some(ScanType::QrCode),
        "datamatrix" => Some(ScanType::DataMatrix),
        "pdf417" => Some(ScanType::Pdf417),
        _ => None,
    }
}

fn parse_scan_types(value: Option<Vec<String>>) -> JSResult<Vec<ScanType>> {
    let mut out: Vec<ScanType> = Vec::new();
    if let Some(list) = value {
        for token in list {
            let t = parse_scan_type_token(token.as_str())
                .ok_or_else(|| RongJSError::Error("invalid scanType token".to_string()))?;
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
struct ChosenMediaEntry {
    path: String,
    kind: String,
    is_original: bool, // true = original, false = compressed
}

impl rong::IntoJSValue<rong::JSEngineValue> for ChosenMediaEntry {
    fn into_js_value(self, ctx: &rong::JSContext) -> rong::JSEngineValue {
        let obj = JSObject::new(ctx);
        obj.set("tempFilePath", self.path).ok();
        obj.set("fileType", self.kind).ok();
        obj.set("isOriginal", self.is_original).ok();
        obj.into_value()
    }
}

impl rong::function::JSParameterType for ChosenMediaEntry {}

fn default_kind() -> String {
    "image".to_string()
}
fn default_is_original() -> bool {
    true
}
#[derive(Deserialize, Serialize, Hash, Clone)]
struct MediaKey {
    uri: String,
    #[serde(rename = "fileType", default = "default_kind")]
    kind: String,
    #[serde(rename = "isOriginal", default = "default_is_original")]
    is_original: bool,
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
        // Default fallback when caller passes empty array.
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

fn label_for_media_source(source: MediaSource) -> &'static str {
    match source {
        MediaSource::Album => "Album",
        MediaSource::Camera => "Camera",
    }
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

fn parse_camera(s: Option<String>) -> Option<CameraFacing> {
    s.map(|v| match v.to_lowercase().as_str() {
        "front" => CameraFacing::Front,
        _ => CameraFacing::Back,
    })
}

async fn choose_media(
    ctx: JSContext,
    options: Optional<JSChooseMediaOptions>,
) -> JSResult<Vec<ChosenMediaEntry>> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

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

    // Start platform flow
    lxapp
        .runtime
        .choose_media(request)
        .map_err(|e| RongJSError::Error(format!("chooseMedia failed to start: {}", e)))?;

    // Await single aggregated callback
    let CallbackResult { success, data } = receiver
        .await
        .map_err(|_| RongJSError::Error("chooseMedia cancelled or failed".to_string()))?;

    if !success {
        return Err(RongJSError::Error(data));
    }

    let parsed: Value = serde_json::from_str(&data)
        .map_err(|_| RongJSError::Error("chooseMedia invalid payload".to_string()))?;

    if parsed.is_null() {
        return Ok(Vec::new());
    }

    if let Some(obj) = parsed.as_object() {
        if obj.get("cancel").and_then(Value::as_bool).unwrap_or(false) {
            // Harmony platform may include index field alongside cancel flag.
            return Ok(Vec::new());
        }
        return Err(RongJSError::Error(
            "chooseMedia invalid payload".to_string(),
        ));
    }

    if !parsed.is_array() {
        return Err(RongJSError::Error(
            "chooseMedia invalid payload".to_string(),
        ));
    }

    // Expect an array of entries: [{ uri, fileType, isOriginal }, ...]
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
        let final_path = if std::path::Path::new(&uri).is_absolute() {
            uri.to_string()
        } else {
            // Build deterministic cache key (serde-friendly + Hash)
            let ext = match kind.as_str() {
                "video" => "mp4",
                _ => "jpg",
            };
            match lxapp.cache().resolve_path_with_ext(&key, ext) {
                lingxia_lxapp::ResolveResult::Exists(path) => path.to_string_lossy().to_string(),
                lingxia_lxapp::ResolveResult::NonExists(path) => {
                    let media_kind = match kind.as_str() {
                        "video" => MediaKind::Video,
                        "image" => MediaKind::Image,
                        _ => MediaKind::Image,
                    };
                    match lxapp
                        .runtime
                        .copy_media_uri_to_path(&uri, &path, media_kind)
                    {
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

async fn scan(ctx: JSContext, options: Optional<JSScanOptions>) -> JSResult<ScanResultObj> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let opts = options.as_ref().cloned().unwrap_or_default();
    let scan_types = parse_scan_types(opts.scan_type)?; // empty vec means all types
    let only_from_camera = opts.only_from_camera.unwrap_or(true);

    let (callback_id, receiver) = get_callback();

    let request = ScanCodeRequest {
        scan_types,
        only_from_camera,
        callback_id,
    };

    lxapp
        .runtime
        .scan_code(request)
        .map_err(|e| RongJSError::Error(format!("scan failed to start: {}", e)))?;

    let CallbackResult { success, data } = receiver
        .await
        .map_err(|_| RongJSError::Error("scan cancelled or failed".to_string()))?;

    if !success {
        return Err(RongJSError::Error(data));
    }

    // Accept empty payload (e.g., cancel). Parse best-effort; empty means empty result.
    let payload: Value = serde_json::from_str(&data).unwrap_or(Value::Null);

    let scan_result = payload
        .get("scanResult")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "".to_string());

    let scan_type = payload
        .get("scanType")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "".to_string());

    Ok(ScanResultObj {
        scan_result,
        scan_type,
    })
}
