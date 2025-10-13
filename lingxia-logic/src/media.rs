use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_callback};
use lingxia_platform::AppRuntime;
use lingxia_platform::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaSource,
    PreviewMediaItem, PreviewMediaRequest, SaveMediaRequest,
};
use rong::{FromJSObj, JSContext, JSFunc, JSObject, JSResult, RongJSError, function::Optional};
use std::fs::File;
use std::io::{self};
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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

    Ok(())
}

#[derive(FromJSObj, Clone)]
struct JSChooseMediaOptions {
    #[rename = "count"]
    count: Option<u32>,
    #[rename = "maxCount"]
    max_count: Option<u32>,
    #[rename = "mediaType"]
    media_type: Option<String>, // "image" | "video" | "mix"
    #[rename = "sourceType"]
    source_type: Option<String>, // "album" | "camera"
    camera: Option<String>, // "front" | "back"
    #[rename = "sizeType"]
    size_type: Option<Vec<String>>, // ["original", "compressed"]
    #[rename = "maxDuration"]
    max_duration: Option<f64>,
}

#[derive(Debug, Clone)]
struct ChosenMediaEntry {
    path: String,
    kind: String,
}

impl rong::IntoJSValue<rong::JSEngineValue> for ChosenMediaEntry {
    fn into_js_value(self, ctx: &rong::JSContext) -> rong::JSEngineValue {
        let obj = JSObject::new(ctx);
        obj.set("path", self.path).ok();
        obj.set("fileType", self.kind).ok();
        obj.into_value()
    }
}

impl rong::function::JSParameterType for ChosenMediaEntry {}

fn parse_choose_mode(s: Option<String>) -> ChooseMediaMode {
    match s
        .unwrap_or_else(|| "mix".to_string())
        .to_lowercase()
        .as_str()
    {
        "video" | "videos" => ChooseMediaMode::Videos,
        "image" | "images" => ChooseMediaMode::Images,
        _ => ChooseMediaMode::Mix,
    }
}

fn parse_source(v: Option<String>) -> MediaSource {
    match v.as_deref() {
        Some("camera") => MediaSource::Camera,
        // default and "album": album-only
        _ => MediaSource::Album,
    }
}

fn parse_camera(s: Option<String>) -> Option<CameraFacing> {
    s.map(|v| match v.to_lowercase().as_str() {
        "front" => CameraFacing::Front,
        _ => CameraFacing::Back,
    })
}

fn parse_size_flags(v: Option<Vec<String>>) -> (bool, bool) {
    if let Some(list) = v {
        let mut allow_original = false;
        let mut allow_compressed = false;
        for s in list {
            match s.to_lowercase().as_str() {
                "original" => allow_original = true,
                "compressed" => allow_compressed = true,
                _ => {}
            }
        }
        // If none specified, default to both true
        if !allow_original && !allow_compressed {
            (true, true)
        } else {
            (allow_original, allow_compressed)
        }
    } else {
        (true, true)
    }
}

async fn choose_media(
    ctx: JSContext,
    options: Optional<JSChooseMediaOptions>,
) -> JSResult<Vec<ChosenMediaEntry>> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    let opts = options.as_ref().cloned().unwrap_or(JSChooseMediaOptions {
        count: None,
        max_count: None,
        media_type: None,
        source_type: None,
        camera: None,
        size_type: None,
        max_duration: None,
    });

    let (callback_id, receiver) = get_callback();
    let cache_root = lxapp.user_cache_dir.clone();

    let (allow_original, allow_compressed) = parse_size_flags(opts.size_type);
    let max_duration_seconds = opts
        .max_duration
        .filter(|v| !v.is_sign_negative())
        .map(|v| v.min(u32::MAX as f64).round() as u32);
    let source = parse_source(opts.source_type);
    let source_types = vec![source];
    let mode = parse_choose_mode(opts.media_type);
    // Mix applies only to album UI; when camera-only, report error
    if matches!(source, MediaSource::Camera) {
        if let ChooseMediaMode::Mix = mode {
            return Err(RongJSError::Error(
                "camera source does not support mediaType \"mix\"; use \"image\" or \"video\""
                    .into(),
            ));
        }
    }

    let request = ChooseMediaRequest {
        max_count: opts.max_count.or(opts.count).unwrap_or(20),
        mode,
        source_types,
        max_duration_seconds,
        camera_facing: parse_camera(opts.camera),
        allow_original,
        allow_compressed,
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

    // Expect an array of entries: [{ uri, fileType, fd? }, ...]
    let parsed: serde_json::Value = serde_json::from_str(&data)
        .map_err(|_| RongJSError::Error("chooseMedia invalid payload".to_string()))?;
    let arr = parsed.as_array().cloned().unwrap_or_else(|| Vec::new());

    let mut out: Vec<ChosenMediaEntry> = Vec::new();
    for item in arr.into_iter() {
        let uri = item.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        let kind = item
            .get("fileType")
            .and_then(|v| v.as_str())
            .unwrap_or("image");
        if uri.is_empty() {
            continue;
        }
        let fd_opt = item.get("fd").and_then(|v| v.as_i64());

        let final_path = if let Some(fd_val) = fd_opt {
            let ext = match kind {
                "video" => "mp4",
                _ => "jpg",
            };
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let filename = format!("{}.{}", now, ext);
            let dest_path = cache_root.join(filename);
            let mut src = unsafe { File::from_raw_fd(fd_val as i32) };
            let mut dst = match File::create(&dest_path) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if io::copy(&mut src, &mut dst).is_err() {
                continue;
            }
            dest_path.to_string_lossy().to_string()
        } else {
            let ext = match kind {
                "video" => "mp4",
                _ => "jpg",
            };
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let filename = format!("{}.{}", now, ext);
            let dest_path = cache_root.join(filename);

            let media_kind = match kind {
                "video" => MediaKind::Video,
                "image" => MediaKind::Image,
                _ => MediaKind::Image,
            };

            match lxapp
                .runtime
                .copy_media_uri_to_path(uri, &dest_path, media_kind)
            {
                Ok(()) => dest_path.to_string_lossy().to_string(),
                Err(err) => {
                    return Err(RongJSError::Error(format!("copyMedia failed: {}", err)));
                }
            }
        };

        out.push(ChosenMediaEntry {
            path: final_path,
            kind: kind.to_string(),
        });
    }
    Ok(out)
}
