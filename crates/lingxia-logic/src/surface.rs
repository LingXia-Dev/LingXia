use futures::channel::oneshot;
use lingxia_platform::traits::ui::{SurfaceKind, SurfacePosition};
use lxapp::{LxApp, PageQueryInput, PageSurfaceRequest, PageSurfaceTarget, PageTarget};
use rong::{
    Class, HostError, IntoJSObj, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise,
    function::{Rest, This},
    js_class, js_export, js_method,
};
use rong_event::{Emitter, EmitterExt, EventEmitter, EventKey};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use uuid::Uuid;

struct ClosedRegistration {
    kind: String,
    sender: oneshot::Sender<JSSurfaceClosed>,
}

static SURFACE_CLOSED: OnceLock<Mutex<HashMap<String, ClosedRegistration>>> = OnceLock::new();

#[derive(Debug, Clone, IntoJSObj)]
struct JSSurfaceClosed {
    id: String,
    kind: String,
    reason: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSSurfaceVisibility {
    id: String,
    kind: String,
    /// Which side initiated the visibility change. "opener" when the caller
    /// holds the opener-side surface, "page" when the page-side surface drove
    /// it. Lets analytics / logging distinguish without having to wire extra
    /// state through the caller.
    source: String,
}

#[js_export]
struct JSSurface {
    id: String,
    kind: String,
    message_port: JSObject,
    /// Bus for surface lifecycle events: "show", "hide", "close". Single
    /// emitter shared across event names — EventKey discriminates listeners.
    event_emitter: EventEmitter,
    /// Pointer to the sibling surface (opener ↔ page). When opener calls
    /// `show()/hide()` the event must also fire on the page-side Surface JS
    /// object so observers there see the visibility transition, and vice
    /// versa. Filled after both instances exist; before that it is None.
    peer: RefCell<Option<JSObject>>,
    /// Last-known visibility, mirrored from native. Reads through the JS
    /// `visible` property; we update both this cell and the JS-visible field
    /// in lockstep so consumers can branch on `surface.visible` declaratively.
    visible: Cell<bool>,
    /// True until close() fires. Becomes false in the close emit path so
    /// post-close `show()`/`hide()` are caught early instead of bouncing off
    /// the platform layer with an opaque error.
    alive: Cell<bool>,
}

#[js_class]
impl JSSurface {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Surface cannot be directly constructed",
        )
        .into())
    }

    #[js_method(rename = "close")]
    fn close(&self, ctx: JSContext) -> JSResult<Promise> {
        let lxapp = LxApp::from_ctx(&ctx)?;
        let id = self.id.clone();
        Promise::from_future(&ctx, None, async move {
            lxapp.close_surface(&id, "programmatic").map_err(|err| {
                surface_error(rong::error::E_INTERNAL, "surface_close_failed", err)
            })?;
            Ok(())
        })
    }

    #[js_method(rename = "postMessage")]
    fn post_message(&self, payload: JSValue) -> JSResult<()> {
        crate::message_port::emit_message(&self.message_port, payload)
    }

    #[js_method(rename = "onMessage")]
    fn on_message(&self, handler: JSFunc) -> JSResult<JSFunc> {
        crate::message_port::add_message_listener(&self.message_port, handler)
    }

    #[js_method(rename = "onClose")]
    fn on_close(this: This<JSObject>, handler: JSFunc) -> JSResult<JSFunc> {
        let target = (*this).clone();
        let ctx = target.context();
        let handler_for_off = handler.clone();
        <Self as EmitterExt>::add_event_listener(
            this,
            EventKey::String("close".to_string()),
            handler,
            false,
            false,
        )?;
        JSFunc::new(&ctx, move || {
            <JSSurface as EmitterExt>::remove_event_listener(
                This(target.clone()),
                EventKey::String("close".to_string()),
                handler_for_off.clone(),
            )
        })
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        mark_fn(self.message_port.as_js_value());
        if let Some(peer) = self.peer.borrow().as_ref() {
            mark_fn(peer.as_js_value());
        }
        self.event_emitter.gc_mark_with(mark_fn);
    }
}

impl Emitter for JSSurface {
    fn get_event_emitter(&self) -> EventEmitter {
        self.event_emitter.clone()
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_hidden_class::<JSSurface>()?;
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    let surface = JSObject::new(ctx);
    surface.set("open", JSFunc::new(ctx, open_surface)?)?;
    lx.set("surface", surface)?;
    Ok(())
}

async fn open_surface(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let mut request = parse_surface_options(&lxapp, &options)?;
    request.id = format!("surface-{}", Uuid::new_v4().simple());
    let surface_id = request.id.clone();
    let kind = surface_kind_label(request.kind).to_string();
    let surface_id_for_closed = surface_id.clone();
    let kind_for_closed = kind.clone();

    let (closed_tx, closed_rx) = oneshot::channel::<JSSurfaceClosed>();
    register_closed_sender(surface_id.clone(), kind.clone(), closed_tx);
    let opened_surface = lxapp.open_surface(request).map_err(|err| {
        unregister_closed_sender(&surface_id);
        surface_error(rong::error::E_INTERNAL, "surface_open_failed", err)
    })?;
    let page_svc = match opened_surface.page_instance_id.as_deref() {
        Some(page_instance_id) => Some(
            lxapp
                .get_page_in_ctx_by_instance_id(&ctx, page_instance_id)
                .await
                .map_err(|err| {
                    unregister_closed_sender(&surface_id);
                    let _ = lxapp.close_surface(&surface_id, "failed");
                    surface_error(rong::error::E_INTERNAL, "surface_open_failed", err)
                })?,
        ),
        None => None,
    };
    let (opener_port, page_port) = crate::message_port::pair(&ctx)?;
    let surface = Class::lookup::<JSSurface>(&ctx)?.instance(JSSurface {
        id: opened_surface.id.clone(),
        kind: kind.clone(),
        message_port: opener_port,
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
        visible: Cell::new(true),
        alive: Cell::new(true),
    });
    surface.set("id", opened_surface.id)?;
    surface.set("kind", kind.clone())?;
    surface.set("visible", true)?;
    surface.set("alive", true)?;
    attach_surface_methods(
        &ctx,
        &surface,
        lxapp.clone(),
        surface_id.clone(),
        surface.clone(),
        "opener",
    )?;
    let mut page_surface_for_close = None;
    if let Some(page_svc) = page_svc.as_ref() {
        let page_kind = surface_kind_label(opened_surface.kind).to_string();
        let page_surface = Class::lookup::<JSSurface>(&ctx)?.instance(JSSurface {
            id: surface_id.clone(),
            kind: page_kind.clone(),
            message_port: page_port,
            event_emitter: EventEmitter::default(),
            peer: RefCell::new(None),
            visible: Cell::new(true),
            alive: Cell::new(true),
        });
        page_surface.set("id", surface_id.clone())?;
        page_surface.set("kind", page_kind)?;
        page_surface.set("visible", true)?;
        page_surface.set("alive", true)?;
        attach_surface_methods(
            &ctx,
            &page_surface,
            lxapp.clone(),
            surface_id.clone(),
            page_surface.clone(),
            "page",
        )?;
        page_svc.bind_surface(page_surface.clone()).map_err(|err| {
            unregister_closed_sender(&surface_id);
            let _ = lxapp.close_surface(&surface_id, "failed");
            surface_error(rong::error::E_INTERNAL, "surface_open_failed", err)
        })?;
        // Link the two surface objects so visibility events fired on one also
        // fire on the other. Borrow scope is tight so we never hold a borrow
        // across the JSObject.clone() call.
        {
            let opener_inner = surface.borrow::<JSSurface>()?;
            *opener_inner.peer.borrow_mut() = Some(page_surface.clone());
        }
        {
            let page_inner = page_surface.borrow::<JSSurface>()?;
            *page_inner.peer.borrow_mut() = Some(surface.clone());
        }
        page_surface_for_close = Some(page_surface);
    }
    let surface_for_close = surface.clone();
    let page_svc_for_closed = page_svc.clone();
    Promise::from_future(&ctx, None, async move {
        let event = match closed_rx.await {
            Ok(event) => event,
            Err(_) => JSSurfaceClosed {
                id: surface_id_for_closed,
                kind: kind_for_closed,
                reason: "unknown".to_string(),
            },
        };
        if let Some(page_svc) = page_svc_for_closed {
            let _ = page_svc.clear_surface();
        }
        let _ = emit_close(&surface_for_close, &event);
        if let Some(page_surface) = page_surface_for_close {
            let _ = emit_close(&page_surface, &event);
        }
    })?;
    Ok(surface)
}

fn attach_surface_methods(
    ctx: &JSContext,
    surface: &JSObject,
    lxapp: Arc<LxApp>,
    surface_id: String,
    surface_ref: JSObject,
    side: &'static str,
) -> JSResult<()> {
    let close_lxapp = lxapp.clone();
    let close_id = surface_id.clone();
    surface.set(
        "close",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = close_lxapp.clone();
            let id = close_id.clone();
            Promise::from_future(&ctx, None, async move {
                lxapp.close_surface(&id, "programmatic").map_err(|err| {
                    surface_error(rong::error::E_INTERNAL, "surface_close_failed", err)
                })?;
                Ok(())
            })
        })?,
    )?;

    let show_lxapp = lxapp.clone();
    let show_id = surface_id.clone();
    let show_self = surface_ref.clone();
    surface.set(
        "show",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = show_lxapp.clone();
            let id = show_id.clone();
            let self_obj = show_self.clone();
            Promise::from_future(&ctx, None, async move {
                if !should_change_visible(&self_obj, true)? {
                    return Ok(());
                }
                lxapp.show_surface(&id).map_err(|err| {
                    surface_error(rong::error::E_INTERNAL, "surface_show_failed", err)
                })?;
                // Emit AFTER the platform call resolves so `await surface.show()`
                // returning implies listeners have been notified. Only fires on
                // state change so consumers don't see duplicate events.
                let _ = mark_visible(&self_obj, true, side);
                Ok(())
            })
        })?,
    )?;

    let hide_lxapp = lxapp.clone();
    let hide_id = surface_id.clone();
    let hide_self = surface_ref.clone();
    surface.set(
        "hide",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = hide_lxapp.clone();
            let id = hide_id.clone();
            let self_obj = hide_self.clone();
            Promise::from_future(&ctx, None, async move {
                if !should_change_visible(&self_obj, false)? {
                    return Ok(());
                }
                lxapp.hide_surface(&id).map_err(|err| {
                    surface_error(rong::error::E_INTERNAL, "surface_hide_failed", err)
                })?;
                let _ = mark_visible(&self_obj, false, side);
                Ok(())
            })
        })?,
    )?;

    let post_surface = surface_ref.clone();
    surface.set(
        "postMessage",
        JSFunc::new(ctx, move |payload: JSValue| {
            let surface = post_surface.borrow::<JSSurface>()?;
            crate::message_port::emit_message(&surface.message_port, payload)
        })?,
    )?;

    let listen_surface = surface_ref.clone();
    surface.set(
        "onMessage",
        JSFunc::new(ctx, move |handler: JSFunc| {
            let surface = listen_surface.borrow::<JSSurface>()?;
            crate::message_port::add_message_listener(&surface.message_port, handler)
        })?,
    )?;

    let on_close_surface = surface_ref.clone();
    surface.set(
        "onClose",
        JSFunc::new(ctx, move |handler: JSFunc| {
            add_event_listener_for(&on_close_surface, "close", handler)
        })?,
    )?;

    let on_show_surface = surface_ref.clone();
    surface.set(
        "onShow",
        JSFunc::new(ctx, move |handler: JSFunc| {
            add_event_listener_for(&on_show_surface, "show", handler)
        })?,
    )?;

    let on_hide_surface = surface_ref;
    surface.set(
        "onHide",
        JSFunc::new(ctx, move |handler: JSFunc| {
            add_event_listener_for(&on_hide_surface, "hide", handler)
        })?,
    )?;

    Ok(())
}

fn add_event_listener_for(
    surface: &JSObject,
    event_name: &str,
    handler: JSFunc,
) -> JSResult<JSFunc> {
    let target = surface.clone();
    let ctx = target.context();
    let handler_for_off = handler.clone();
    let name_owned = event_name.to_string();
    let name_for_off = name_owned.clone();
    <JSSurface as EmitterExt>::add_event_listener(
        This(target.clone()),
        EventKey::String(name_owned),
        handler,
        false,
        false,
    )?;
    JSFunc::new(&ctx, move || {
        <JSSurface as EmitterExt>::remove_event_listener(
            This(target.clone()),
            EventKey::String(name_for_off.clone()),
            handler_for_off.clone(),
        )
    })
}

fn should_change_visible(surface: &JSObject, visible: bool) -> JSResult<bool> {
    let inner = surface.borrow::<JSSurface>()?;
    Ok(inner.alive.get() && inner.visible.get() != visible)
}

/// Push a visibility change through one surface object: if it represents a real
/// state transition, update the cached flag + JS-visible property on this side
/// AND the peer, then emit `show` / `hide` on both. Idempotent: a no-op state
/// transition is silent (no event, no extra property writes).
fn mark_visible(surface: &JSObject, visible: bool, source: &str) -> JSResult<()> {
    let (id, kind, peer, changed) = {
        let inner = surface.borrow::<JSSurface>()?;
        if !inner.alive.get() {
            return Ok(());
        }
        let changed = inner.visible.get() != visible;
        if changed {
            inner.visible.set(visible);
        }
        let peer = inner.peer.borrow().clone();
        (inner.id.clone(), inner.kind.clone(), peer, changed)
    };
    if !changed {
        return Ok(());
    }
    surface.set("visible", visible)?;
    emit_visibility(surface, &id, &kind, visible, source)?;
    if let Some(peer_obj) = peer {
        let peer_changed = {
            let inner = peer_obj.borrow::<JSSurface>()?;
            // Peer should already be in sync with us via this same call from
            // the originating side; guard anyway so a future native-triggered
            // path that only updates one side still leaves both consistent.
            let was = inner.visible.get();
            if was != visible {
                inner.visible.set(visible);
            }
            was != visible
        };
        if peer_changed {
            peer_obj.set("visible", visible)?;
        }
        // Always emit on the peer when self transitioned, even if peer's flag
        // was already in sync — observers on the peer should see the event in
        // lockstep with observers on self.
        emit_visibility(&peer_obj, &id, &kind, visible, source)?;
    }
    Ok(())
}

fn emit_visibility(
    surface: &JSObject,
    id: &str,
    kind: &str,
    visible: bool,
    source: &str,
) -> JSResult<()> {
    let ctx = surface.context();
    let detail = JSSurfaceVisibility {
        id: id.to_string(),
        kind: kind.to_string(),
        source: source.to_string(),
    };
    let value = JSValue::from_rust(&ctx, detail);
    let event_name = if visible { "show" } else { "hide" };
    <JSSurface as EmitterExt>::do_emit(
        This(surface.clone()),
        EventKey::String(event_name.to_string()),
        Rest(vec![value]),
    )?;
    Ok(())
}

fn emit_close(surface: &JSObject, event: &JSSurfaceClosed) -> JSResult<()> {
    // Mark closed: alive→false, visible→false. This pair of writes is what
    // lets `surface.alive` / `surface.visible` remain a reliable source of
    // truth for declarative consumers across the close transition.
    {
        let inner = surface.borrow::<JSSurface>()?;
        inner.alive.set(false);
        inner.visible.set(false);
    }
    let _ = surface.set("alive", false);
    let _ = surface.set("visible", false);
    let ctx = surface.context();
    let value = JSValue::from_rust(&ctx, event.clone());
    <JSSurface as EmitterExt>::do_emit(
        This(surface.clone()),
        EventKey::String("close".to_string()),
        Rest(vec![value]),
    )?;
    Ok(())
}

pub(crate) fn notify_surface_closed(id: &str, reason: &str) -> bool {
    let id = id.trim();
    if id.is_empty() {
        return false;
    }
    let Some(tx) = SURFACE_CLOSED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(id))
    else {
        return false;
    };

    let _ = tx.sender.send(JSSurfaceClosed {
        id: id.to_string(),
        kind: tx.kind,
        reason: normalize_close_reason(id, Some(reason)),
    });
    true
}

fn register_closed_sender(id: String, kind: String, sender: oneshot::Sender<JSSurfaceClosed>) {
    if let Ok(mut guard) = SURFACE_CLOSED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        guard.insert(id, ClosedRegistration { kind, sender });
    }
}

fn unregister_closed_sender(id: &str) {
    if let Ok(mut guard) = SURFACE_CLOSED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        guard.remove(id);
    }
}

fn parse_surface_options(lxapp: &LxApp, options: &JSValue) -> JSResult<PageSurfaceRequest> {
    let Some(obj) = options.clone().into_object() else {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            "lx.surface.open expects an options object",
        ));
    };

    let target = parse_surface_target(lxapp, &obj)?;
    let query = parse_query(&obj)?;
    let kind = parse_surface_kind(&obj)?;
    let position = parse_position(&obj, kind)?;
    let (width, height, width_ratio, height_ratio) = parse_size(&obj, kind)?;

    Ok(PageSurfaceRequest {
        id: String::new(),
        target,
        query,
        kind,
        width,
        height,
        width_ratio,
        height_ratio,
        position,
    })
}

fn parse_surface_target(lxapp: &LxApp, obj: &JSObject) -> JSResult<PageSurfaceTarget> {
    let page = read_optional_string(obj, "page")?;
    let path = read_optional_string(obj, "path")?;
    let url = read_optional_string(obj, "url")?;

    match (page, path, url) {
        (Some(page), None, None) => {
            let page = page.trim();
            if page.is_empty() || is_http_url(page) {
                return Err(invalid_surface_target(
                    "page must be a non-empty local page name",
                ));
            }
            Ok(PageSurfaceTarget::Page(PageTarget::Name(page.to_string())))
        }
        (None, Some(path), None) => {
            let path = path.trim();
            if path.is_empty() || is_http_url(path) {
                return Err(invalid_surface_target(
                    "path must be a non-empty local page path",
                ));
            }
            Ok(PageSurfaceTarget::Page(PageTarget::Path(path.to_string())))
        }
        (None, None, Some(url)) => {
            if get_property(obj, "query").is_some() {
                return Err(invalid_surface_target(
                    "query is not supported when opening a url surface",
                ));
            }
            let url = validate_url_target(lxapp, &url)?;
            Ok(PageSurfaceTarget::Url(url))
        }
        _ => Err(invalid_surface_target(
            "pass exactly one of page, path, or url",
        )),
    }
}

fn parse_query(obj: &JSObject) -> JSResult<Option<PageQueryInput>> {
    let Some(query) = get_property(obj, "query") else {
        return Ok(None);
    };
    let Some(query_obj) = query.into_object() else {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            "query must be an object",
        ));
    };

    let json: Value = serde_json::from_str(&query_obj.to_json_string()?).map_err(|err| {
        surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            format!("query must be JSON serializable: {err}"),
        )
    })?;
    let Some(map) = json.as_object() else {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            "query must be an object",
        ));
    };

    let mut params = BTreeMap::new();
    for (key, value) in map {
        if value.is_null() {
            continue;
        }
        let value = match value {
            Value::String(value) => value.clone(),
            other => other.to_string(),
        };
        params.insert(key.clone(), value);
    }

    Ok(Some(PageQueryInput::Params(params)))
}

fn parse_surface_kind(obj: &JSObject) -> JSResult<SurfaceKind> {
    let raw = get_property(obj, "kind")
        .ok_or_else(|| {
            surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                "lx.surface.open requires kind",
            )
        })?
        .to_rust::<String>()
        .map_err(|_| {
            surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                "kind must be a string",
            )
        })?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "overlay" => Ok(SurfaceKind::Overlay),
        "window" => Ok(SurfaceKind::Window),
        _ => Err(surface_error(
            rong::error::E_INVALID_ARG,
            "unsupported_surface_kind",
            format!("unsupported surface kind: {raw}; supported kinds are overlay and window"),
        )),
    }
}

fn parse_position(obj: &JSObject, kind: SurfaceKind) -> JSResult<SurfacePosition> {
    let Some(value) = get_property(obj, "position") else {
        return Ok(SurfacePosition::Center);
    };
    if kind == SurfaceKind::Window {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            "position is only supported for overlay surfaces",
        ));
    }
    let raw = value.to_rust::<String>().map_err(|_| {
        surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            "position must be a string",
        )
    })?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "center" => Ok(SurfacePosition::Center),
        "bottom" => Ok(SurfacePosition::Bottom),
        "left" => Ok(SurfacePosition::Left),
        "right" => Ok(SurfacePosition::Right),
        "top" => Ok(SurfacePosition::Top),
        _ => Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            format!("unsupported position: {raw}"),
        )),
    }
}

#[allow(clippy::type_complexity)]
fn parse_size(
    obj: &JSObject,
    kind: SurfaceKind,
) -> JSResult<(Option<f64>, Option<f64>, Option<f64>, Option<f64>)> {
    let Some(size) = get_property(obj, "size") else {
        return Ok((None, None, None, None));
    };
    let Some(size_obj) = size.into_object() else {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_options",
            "size must be an object",
        ));
    };
    let allow_percentage = kind == SurfaceKind::Overlay;
    let (width, width_ratio) = parse_size_value(&size_obj, "width", allow_percentage)?;
    let (height, height_ratio) = parse_size_value(&size_obj, "height", allow_percentage)?;
    Ok((width, height, width_ratio, height_ratio))
}

fn parse_size_value(
    obj: &JSObject,
    field: &str,
    allow_percentage: bool,
) -> JSResult<(Option<f64>, Option<f64>)> {
    let Some(value) = get_property(obj, field) else {
        return Ok((None, None));
    };
    if value.is_number() {
        let number = value.to_rust::<f64>().map_err(|_| {
            surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                format!("size.{field} must be a positive number or percentage"),
            )
        })?;
        if !number.is_finite() || number <= 0.0 {
            return Err(surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                format!("size.{field} must be positive"),
            ));
        }
        return Ok((Some(number), None));
    }
    if value.is_string() {
        let raw = value.to_rust::<String>().map_err(|_| {
            surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                format!("size.{field} must be a positive number or percentage"),
            )
        })?;
        if !allow_percentage {
            return Err(surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                format!("size.{field} percentage is only supported for overlay surfaces"),
            ));
        }
        let Some(percent) = raw.trim().strip_suffix('%') else {
            return Err(surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                format!("size.{field} string must be a percentage"),
            ));
        };
        let ratio = percent.trim().parse::<f64>().map_err(|_| {
            surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                format!("size.{field} percentage is invalid"),
            )
        })? / 100.0;
        if !ratio.is_finite() || ratio <= 0.0 || ratio > 1.0 {
            return Err(surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_options",
                format!("size.{field} percentage must be > 0% and <= 100%"),
            ));
        }
        return Ok((None, Some(ratio)));
    }

    Err(surface_error(
        rong::error::E_INVALID_ARG,
        "invalid_surface_options",
        format!("size.{field} must be a positive number or percentage"),
    ))
}

fn read_optional_string(obj: &JSObject, field: &str) -> JSResult<Option<String>> {
    let Some(value) = get_property(obj, field) else {
        return Ok(None);
    };
    if !value.is_string() {
        return Err(invalid_surface_target(format!("{field} must be a string")));
    }
    value
        .to_rust::<String>()
        .map(Some)
        .map_err(|_| invalid_surface_target(format!("{field} must be a string")))
}

fn invalid_surface_target(detail: impl AsRef<str>) -> rong::RongJSError {
    surface_error(
        rong::error::E_INVALID_ARG,
        "invalid_surface_target",
        detail.as_ref(),
    )
}

fn validate_url_target(lxapp: &LxApp, raw: &str) -> JSResult<String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err(invalid_surface_target("url must be non-empty"));
    }
    let Some((scheme, host)) = split_url_scheme_host(url) else {
        return Err(invalid_surface_target(
            "url must be an absolute http(s) URL",
        ));
    };
    if scheme != "https" && scheme != "http" {
        return Err(invalid_surface_target("url must use http(s)"));
    }
    if !lxapp.is_domain_allowed(host) {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "security_denied",
            format!("domain '{host}' is not allowed by lxapp security policy"),
        ));
    }
    Ok(url.to_string())
}

fn split_url_scheme_host(url: &str) -> Option<(String, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let host_port = rest.split(['/', '?', '#']).next()?.trim();
    if host_port.is_empty() {
        return None;
    }
    let host = if let Some(host) = host_port
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']').map(|(host, _)| host))
    {
        host
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if host.is_empty() {
        None
    } else {
        Some((scheme.to_ascii_lowercase(), host.trim_end_matches('.')))
    }
}

fn is_http_url(value: &str) -> bool {
    value
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        || value
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
}

fn get_property(obj: &JSObject, field: &str) -> Option<JSValue> {
    obj.get::<_, JSValue>(field)
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
}

fn normalize_close_reason(surface_id: &str, reason: Option<&str>) -> String {
    match reason.map(str::trim).filter(|value| !value.is_empty()) {
        Some("user") => "user".to_string(),
        Some("programmatic") => "programmatic".to_string(),
        Some("owner_closed") => "owner_closed".to_string(),
        Some("app_closed") => "app_closed".to_string(),
        Some("reclaimed") => "reclaimed".to_string(),
        Some("failed") | Some("surface_failed") => "failed".to_string(),
        Some("unknown") => "unknown".to_string(),
        Some(raw) => {
            log::warn!(
                "unknown surface close reason: surface_id={}, reason={}",
                surface_id,
                raw
            );
            "unknown".to_string()
        }
        None => "unknown".to_string(),
    }
}

fn surface_kind_label(kind: SurfaceKind) -> &'static str {
    match kind {
        SurfaceKind::Overlay => "overlay",
        SurfaceKind::Window => "window",
    }
}

fn surface_error(
    host_code: &'static str,
    surface_code: &'static str,
    detail: impl std::fmt::Display,
) -> rong::RongJSError {
    HostError::new(host_code, detail.to_string())
        .with_data(rong::err_data!({ code: (surface_code) }))
        .into()
}
