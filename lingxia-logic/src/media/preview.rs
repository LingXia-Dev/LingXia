use lingxia_lxapp::{LxApp, lx};
use lingxia_platform::{MediaInteraction, MediaKind, PreviewMediaItem, PreviewMediaRequest};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};

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

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let preview_media_func = JSFunc::new(ctx, preview_media)?;
    lx::register_js_api(ctx, "previewMedia", preview_media_func)?;
    Ok(())
}

fn preview_media(ctx: JSContext, options: JSPreviewMediaOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

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

    lxapp
        .runtime
        .preview_media(request)
        .map_err(|e| RongJSError::Error(format!("previewMedia failed: {}", e)))
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
