use crate::i18n::{
    js_error_from_business_code, js_error_from_lxapp_error, js_error_from_platform_error,
    js_internal_error, js_invalid_parameter_error, js_timeout_error,
};
use futures::channel::oneshot;
use futures::future::{Either, select};
use lingxia_messaging::{CallbackResult, get_callback, remove_callback};
use lingxia_platform::traits::media_interaction::{
    MediaInteraction, MediaKind, MediaObjectFit, PreviewMediaAdvance, PreviewMediaItem,
    PreviewMediaRequest,
};
use lxapp::{LxApp, lx};
use rong::{
    HostError, IntoJSObj, JSArray, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError,
};
use serde::Deserialize;

#[derive(Debug, Clone)]
struct RawPreviewMediaSource {
    path: Option<String>,
    kind: Option<String>,
    cover_path: Option<String>,
    rotate: Option<u16>,
    object_fit: Option<String>,
    duration_ms: Option<f64>,
}

#[derive(Debug, Clone)]
struct RawPreviewMediaSequenceOptions {
    sources: Vec<RawPreviewMediaSource>,
    start_index: Option<f64>,
    advance: Option<String>,
    show_index_indicator: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, IntoJSObj)]
struct PreviewMediaResultObj {
    reason: String,
    #[serde(rename = "lastIndex")]
    #[rename = "lastIndex"]
    last_index: u32,
}

struct AbortListener {
    signal: JSObject,
    callback: JSFunc,
}

struct ParsedPreviewMediaRequest {
    items: Vec<PreviewMediaItem>,
    start_index: i32,
    advance: PreviewMediaAdvance,
    show_index_indicator: bool,
    signal: Option<JSObject>,
}

impl AbortListener {
    fn attach(ctx: &JSContext, signal: JSObject) -> JSResult<(Self, oneshot::Receiver<()>)> {
        let add_event_listener = signal.get::<_, JSFunc>("addEventListener").map_err(|_| {
            js_invalid_parameter_error("previewMedia signal must be an AbortSignal")
        })?;

        let (abort_tx, abort_rx) = oneshot::channel();
        let callback = JSFunc::new_once(ctx, move || -> JSResult<()> {
            let _ = abort_tx.send(());
            Ok(())
        })?;

        add_event_listener.call::<_, ()>(Some(signal.clone()), ("abort", callback.clone()))?;

        Ok((Self { signal, callback }, abort_rx))
    }

    fn detach(self) {
        if let Ok(remove_event_listener) = self.signal.get::<_, JSFunc>("removeEventListener") {
            let _ = remove_event_listener
                .call::<_, ()>(Some(self.signal.clone()), ("abort", self.callback));
        }
    }
}

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let preview_media_func = JSFunc::new(ctx, |ctx, options| async move {
        preview_media(ctx, options).await
    })?;
    lx::register_js_api(ctx, "previewMedia", preview_media_func)?;
    Ok(())
}

async fn preview_media(ctx: JSContext, options: JSValue) -> JSResult<PreviewMediaResultObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let ParsedPreviewMediaRequest {
        items,
        start_index,
        advance,
        show_index_indicator,
        signal,
    } = parse_preview_request(&lxapp, options)?;

    if let Some(signal_obj) = signal.as_ref() {
        if signal_obj.get::<_, bool>("aborted").unwrap_or(false) {
            return Err(js_abort_error("previewMedia aborted"));
        }
    }

    let (mut abort_listener, abort_rx) = match signal {
        Some(signal_obj) => {
            let (listener, rx) = AbortListener::attach(&ctx, signal_obj)?;
            (Some(listener), Some(rx))
        }
        None => (None, None),
    };

    let (callback_id, receiver) = get_callback();
    let request = PreviewMediaRequest {
        items,
        start_index,
        advance,
        show_index_indicator,
        callback_id,
    };

    if let Err(err) = lxapp.runtime.preview_media(request) {
        if let Some(listener) = abort_listener.take() {
            listener.detach();
        }
        let _ = remove_callback(callback_id);
        return Err(js_error_from_platform_error(&err));
    }

    let callback_result = if let Some(abort_rx) = abort_rx {
        match select(receiver, abort_rx).await {
            Either::Left((callback_result, _)) => {
                callback_result.map_err(|_| js_timeout_error("previewMedia callback timed out"))?
            }
            Either::Right((_aborted, _)) => {
                let _ = remove_callback(callback_id);
                let _ = lxapp.runtime.cancel_preview(callback_id);
                if let Some(listener) = abort_listener.take() {
                    listener.detach();
                }
                return Err(js_abort_error("previewMedia aborted"));
            }
        }
    } else {
        receiver
            .await
            .map_err(|_| js_timeout_error("previewMedia callback timed out"))?
    };

    if let Some(listener) = abort_listener.take() {
        listener.detach();
    }

    parse_preview_callback_result(callback_result)
}

fn parse_preview_request(lxapp: &LxApp, options: JSValue) -> JSResult<ParsedPreviewMediaRequest> {
    if options.is_string() {
        let path = options
            .clone()
            .to_rust::<String>()
            .map_err(|_| js_invalid_parameter_error("previewMedia path must be a string"))?;
        let item = parse_media_source(
            lxapp,
            RawPreviewMediaSource {
                path: Some(path),
                kind: None,
                cover_path: None,
                rotate: None,
                object_fit: None,
                duration_ms: None,
            },
        )?;
        return Ok(ParsedPreviewMediaRequest {
            items: vec![item],
            start_index: 0,
            advance: PreviewMediaAdvance::Manual,
            show_index_indicator: false,
            signal: None,
        });
    }

    let Some(obj) = options.into_object() else {
        return Err(js_invalid_parameter_error(
            "previewMedia expects a string path or an options object",
        ));
    };

    let signal = parse_optional_signal(&obj)?;

    let (items, start_index, advance, show_index_indicator) =
        if let Some(sources_value) = get_present_property(&obj, "sources") {
            let options = parse_sequence_options(obj.clone(), sources_value)?;
            if options.sources.is_empty() {
                return Err(js_invalid_parameter_error(
                    "previewMedia requires at least one item",
                ));
            }
            (
                options
                    .sources
                    .into_iter()
                    .map(|item| parse_media_source(lxapp, item))
                    .collect::<Result<Vec<_>, _>>()?,
                normalize_start_index(options.start_index)?,
                parse_advance(options.advance)?,
                options.show_index_indicator,
            )
        } else {
            let (source, advance, show_index_indicator) = parse_single_options(&obj)?;
            (
                vec![parse_media_source(lxapp, source)?],
                0,
                parse_advance(advance)?,
                show_index_indicator,
            )
        };
    let resolved_show_index_indicator = show_index_indicator.unwrap_or(items.len() > 1);

    Ok(ParsedPreviewMediaRequest {
        items,
        start_index,
        advance,
        show_index_indicator: resolved_show_index_indicator,
        signal,
    })
}

fn parse_sequence_options(
    obj: JSObject,
    sources_value: JSValue,
) -> JSResult<RawPreviewMediaSequenceOptions> {
    let source_values: Vec<JSValue> = sources_value
        .into_object()
        .and_then(JSArray::from_object)
        .ok_or_else(|| js_invalid_parameter_error("previewMedia sources must be an array"))?
        .iter_values()?
        .collect::<JSResult<Vec<_>>>()?;

    let mut sources = Vec::with_capacity(source_values.len());
    for (index, value) in source_values.into_iter().enumerate() {
        let field = format!("sources[{}]", index);
        let Some(source_obj) = value.into_object() else {
            return Err(js_invalid_parameter_error(format!(
                "previewMedia {} must be an object",
                field
            )));
        };
        sources.push(parse_raw_source_object(&source_obj, Some(field.as_str()))?);
    }

    Ok(RawPreviewMediaSequenceOptions {
        sources,
        start_index: read_optional_number_field(&obj, "startIndex", None)?,
        advance: read_optional_string_field(&obj, "advance", None)?,
        show_index_indicator: read_optional_bool_field(&obj, "showIndexIndicator", None)?,
    })
}

fn parse_single_options(
    obj: &JSObject,
) -> JSResult<(RawPreviewMediaSource, Option<String>, Option<bool>)> {
    Ok((
        parse_raw_source_object(obj, None)?,
        read_optional_string_field(obj, "advance", None)?,
        read_optional_bool_field(obj, "showIndexIndicator", None)?,
    ))
}

fn parse_raw_source_object(
    obj: &JSObject,
    context: Option<&str>,
) -> JSResult<RawPreviewMediaSource> {
    Ok(RawPreviewMediaSource {
        path: read_optional_string_field(obj, "path", context)?,
        kind: read_optional_string_field(obj, "type", context)?,
        cover_path: read_optional_string_field(obj, "coverPath", context)?,
        rotate: read_optional_u16_field(obj, "rotate", context)?,
        object_fit: read_optional_string_field(obj, "objectFit", context)?,
        duration_ms: read_optional_number_field(obj, "durationMs", context)?,
    })
}

fn parse_optional_signal(obj: &JSObject) -> JSResult<Option<JSObject>> {
    let Some(signal_value) = get_present_property(obj, "signal") else {
        return Ok(None);
    };
    signal_value
        .into_object()
        .map(Some)
        .ok_or_else(|| js_invalid_parameter_error("previewMedia signal must be an AbortSignal"))
}

fn get_present_property(obj: &JSObject, field: &str) -> Option<JSValue> {
    obj.get::<_, JSValue>(field)
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn read_optional_string_field(
    obj: &JSObject,
    field: &str,
    context: Option<&str>,
) -> JSResult<Option<String>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_string() {
        return Err(invalid_preview_field(context, field, "a string"));
    }
    value
        .to_rust::<String>()
        .map(Some)
        .map_err(|_| invalid_preview_field(context, field, "a string"))
}

fn read_optional_number_field(
    obj: &JSObject,
    field: &str,
    context: Option<&str>,
) -> JSResult<Option<f64>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_number() {
        return Err(invalid_preview_field(context, field, "a number"));
    }
    value
        .to_rust::<f64>()
        .map(Some)
        .map_err(|_| invalid_preview_field(context, field, "a number"))
}

fn read_optional_bool_field(
    obj: &JSObject,
    field: &str,
    context: Option<&str>,
) -> JSResult<Option<bool>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_boolean() {
        return Err(invalid_preview_field(context, field, "a boolean"));
    }
    value
        .to_rust::<bool>()
        .map(Some)
        .map_err(|_| invalid_preview_field(context, field, "a boolean"))
}

fn read_optional_u16_field(
    obj: &JSObject,
    field: &str,
    context: Option<&str>,
) -> JSResult<Option<u16>> {
    let Some(value) = get_present_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_number() {
        return Err(invalid_preview_field(context, field, "an integer"));
    }
    let number = value
        .to_rust::<f64>()
        .map_err(|_| invalid_preview_field(context, field, "an integer"))?;
    if !number.is_finite() || number.fract() != 0.0 || number < 0.0 || number > u16::MAX as f64 {
        return Err(invalid_preview_field(context, field, "an integer"));
    }
    Ok(Some(number as u16))
}

fn invalid_preview_field(context: Option<&str>, field: &str, expected: &str) -> RongJSError {
    js_invalid_parameter_error(format!(
        "previewMedia {} must be {}",
        field_label(context, field),
        expected
    ))
}

fn field_label(context: Option<&str>, field: &str) -> String {
    match context {
        Some(prefix) if !prefix.is_empty() => format!("{}.{}", prefix, field),
        _ => field.to_string(),
    }
}

fn parse_preview_callback_result(result: CallbackResult) -> JSResult<PreviewMediaResultObj> {
    let data = match result {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => return Err(js_error_from_business_code(code)),
    };

    let parsed: PreviewMediaResultObj = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("previewMedia invalid payload: {}", e)))?;
    if parsed.reason.trim().is_empty() {
        return Err(js_internal_error(
            "previewMedia payload reason must be a non-empty string",
        ));
    }
    Ok(parsed)
}

fn parse_media_source(lxapp: &LxApp, item: RawPreviewMediaSource) -> JSResult<PreviewMediaItem> {
    let raw_path = item
        .path
        .ok_or_else(|| js_invalid_parameter_error("previewMedia item requires path"))?;
    let normalized_path = resolve_preview_path(lxapp, raw_path.as_str())?;

    let cover_path = item
        .cover_path
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|cover| -> Result<String, RongJSError> { resolve_preview_path(lxapp, cover.as_str()) })
        .transpose()?;

    Ok(PreviewMediaItem {
        media_type: parse_media_kind(item.kind, normalized_path.as_str())?,
        path: normalized_path,
        cover_path,
        rotate: parse_rotation(item.rotate)?,
        object_fit: parse_object_fit(item.object_fit)?,
        duration_ms: normalize_duration_ms(item.duration_ms, "previewMedia durationMs")?,
    })
}

fn parse_advance(value: Option<String>) -> JSResult<PreviewMediaAdvance> {
    let Some(raw) = value else {
        return Ok(PreviewMediaAdvance::Manual);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "manual" => Ok(PreviewMediaAdvance::Manual),
        "next" => Ok(PreviewMediaAdvance::Next),
        "loop" => Ok(PreviewMediaAdvance::Loop),
        _ => Err(js_invalid_parameter_error(format!(
            "previewMedia invalid advance: {}",
            raw
        ))),
    }
}

fn normalize_duration_ms(value: Option<f64>, field_name: &str) -> JSResult<Option<u64>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    if !raw.is_finite() || raw < 0.0 {
        return Err(js_invalid_parameter_error(format!(
            "{} must be a finite non-negative number",
            field_name
        )));
    }
    if raw == 0.0 {
        return Ok(None);
    }
    Ok(Some(raw.round().min(u64::MAX as f64) as u64))
}

fn normalize_start_index(value: Option<f64>) -> JSResult<i32> {
    let Some(raw) = value else {
        return Ok(0);
    };
    if !raw.is_finite() || raw.fract() != 0.0 {
        return Err(js_invalid_parameter_error(
            "previewMedia startIndex must be a finite integer",
        ));
    }
    Ok(raw.clamp(i32::MIN as f64, i32::MAX as f64) as i32)
}

fn parse_media_kind(value: Option<String>, path: &str) -> JSResult<MediaKind> {
    let Some(raw) = value else {
        return Ok(infer_media_kind_from_path(path));
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

fn infer_media_kind_from_path(path: &str) -> MediaKind {
    let normalized = path
        .split(['?', '#'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    let extension = normalized.rsplit('.').next().unwrap_or("");
    match extension {
        // Universal: supported on all platforms
        "mp4" | "mov" | "m4v" | "3gp" | "3gpp" => MediaKind::Video,
        // Android (ExoPlayer): broad codec support
        #[cfg(target_os = "android")]
        "mkv" | "webm" => MediaKind::Video,
        _ => MediaKind::Image,
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
    if trimmed.contains("[object Object]") {
        return Err(js_invalid_parameter_error(
            "previewMedia item path must be a string path, got [object Object]",
        ));
    }

    let resolved = lxapp
        .resolve_accessible_path(trimmed)
        .map_err(|err| js_error_from_lxapp_error(&err))?;
    Ok(resolved.to_string_lossy().into_owned())
}

fn js_abort_error(detail: impl AsRef<str>) -> RongJSError {
    HostError::new(rong::error::E_ABORT, detail.as_ref()).into()
}
