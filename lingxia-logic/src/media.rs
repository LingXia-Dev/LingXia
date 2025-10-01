use lingxia_lxapp::{LxApp, lx};
use lingxia_platform::{MediaInteraction, MediaKind, PreviewMediaItem, PreviewMediaRequest, SaveMediaRequest};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::sync::Arc;

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

    Ok(())
}
