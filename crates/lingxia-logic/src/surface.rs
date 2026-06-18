use futures::channel::oneshot;
use lingxia_platform::traits::ui::{SurfaceKind, SurfacePosition};
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lxapp::{
    LxApp, PageQueryInput, PageSurfaceRequest, PageSurfaceTarget, PageTarget, list_lxapps, lx,
    publish_app_event, register_app_handler, try_get, unregister_app_handler,
};
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
    lx::register_js_api(ctx, "openSurface", JSFunc::new(ctx, open_surface_spec)?)?;
    lx::register_js_api(ctx, "openExternal", JSFunc::new(ctx, open_external)?)?;
    lx::register_js_api(ctx, "onSurfaceContext", JSFunc::new(ctx, surface_on_change)?)?;
    Ok(())
}

/// Event name on the per-app bus carrying `{ sizeClass, bottomOwner }`.
const SURFACE_CONTEXT_EVENT: &str = "SurfaceContextChange";

/// `lx.onSurfaceContext(handler)` — register a JS callback (scoped to this
/// lxapp's JS context) invoked with `{ sizeClass, bottomOwner }` whenever the
/// window's adaptive context flips. Returns an unsubscribe fn.
fn surface_on_change(ctx: JSContext, handler: JSFunc) -> JSResult<JSFunc> {
    register_app_handler(&ctx, SURFACE_CONTEXT_EVENT, handler.clone())?;
    let off_ctx = ctx.clone();
    let off_handler = handler;
    JSFunc::new(&ctx, move || {
        unregister_app_handler(&off_ctx, SURFACE_CONTEXT_EVENT, Some(off_handler.clone()));
    })
}

/// lxapp-side observer handler (registered at runtime init): a window's adaptive
/// context flipped, so push the new context to every active lxapp's
/// `onChange` subscribers via the per-app event bus (same dispatch as
/// onNetworkChange). The surface graph is window-global today, so all lxapps
/// share this window's derived context; each is recomputed from its own LxApp.
pub(crate) fn notify_surface_context_changed(_window_id: &str) {
    for info in list_lxapps() {
        let Some(lxapp) = try_get(&info.appid) else {
            continue;
        };
        let context = surface_context_for(&lxapp);
        let payload = serde_json::json!({
            "sizeClass": context.size_class,
            "bottomOwner": context.bottom_owner,
        })
        .to_string();
        publish_app_event(&info.appid, SURFACE_CONTEXT_EVENT, Some(payload));
    }
}

#[derive(Debug, Clone, IntoJSObj)]
struct PageSurfaceOptions {
    path: String,
    kind: String,
    position: String,
    role: String,
}

#[derive(Debug, Clone, IntoJSObj)]
struct WebSurfaceOptions {
    url: String,
    kind: String,
    position: String,
    role: String,
}

/// `lx.openSurface(spec)` — unified surface entry point. The spec is a
/// discriminated union keyed by exactly one of `page`, `surface`, or `url`:
///
/// - `{ page, as, edge?, position?, size?, query? }` opens one of this lxapp's
///   own pages as an `aside` (docked beside the main), a `float` (overlay
///   popup), or a `window` (bare standalone desktop window).
/// - `{ surface, edge?, query? }` shows a host-declared surface by its `ui` id.
/// - `{ url }` opens an http(s)/lingxia url in the in-app chromed browser.
async fn open_surface_spec(ctx: JSContext, spec: JSValue) -> JSResult<JSValue> {
    let Some(obj) = spec.clone().into_object() else {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            "lx.openSurface expects a spec object",
        ));
    };
    let has_page = get_property(&obj, "page").is_some();
    let has_surface = get_property(&obj, "surface").is_some();
    let has_url = get_property(&obj, "url").is_some();
    match (has_page, has_surface, has_url) {
        (true, false, false) => open_page_spec(ctx, &obj).await.map(JSObject::into_js_value),
        (false, true, false) => open_declared_surface_spec(&ctx, &obj).map(JSObject::into_js_value),
        (false, false, true) => open_url_spec(ctx, &obj).await,
        _ => Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            "spec must contain exactly one of page, surface, or url",
        )),
    }
}

/// `{ page, as, edge?, position?, size?, query? }` branch of `lx.openSurface`.
/// Resolves the page name to a path, maps `as` to the underlying open path
/// (overlay aside/float, or a standalone window on desktop), and returns the
/// surface handle.
async fn open_page_spec(ctx: JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let page = read_required_string(spec, "page")?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = lxapp.find_page_path_by_name(page.trim()).ok_or_else(|| {
        HostError::new(rong::error::E_NOT_FOUND, format!("unknown page: {page}"))
            .with_data(rong::err_data!({ code: ("page_not_found") }))
    })?;
    let path_value = JSValue::from_rust(&ctx, path);

    let as_role = read_required_string(spec, "as")?;
    let size = get_property(spec, "size");
    let query = get_property(spec, "query");
    let edge = read_optional_string(spec, "edge")?;
    let position = read_optional_string(spec, "position")?;

    let options = match as_role.trim() {
        "aside" => {
            let position = edge.unwrap_or_else(|| "right".to_string());
            build_open_options(&ctx, &path_value, "overlay", &position, "aside", size.as_ref())?
        }
        "float" => {
            let position = position
                .or(edge)
                .unwrap_or_else(|| "center".to_string());
            build_open_options(&ctx, &path_value, "overlay", &position, "float", size.as_ref())?
        }
        "window" => {
            #[cfg(any(target_os = "ios", target_os = "android", target_env = "ohos"))]
            {
                return Err(surface_error(
                    rong::error::E_NOT_SUPPORTED,
                    "window_unsupported_platform",
                    "as: 'window' opens a separate desktop window and is not available on this platform",
                ));
            }
            #[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
            {
                build_window_options(&ctx, &path_value, size.as_ref())?
            }
        }
        other => {
            return Err(surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_spec",
                format!("as must be one of aside, float, or window; got {other}"),
            ));
        }
    };
    if let Some(query) = query
        && let Some(opts) = options.clone().into_object()
    {
        opts.set("query", query)?;
    }
    open_surface(ctx, options).await
}

/// `{ surface, edge?, query? }` branch of `lx.openSurface`. Shows a
/// host-declared top-level surface by its `ui` id and returns a handle whose
/// `show`/`hide`/`close` drive the host shell's visibility. `edge` and `query`
/// are accepted for forward shape compatibility but the host shell positions a
/// declared surface itself, so they are not threaded through here.
fn open_declared_surface_spec(ctx: &JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let id = read_required_string(spec, "surface")?;
    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp
        .set_shell_surface_visible(id.trim(), true)
        .map_err(|err| {
            surface_error(rong::error::E_INTERNAL, "shell_surface_failed", err)
        })?;
    declared_surface_handle(ctx, lxapp, id.trim().to_string())
}

/// `{ url, as?, edge? }` branch of `lx.openSurface`. Without `as`, the url opens
/// as a full in-app browser tab in the main content (host-owned chrome, no
/// handle), in contrast to `lx.openExternal` which hands off to the OS browser.
/// With `as: 'aside'` the url is docked beside the main as a closable browser
/// surface with its own chrome (address bar + close), driven through the surface
/// graph exactly like a page aside.
async fn open_url_spec(ctx: JSContext, spec: &JSObject) -> JSResult<JSValue> {
    let url = read_required_string(spec, "url")?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    let url = lxapp_url(&lxapp, url.trim())?;

    match read_optional_string(spec, "as")?.as_deref().map(str::trim) {
        Some("aside") => {
            let position = read_optional_string(spec, "edge")?
                .unwrap_or_else(|| "right".to_string());
            let size = get_property(spec, "size");
            let options = JSValue::from_rust(
                &ctx,
                WebSurfaceOptions {
                    url,
                    kind: "overlay".to_string(),
                    position,
                    role: "aside".to_string(),
                },
            );
            attach_size(&options, size.as_ref())?;
            open_surface(ctx, options).await.map(JSObject::into_js_value)
        }
        None => {
            lxapp
                .runtime
                .open_url(OpenUrlRequest {
                    owner_appid: lxapp.appid.clone(),
                    owner_session_id: lxapp.session_id(),
                    url,
                    target: OpenUrlTarget::SelfTarget,
                })
                .map_err(|err| {
                    surface_error(rong::error::E_INTERNAL, "open_url_failed", err)
                })?;
            // The in-app browser tab is host chrome, not tracked as a closable
            // surface here, so there is no handle to return.
            Ok(JSValue::null(&ctx))
        }
        Some(other) => Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            format!(
                "a url surface supports as: 'aside' (or omit `as` for a browser tab); got {other}"
            ),
        )),
    }
}

/// `lx.openExternal(url)` — hand the url off to the OS default browser.
fn open_external(ctx: JSContext, url: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    if url.trim().is_empty() {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_url",
            "openExternal requires a url",
        ));
    }
    lxapp
        .runtime
        .open_url(OpenUrlRequest {
            owner_appid: lxapp.appid.clone(),
            owner_session_id: lxapp.session_id(),
            url: url.trim().to_string(),
            target: OpenUrlTarget::External,
        })
        .map_err(|err| surface_error(rong::error::E_INTERNAL, "open_url_failed", err))
}

/// Build a minimal handle bound to a surface id (host-declared or already-open).
/// `show`/`hide` drive the lxapp surface visibility APIs and `close` hides the
/// surface; messaging/lifecycle events are not wired for these ids.
fn declared_surface_handle(
    ctx: &JSContext,
    lxapp: Arc<LxApp>,
    id: String,
) -> JSResult<JSObject> {
    let handle = JSObject::new(ctx);
    handle.set("id", id.clone())?;

    let show_lxapp = lxapp.clone();
    let show_id = id.clone();
    handle.set(
        "show",
        JSFunc::new(ctx, move || {
            show_lxapp
                .set_shell_surface_visible(&show_id, true)
                .map_err(|err| surface_error(rong::error::E_INTERNAL, "shell_surface_failed", err))
        })?,
    )?;

    let hide_lxapp = lxapp.clone();
    let hide_id = id.clone();
    handle.set(
        "hide",
        JSFunc::new(ctx, move || {
            hide_lxapp
                .set_shell_surface_visible(&hide_id, false)
                .map_err(|err| surface_error(rong::error::E_INTERNAL, "shell_surface_failed", err))
        })?,
    )?;

    let close_lxapp = lxapp;
    let close_id = id;
    handle.set(
        "close",
        JSFunc::new(ctx, move || {
            close_lxapp
                .set_shell_surface_visible(&close_id, false)
                .map_err(|err| surface_error(rong::error::E_INTERNAL, "shell_surface_failed", err))
        })?,
    )?;

    Ok(handle)
}

/// Validate a `{ url }` target: an http(s) URL must satisfy the lxapp's domain
/// policy; a `lingxia://` URL is an in-app scheme and is passed through.
fn lxapp_url(lxapp: &LxApp, raw: &str) -> JSResult<String> {
    if raw.is_empty() {
        return Err(invalid_surface_target("url must be non-empty"));
    }
    if raw
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("lingxia://"))
    {
        return Ok(raw.to_string());
    }
    validate_url_target(lxapp, raw)
}

fn read_required_string(obj: &JSObject, field: &str) -> JSResult<String> {
    read_optional_string(obj, field)?.ok_or_else(|| {
        surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            format!("{field} must be a string"),
        )
    })
}

/// Attach an optional `{ width?, height? }` size hint to a built options object.
/// It is a preferred size, not a mandate: the Host may clamp or override it (an
/// aside stays user-resizable; on a compact window it is ignored). `parse_size`
/// validates the shape downstream.
fn attach_size(options: &JSValue, size: Option<&JSValue>) -> JSResult<()> {
    if let Some(size) = size
        && let Some(obj) = options.clone().into_object()
    {
        obj.set("size", size.clone())?;
    }
    Ok(())
}

/// Translate a `target` page path of this lxapp + role-derived kind/position
/// into the underlying open options. `target` is a path to one of this lxapp's
/// own pages; an aside/float only ever hosts the app's own content. External web
/// is rejected here and belongs in the in-app chromed browser via a `{ url }`
/// spec.
fn build_open_options(
    ctx: &JSContext,
    target: &JSValue,
    kind: &str,
    position: &str,
    role: &str,
    size: Option<&JSValue>,
) -> JSResult<JSValue> {
    if target.is_string() {
        let path = target
            .clone()
            .to_rust::<String>()
            .map_err(|_| invalid_surface_target("target string must be a page path"))?;
        let options = JSValue::from_rust(
            ctx,
            PageSurfaceOptions {
                path,
                kind: kind.to_string(),
                position: position.to_string(),
                role: role.to_string(),
            },
        );
        attach_size(&options, size)?;
        return Ok(options);
    }
    if let Some(obj) = target.clone().into_object()
        && (read_optional_string(&obj, "url")?.is_some()
            || read_optional_string(&obj, "browser")?.is_some())
    {
        return Err(invalid_surface_target(
            "a page surface hosts this lxapp's own pages; open external web with a { url } spec",
        ));
    }
    Err(invalid_surface_target(
        "target must be a page path of this lxapp",
    ))
}

#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
#[derive(Debug, Clone, IntoJSObj)]
struct PageWindowOptions {
    path: String,
    kind: String,
}

/// Build the open options for a standalone window surface. `target` is a path to
/// one of this lxapp's own pages — a window only ever hosts the app's own
/// content. A window carries no `position`/`role` (parse rejects a position on a
/// window kind; the role is always `Main`). External web is rejected here: a
/// chromeless window showing attacker-controllable web content is a spoofing
/// vector, and the in-app chromed browser (`{ url }`) covers external sites with
/// a proper address-bar chrome.
#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
fn build_window_options(
    ctx: &JSContext,
    target: &JSValue,
    size: Option<&JSValue>,
) -> JSResult<JSValue> {
    if target.is_string() {
        let path = target
            .clone()
            .to_rust::<String>()
            .map_err(|_| invalid_surface_target("target string must be a page path"))?;
        let options = JSValue::from_rust(
            ctx,
            PageWindowOptions {
                path,
                kind: "window".to_string(),
            },
        );
        attach_size(&options, size)?;
        return Ok(options);
    }
    if let Some(obj) = target.clone().into_object()
        && (read_optional_string(&obj, "browser")?.is_some()
            || read_optional_string(&obj, "url")?.is_some())
    {
        return Err(invalid_surface_target(
            "a window surface hosts this lxapp's own pages; open external web with a { url } spec",
        ));
    }
    Err(invalid_surface_target(
        "a window surface target must be a page path of this lxapp",
    ))
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSSurfaceContext {
    #[rename = "sizeClass"]
    size_class: String,
    #[rename = "bottomOwner"]
    bottom_owner: String,
}

/// Adaptive context (`{ sizeClass, bottomOwner }`) derived from an lxapp's
/// window layout, reported by the `onSurfaceContext` dispatch.
fn surface_context_for(lxapp: &LxApp) -> JSSurfaceContext {
    use lingxia_surface::{BottomOwner, SizeClass};
    let layout = lxapp.surface_derived_layout();
    let size_class = match layout.as_ref().map(|l| l.size_class) {
        Some(SizeClass::Compact) => "compact",
        Some(SizeClass::Medium) => "medium",
        _ => "expanded",
    };
    let bottom_owner = match layout.as_ref().map(|l| l.bottom_owner) {
        Some(BottomOwner::Host) => "host",
        _ => "app",
    };
    JSSurfaceContext {
        size_class: size_class.to_string(),
        bottom_owner: bottom_owner.to_string(),
    }
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
    // Windows: the platform presents the surface's page-instance webview before
    // it mounts, so it never received a visibility transition. Now that the
    // page is ready, mark it visible so it fires onShow and is not reclaimed by
    // the page-instance dispose timer (which would close the surface). Other
    // platforms drive this from their native presenter.
    #[cfg(target_os = "windows")]
    if let Some(page_instance_id) = opened_surface.page_instance_id.as_deref() {
        let _ =
            lxapp::notify_page_instance_by_id(page_instance_id, lxapp::PageInstanceEvent::Visible);
    }
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
            "surface options must be an object",
        ));
    };

    let target = parse_surface_target(lxapp, &obj)?;
    let query = parse_query(&obj)?;
    let kind = parse_surface_kind(&obj)?;
    let position = parse_position(&obj, kind)?;
    let (width, height, width_ratio, height_ratio) = parse_size(&obj, kind)?;
    // Resolve the authoritative core role. A window is always the top-level
    // main; for an overlay, `role: "aside"` docks (splits the main); any other
    // overlay is a float popup.
    let role = match kind {
        SurfaceKind::Window => lingxia_surface::Role::Main,
        SurfaceKind::Overlay => match read_optional_string(&obj, "role")?.as_deref() {
            Some("aside") => lingxia_surface::Role::Aside,
            _ => lingxia_surface::Role::Float,
        },
    };

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
        role,
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
                "surface options require kind",
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
