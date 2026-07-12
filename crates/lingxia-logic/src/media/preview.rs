use crate::i18n::{
    js_error_from_business_code, js_error_from_lxapp_error, js_error_from_platform_error,
    js_internal_error, js_invalid_parameter_error,
};
use futures::channel::oneshot;
use futures::future::{Either, select};
use lingxia_messaging::{CallbackResult, get_callback, get_stream_callback, remove_callback};
use lingxia_service::media::{
    MediaKind, MediaObjectFit, PreviewMediaAdvance, PreviewMediaItem, PreviewMediaRequest,
};
use lxapp::LxApp;
use rong::{
    HostError, JSArray, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise, RongJSError,
};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
struct RawPreviewMediaSource {
    path: Option<String>,
    kind: Option<String>,
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

/// Wire payload of the native session-end callback. Internal — the JS-facing
/// result is rebuilt as `{ reason, index, source }`.
#[derive(Debug, Clone, Deserialize)]
struct RawPreviewResult {
    reason: String,
    #[serde(rename = "lastIndex")]
    last_index: i64,
}

/// Wire payload of one native change-stream fire.
#[derive(Debug, Clone, Deserialize)]
struct RawChangePayload {
    index: i64,
}

/// What the caller passed for one item, kept verbatim so change events and
/// the completion result can hand the source back without the caller
/// re-indexing their own array.
#[derive(Debug, Clone)]
struct SourceMeta {
    /// The path exactly as the caller provided it (before resolution).
    path: String,
    kind: MediaKind,
}

type ChangeListeners = Rc<RefCell<Vec<Option<JSFunc>>>>;

struct AbortListener {
    signal: JSObject,
    callback: JSFunc,
}

struct ParsedPreviewMediaRequest {
    items: Vec<PreviewMediaItem>,
    metas: Vec<SourceMeta>,
    start_index: i32,
    advance: PreviewMediaAdvance,
    show_index_indicator: bool,
    signal: Option<JSObject>,
}

enum PreviewCompletion {
    Native(Result<CallbackResult, tokio::sync::oneshot::error::RecvError>),
    Aborted,
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

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn previewMedia(
            ts_params = "options: PreviewMediaOptions",
            ts_return = "PreviewMediaHandle"
        ) = preview_media;
    }
}

/// Synchronously returns a JS handle so listeners can be attached before the
/// first event fires:
/// - `presented`: Promise, resolves with no value when the first pixel of the
///   underlying media is composited to screen. Also resolves unconditionally
///   once `completed` settles, so consumers can safely ignore it (it never
///   rejects).
/// - `current`: `{ index, source }` snapshot of the item on screen, updated
///   live as the user swipes / the session auto-advances.
/// - `onChange(listener)`: fires `{ index, source }` for every item change
///   (the initial item is seeded into `current`, and re-fired by native so
///   late platforms still converge); returns an unsubscribe function.
/// - `completed`: Promise resolving `{ reason, index, source }` when the
///   session ends, or rejecting on abort / error. `source` is the item that
///   was on screen when the preview closed — handed back verbatim so the
///   caller never re-indexes their own array.
fn preview_media(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let parsed = parse_preview_request(&lxapp, options)?;
    let ParsedPreviewMediaRequest {
        items,
        metas,
        start_index,
        advance,
        show_index_indicator,
        signal,
    } = parsed;

    let metas: Rc<[SourceMeta]> = metas.into();
    let initial_index = clamp_item_index(start_index as i64, metas.len());

    // Early-abort: build a handle whose Promises are pre-settled.
    if let Some(signal_obj) = signal.as_ref()
        && signal_obj.get::<_, bool>("aborted").unwrap_or(false)
    {
        return build_pre_aborted_handle(&ctx, initial_index, &metas);
    }

    let (abort_listener, abort_rx) = match signal {
        Some(signal_obj) => {
            let (listener, rx) = AbortListener::attach(&ctx, signal_obj)?;
            (Some(listener), Some(rx))
        }
        None => (None, None),
    };

    let (completed_cb_id, completed_rx) = get_callback();
    let (presented_cb_id, presented_rx) = get_callback();
    let (change_cb_id, mut change_rx) = get_stream_callback();

    let request = PreviewMediaRequest {
        items,
        start_index,
        advance,
        show_index_indicator,
        callback_id: completed_cb_id,
        presented_callback_id: presented_cb_id,
        change_callback_id: change_cb_id,
    };

    if let Err(err) = lingxia_service::media::preview_media(&*lxapp.runtime, request) {
        if let Some(listener) = abort_listener {
            listener.detach();
        }
        let _ = remove_callback(completed_cb_id);
        let _ = remove_callback(presented_cb_id);
        let _ = remove_callback(change_cb_id);
        return Err(js_error_from_platform_error(&err));
    }

    let handle = JSObject::new(&ctx);
    let listeners: ChangeListeners = Rc::new(RefCell::new(Vec::new()));
    handle.set("current", build_change_obj(&ctx, initial_index, &metas)?)?;
    install_on_change(&ctx, &handle, &listeners)?;

    // Change pump: runs on the JS thread (Promise::from_future futures need
    // no Send bounds), reading the native change stream until the channel
    // closes — `completed` removes the stream callback on every exit path,
    // which drops the sender and ends this loop. The pump Promise is
    // retained on the handle below: an unreferenced Promise gets collected
    // and its future cancelled, which would silently kill the stream.
    let pump = {
        let ctx = ctx.clone();
        let handle = handle.clone();
        let listeners = listeners.clone();
        let metas = metas.clone();
        Promise::from_future(&ctx.clone(), None, async move {
            let mut last_index = initial_index as i64;
            while let Some(result) = change_rx.recv().await {
                let CallbackResult::Success(data) = result else {
                    continue;
                };
                let Ok(payload) = serde_json::from_str::<RawChangePayload>(&data) else {
                    log::warn!("previewMedia change payload is not valid JSON: {data}");
                    continue;
                };
                if payload.index < 0
                    || payload.index as usize >= metas.len()
                    || payload.index == last_index
                {
                    continue;
                }
                last_index = payload.index;
                let index = payload.index as u32;
                let change = build_change_obj(&ctx, index, &metas)?;
                handle.set("current", change.clone())?;
                let snapshot: Vec<JSFunc> = listeners.borrow().iter().flatten().cloned().collect();
                for listener in snapshot {
                    // A throwing listener must not break the stream or
                    // its peers.
                    let _ = listener.call::<_, JSValue>(None, (change.clone(),));
                }
            }
            Ok::<(), RongJSError>(())
        })?
    };
    handle.set("__lxChangePump", pump)?;

    // Fallback channel: the completed-Promise's future signals this when it
    // finishes (cleanly or via abort) so the presented-Promise always settles
    // even if native never fired the presented callback. Both futures run on
    // the same JS thread via Promise::from_future, so no Send bounds need to
    // be satisfied and we don't need an external executor.
    let (presented_fallback_tx, presented_fallback_rx) = oneshot::channel::<()>();

    let presented = Promise::from_future(&ctx, None, async move {
        // Whichever arrives first — native callback or fallback — resolves
        // the presented Promise. Drops the other receiver after the race.
        let _ = select(presented_rx, presented_fallback_rx).await;
        Ok::<(), RongJSError>(())
    })?;

    let lxapp_for_cancel = lxapp.clone();
    let completed_ctx = ctx.clone();
    let completed_metas = metas.clone();
    let completed = Promise::from_future(&ctx, None, async move {
        // `completed` settles on either the native callback or an external
        // abort. No wall-clock timeout: a preview session is bound to a
        // user-visible resource that the JS layer (or its AbortSignal)
        // is responsible for closing. A lost native callback would be a
        // bug to fix at the source; quietly tearing down a working
        // session here would mask it.
        let outcome: PreviewCompletion = match abort_rx {
            Some(abort_rx) => match select(completed_rx, abort_rx).await {
                Either::Left((cb, _)) => PreviewCompletion::Native(cb),
                Either::Right(_) => PreviewCompletion::Aborted,
            },
            None => PreviewCompletion::Native(completed_rx.await),
        };

        let result: JSResult<JSObject> = match outcome {
            PreviewCompletion::Native(cb) => cb
                .map_err(|_| js_internal_error("previewMedia callback channel closed"))
                .and_then(parse_preview_callback_result)
                .and_then(|raw| {
                    let index = clamp_item_index(raw.last_index, completed_metas.len());
                    let result = JSObject::new(&completed_ctx);
                    result.set("reason", raw.reason)?;
                    result.set("index", index)?;
                    result.set(
                        "source",
                        build_source_obj(&completed_ctx, index, &completed_metas)?,
                    )?;
                    Ok(result)
                }),
            PreviewCompletion::Aborted => {
                let _ = remove_callback(completed_cb_id);
                let _ = lingxia_service::media::cancel_preview(
                    &*lxapp_for_cancel.runtime,
                    completed_cb_id,
                );
                Err(js_abort_error("previewMedia aborted"))
            }
        };

        if let Some(listener) = abort_listener {
            listener.detach();
        }
        // Drop any still-registered presented callback (degenerate case
        // where native never signaled it before the session ended).
        let _ = remove_callback(presented_cb_id);
        // Close the change stream; the pump loop ends when the sender drops.
        let _ = remove_callback(change_cb_id);
        // Always wake the presented Promise; if it already resolved via the
        // native callback, this sender just drops harmlessly.
        let _ = presented_fallback_tx.send(());

        result
    })?;

    handle.set("presented", presented)?;
    handle.set("completed", completed)?;
    Ok(handle)
}

fn build_pre_aborted_handle(
    ctx: &JSContext,
    initial_index: u32,
    metas: &Rc<[SourceMeta]>,
) -> JSResult<JSObject> {
    let presented = Promise::from_future(ctx, None, async { Ok::<(), RongJSError>(()) })?;
    let completed = Promise::from_future(ctx, None, async {
        Err::<JSObject, RongJSError>(js_abort_error("previewMedia aborted"))
    })?;
    let handle = JSObject::new(ctx);
    handle.set("current", build_change_obj(ctx, initial_index, metas)?)?;
    install_on_change(ctx, &handle, &Rc::new(RefCell::new(Vec::new())))?;
    handle.set("presented", presented)?;
    handle.set("completed", completed)?;
    Ok(handle)
}

/// Clamp a native-reported item index into the items range.
fn clamp_item_index(index: i64, len: usize) -> u32 {
    let max = len.saturating_sub(1) as i64;
    index.clamp(0, max) as u32
}

/// `{ index, source }` — one change-stream event / the `current` snapshot.
fn build_change_obj(ctx: &JSContext, index: u32, metas: &[SourceMeta]) -> JSResult<JSObject> {
    let obj = JSObject::new(ctx);
    obj.set("index", index)?;
    obj.set("source", build_source_obj(ctx, index, metas)?)?;
    Ok(obj)
}

/// `{ path, type }` — the item as the caller passed it.
fn build_source_obj(ctx: &JSContext, index: u32, metas: &[SourceMeta]) -> JSResult<JSObject> {
    let source = JSObject::new(ctx);
    if let Some(meta) = metas.get(index as usize) {
        source.set("path", meta.path.as_str())?;
        let kind = match meta.kind {
            MediaKind::Video => "video",
            MediaKind::Image | MediaKind::Unknown => "image",
        };
        source.set("type", kind)?;
    }
    Ok(source)
}

/// Install `onChange(listener) -> unsubscribe` on the handle.
fn install_on_change(
    ctx: &JSContext,
    handle: &JSObject,
    listeners: &ChangeListeners,
) -> JSResult<()> {
    let listeners = listeners.clone();
    let on_change = JSFunc::new(
        ctx,
        move |ctx: JSContext, listener: JSFunc| -> JSResult<JSFunc> {
            let slot = {
                let mut slots = listeners.borrow_mut();
                slots.push(Some(listener));
                slots.len() - 1
            };
            let listeners = listeners.clone();
            JSFunc::new(&ctx, move || -> JSResult<()> {
                if let Some(entry) = listeners.borrow_mut().get_mut(slot) {
                    *entry = None;
                }
                Ok(())
            })
        },
    )?
    .name("onChange")?;
    handle.set("onChange", on_change)?;
    Ok(())
}

fn parse_preview_request(lxapp: &LxApp, options: JSValue) -> JSResult<ParsedPreviewMediaRequest> {
    if options.is_string() {
        let path = options
            .clone()
            .to_rust::<String>()
            .map_err(|_| js_invalid_parameter_error("previewMedia path must be a string"))?;
        let (item, meta) = parse_media_source(
            lxapp,
            RawPreviewMediaSource {
                path: Some(path),
                kind: None,
                rotate: None,
                object_fit: None,
                duration_ms: None,
            },
        )?;
        return Ok(ParsedPreviewMediaRequest {
            items: vec![item],
            metas: vec![meta],
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

    let (sources, start_index, advance, show_index_indicator) =
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
    let (items, metas): (Vec<_>, Vec<_>) = sources.into_iter().unzip();
    let resolved_show_index_indicator = show_index_indicator.unwrap_or(items.len() > 1);

    Ok(ParsedPreviewMediaRequest {
        items,
        metas,
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

fn parse_preview_callback_result(result: CallbackResult) -> JSResult<RawPreviewResult> {
    let data = match result {
        CallbackResult::Success(data) => data,
        CallbackResult::Error(code) => return Err(js_error_from_business_code(code)),
    };

    let parsed: RawPreviewResult = serde_json::from_str(&data)
        .map_err(|e| js_internal_error(format!("previewMedia invalid payload: {}", e)))?;
    if parsed.reason.trim().is_empty() {
        return Err(js_internal_error(
            "previewMedia payload reason must be a non-empty string",
        ));
    }
    Ok(parsed)
}

fn parse_media_source(
    lxapp: &LxApp,
    item: RawPreviewMediaSource,
) -> JSResult<(PreviewMediaItem, SourceMeta)> {
    let raw_path = item
        .path
        .ok_or_else(|| js_invalid_parameter_error("previewMedia item requires path"))?;
    let normalized_path = resolve_preview_path(lxapp, raw_path.as_str())?;
    let media_type = parse_media_kind(item.kind, normalized_path.as_str())?;

    let item = PreviewMediaItem {
        media_type,
        path: normalized_path,
        rotate: parse_rotation(item.rotate)?,
        object_fit: parse_object_fit(item.object_fit)?,
        duration_ms: normalize_duration_ms(item.duration_ms, "previewMedia durationMs")?,
    };
    let meta = SourceMeta {
        path: raw_path,
        kind: media_type,
    };
    Ok((item, meta))
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
