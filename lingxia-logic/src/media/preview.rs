use crate::i18n::{
    js_error_from_lxapp_error, js_error_from_platform_error, js_invalid_parameter_error,
};
use lingxia_platform::traits::media_interaction::{
    MediaInteraction, MediaKind, MediaObjectFit, PreviewMediaItem, PreviewMediaRequest,
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
    rotate: Option<u16>,
    #[rename = "objectFit"]
    object_fit: Option<String>,
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
                rotate,
                object_fit,
            } = item;

            let raw_path =
                path.ok_or_else(|| js_invalid_parameter_error("previewMedia item requires path"))?;
            let normalized_path = resolve_preview_path(&lxapp, raw_path.as_str())?;

            let cover_path = cover_path
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .map(|cover| -> Result<String, RongJSError> {
                    resolve_preview_path(&lxapp, cover.as_str())
                })
                .transpose()?;

            Ok(PreviewMediaItem {
                path: normalized_path,
                media_type: parse_media_kind(kind)?,
                cover_path,
                rotate: parse_rotation(rotate)?,
                object_fit: parse_object_fit(object_fit)?,
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
    match raw.trim().to_ascii_lowercase().as_str() {
        "video" => Ok(MediaKind::Video),
        "image" => Ok(MediaKind::Image),
        _ => Err(js_invalid_parameter_error(format!(
            "previewMedia invalid type: {}",
            raw
        ))),
    }
}

fn parse_rotation(value: Option<u16>) -> JSResult<Option<u16>> {
    let Some(rotation) = value else {
        return Ok(None);
    };
    match rotation {
        0 | 90 | 180 | 270 => Ok(Some(rotation)),
        _ => Err(js_invalid_parameter_error(format!(
            "previewMedia invalid rotate: {}",
            rotation
        ))),
    }
}

fn parse_object_fit(value: Option<String>) -> JSResult<Option<MediaObjectFit>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "cover" => Ok(Some(MediaObjectFit::Cover)),
        "contain" => Ok(Some(MediaObjectFit::Contain)),
        "fill" => Ok(Some(MediaObjectFit::Fill)),
        "fit" => Ok(Some(MediaObjectFit::Fit)),
        _ => Err(js_invalid_parameter_error(format!(
            "previewMedia invalid objectFit: {}",
            raw
        ))),
    }
}

fn resolve_preview_path(lxapp: &LxApp, raw: &str) -> JSResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(js_invalid_parameter_error(
            "previewMedia item path cannot be empty",
        ));
    }

    let resolved = lxapp
        .resolve_accessible_path(trimmed)
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    Ok(resolved.to_string_lossy().into_owned())
}
