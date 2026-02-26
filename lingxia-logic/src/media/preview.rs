use crate::i18n::{
    js_error_from_lxapp_error, js_error_from_platform_error, js_invalid_parameter_error,
};
use lingxia_platform::traits::media_interaction::{
    MediaInteraction, MediaKind, PreviewMediaItem, PreviewMediaRequest,
};
use lxapp::{LxApp, lx};
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
        return Err(js_invalid_parameter_error(
            "previewMedia requires at least one item",
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
                path.ok_or_else(|| js_invalid_parameter_error("previewMedia item requires path"))?;

            let resolved_path = lxapp
                .resolve_accessible_path(raw_path.trim())
                .map_err(|err| js_error_from_lxapp_error(&err))?;
            let normalized_path = resolved_path.to_string_lossy().into_owned();

            let cover_path = cover_path
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .map(|cover| -> Result<String, RongJSError> {
                    if cover.starts_with("http://") || cover.starts_with("https://") {
                        Ok(cover)
                    } else {
                        let resolved = lxapp
                            .resolve_accessible_path(&cover)
                            .map_err(|err| js_error_from_lxapp_error(&err))?;
                        Ok(resolved.to_string_lossy().into_owned())
                    }
                })
                .transpose()?;

            Ok(PreviewMediaItem {
                path: normalized_path,
                media_type: parse_media_kind(kind)?,
                cover_path,
            })
        })
        .collect::<Result<_, _>>()?;

    let request = PreviewMediaRequest { items };

    lxapp
        .runtime
        .preview_media(request)
        .map_err(|e| js_error_from_platform_error(&e))
}

fn parse_media_kind(value: Option<String>) -> JSResult<MediaKind> {
    let Some(raw) = value else {
        return Ok(MediaKind::Image);
    };
    match raw.to_lowercase().as_str() {
        "video" => Ok(MediaKind::Video),
        "image" => Ok(MediaKind::Image),
        _ => Err(js_invalid_parameter_error(format!(
            "previewMedia invalid type: {}",
            raw
        ))),
    }
}
