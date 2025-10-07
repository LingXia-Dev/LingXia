use futures::stream;
use lingxia_lxapp::{LxApp, lx};
use lingxia_messaging::{CallbackResult, get_stream_callback, remove_callback};
use lingxia_platform::{
    CameraFacing, ChooseMediaMode, ChooseMediaRequest, MediaInteraction, MediaKind, MediaSource,
    PreviewMediaItem, PreviewMediaRequest, SaveMediaRequest,
};
use rong::{FromJSObj, IntoJSAsyncIteratorExt, JSContext, JSFunc, JSObject, JSResult, RongJSError};
use std::fs::File;
use std::io::{self};
use std::os::fd::FromRawFd;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(FromJSObj)]
struct JSPreviewMediaItem {
    url: Option<String>,
    #[rename = "type"]
    kind: Option<String>,
    #[rename = "coverUrl"]
    cover_url: Option<String>,
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
                url,
                kind,
                cover_url,
            } = item;

            let resolved_path =
                url.ok_or_else(|| RongJSError::Error("previewMedia item requires url".into()))?;

            Ok(PreviewMediaItem {
                path: resolved_path,
                media_type: parse_media_kind(kind),
                cover_url,
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
    source_type: Option<Vec<String>>, // ["album", "camera"]
    camera: Option<String>, // "front" | "back"
    #[rename = "sizeType"]
    size_type: Option<Vec<String>>, // ["original", "compressed"]
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
        .unwrap_or_else(|| "image".to_string())
        .to_lowercase()
        .as_str()
    {
        "video" | "videos" => ChooseMediaMode::Videos,
        "mix" => ChooseMediaMode::Mix,
        _ => ChooseMediaMode::Images,
    }
}

fn parse_sources(v: Option<Vec<String>>) -> Vec<MediaSource> {
    v.unwrap_or_else(|| vec!["album".to_string(), "camera".to_string()])
        .into_iter()
        .filter_map(|s| match s.to_lowercase().as_str() {
            "album" => Some(MediaSource::Album),
            "camera" => Some(MediaSource::Camera),
            _ => None,
        })
        .collect()
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

fn choose_media(
    ctx: JSContext,
    options: rong::function::Optional<JSChooseMediaOptions>,
) -> JSResult<JSObject> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    let opts = options.as_ref().cloned().unwrap_or(JSChooseMediaOptions {
        count: None,
        max_count: None,
        media_type: None,
        source_type: None,
        camera: None,
        size_type: None,
    });

    let (callback_id, receiver) = get_stream_callback();
    let cache_root = lxapp.user_cache_dir.clone();

    let (allow_original, allow_compressed) = parse_size_flags(opts.size_type);
    let request = ChooseMediaRequest {
        max_count: opts.max_count.or(opts.count).unwrap_or(20),
        mode: parse_choose_mode(opts.media_type),
        source_types: parse_sources(opts.source_type),
        max_duration_seconds: None,
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

    let stream = stream::unfold(
        (Some(receiver), callback_id, cache_root.clone()),
        |(receiver_opt, callback_id, cache_root)| async move {
            let mut receiver = match receiver_opt {
                Some(r) => r,
                None => return None,
            };

            match receiver.recv().await {
                Some(CallbackResult { success, data }) => {
                    // Close on error or done
                    let parsed: serde_json::Value = match serde_json::from_str(&data) {
                        Ok(v) => v,
                        Err(_) => {
                            remove_callback(callback_id);
                            return None;
                        }
                    };

                    if !success {
                        remove_callback(callback_id);
                        return None;
                    }

                    if parsed
                        .get("done")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        remove_callback(callback_id);
                        return None;
                    }

                    if parsed
                        .get("cancel")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        remove_callback(callback_id);
                        return None;
                    }

                    // Expect raw UI item; parse original uri + fileType + optional fd
                    let uri = parsed
                        .get("uri")
                        .and_then(|v| v.as_str())
                        .or_else(|| parsed.get("path").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let kind = parsed
                        .get("fileType")
                        .and_then(|v| v.as_str())
                        .or_else(|| parsed.get("type").and_then(|v| v.as_str()))
                        .unwrap_or("image");
                    let fd_opt = parsed.get("fd").and_then(|v| v.as_i64());

                    if uri.is_empty() {
                        remove_callback(callback_id);
                        return None;
                    }

                    // If fd present (Android), copy to cache immediately and return final path
                    let final_path = if let Some(fd_val) = fd_opt {
                        let cache_dir = cache_root.clone();
                        let ext = match kind {
                            "video" => "mp4",
                            _ => "jpg",
                        };
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        let filename = format!("{}.{}", now, ext);
                        let dest_path = cache_dir.join(filename);

                        // Safety: take ownership of the fd here
                        let mut src = unsafe { File::from_raw_fd(fd_val as i32) };
                        let mut dst = match File::create(&dest_path) {
                            Ok(f) => f,
                            Err(_) => {
                                remove_callback(callback_id);
                                return None;
                            }
                        };
                        if let Err(_) = io::copy(&mut src, &mut dst) {
                            remove_callback(callback_id);
                            return None;
                        }
                        dest_path.to_string_lossy().to_string()
                    } else {
                        uri.to_string()
                    };

                    let entry = ChosenMediaEntry {
                        path: final_path,
                        kind: kind.to_string(),
                    };

                    Some((entry, (Some(receiver), callback_id, cache_root)))
                }
                None => {
                    remove_callback(callback_id);
                    None
                }
            }
        },
    );

    stream.to_js_async_iter(&ctx)
}
