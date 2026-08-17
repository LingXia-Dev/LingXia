use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
};
use lingxia_platform::traits::app_runtime::{
    AppRuntime, BuiltinBrowserPage, OpenUrlRequest, OpenUrlTarget,
};
use lingxia_platform::traits::ui::WindowChrome;
use lingxia_platform::traits::ui::{SurfaceKind, SurfacePosition};
use lxapp::{
    LxApp, LxAppError, PageQueryInput, PageSurfaceRequest, PageSurfaceTarget, PageTarget,
    app_handler_unsub, publish_app_event, register_app_handler, try_get,
    unregister_app_handler_token,
};
use rong::{
    Class, HostError, IntoJSObject, JSContext, JSContextService, JSFunc, JSObject, JSResult,
    JSValue, Promise,
    function::{Optional, Rest, This},
    js_class, js_method,
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
    sender: oneshot::Sender<JSSurfaceClosed>,
}

static SURFACE_CLOSED: OnceLock<Mutex<HashMap<String, Vec<ClosedRegistration>>>> = OnceLock::new();
struct VisibilityRegistration {
    sender: mpsc::UnboundedSender<bool>,
    // Suppress delayed native echoes that would otherwise replay an older
    // visibility state after a newer opener-driven transition.
    last_visible: bool,
}

static SURFACE_VISIBILITY: OnceLock<Mutex<HashMap<String, Vec<VisibilityRegistration>>>> =
    OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ManagedHandleKey {
    app_id: String,
    session_id: u64,
    surface_id: String,
}

thread_local! {
    static MANAGED_HANDLE_CACHE: RefCell<HashMap<ManagedHandleKey, JSObject>> =
        RefCell::new(HashMap::new());
}

#[derive(Clone)]
struct ManagedHandleCacheScope {
    app_id: String,
    session_id: u64,
}

impl JSContextService for ManagedHandleCacheScope {
    fn on_shutdown(&self) {
        MANAGED_HANDLE_CACHE.with(|cache| {
            remove_managed_handles_for_session(
                &mut cache.borrow_mut(),
                &self.app_id,
                self.session_id,
            );
        });
    }
}

fn remove_managed_handles_for_session<T>(
    cache: &mut HashMap<ManagedHandleKey, T>,
    app_id: &str,
    session_id: u64,
) {
    cache.retain(|key, _| key.app_id != app_id || key.session_id != session_id);
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSSurfaceClosed {
    id: String,
    reason: String,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSSurfaceVisibility {
    id: String,
    /// Which side initiated the visibility change. "opener" when the caller
    /// holds the opener-side surface, "page" when the page-side surface drove
    /// it. Lets analytics / logging distinguish without having to wire extra
    /// state through the caller.
    source: String,
}

#[js_class(clone)]
struct JSSurface {
    id: String,
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

    #[js_method(rename = "close", ts_return = "Promise<void>")]
    fn close(&self, ctx: JSContext) -> JSResult<Promise> {
        let lxapp = LxApp::from_ctx(&ctx)?;
        let id = self.id.clone();
        Promise::from_future(&ctx, None, async move {
            lxapp
                .close_surface(&id, "programmatic")
                .map_err(|err| surface_error(SurfaceErrorCode::Failed, err))?;
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
    register_surface_api(ctx)?;
    register_surface_namespace(ctx)?;
    register_shell_surface_api(ctx)
}

rong::js_api! {
    fn register_surface_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn openExternal = open_external;
        const surface: "SurfaceApi" = surface_namespace(ctx)?;
    }
}

rong::js_api! {
    fn register_surface_namespace(ctx) {
        namespace SurfaceApi = surface_namespace(ctx)?;
        fn openPage(
            ts_params = "page: string, options?: OpenPageOptions",
            ts_return = "Promise<PageSurface>"
        ) = open_page;
        fn openUrl(
            ts_params = "url: string, options?: OpenUrlOptions",
            ts_return = "Promise<TabSurface>"
        ) = open_url;
        fn openDeclared(
            ts_params = "id: string",
            ts_return = "Promise<DeclaredSurface>"
        ) = open_declared;
        fn get(
            ts_params = "keyOrId: string",
            ts_return = "AnySurface | undefined"
        ) = get_surface;
        fn onContext(
            ts_params = "handler: (context: SurfaceContext) => void",
            ts_return = "() => void"
        ) = surface_on_change;
    }
}

rong::js_api! {
    fn register_shell_surface_api(ctx) {
        namespace ShellApi = shell_namespace(ctx)?;
        fn openApp(
            ts_params = "appId: string, options: ShellOpenAppOptions",
            ts_return = "Promise<AppSurface>"
        ) = shell_open_app;
        fn openBuiltin(
            ts_params = "page: BuiltinShellPage",
            ts_return = "Promise<BuiltinSurface>"
        ) = shell_open_builtin;
        fn openDeclared(
            ts_params = "id: string, options?: ShellOpenDeclaredOptions",
            ts_return = "Promise<DeclaredSurface>"
        ) = shell_open_declared;
        fn reconfigure(
            ts_params = "id: string, patch: ShellSurfacePatch",
            ts_return = "Promise<void>"
        ) = shell_reconfigure;
    }
}

/// `lx.surface`, created on demand so registration order does not matter.
fn surface_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("surface") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("surface", obj.clone())?;
            Ok(obj)
        }
    }
}

/// `lx.shell`, shared with `crate::ui::shell`.
fn shell_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("shell") {
        Ok(obj) => Ok(obj),
        Err(_) => {
            let obj = JSObject::new(ctx);
            lx.set("shell", obj.clone())?;
            Ok(obj)
        }
    }
}

/// Event name on the per-app bus carrying `{ sizeClass, width, height }`.
const SURFACE_CONTEXT_EVENT: &str = "SurfaceContextChange";

/// `lx.surface.onContext(handler)` — register a JS callback (scoped to this
/// lxapp's JS context), invoke it immediately, then again whenever that
/// presentation's actual viewport changes. Returns an unsubscribe fn.
fn surface_on_change(ctx: JSContext, handler: JSFunc) -> JSResult<JSFunc> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let initial = surface_context_for(&lxapp);
    let token = register_app_handler(&ctx, SURFACE_CONTEXT_EVENT, handler.clone())?;
    let payload = JSValue::from_rust(&ctx, initial);
    if let Err(err) = handler.call::<_, ()>(None, (payload,)) {
        unregister_app_handler_token(&ctx, SURFACE_CONTEXT_EVENT, token);
        return Err(err);
    }
    let off = app_handler_unsub(&ctx, SURFACE_CONTEXT_EVENT, token);
    let unsubscribed = Cell::new(false);
    JSFunc::new(&ctx, move || {
        if unsubscribed.replace(true) {
            return;
        }
        off.unsubscribe();
    })
}

/// lxapp-side observer handler (registered at runtime init): a window's adaptive
/// context flipped, so push the new context to every active lxapp's
/// `onChange` subscribers via the per-app event bus (same dispatch as
/// onNetworkChange). The surface graph is window-global today, so all lxapps
/// share this window's derived context; each is recomputed from its own LxApp.
pub(crate) fn notify_surface_context_changed(appid: &str) {
    let Some(lxapp) = try_get(appid) else {
        return;
    };
    let context = surface_context_for(&lxapp);
    let payload = serde_json::json!({
        "sizeClass": context.size_class,
        "width": context.width,
        "height": context.height,
    })
    .to_string();
    publish_app_event(appid, SURFACE_CONTEXT_EVENT, Some(payload));
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct PageSurfaceOptions {
    path: String,
    kind: String,
    position: String,
    role: String,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct WebSurfaceOptions {
    url: String,
    kind: String,
    position: String,
    role: String,
}

/// `lx.surface.openPage(page, options?)` — one of this lxapp's own pages as a
/// float or a window. A page can never be an aside: asides carry external
/// content only, which is why that member does not exist on this signature.
async fn open_page(
    ctx: JSContext,
    page: String,
    options: Optional<JSObject>,
) -> JSResult<JSObject> {
    let options = options.0.unwrap_or_else(|| JSObject::new(&ctx));
    reject_unknown_options(
        &options,
        &[
            "as",
            "chrome",
            "position",
            "size",
            "interaction",
            "query",
            "key",
        ],
        "lx.surface.openPage",
    )?;
    let realized = resolve_placement(&ctx, &options, &["float", "window"], "float", |placement| {
        placement != "window" || window_placement_available()
    })?;

    let asked_one_placement = get_property(&options, "as")
        .and_then(|value| value.into_object().filter(|obj| obj.is_array()))
        .is_none();
    let chrome = read_optional_string(&options, "chrome")?;
    if let Some(chrome) = chrome.as_deref() {
        if !matches!(chrome, "system" | "full") {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("chrome must be 'system' or 'full'; got {chrome}"),
            ));
        }
        // A single `as` is a strict request, so a window-only option with a
        // float is a mistake worth reporting. An ordered preference is the
        // opposite: the caller asked the host to choose, so an option that
        // does not apply to the chosen placement is simply dropped — otherwise
        // "prefer a full-chrome window, else a float" could never be written.
        if realized != "window" && asked_one_placement {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                "chrome applies to as: 'window' only",
            ));
        }
    }

    let spec = JSObject::new(&ctx);
    spec.set("page", page)?;
    spec.set("as", realized.as_str())?;
    let mut carried = vec!["size", "interaction", "query"];
    // `position` anchors a float and is meaningless on a window; carry it only
    // where it applies, for the same reason `chrome` is dropped above.
    if realized == "float" {
        carried.push("position");
    } else if asked_one_placement && get_property(&options, "position").is_some() {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "position applies to as: 'float' only",
        ));
    }
    copy_options(&options, &spec, &carried)?;
    if realized == "window"
        && let Some(chrome) = chrome
    {
        spec.set("chrome", chrome)?;
    }

    let key = read_surface_key(&options)?;
    let handle = open_page_spec(ctx.clone(), &spec).await?;
    finish_handle(&ctx, &handle, "page", &realized, key, None)
}

/// `lx.surface.openUrl(url, options?)` — external content in the in-app
/// browser, as a tab or docked as an aside.
async fn open_url(ctx: JSContext, url: String, options: Optional<JSObject>) -> JSResult<JSObject> {
    let options = options.0.unwrap_or_else(|| JSObject::new(&ctx));
    reject_unknown_options(
        &options,
        &["as", "edge", "size", "key"],
        "lx.surface.openUrl",
    )?;
    let _lxapp = LxApp::from_ctx(&ctx)?;
    // Every placement the browser offers is realizable: a compact layout has no
    // dock, but it still projects an aside through the in-app browser's own
    // chrome. Refusing it here would take URL asides away from every phone —
    // `scope` is how that platform difference is reported, not a rejection.
    let realized = resolve_placement(&ctx, &options, &["tab", "aside"], "tab", |_| true)?;

    let spec = JSObject::new(&ctx);
    spec.set("url", url)?;
    if realized == "aside" {
        spec.set("as", "aside")?;
    }
    copy_options(&options, &spec, &["edge", "size"])?;

    let key = read_surface_key(&options)?;
    let opened = open_url_spec(ctx.clone(), &spec).await?;
    browser_tab_handle(&ctx, opened, &realized, key)
}

/// `lx.surface.openDeclared(id, options?)` — a surface the host declared in
/// `lingxia.yaml`, opened with the declaration's own presentation.
async fn open_declared(
    ctx: JSContext,
    id: String,
    options: Optional<JSObject>,
) -> JSResult<JSObject> {
    // Every override this could carry — `key`, `as`, `edge` — mutates shared
    // shell composition, so it lives on `lx.shell.openDeclared` behind the home
    // privilege. Taking one silently would open a surface nobody asked for.
    if let Some(options) = options.0.as_ref()
        && !options.keys_as::<String>()?.is_empty()
    {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "lx.surface.openDeclared takes no options; use lx.shell.openDeclared to set key, as, or edge",
        ));
    }
    let spec = JSObject::new(&ctx);
    spec.set("surface", id)?;
    let handle = open_declared_surface_spec(&ctx, &spec).await?;
    let realized = handle_realized_placement(&handle);
    finish_handle(&ctx, &handle, "declared", &realized, None, None)
}

/// `lx.surface.get(keyOrId)` — the live handle for a surface this lxapp opened
/// **with a `key`**, so no caller has to cache one in order to reuse or close
/// it. An unkeyed surface is not addressable: nothing registers it, because
/// holding one for the session costs its closures and its message port and
/// nobody can look up a uuid they never chose.
///
/// A `key` you chose wins over a runtime-assigned `id`, so a key that happens
/// to spell another surface's id still finds yours.
fn get_surface(ctx: JSContext, key_or_id: String) -> JSResult<JSValue> {
    let registry = surface_registry(&ctx)?;
    // Drop anything already closed, so a dead handle is never handed back and
    // the registry cannot grow across a session of opens and closes.
    let mut by_id = None;
    let mut by_key = None;
    for entry_key in registry.keys_as::<String>()? {
        let Ok(handle) = registry.get::<_, JSObject>(entry_key.as_str()) else {
            continue;
        };
        if !handle.get::<_, bool>("alive").unwrap_or(false) {
            let _ = registry.delete(entry_key.as_str());
            continue;
        }
        if entry_key == key_or_id {
            by_key = Some(handle);
        } else if handle
            .get::<_, String>("id")
            .is_ok_and(|id| id == key_or_id)
        {
            by_id = Some(handle);
        }
    }
    // A caller-chosen key wins over a runtime-assigned id. Both can name a
    // surface and nothing stops one lxapp's key from spelling another
    // surface's id, so the tie has to resolve the same way every time — and
    // the name the caller chose is the one they meant.
    let found = by_key.or(by_id);
    Ok(found.map_or_else(|| JSValue::undefined(&ctx), JSObject::into_js_value))
}

/// `lx.shell.openApp(appId, options)` — compose another lxapp into a shell
/// slot. Home-lxapp only; the namespace is the privilege.
async fn shell_open_app(ctx: JSContext, app_id: String, options: JSObject) -> JSResult<JSObject> {
    reject_unknown_options(
        &options,
        &[
            "as",
            "edge",
            "page",
            "query",
            "envVersion",
            "targetVersion",
            "key",
        ],
        "lx.shell.openApp",
    )?;
    let spec = JSObject::new(&ctx);
    spec.set("appId", app_id)?;
    copy_options(
        &options,
        &spec,
        &["as", "edge", "page", "query", "envVersion", "targetVersion"],
    )?;
    let key = read_surface_key(&options)?;
    let handle = open_app_spec(ctx.clone(), &spec).await?;
    let realized = handle_realized_placement(&handle);
    finish_handle(&ctx, &handle, "app", &realized, key, None)
}

/// `lx.shell.openBuiltin(page)` — a host builtin page. Home-lxapp only.
async fn shell_open_builtin(ctx: JSContext, page: String) -> JSResult<JSObject> {
    let url = match page.trim() {
        "settings" => "lingxia://settings",
        "downloads" => "lingxia://downloads",
        other => {
            return Err(surface_error(
                SurfaceErrorCode::NotDeclared,
                format!("unknown builtin page: {other}"),
            ));
        }
    };
    let spec = JSObject::new(&ctx);
    spec.set("url", url)?;
    open_url_spec(ctx.clone(), &spec).await?;
    builtin_surface_handle(&ctx, page.trim())
}

/// `lx.shell.openDeclared(id, options?)` — the declared surface, plus the
/// keyed multi-instance form and placement overrides. Home-lxapp only.
async fn shell_open_declared(
    ctx: JSContext,
    id: String,
    options: Optional<JSObject>,
) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    require_home_caller(&lxapp, "lx.shell.openDeclared")?;
    let options = options.0.unwrap_or_else(|| JSObject::new(&ctx));
    reject_unknown_options(&options, &["key", "as", "edge"], "lx.shell.openDeclared")?;
    let key = read_surface_key(&options)?;
    let spec = JSObject::new(&ctx);
    spec.set("surface", id)?;
    if let Some(key) = key.as_deref() {
        spec.set("key", key)?;
    }
    // Overriding the declared placement is the privilege; opening first and
    // reconfiguring after would present the wrong role for a frame.
    copy_options(&options, &spec, &["as", "edge"])?;
    let handle = open_declared_surface_spec(&ctx, &spec).await?;
    let realized = handle_realized_placement(&handle);
    finish_handle(&ctx, &handle, "declared", &realized, key, None)
}

/// `lx.shell.reconfigure(id, patch)` — re-place a live declared surface.
async fn shell_reconfigure(ctx: JSContext, id: String, patch: JSObject) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    require_home_caller(&lxapp, "lx.shell.reconfigure")?;
    reject_unknown_options(&patch, &["as", "edge"], "lx.shell.reconfigure")?;
    let spec = JSObject::new(&ctx);
    spec.set("surface", id)?;
    copy_options(&patch, &spec, &["as", "edge"])?;
    let handle = open_declared_surface_spec(&ctx, &spec).await?;
    // `openDeclared` hands back a cached object and stamps `realized` once, so
    // without this the handle the caller still holds reports the placement it
    // had before this call.
    let realized = handle_realized_placement(&handle);
    handle.set("realized", realized)?;
    Ok(())
}

/// Resolves an `as` option against what the host can realize. A single value
/// is a strict requirement; an ordered array is a preference list.
fn resolve_placement(
    ctx: &JSContext,
    options: &JSObject,
    allowed: &[&str],
    default: &str,
    available: impl Fn(&str) -> bool,
) -> JSResult<String> {
    let requested: Vec<String> = match get_property(options, "as") {
        None => vec![default.to_string()],
        Some(value) => {
            if let Some(list) = value.clone().into_object().filter(|obj| obj.is_array()) {
                let length = list.get::<_, u32>("length").unwrap_or(0);
                (0..length)
                    .map(|index| {
                        list.get::<_, String>(index.to_string().as_str())
                            .map_err(|_| {
                                surface_error(
                                    SurfaceErrorCode::InvalidArg,
                                    "every entry of as must be a placement name",
                                )
                            })
                    })
                    .collect::<JSResult<Vec<_>>>()?
            } else {
                vec![value.to_rust::<String>().map_err(|_| {
                    surface_error(
                        SurfaceErrorCode::InvalidArg,
                        "as must be a placement or an ordered array of placements",
                    )
                })?]
            }
        }
    };
    let _ = ctx;
    if requested.is_empty() {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "as must name at least one placement",
        ));
    }
    for placement in &requested {
        let placement = placement.trim();
        if !allowed.contains(&placement) {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!(
                    "as must be one of {}; got {placement}",
                    allowed
                        .iter()
                        .map(|value| format!("'{value}'"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ),
            ));
        }
        if available(placement) {
            return Ok(placement.to_string());
        }
    }
    Err(surface_error(
        SurfaceErrorCode::UnsupportedPlacement,
        format!(
            "this host build cannot realize {}; check lx.supports({{ capability: 'surface', value: … }}) first",
            requested.join(" or ")
        ),
    ))
}

fn copy_options(from: &JSObject, to: &JSObject, fields: &[&str]) -> JSResult<()> {
    for field in fields {
        if let Some(value) = get_property(from, field) {
            to.set(*field, value)?;
        }
    }
    Ok(())
}

/// Rejects an option this signature does not accept, naming it. Silently
/// ignoring a key is how a misspelling — or an option that belongs to a
/// different content source — turns into a surface nobody asked for.
fn reject_unknown_options(options: &JSObject, allowed: &[&str], api: &str) -> JSResult<()> {
    for key in options.keys_as::<String>()? {
        if allowed.contains(&key.as_str()) {
            continue;
        }
        // A key that belongs to a different content source gets the same
        // guidance it would have got there, rather than a bare "unknown".
        let detail = match (api, key.as_str()) {
            (_, "path") => {
                "path is not supported; pass the configured page name in page".to_string()
            }
            ("lx.surface.openPage", "edge") => {
                "edge is not supported for page surfaces; use position with as: 'float'".to_string()
            }
            ("lx.surface.openPage", "appId") | ("lx.surface.openUrl", "appId") => {
                "composing another lxapp is a shell operation; use lx.shell.openApp".to_string()
            }
            _ => format!(
                "{api} has no option '{key}'; it accepts {}",
                allowed
                    .iter()
                    .map(|value| format!("'{value}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        return Err(surface_error(SurfaceErrorCode::InvalidArg, detail));
    }
    Ok(())
}

fn read_surface_key(options: &JSObject) -> JSResult<Option<String>> {
    let Some(key) = read_optional_string(options, "key")? else {
        return Ok(None);
    };
    let key = key.trim().to_string();
    if key.is_empty() || key.len() > 128 {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "key must be 1 to 128 UTF-8 bytes",
        ));
    }
    Ok(Some(key))
}

/// `key -> handle`, kept on a non-enumerable slot of `lx.surface` so the JS
/// engine keeps the handles alive for us.
fn surface_registry(ctx: &JSContext) -> JSResult<JSObject> {
    let namespace = surface_namespace(ctx)?;
    if let Ok(existing) = namespace.get::<_, JSObject>(SURFACE_REGISTRY_SLOT) {
        return Ok(existing);
    }
    let registry = JSObject::new(ctx);
    namespace.define_property(
        SURFACE_REGISTRY_SLOT,
        rong::PropertyDescriptor::from_value(registry.clone().into_js_value()),
    )?;
    Ok(registry)
}

const SURFACE_REGISTRY_SLOT: &str = "__lxSurfaces";

/// Stamps the content-keyed fields onto a handle and records it for
/// `lx.surface.get`.
fn finish_handle(
    ctx: &JSContext,
    handle: &JSObject,
    kind: &str,
    realized: &str,
    key: Option<String>,
    scope: Option<&str>,
) -> JSResult<JSObject> {
    handle.set("kind", kind)?;
    handle.set("realized", realized)?;
    if let Some(scope) = scope {
        handle.set("scope", scope)?;
    }
    // The page-side twin is the same surface seen from inside, and page code
    // narrows it on `kind`. Stamp it here or `this.surface.kind` reads
    // undefined for the one caller that cannot reach the opener's handle.
    // Clone the peer out first — never hold the class borrow across a set().
    let peer = handle
        .borrow::<JSSurface>()
        .ok()
        .and_then(|inner| inner.peer.borrow().clone());
    if let Some(peer) = peer {
        peer.set("kind", kind)?;
        peer.set("realized", realized)?;
        if let Some(key) = key.as_deref() {
            peer.set("key", key)?;
        }
    }
    // Only a keyed surface is registered. Registering every generated id kept
    // a strong reference — with its closures, message port, and emitter — for
    // the whole session, and nobody can look up a uuid they never chose.
    if let Some(key) = key {
        handle.set("key", key.as_str())?;
        surface_registry(ctx)?.set(key.as_str(), handle.clone())?;
    }
    Ok(handle.clone())
}

/// A `TabSurface` for content the in-app browser owns. Where the host hands
/// back a tab identity the handle owns exactly that tab; where compact browser
/// chrome owns the strip it reports `scope: 'group'` — a readable field rather
/// than the `null` the old shape returned on one platform and not another.
fn browser_tab_handle(
    ctx: &JSContext,
    opened: JSValue,
    realized: &str,
    key: Option<String>,
) -> JSResult<JSObject> {
    if let Some(handle) = opened.clone().into_object() {
        if let Ok(tab_id) = handle.get::<_, String>("tabId")
            && !tab_id.is_empty()
        {
            return owned_browser_tab_handle(ctx, tab_id, realized, key);
        }
        // A docked aside owns its surface, so `activate` is "bring it
        // forward" — the type promises the method on every TabSurface.
        let show_handle = handle.clone();
        handle.set(
            "activate",
            JSFunc::new(ctx, move |ctx: JSContext| {
                let show: JSResult<JSFunc> = show_handle.get("show");
                let target = show_handle.clone();
                Promise::from_future(&ctx, None, async move {
                    // `show` is itself async: awaiting its promise is what
                    // makes `await activate()` mean "it is forward now", and
                    // what turns a failed show into this call's rejection
                    // instead of an unhandled one.
                    let pending: Promise = show?.call(Some(target), ())?;
                    let _: JSValue = pending.into_future().await?;
                    Ok(())
                })
            })?,
        )?;
        return finish_handle(ctx, &handle, "tab", realized, key, Some("tab"));
    }

    // The browser owns the tab strip here, so the handle addresses the group.
    let handle = JSObject::new(ctx);
    let lxapp = LxApp::from_ctx(ctx)?;
    handle.set("id", format!("browser-group:{}", lxapp.appid))?;
    handle.set("alive", true)?;
    handle.set("visible", true)?;
    attach_browser_group_methods(ctx, &handle)?;
    finish_handle(ctx, &handle, "tab", realized, key, Some("group"))
}

/// Lifetime methods for a handle that does not own its content. `reason` names
/// the owner, so the rejection points at the field or the concept the caller
/// can actually branch on rather than at a member this shape may not carry.
fn attach_unowned_lifetime_methods(
    ctx: &JSContext,
    handle: &JSObject,
    methods: &[&str],
    reason: &'static str,
) -> JSResult<()> {
    for owned_elsewhere in methods {
        handle.set(
            *owned_elsewhere,
            JSFunc::new(ctx, move |ctx: JSContext| {
                Promise::from_future(&ctx, None, async move {
                    Err::<(), _>(surface_error(
                        SurfaceErrorCode::UnsupportedPlacement,
                        reason,
                    ))
                })
            })?,
        )?;
    }
    handle.set(
        "onClose",
        JSFunc::new(ctx, |ctx: JSContext, _handler: JSFunc| {
            // The owner does not publish a close event, so there is nothing to
            // subscribe to; the unsubscribe fn keeps the signature honest.
            JSFunc::new(&ctx, || {})
        })?,
    )?;
    Ok(())
}

fn attach_browser_group_methods(ctx: &JSContext, handle: &JSObject) -> JSResult<()> {
    attach_unowned_lifetime_methods(
        ctx,
        handle,
        &["close", "activate"],
        "the browser chrome owns this group; check `scope` before calling",
    )
}

/// A `TabSurface` that owns exactly the tab `open_url` named.
fn owned_browser_tab_handle(
    ctx: &JSContext,
    tab_id: String,
    realized: &str,
    key: Option<String>,
) -> JSResult<JSObject> {
    let handle = JSObject::new(ctx);
    handle.set("id", tab_id.as_str())?;
    handle.set("alive", true)?;
    handle.set("visible", true)?;
    let close_id = tab_id.clone();
    handle.set(
        "close",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = LxApp::from_ctx(&ctx)?;
            let tab_id = close_id.clone();
            Promise::from_future(&ctx, None, async move {
                lxapp
                    .runtime
                    .close_browser_tab(&tab_id)
                    .map_err(|err| surface_error(SurfaceErrorCode::Failed, err))?;
                Ok(())
            })
        })?,
    )?;
    let activate_id = tab_id.clone();
    handle.set(
        "activate",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = LxApp::from_ctx(&ctx)?;
            let tab_id = activate_id.clone();
            Promise::from_future(&ctx, None, async move {
                lxapp
                    .runtime
                    .activate_browser_tab(&tab_id)
                    .map_err(|err| surface_error(SurfaceErrorCode::Failed, err))?;
                Ok(())
            })
        })?,
    )?;
    handle.set(
        "onClose",
        JSFunc::new(ctx, |ctx: JSContext, _handler: JSFunc| {
            JSFunc::new(&ctx, || {})
        })?,
    )?;
    finish_handle(ctx, &handle, "tab", realized, key, Some("tab"))
}

/// A `BuiltinSurface` for a host product page. The shell owns its lifetime,
/// so the handle reports identity and leaves control to the shell.
fn builtin_surface_handle(ctx: &JSContext, page: &str) -> JSResult<JSObject> {
    let handle = JSObject::new(ctx);
    handle.set("id", format!("builtin:{page}"))?;
    handle.set("alive", true)?;
    handle.set("visible", true)?;
    attach_unowned_lifetime_methods(
        ctx,
        &handle,
        &["close"],
        "the shell owns this builtin page; it closes with the shell",
    )?;
    finish_handle(ctx, &handle, "builtin", "main", None, None)
}

/// Reads back the placement the host actually produced for a managed handle.
fn handle_realized_placement(handle: &JSObject) -> String {
    handle
        .get::<_, String>("role")
        .unwrap_or_else(|_| "aside".to_string())
}

/// Reject callers other than the home lxapp for the privileged content keys
/// (`appId`) — the same single-writer model as `lx.shell`. Gates on
/// the configured home appId (like `ensure_home_lxapp`), not the instance
/// flag, which a dev-mode reinstall can recreate without.
fn require_home_caller(lxapp: &LxApp, key: &str) -> JSResult<()> {
    if lingxia_app_context::home_app_id().is_some_and(|home| lxapp.appid == home) {
        return Ok(());
    }
    Err(surface_error(
        SurfaceErrorCode::Denied,
        format!("{key} is restricted to the home lxapp"),
    ))
}

/// Backs `lx.shell.openApp`. Opens another lxapp
/// by appId and optional configured page name. The target version is prepared
/// through the same update path as app navigation, then composed independently
/// from any YAML declaration. Dynamic composition supports main/aside only;
/// floats need a declaration-owned presentation contract.
async fn open_app_spec(ctx: JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let app_id = read_required_string(spec, "appId")?;
    let app_id = app_id.trim().to_string();
    if app_id.is_empty() {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "appId must be non-empty",
        ));
    }
    let edge = read_validated_edge(spec)?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    require_home_caller(&lxapp, "lx.shell.openApp")?;

    let as_role = read_required_string(spec, "as")?;
    let as_role = as_role.trim();
    if !matches!(as_role, "main" | "aside") {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            format!("an app surface supports as: 'main' | 'aside'; got {as_role}"),
        ));
    }
    if edge.is_some() && as_role != "aside" {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "edge is only valid with as: 'aside'",
        ));
    }

    let query = match get_property(spec, "query") {
        Some(value) => Some(value.into_object().ok_or_else(|| {
            surface_error(SurfaceErrorCode::InvalidArg, "query must be an object")
        })?),
        None => None,
    };
    let target = crate::navigator::NavigateToAppOptions {
        appid: app_id.clone(),
        path: read_optional_string(spec, "path")?,
        page: read_optional_string(spec, "page")?,
        query,
        env_version: read_optional_string(spec, "envVersion")?,
        target_version: read_optional_string(spec, "targetVersion")?,
    };
    let requested_region = match as_role {
        "main" => lxapp::LxAppOpenRegion::Main,
        "aside" => lxapp::LxAppOpenRegion::Aside,
        _ => unreachable!("validated above"),
    };
    if let Some(current_region) = lxapp::open_region(&app_id) {
        if current_region != requested_region {
            return Err(surface_error(
                SurfaceErrorCode::AlreadyOpenOtherRole,
                format!(
                    "lxapp '{app_id}' is already open as {}; close it before opening as {}",
                    current_region.as_str(),
                    requested_region.as_str(),
                ),
            ));
        }
        show_lxapp_region(&lxapp, &app_id, &app_id, current_region, edge.as_deref()).await?;
        return lxapp_surface_handle(&ctx, lxapp, app_id.clone(), app_id, current_region);
    }
    let (startup_options, release_type) =
        crate::navigator::prepare_app_open(&lxapp, &target).await?;

    let (region, shell_surface_id) = match as_role {
        "main" => {
            open_lxapp_region(
                &app_id,
                lxapp::LxAppOpenRegion::Main,
                &app_id,
                startup_options.clone(),
            )?;
            (lxapp::LxAppOpenRegion::Main, app_id.clone())
        }
        "aside" => {
            open_lxapp_region(
                &app_id,
                lxapp::LxAppOpenRegion::Aside,
                &app_id,
                startup_options.clone(),
            )?;
            lxapp.register_host_aside(&app_id, edge.as_deref().unwrap_or("right"));
            (lxapp::LxAppOpenRegion::Aside, app_id.clone())
        }
        _ => unreachable!("validated above"),
    };
    lxapp::schedule_lxapp_update_check(&app_id, release_type);
    lxapp_surface_handle(&ctx, lxapp, app_id, shell_surface_id, region)
}

fn open_lxapp_region(
    app_id: &str,
    region: lxapp::LxAppOpenRegion,
    shell_surface_id: &str,
    mut options: lxapp::LxAppStartupOptions,
) -> JSResult<()> {
    if region == lxapp::LxAppOpenRegion::Aside {
        options.open_mode = lingxia_platform::traits::app_runtime::LxAppOpenMode::Panel;
        options.panel_id = shell_surface_id.to_string();
    }
    lxapp::open_lxapp(app_id, options)
        .map(|_| ())
        .map_err(lxapp_open_error)
}

async fn show_lxapp_region(
    shell: &LxApp,
    app_id: &str,
    shell_surface_id: &str,
    region: lxapp::LxAppOpenRegion,
    edge: Option<&str>,
) -> JSResult<()> {
    match region {
        lxapp::LxAppOpenRegion::Main => {
            let app = lxapp::try_get(app_id).ok_or_else(|| {
                surface_error(
                    SurfaceErrorCode::NotDeclared,
                    format!("lxapp is not active: {app_id}"),
                )
            })?;
            app.activate_main();
            Ok(())
        }
        lxapp::LxAppOpenRegion::Aside => {
            if shell
                .set_shell_surface_visible(shell_surface_id, true, None, edge)
                .await
                .is_ok()
            {
                Ok(())
            } else {
                open_lxapp_region(
                    app_id,
                    region,
                    shell_surface_id,
                    lxapp::LxAppStartupOptions::new(""),
                )?;
                shell.register_host_aside(shell_surface_id, edge.unwrap_or("right"));
                Ok(())
            }
        }
    }
}

fn lxapp_open_error(err: LxAppError) -> rong::RongJSError {
    match err {
        LxAppError::SurfaceConflict(message) => {
            surface_error(SurfaceErrorCode::AlreadyOpenOtherRole, message)
        }
        other => surface_error(SurfaceErrorCode::NotDeclared, other.to_string()),
    }
}

/// Declaration-id form. Provider kinds stay behind the declaration boundary.
async fn open_declared_surface_spec(ctx: &JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let id = read_required_string(spec, "surface")?;
    let key = read_optional_string(spec, "key")?.map(|key| key.trim().to_string());
    if key.as_deref().is_some_and(str::is_empty) || key.as_ref().is_some_and(|key| key.len() > 128)
    {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "key must contain 1 to 128 UTF-8 bytes",
        ));
    }
    let edge = read_validated_edge(spec)?;
    let lxapp = LxApp::from_ctx(ctx)?;
    let id = id.trim();
    if id.is_empty() {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "surface must be non-empty",
        ));
    }
    let requested_role = read_optional_managed_role(spec)?;
    if key.is_none()
        && requested_role.is_some_and(|role| role != lingxia_surface::Role::Main)
        && lxapp.surface_switcher_snapshot().root_surface_id.as_deref() == Some(id)
    {
        return Err(surface_error(
            SurfaceErrorCode::AlreadyOpenOtherRole,
            "the stable root main surface cannot change role",
        ));
    }
    let role = requested_role.or_else(|| lxapp.shell_surface_role(id));
    if role.is_some_and(|role| role != lingxia_surface::Role::Aside) && edge.is_some() {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "edge is only valid for an aside surface",
        ));
    }
    let declared_app_id = declared_lxapp_app_id(&lxapp, id);
    if has_declared_surface_orchestration_override(key.as_deref(), requested_role, edge.as_deref())
    {
        // Every lxapp may consume a declaration exactly as the host authored
        // it. Instance creation and placement overrides mutate shared shell
        // composition, so they stay under the home lxapp's single-writer role.
        require_home_caller(&lxapp, "overriding a declared surface")?;
    }
    if key.is_some() && declared_app_id.is_some() {
        return Err(surface_error(
            SurfaceErrorCode::CapabilityMissing,
            "key is supported only for declared native surfaces",
        ));
    }
    if let Some(app_id) = declared_app_id {
        lxapp::prepare_lxapp_open(&app_id, lxapp::ReleaseType::Release)
            .await
            .map_err(|err| surface_error(SurfaceErrorCode::NotDeclared, err.to_string()))?;
    }
    if key.is_some() {
        let resolved = lxapp
            .open_shell_native_surface(id, key.as_deref(), requested_role, edge.as_deref())
            .await
            .map_err(|err| surface_lifecycle_error("open", err))?;
        return managed_surface_handle(ctx, lxapp, resolved.surface_id, Some(resolved.role));
    }
    lxapp
        .set_shell_surface_visible(id, true, role, edge.as_deref())
        .await
        .map_err(|err| surface_lifecycle_error("open", err))?;
    managed_surface_handle(ctx, lxapp, id.to_string(), role)
}

fn has_declared_surface_orchestration_override(
    key: Option<&str>,
    role: Option<lingxia_surface::Role>,
    edge: Option<&str>,
) -> bool {
    key.is_some() || role.is_some() || edge.is_some()
}

fn declared_lxapp_app_id(lxapp: &LxApp, surface_id: &str) -> Option<String> {
    if let Some(lingxia_surface::SurfaceContent::Lxapp { app_id, .. }) =
        lxapp.main_surface_content(surface_id)
    {
        return Some(app_id);
    }
    lingxia_app_context::app_config()?
        .panels
        .as_ref()?
        .items
        .iter()
        .find(|item| item.id == surface_id && item.content.kind.is_lxapp())
        .map(|item| item.content.app_id.clone())
}

fn read_optional_managed_role(spec: &JSObject) -> JSResult<Option<lingxia_surface::Role>> {
    match read_optional_string(spec, "as")?.as_deref().map(str::trim) {
        None => Ok(None),
        Some("main") => Ok(Some(lingxia_surface::Role::Main)),
        Some("aside") => Ok(Some(lingxia_surface::Role::Aside)),
        Some("float") => Ok(Some(lingxia_surface::Role::Float)),
        Some(other) => Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            format!("as must be main, aside, or float; got {other}"),
        )),
    }
}

fn read_validated_edge(spec: &JSObject) -> JSResult<Option<String>> {
    let edge = read_optional_string(spec, "edge")?;
    if let Some(edge) = edge.as_deref()
        && !matches!(edge.trim(), "left" | "right" | "top" | "bottom")
    {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            format!("edge must be left, right, top, or bottom; got {edge}"),
        ));
    }
    Ok(edge.map(|edge| edge.trim().to_string()))
}

/// Backs `lx.surface.openPage`.
/// Resolves the page name to a path, maps `as` to the underlying open path
/// (overlay aside/float, or a standalone window on desktop), and returns the
/// surface handle.
async fn open_page_spec(ctx: JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let page = read_required_string(spec, "page")?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    let path = lxapp.find_page_path_by_name(page.trim()).ok_or_else(|| {
        surface_error(
            SurfaceErrorCode::NotDeclared,
            format!("unknown page: {page}"),
        )
    })?;
    let path_value = JSValue::from_rust(&ctx, path);

    let as_role = read_required_string(spec, "as")?;
    let size = get_property(spec, "size");
    let query = get_property(spec, "query");
    let interaction = get_property(spec, "interaction");
    let edge = read_optional_string(spec, "edge")?;
    let position = read_optional_string(spec, "position")?;
    if edge.is_some() {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "edge is not supported for page surfaces; use position with as: 'float'",
        ));
    }

    let options = match as_role.trim() {
        "float" => {
            let position = position.unwrap_or_else(|| "center".to_string());
            build_open_options(
                &ctx,
                &path_value,
                "overlay",
                &position,
                "float",
                size.as_ref(),
            )?
        }
        "window" => {
            #[cfg(any(target_os = "ios", target_os = "android", target_env = "ohos"))]
            {
                return Err(surface_error(
                    SurfaceErrorCode::UnsupportedPlacement,
                    "as: 'window' opens a separate desktop window, which this host build cannot do; check lx.supports({ capability: 'surface', value: 'window' }) first",
                ));
            }
            #[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
            {
                if !window_placement_available() {
                    return Err(surface_error(
                        SurfaceErrorCode::UnsupportedPlacement,
                        "as: 'window' opens a separate desktop window, which this host build cannot do; check lx.supports({ capability: 'surface', value: 'window' }) first",
                    ));
                }
                build_window_options(&ctx, &path_value, size.as_ref())?
            }
        }
        other => {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!(
                    "as must be 'float' or 'window' (a page cannot be an aside — asides are external content only); got {other}"
                ),
            ));
        }
    };
    if let Some(chrome) = read_optional_string(spec, "chrome")?
        && let Some(opts) = options.clone().into_object()
    {
        opts.set("chrome", chrome)?;
    }
    if let Some(query) = query
        && let Some(opts) = options.clone().into_object()
    {
        opts.set("query", query)?;
    }
    if let Some(interaction) = interaction
        && let Some(opts) = options.clone().into_object()
    {
        opts.set("interaction", interaction)?;
    }
    open_surface(ctx, options).await
}

/// Backs `lx.surface.openUrl`. Without `as`, the url opens
/// as a full in-app browser tab in the main content (host-owned chrome, no
/// handle), in contrast to `lx.openExternal` which hands off to the OS browser.
/// With `as: 'aside'` the url is docked beside the main as a closable browser
/// tab strip on desktop. Compact hosts project the same request into the
/// full-screen in-app browser with aside chrome.
async fn open_url_spec(ctx: JSContext, spec: &JSObject) -> JSResult<JSValue> {
    let raw_url = read_required_string(spec, "url")?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    let trimmed_url = raw_url.trim_matches(|character: char| character.is_ascii_whitespace());

    if let Some(page) = parse_builtin_browser_page(trimmed_url) {
        validate_builtin_browser_surface_keys(spec)?;
        require_home_caller(&lxapp, "lx.shell.openBuiltin")?;
        if !lingxia_app_context::browser_enabled() {
            return Err(surface_error(
                SurfaceErrorCode::UnsupportedPlacement,
                "built-in browser pages require capabilities.browser",
            ));
        }
        lxapp
            .runtime
            .open_builtin_browser_page(page)
            .map_err(|err| match err {
                lingxia_platform::error::PlatformError::NotSupported(_) => {
                    surface_error(SurfaceErrorCode::UnsupportedPlacement, err)
                }
                _ => surface_error(SurfaceErrorCode::Failed, err),
            })?;
        return Ok(JSValue::null(&ctx));
    }
    if trimmed_url
        .get(.."lingxia:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("lingxia:"))
    {
        return Err(invalid_surface_target(
            "only lingxia://settings and lingxia://downloads are public built-in URLs",
        ));
    }

    match read_optional_string(spec, "as")?.as_deref().map(str::trim) {
        Some("aside") => {
            let position = read_validated_edge(spec)?.unwrap_or_else(|| "right".to_string());
            let _ = parse_size(spec, SurfaceKind::Overlay)?;
            if url_aside_uses_compact_browser(&lxapp) {
                return open_url_in_browser(&ctx, &lxapp, trimmed_url, true);
            }
            let url = validate_url_target(&lxapp, trimmed_url)?;
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
            open_surface(ctx, options)
                .await
                .map(JSObject::into_js_value)
        }
        None => open_url_in_browser(&ctx, &lxapp, trimmed_url, false),
        Some(other) => Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            format!(
                "a url surface supports as: 'aside' (or omit `as` for a browser tab); got {other}"
            ),
        )),
    }
}

fn parse_builtin_browser_page(url: &str) -> Option<BuiltinBrowserPage> {
    match url {
        "lingxia://settings" => Some(BuiltinBrowserPage::Settings),
        "lingxia://downloads" => Some(BuiltinBrowserPage::Downloads),
        _ => None,
    }
}

fn validate_builtin_browser_surface_keys(spec: &JSObject) -> JSResult<()> {
    let keys = spec.keys_as::<String>()?;
    if keys.len() == 1 && keys[0] == "url" {
        return Ok(());
    }
    Err(surface_error(
        SurfaceErrorCode::InvalidArg,
        "a built-in browser surface accepts only the url field",
    ))
}

/// `lx.openExternal(url)` — hand the url off to the OS default browser.
fn open_external(ctx: JSContext, url: String) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let url = validate_external_url(&lxapp, &url)?;
    lxapp
        .runtime
        .open_url(OpenUrlRequest {
            owner_appid: lxapp.appid.clone(),
            owner_session_id: lxapp.session_id(),
            url,
            target: OpenUrlTarget::External,
            want_tab_id: false,
        })
        .map_err(|err| surface_error(SurfaceErrorCode::Failed, err))?;
    Ok(())
}

/// Handle for a live lxapp presentation. Hide preserves the claimed region;
/// close tears the runtime down and releases it so a later open may choose a
/// different role.
fn lxapp_surface_handle(
    ctx: &JSContext,
    shell: Arc<LxApp>,
    app_id: String,
    shell_surface_id: String,
    region: lxapp::LxAppOpenRegion,
) -> JSResult<JSObject> {
    let session_id = lxapp::try_get(&app_id)
        .map(|app| app.session_id())
        .unwrap_or_default();
    let (message_port, _) = crate::message_port::pair(ctx)?;
    let handle = Class::lookup::<JSSurface>(ctx)?.instance(JSSurface {
        id: app_id.clone(),
        message_port,
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
        visible: Cell::new(true),
        alive: Cell::new(true),
    });
    handle.set("id", app_id.clone())?;
    handle.set(
        "role",
        match region {
            lxapp::LxAppOpenRegion::Main => "main",
            lxapp::LxAppOpenRegion::Aside => "aside",
        },
    )?;
    handle.set(
        "presentation",
        match region {
            lxapp::LxAppOpenRegion::Main => "main",
            lxapp::LxAppOpenRegion::Aside => shell
                .shell_surface_presentation(&shell_surface_id)
                .unwrap_or("dock"),
        },
    )?;
    handle.set("visible", true)?;
    handle.set("alive", true)?;

    let show_shell = shell.clone();
    let show_id = app_id.clone();
    let show_surface_id = shell_surface_id.clone();
    let show_session_id = session_id;
    let show_handle = handle.clone();
    handle.set(
        "show",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let shell = show_shell.clone();
            let id = show_id.clone();
            let surface_id = show_surface_id.clone();
            let handle = show_handle.clone();
            Promise::from_future(&ctx, None, async move {
                ensure_lxapp_surface_open(&handle, &id, region, show_session_id)?;
                show_lxapp_region(&shell, &id, &surface_id, region, None).await?;
                mark_visible(&handle, true, "opener")
            })
        })?,
    )?;

    let hide_shell = shell.clone();
    let hide_id = app_id.clone();
    let hide_surface_id = shell_surface_id.clone();
    let hide_session_id = session_id;
    let hide_handle = handle.clone();
    handle.set(
        "hide",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let shell = hide_shell.clone();
            let id = hide_id.clone();
            let surface_id = hide_surface_id.clone();
            let handle = hide_handle.clone();
            Promise::from_future(&ctx, None, async move {
                ensure_lxapp_surface_open(&handle, &id, region, hide_session_id)?;
                match region {
                    lxapp::LxAppOpenRegion::Main => Err(surface_error(
                        SurfaceErrorCode::UnsupportedPlacement,
                        "a main surface cannot be hidden; select another main or close it",
                    )),
                    lxapp::LxAppOpenRegion::Aside => {
                        hide_lxapp_aside(&shell, &id, &surface_id).await?;
                        mark_visible(&handle, false, "opener")
                    }
                }
            })
        })?,
    )?;

    let close_shell = shell;
    let close_id = app_id;
    let close_surface_id = shell_surface_id.clone();
    let close_session_id = session_id;
    let close_handle = handle.clone();
    handle.set(
        "close",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let shell = close_shell.clone();
            let id = close_id.clone();
            let surface_id = close_surface_id.clone();
            let handle = close_handle.clone();
            Promise::from_future(&ctx, None, async move {
                if !handle.borrow::<JSSurface>()?.alive.get() {
                    return Ok(());
                }
                if !lxapp_surface_session_is_current(&id, region, close_session_id) {
                    return emit_lxapp_handle_close(&handle, &id, "app_closed");
                }
                if region == lxapp::LxAppOpenRegion::Main
                    && shell.surface_switcher_snapshot().root_surface_id.as_deref()
                        == Some(surface_id.as_str())
                {
                    return Err(surface_error(
                        SurfaceErrorCode::UnsupportedPlacement,
                        "the stable root main surface cannot be closed",
                    ));
                }
                if region == lxapp::LxAppOpenRegion::Aside {
                    hide_lxapp_aside(&shell, &id, &surface_id).await?;
                }
                lxapp::close_lxapp(&id)
                    .map_err(|err| surface_error(SurfaceErrorCode::Failed, err.to_string()))?;
                // Closing the provider alone leaves a dead main/aside node in
                // the window graph. Remove that shell identity as part of the
                // same operation so the successor and every retained handle
                // converge before the promise resolves.
                let _ = shell.forget_surface_with_reason(&surface_id, "programmatic");
                emit_lxapp_handle_close(&handle, &id, "programmatic")?;
                let _ = notify_surface_closed(&surface_id, "programmatic");
                Ok(())
            })
        })?,
    )?;

    for (method, event) in [("onShow", "show"), ("onHide", "hide"), ("onClose", "close")] {
        let listen_handle = handle.clone();
        handle.set(
            method,
            JSFunc::new(ctx, move |handler: JSFunc| {
                add_event_listener_for(&listen_handle, event, handler)
            })?,
        )?;
    }

    attach_host_surface_lifecycle(ctx, &handle, shell_surface_id, true, None)?;

    Ok(handle)
}

fn ensure_lxapp_surface_open(
    handle: &JSObject,
    app_id: &str,
    region: lxapp::LxAppOpenRegion,
    session_id: u64,
) -> JSResult<()> {
    if !handle.borrow::<JSSurface>()?.alive.get() {
        return Err(closed_surface_error());
    }
    if !lxapp_surface_session_is_current(app_id, region, session_id) {
        emit_lxapp_handle_close(handle, app_id, "app_closed")?;
        return Err(closed_surface_error());
    }
    Ok(())
}

fn lxapp_surface_session_is_current(
    app_id: &str,
    region: lxapp::LxAppOpenRegion,
    session_id: u64,
) -> bool {
    lxapp_surface_identity_matches(
        region,
        session_id,
        lxapp::open_region(app_id),
        lxapp::try_get(app_id).map(|app| app.session_id()),
    )
}

fn lxapp_surface_identity_matches(
    expected_region: lxapp::LxAppOpenRegion,
    expected_session_id: u64,
    current_region: Option<lxapp::LxAppOpenRegion>,
    current_session_id: Option<u64>,
) -> bool {
    current_region == Some(expected_region) && current_session_id == Some(expected_session_id)
}

fn emit_lxapp_handle_close(handle: &JSObject, app_id: &str, reason: &str) -> JSResult<()> {
    emit_close(
        handle,
        &JSSurfaceClosed {
            id: app_id.to_string(),
            reason: reason.to_string(),
        },
    )
}

async fn hide_lxapp_aside(shell: &LxApp, app_id: &str, shell_surface_id: &str) -> JSResult<()> {
    if shell
        .set_shell_surface_visible(shell_surface_id, false, None, None)
        .await
        .is_ok()
    {
        return Ok(());
    }
    let app = lxapp::try_get(app_id).ok_or_else(|| {
        surface_error(
            SurfaceErrorCode::NotDeclared,
            format!("lxapp is not active: {app_id}"),
        )
    })?;
    app.runtime
        .hide_lxapp(app_id.to_string(), app.session_id())
        .map_err(|err| surface_error(SurfaceErrorCode::Failed, err.to_string()))?;
    shell.unregister_host_aside(shell_surface_id);
    Ok(())
}

/// Lifecycle-complete handle for a host-managed native surface. Native
/// capabilities without messaging omit only postMessage/onMessage.
fn managed_surface_handle(
    ctx: &JSContext,
    lxapp: Arc<LxApp>,
    id: String,
    role: Option<lingxia_surface::Role>,
) -> JSResult<JSObject> {
    if ctx.get_service::<ManagedHandleCacheScope>().is_none() {
        ctx.set_service(ManagedHandleCacheScope {
            app_id: lxapp.appid.clone(),
            session_id: lxapp.session_id(),
        });
    }
    let role = role.or_else(|| lxapp.shell_surface_role(&id));
    let is_main = role == Some(lingxia_surface::Role::Main);
    let kind = if is_main { "window" } else { "overlay" };
    let surface_role = match role {
        Some(lingxia_surface::Role::Main) => "main",
        Some(lingxia_surface::Role::Float) => "float",
        Some(lingxia_surface::Role::Aside) | None => "aside",
    };
    let presentation = lxapp.shell_surface_presentation(&id).unwrap_or(if is_main {
        "main"
    } else if role == Some(lingxia_surface::Role::Float) {
        "popover"
    } else {
        "dock"
    });
    let visible = lxapp.shell_surface_visible(&id).unwrap_or(true);
    let cache_key = ManagedHandleKey {
        app_id: lxapp.appid.clone(),
        session_id: lxapp.session_id(),
        surface_id: id.clone(),
    };
    if let Some(cached) = MANAGED_HANDLE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache.retain(|key, _| {
            key.app_id != cache_key.app_id || key.session_id == cache_key.session_id
        });
        cache.get(&cache_key).cloned()
    }) && cached
        .borrow::<JSSurface>()
        .ok()
        .is_some_and(|surface| surface.alive.get())
    {
        cached.set("role", surface_role)?;
        cached.set("presentation", presentation)?;
        mark_visible(&cached, visible, "shell")?;
        return Ok(cached);
    }
    let (message_port, _) = crate::message_port::pair(ctx)?;
    let handle = Class::lookup::<JSSurface>(ctx)?.instance(JSSurface {
        id: id.clone(),
        message_port,
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
        visible: Cell::new(visible),
        alive: Cell::new(true),
    });
    handle.set("id", id.clone())?;
    handle.set("kind", kind)?;
    handle.set("role", surface_role)?;
    handle.set("presentation", presentation)?;
    handle.set("visible", visible)?;
    handle.set("alive", true)?;

    let show_lxapp = lxapp.clone();
    let show_id = id.clone();
    let show_handle = handle.clone();
    handle.set(
        "show",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = show_lxapp.clone();
            let id = show_id.clone();
            let handle = show_handle.clone();
            Promise::from_future(&ctx, None, async move {
                ensure_surface_object_open(&handle)?;
                let role = lxapp.shell_surface_role(&id);
                lxapp
                    .set_shell_surface_visible(&id, true, role, None)
                    .await
                    .map_err(|err| surface_lifecycle_error("show", err))?;
                mark_visible(&handle, true, "opener")
            })
        })?,
    )?;

    let hide_lxapp = lxapp.clone();
    let hide_id = id.clone();
    let hide_handle = handle.clone();
    handle.set(
        "hide",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = hide_lxapp.clone();
            let id = hide_id.clone();
            let handle = hide_handle.clone();
            Promise::from_future(&ctx, None, async move {
                ensure_surface_object_open(&handle)?;
                let role = lxapp.shell_surface_role(&id);
                if role == Some(lingxia_surface::Role::Main) {
                    return Err(surface_error(
                        SurfaceErrorCode::UnsupportedPlacement,
                        "a main surface cannot be hidden; close it instead",
                    ));
                }
                lxapp
                    .set_shell_surface_visible(&id, false, role, None)
                    .await
                    .map_err(|err| surface_lifecycle_error("hide", err))?;
                mark_visible(&handle, false, "opener")
            })
        })?,
    )?;

    let close_lxapp = lxapp;
    let close_id = id.clone();
    let close_handle = handle.clone();
    let close_cache_key = cache_key.clone();
    handle.set(
        "close",
        JSFunc::new(ctx, move |ctx: JSContext| {
            let lxapp = close_lxapp.clone();
            let id = close_id.clone();
            let handle = close_handle.clone();
            let cache_key = close_cache_key.clone();
            Promise::from_future(&ctx, None, async move {
                if !handle.borrow::<JSSurface>()?.alive.get() {
                    return Ok(());
                }
                let role = lxapp.shell_surface_role(&id);
                lxapp
                    .close_shell_managed_surface(&id, role)
                    .await
                    .map_err(|err| surface_lifecycle_error("close", err))?;
                if handle.borrow::<JSSurface>()?.alive.get() {
                    emit_close(
                        &handle,
                        &JSSurfaceClosed {
                            id: id.clone(),
                            reason: normalize_close_reason(&id, Some("programmatic")),
                        },
                    )?;
                }
                MANAGED_HANDLE_CACHE.with(|cache| {
                    cache.borrow_mut().remove(&cache_key);
                });
                let _ = notify_surface_closed(&id, "programmatic");
                Ok(())
            })
        })?,
    )?;

    for (method, event) in [("onShow", "show"), ("onHide", "hide"), ("onClose", "close")] {
        let listen_handle = handle.clone();
        handle.set(
            method,
            JSFunc::new(ctx, move |handler: JSFunc| {
                add_event_listener_for(&listen_handle, event, handler)
            })?,
        )?;
    }

    attach_host_surface_lifecycle(ctx, &handle, id, visible, Some(cache_key.clone()))?;

    MANAGED_HANDLE_CACHE.with(|cache| {
        cache.borrow_mut().insert(cache_key, handle.clone());
    });

    Ok(handle)
}

/// Mirror authoritative host close/visibility publications into a retained JS
/// handle. App surfaces and declaration-backed surfaces share this lifecycle;
/// only managed declarations additionally evict their identity cache on close.
fn attach_host_surface_lifecycle(
    ctx: &JSContext,
    handle: &JSObject,
    surface_id: String,
    visible: bool,
    cache_key: Option<ManagedHandleKey>,
) -> JSResult<()> {
    let (closed_tx, closed_rx) = oneshot::channel::<JSSurfaceClosed>();
    register_closed_sender(surface_id.clone(), closed_tx);
    let handle_for_close = handle.clone();
    Promise::from_future(ctx, None, async move {
        if let Ok(event) = closed_rx.await
            && handle_for_close
                .borrow::<JSSurface>()
                .ok()
                .is_some_and(|surface| surface.alive.get())
        {
            let _ = emit_close(&handle_for_close, &event);
        }
        if let Some(cache_key) = cache_key {
            MANAGED_HANDLE_CACHE.with(|cache| {
                cache.borrow_mut().remove(&cache_key);
            });
        }
    })?;

    let (visibility_tx, mut visibility_rx) = mpsc::unbounded();
    register_visibility_sender(surface_id, visible, visibility_tx);
    let handle_for_visibility = handle.clone();
    Promise::from_future(ctx, None, async move {
        while let Some(visible) = visibility_rx.next().await {
            let _ = mark_visible(&handle_for_visibility, visible, "shell");
        }
    })?;

    Ok(())
}

/// Validate a `{ url }` target for the in-app browser surface policy.
fn lxapp_url(lxapp: &LxApp, raw: &str) -> JSResult<String> {
    validate_url_target(lxapp, raw)
}

fn read_required_string(obj: &JSObject, field: &str) -> JSResult<String> {
    read_optional_string(obj, field)?.ok_or_else(|| {
        surface_error(
            SurfaceErrorCode::InvalidArg,
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
#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
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

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSSurfaceContext {
    #[js_name = "sizeClass"]
    size_class: String,
    width: f64,
    height: f64,
}

/// Adaptive context derived from this lxapp presentation's actual viewport.
fn surface_context_for(lxapp: &LxApp) -> JSSurfaceContext {
    use lingxia_surface::SizeClass;
    let (width, height, viewport_class) = lxapp.surface_viewport().unwrap_or_else(|| {
        let layout = lxapp.surface_derived_layout();
        let width = layout.as_ref().map(|_| 0.0).unwrap_or(0.0);
        let size_class = layout
            .as_ref()
            .map(|layout| layout.size_class)
            .unwrap_or(SizeClass::Compact);
        (width, 0.0, size_class)
    });
    let size_class = match viewport_class {
        SizeClass::Compact => "compact",
        SizeClass::Medium => "medium",
        SizeClass::Expanded => "expanded",
    };
    JSSurfaceContext {
        size_class: size_class.to_string(),
        width,
        height,
    }
}

/// Whether `as: 'window'` can open a separate top-level window in this host
/// build. A property of the build and of which device the runner simulates —
/// never of how wide the user has dragged the current window.
pub(crate) fn window_placement_available() -> bool {
    #[cfg(any(target_os = "ios", target_os = "android", target_env = "ohos"))]
    {
        false
    }
    #[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
    {
        !simulating_handheld_device()
    }
}

/// Whether `chrome: 'full'` can keep the system window controls while the page
/// runs to the edge. Desktop skins own the caption; mobile has no window.
pub(crate) fn window_full_chrome_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

/// A runner framed as a phone or tablet has no room for a second top-level
/// window, and refusing one is what the simulation is for. A desktop preset,
/// or a host with no device controller at all, is a real desktop.
#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
fn simulating_handheld_device() -> bool {
    matches!(
        lxapp::device::device_get(),
        Ok(state) if state.group == "phone" || state.group == "tablet"
    )
}

/// Whether a docked aside region exists right now. Live: a desktop window
/// dragged below the compact breakpoint loses its dock.
pub(crate) fn aside_dock_available(lxapp: &LxApp) -> bool {
    !url_aside_uses_compact_browser(lxapp)
}

/// Compact has no dock region. A URL aside therefore uses the native in-app
/// browser's aside projection instead of the generic URL-surface presenter.
/// Treat an unavailable layout as compact: mobile hosts can receive an open
/// before their first measured viewport, and showing desktop surface chrome in
/// that interval is the more disruptive fallback.
fn url_aside_uses_compact_browser(lxapp: &LxApp) -> bool {
    use lingxia_surface::SizeClass;
    !matches!(
        lxapp
            .surface_derived_layout()
            .as_ref()
            .map(|layout| layout.size_class),
        Some(SizeClass::Medium) | Some(SizeClass::Expanded)
    )
}

/// Open a URL as an in-app browser tab; `aside` selects compact aside chrome.
/// Returns `{ tabId }` when the host named the tab, otherwise null so the
/// handle reports `scope: 'group'`.
fn open_url_in_browser(
    ctx: &JSContext,
    lxapp: &LxApp,
    raw_url: &str,
    aside: bool,
) -> JSResult<JSValue> {
    let url = lxapp_url(lxapp, raw_url)?;
    let opened = lxapp
        .runtime
        .open_url(OpenUrlRequest {
            owner_appid: lxapp.appid.clone(),
            owner_session_id: lxapp.session_id(),
            url,
            target: if aside {
                OpenUrlTarget::AsideBrowser
            } else {
                OpenUrlTarget::SelfTarget
            },
            want_tab_id: true,
        })
        .map_err(|err| surface_error(SurfaceErrorCode::Failed, err))?;
    if let Some(tab_id) = opened.tab_id.filter(|id| !id.is_empty()) {
        let named = JSObject::new(ctx);
        named.set("tabId", tab_id)?;
        return Ok(named.into_js_value());
    }
    Ok(JSValue::null(ctx))
}

async fn open_surface(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let (mut request, chrome) = parse_surface_options(&lxapp, &options)?;
    request.id = format!("surface-{}", Uuid::new_v4().simple());
    let requested_surface_id = request.id.clone();

    let (closed_tx, closed_rx) = oneshot::channel::<JSSurfaceClosed>();
    register_closed_sender(requested_surface_id.clone(), closed_tx);
    let opened_surface = lxapp
        .open_surface_with_chrome(request, chrome)
        .map_err(|err| {
            unregister_closed_sender(&requested_surface_id);
            match err {
                LxAppError::UnsupportedOperation(detail) => {
                    surface_error(SurfaceErrorCode::UnsupportedPlacement, detail)
                }
                other => surface_error(SurfaceErrorCode::Failed, other),
            }
        })?;
    let surface_id = opened_surface.id.clone();
    if surface_id != requested_surface_id {
        move_closed_senders(&requested_surface_id, &surface_id);
    }
    let surface_id_for_closed = surface_id.clone();
    if let Some(page_instance_id) = opened_surface.page_instance_id.as_deref() {
        let page_path = opened_surface.page_path.clone().or_else(|| {
            lxapp
                .get_page_by_instance_id_str(page_instance_id)
                .map(|page| page.path())
        });
        // An isolated page's setup waits for the service created below, so a
        // path we cannot resolve has to fail the open here. Skipping would
        // hand back a surface whose page never mounts.
        let Some(page_path) = page_path else {
            unregister_closed_sender(&surface_id);
            let _ = lxapp.close_surface(&surface_id, "failed");
            return Err(surface_error(
                SurfaceErrorCode::Failed,
                format!("surface page instance has no page path: {page_instance_id}"),
            ));
        };
        lxapp
            .prepare_isolated_page_svc(&ctx, &page_path, page_instance_id)
            .await
            .map_err(|err| {
                unregister_closed_sender(&surface_id);
                let _ = lxapp.close_surface(&surface_id, "failed");
                surface_error(SurfaceErrorCode::Failed, err)
            })?;
    }
    // Windows: the platform presents the surface's page-instance webview before
    // it mounts, so it never receives a visibility transition of its own. Try
    // here for overlays that carry a dispose timer — a page still "hidden"
    // while its webview is being created can be reclaimed mid-wait. This is a
    // no-op until Mounted (`visible before mounted is not allowed`); the
    // presenter retries, and the notify after wait_webview_ready below is the
    // deterministic one. Other platforms drive this from their native presenter.
    #[cfg(target_os = "windows")]
    if let Some(page_instance_id) = opened_surface.page_instance_id.as_deref() {
        let _ =
            lxapp::notify_page_instance_by_id(page_instance_id, lxapp::PageInstanceEvent::Visible);
    }
    let page_svc = match opened_surface.page_instance_id.as_deref() {
        Some(page_instance_id) => Some(
            lxapp
                .get_page_in_ctx_by_instance_id(&ctx, page_instance_id)
                .await
                .map_err(|err| {
                    unregister_closed_sender(&surface_id);
                    let _ = lxapp.close_surface(&surface_id, "failed");
                    surface_error(SurfaceErrorCode::Failed, err)
                })?,
        ),
        None => None,
    };
    // The page is mounted now. A Visible that raced ahead of Mounted is
    // ignored; this one is what fires onShow and cancels the dispose timer.
    #[cfg(target_os = "windows")]
    if let Some(page_instance_id) = opened_surface.page_instance_id.as_deref() {
        let _ =
            lxapp::notify_page_instance_by_id(page_instance_id, lxapp::PageInstanceEvent::Visible);
    }
    let (opener_port, page_port) = crate::message_port::pair(&ctx)?;
    let surface = Class::lookup::<JSSurface>(&ctx)?.instance(JSSurface {
        id: opened_surface.id.clone(),
        message_port: opener_port,
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
        visible: Cell::new(true),
        alive: Cell::new(true),
    });
    surface.set("id", opened_surface.id.clone())?;
    surface.set("role", surface_role_label(opened_surface.role))?;
    surface.set("presentation", opened_surface.presentation.clone())?;
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
        let page_surface = Class::lookup::<JSSurface>(&ctx)?.instance(JSSurface {
            id: surface_id.clone(),
            message_port: page_port,
            event_emitter: EventEmitter::default(),
            peer: RefCell::new(None),
            visible: Cell::new(true),
            alive: Cell::new(true),
        });
        page_surface.set("id", surface_id.clone())?;
        page_surface.set("role", surface_role_label(opened_surface.role))?;
        page_surface.set("presentation", opened_surface.presentation.clone())?;
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
            surface_error(SurfaceErrorCode::Failed, err)
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
                lxapp
                    .close_surface(&id, "programmatic")
                    .map_err(|err| surface_error(SurfaceErrorCode::Failed, err))?;
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
                ensure_surface_object_open(&self_obj)?;
                lxapp
                    .show_surface(&id)
                    .map_err(|err| surface_lifecycle_error("show", err))?;
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
                ensure_surface_object_open(&self_obj)?;
                lxapp
                    .hide_surface(&id)
                    .map_err(|err| surface_lifecycle_error("hide", err))?;
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
            if !surface.alive.get() {
                return Err(closed_surface_error());
            }
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

fn ensure_surface_object_open(surface: &JSObject) -> JSResult<()> {
    let inner = surface.borrow::<JSSurface>()?;
    if !inner.alive.get() {
        return Err(closed_surface_error());
    }
    Ok(())
}

fn closed_surface_error() -> rong::RongJSError {
    surface_error(SurfaceErrorCode::Closed, "surface handle is closed")
}

fn surface_lifecycle_error(operation: &str, error: LxAppError) -> rong::RongJSError {
    match error {
        LxAppError::InvalidParameter(detail) => surface_error(SurfaceErrorCode::InvalidArg, detail),
        LxAppError::ResourceNotFound(detail) => {
            surface_error(SurfaceErrorCode::NotDeclared, detail)
        }
        LxAppError::UnsupportedOperation(detail) => {
            surface_error(SurfaceErrorCode::UnsupportedPlacement, detail)
        }
        LxAppError::SurfaceConflict(detail) => {
            surface_error(SurfaceErrorCode::AlreadyOpenOtherRole, detail)
        }
        other => surface_error(
            SurfaceErrorCode::Failed,
            format!("surface {operation} failed: {other}"),
        ),
    }
}

/// Push a visibility change through one surface object: if it represents a real
/// state transition, update the cached flag + JS-visible property on this side
/// AND the peer, then emit `show` / `hide` on both. Idempotent: a no-op state
/// transition is silent (no event, no extra property writes).
fn mark_visible(surface: &JSObject, visible: bool, source: &str) -> JSResult<()> {
    let (id, peer, changed) = {
        let inner = surface.borrow::<JSSurface>()?;
        if !inner.alive.get() {
            return Ok(());
        }
        let changed = inner.visible.get() != visible;
        if changed {
            inner.visible.set(visible);
        }
        let peer = inner.peer.borrow().clone();
        (inner.id.clone(), peer, changed)
    };
    if !changed {
        return Ok(());
    }
    surface.set("visible", visible)?;
    record_surface_visibility(&id, visible);
    emit_visibility(surface, &id, visible, source)?;
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
        emit_visibility(&peer_obj, &id, visible, source)?;
    }
    Ok(())
}

fn emit_visibility(surface: &JSObject, id: &str, visible: bool, source: &str) -> JSResult<()> {
    let ctx = surface.context();
    let detail = JSSurfaceVisibility {
        id: id.to_string(),
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
    if let Ok(mut visibility) = SURFACE_VISIBILITY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        visibility.remove(id);
    }
    let Some(registrations) = SURFACE_CLOSED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(id))
    else {
        return false;
    };

    let reason = normalize_close_reason(id, Some(reason));
    for registration in registrations {
        let _ = registration.sender.send(JSSurfaceClosed {
            id: id.to_string(),
            reason: reason.clone(),
        });
    }
    true
}

pub(crate) fn notify_active_main_changed(previous: Option<&str>, current: Option<&str>) -> bool {
    let mut notified = false;
    if let Some(previous) = previous
        && Some(previous) != current
    {
        notified |= notify_surface_visibility(previous, false);
    }
    if let Some(current) = current {
        notified |= notify_surface_visibility(current, true);
    }
    notified
}

pub(crate) fn notify_surface_visibility(id: &str, visible: bool) -> bool {
    let Ok(mut registrations) = SURFACE_VISIBILITY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    else {
        return false;
    };
    let Some(senders) = registrations.get_mut(id) else {
        return false;
    };
    let mut notified = false;
    senders.retain_mut(|registration| {
        if registration.sender.is_closed() {
            return false;
        }
        if registration.last_visible == visible {
            return true;
        }
        if registration.sender.unbounded_send(visible).is_err() {
            return false;
        }
        registration.last_visible = visible;
        notified = true;
        true
    });
    if senders.is_empty() {
        registrations.remove(id);
        return false;
    }
    notified
}

fn register_closed_sender(id: String, sender: oneshot::Sender<JSSurfaceClosed>) {
    if let Ok(mut guard) = SURFACE_CLOSED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        let registrations = guard.entry(id).or_default();
        registrations.retain(|registration| !registration.sender.is_canceled());
        registrations.push(ClosedRegistration { sender });
    }
}

fn register_visibility_sender(id: String, visible: bool, sender: mpsc::UnboundedSender<bool>) {
    if let Ok(mut guard) = SURFACE_VISIBILITY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        let registrations = guard.entry(id).or_default();
        registrations.retain(|registration| !registration.sender.is_closed());
        registrations.push(VisibilityRegistration {
            sender,
            last_visible: visible,
        });
    }
}

fn record_surface_visibility(id: &str, visible: bool) {
    if let Ok(mut guard) = SURFACE_VISIBILITY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        && let Some(registrations) = guard.get_mut(id)
    {
        for registration in registrations {
            registration.last_visible = visible;
        }
    }
}

fn move_closed_senders(from: &str, to: &str) {
    if from == to {
        return;
    }
    if let Ok(mut guard) = SURFACE_CLOSED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        && let Some(mut registrations) = guard.remove(from)
    {
        guard
            .entry(to.to_string())
            .or_default()
            .append(&mut registrations);
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

fn parse_surface_options(
    lxapp: &LxApp,
    options: &JSValue,
) -> JSResult<(PageSurfaceRequest, WindowChrome)> {
    let Some(obj) = options.clone().into_object() else {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "surface options must be an object",
        ));
    };

    let target = parse_surface_target(lxapp, &obj)?;
    let query = parse_query(&obj)?;
    let kind = parse_surface_kind(&obj)?;
    let position = parse_position(&obj, kind)?;
    let (width, height, width_ratio, height_ratio) = parse_size(&obj, kind)?;
    let interaction = parse_surface_interaction(&obj, kind)?;
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

    let chrome = parse_window_chrome(&obj)?;
    Ok((
        PageSurfaceRequest::new(String::new(), target)
            .query(query)
            .kind(kind)
            .width(width)
            .height(height)
            .width_ratio(width_ratio)
            .height_ratio(height_ratio)
            .position(position)
            .role(role)
            .interaction(interaction),
        chrome,
    ))
}

/// `chrome` is a window property; a float has no decoration to configure.
fn parse_window_chrome(obj: &JSObject) -> JSResult<WindowChrome> {
    match read_optional_string(obj, "chrome")?
        .as_deref()
        .map(str::trim)
    {
        None | Some("system") => Ok(WindowChrome::System),
        Some("full") => Ok(WindowChrome::Full),
        Some(other) => Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            format!("chrome must be 'system' or 'full'; got {other}"),
        )),
    }
}

fn parse_surface_interaction(
    obj: &JSObject,
    kind: SurfaceKind,
) -> JSResult<Option<lxapp::lingxia_surface::SurfaceInteraction>> {
    let Some(value) = get_property(obj, "interaction") else {
        return Ok(None);
    };
    let Some(interaction) = value.into_object() else {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
            "interaction must be an object",
        ));
    };
    let close_button = read_optional_bool(&interaction, "closeButton")?.unwrap_or(false);
    let modal = read_optional_bool(&interaction, "modal")?.unwrap_or(false);
    let dismiss = match read_optional_string(&interaction, "dismiss")?
        .as_deref()
        .unwrap_or(if kind == SurfaceKind::Window {
            "manual"
        } else {
            "tapOutside"
        }) {
        "tapOutside" => lxapp::lingxia_surface::FloatDismiss::TapOutside,
        "manual" => lxapp::lingxia_surface::FloatDismiss::Manual,
        other => {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("unsupported interaction.dismiss: {other}"),
            ));
        }
    };
    Ok(Some(lxapp::lingxia_surface::SurfaceInteraction {
        close_button,
        dismiss,
        modal,
    }))
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
            SurfaceErrorCode::InvalidArg,
            "query must be an object",
        ));
    };

    let json: Value = serde_json::from_str(&query_obj.to_json_string()?).map_err(|err| {
        surface_error(
            SurfaceErrorCode::InvalidArg,
            format!("query must be JSON serializable: {err}"),
        )
    })?;
    let Some(map) = json.as_object() else {
        return Err(surface_error(
            SurfaceErrorCode::InvalidArg,
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
        .ok_or_else(|| surface_error(SurfaceErrorCode::InvalidArg, "surface options require kind"))?
        .to_rust::<String>()
        .map_err(|_| surface_error(SurfaceErrorCode::InvalidArg, "kind must be a string"))?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "overlay" => Ok(SurfaceKind::Overlay),
        "window" => Ok(SurfaceKind::Window),
        _ => Err(surface_error(
            SurfaceErrorCode::InvalidArg,
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
            SurfaceErrorCode::InvalidArg,
            "position is only supported for overlay surfaces",
        ));
    }
    let raw = value
        .to_rust::<String>()
        .map_err(|_| surface_error(SurfaceErrorCode::InvalidArg, "position must be a string"))?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "center" => Ok(SurfacePosition::Center),
        "bottom" => Ok(SurfacePosition::Bottom),
        "left" => Ok(SurfacePosition::Left),
        "right" => Ok(SurfacePosition::Right),
        "top" => Ok(SurfacePosition::Top),
        _ => Err(surface_error(
            SurfaceErrorCode::InvalidArg,
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
            SurfaceErrorCode::InvalidArg,
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
                SurfaceErrorCode::InvalidArg,
                format!("size.{field} must be a positive number or percentage"),
            )
        })?;
        if !number.is_finite() || number <= 0.0 {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("size.{field} must be positive"),
            ));
        }
        return Ok((Some(number), None));
    }
    if value.is_string() {
        let raw = value.to_rust::<String>().map_err(|_| {
            surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("size.{field} must be a positive number or percentage"),
            )
        })?;
        if !allow_percentage {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("size.{field} percentage is only supported for overlay surfaces"),
            ));
        }
        let Some(percent) = raw.trim().strip_suffix('%') else {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("size.{field} string must be a percentage"),
            ));
        };
        let ratio = percent.trim().parse::<f64>().map_err(|_| {
            surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("size.{field} percentage is invalid"),
            )
        })? / 100.0;
        if !ratio.is_finite() || ratio <= 0.0 || ratio > 1.0 {
            return Err(surface_error(
                SurfaceErrorCode::InvalidArg,
                format!("size.{field} percentage must be > 0% and <= 100%"),
            ));
        }
        return Ok((None, Some(ratio)));
    }

    Err(surface_error(
        SurfaceErrorCode::InvalidArg,
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

fn read_optional_bool(obj: &JSObject, field: &str) -> JSResult<Option<bool>> {
    let Some(value) = get_property(obj, field) else {
        return Ok(None);
    };
    value.to_rust::<bool>().map(Some).map_err(|_| {
        surface_error(
            SurfaceErrorCode::InvalidArg,
            format!("interaction.{field} must be a boolean"),
        )
    })
}

fn invalid_surface_target(detail: impl AsRef<str>) -> rong::RongJSError {
    surface_error(SurfaceErrorCode::InvalidArg, detail.as_ref())
}

fn validate_url_target(lxapp: &LxApp, raw: &str) -> JSResult<String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err(invalid_surface_target("url must be non-empty"));
    }
    if url
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        let path = file_url_path(url)?;
        lxapp.resolve_accessible_path(&path).map_err(|_| {
            surface_error(
                SurfaceErrorCode::Denied,
                "file URL is outside this lxapp's host-authorized paths",
            )
        })?;
        return Ok(url.to_string());
    }
    let Some((scheme, host)) = split_url_scheme_host(url) else {
        return Err(invalid_surface_target(
            "url must be an absolute https or file URL",
        ));
    };
    if !surface_network_scheme_allowed(&scheme) {
        return Err(invalid_surface_target(
            "url must use https or an authorized file URL",
        ));
    }
    if !lxapp.is_domain_allowed(host) {
        return Err(surface_error(
            SurfaceErrorCode::Denied,
            format!("domain '{host}' is not allowed by lxapp security policy"),
        ));
    }
    Ok(url.to_string())
}

fn surface_network_scheme_allowed(scheme: &str) -> bool {
    scheme == "https"
}

/// External hand-off is intentionally narrower than “any syntactically valid
/// scheme”. This host allowlist prevents an lxapp from invoking arbitrary OS
/// protocol handlers. HTTPS keeps the lxapp domain policy; mail and telephone
/// links are explicit, non-browser system intents.
fn validate_external_url(lxapp: &LxApp, raw: &str) -> JSResult<String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err(invalid_surface_target("openExternal requires a url"));
    }
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .filter(|scheme| valid_url_scheme(scheme))
        .ok_or_else(|| invalid_surface_target("openExternal requires an absolute URL"))?;
    if !external_scheme_allowed(&scheme) {
        return Err(surface_error(
            SurfaceErrorCode::Denied,
            format!("URL scheme '{scheme}' is not allowed for external hand-off"),
        ));
    }
    match scheme.as_str() {
        "https" => validate_url_target(lxapp, url),
        "mailto" | "tel" => Ok(url.to_string()),
        _ => unreachable!("allowlist checked above"),
    }
}

fn external_scheme_allowed(scheme: &str) -> bool {
    matches!(scheme, "https" | "mailto" | "tel")
}

fn valid_url_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

fn file_url_path(url: &str) -> JSResult<String> {
    let Some((scheme, rest)) = url.split_once(':') else {
        return Err(invalid_surface_target("invalid file URL"));
    };
    if !scheme.eq_ignore_ascii_case("file") || !rest.starts_with("//") {
        return Err(invalid_surface_target("file URL must use file://"));
    }
    let rest = &rest[2..];
    let (authority, encoded_path) = if rest.starts_with('/') {
        ("", rest)
    } else {
        rest.split_once('/')
            .map(|(authority, _)| (authority, &rest[authority.len()..]))
            .ok_or_else(|| invalid_surface_target("file URL must contain an absolute path"))?
    };
    if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
        return Err(surface_error(
            SurfaceErrorCode::Denied,
            "remote file URL authorities are not allowed",
        ));
    }
    if encoded_path.contains(['?', '#']) {
        return Err(invalid_surface_target(
            "file URL query and fragment components are not supported",
        ));
    }
    let decoded = urlencoding::decode(encoded_path)
        .map_err(|_| invalid_surface_target("file URL contains invalid percent encoding"))?
        .into_owned();
    #[cfg(target_os = "windows")]
    let decoded = decoded
        .strip_prefix('/')
        .filter(|path| path.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(&decoded)
        .to_string();
    if decoded.is_empty() || !std::path::Path::new(&decoded).is_absolute() {
        return Err(invalid_surface_target("file URL path must be absolute"));
    }
    Ok(decoded)
}

fn split_url_scheme_host(url: &str) -> Option<(String, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let host_port = rest.split(['/', '?', '#']).next()?.trim();
    if host_port.is_empty() || host_port.contains('@') {
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

fn surface_role_label(role: lingxia_surface::Role) -> &'static str {
    use lingxia_surface::Role;
    match role {
        Role::Main => "main",
        Role::Aside => "aside",
        Role::Float => "float",
    }
}

/// Why a surface operation was refused. The JS `SurfaceErrorCode` union is
/// generated from this enum, so no caller has to match on message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceErrorCode {
    UnsupportedPlacement,
    Denied,
    NotDeclared,
    InvalidArg,
    AlreadyOpenOtherRole,
    Closed,
    CapabilityMissing,
    Failed,
}

impl SurfaceErrorCode {
    /// The `code` an lxapp reads off the error.
    const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedPlacement => "unsupported_placement",
            Self::Denied => "denied",
            Self::NotDeclared => "not_declared",
            Self::InvalidArg => "invalid_arg",
            Self::AlreadyOpenOtherRole => "already_open_other_role",
            Self::Closed => "closed",
            Self::CapabilityMissing => "capability_missing",
            Self::Failed => "failed",
        }
    }

    /// The transport-level host code, kept so existing error plumbing and the
    /// i18n registry keep working unchanged.
    const fn host_code(self) -> &'static str {
        match self {
            Self::UnsupportedPlacement => rong::error::E_NOT_SUPPORTED,
            Self::Denied => rong::error::E_PERMISSION_DENIED,
            Self::NotDeclared => rong::error::E_NOT_FOUND,
            Self::InvalidArg => rong::error::E_INVALID_ARG,
            // Documented in the shell UI spec and asserted by hosts.
            Self::AlreadyOpenOtherRole => "E_SURFACE_CONFLICT",
            Self::Closed => "E_SURFACE_CLOSED",
            Self::CapabilityMissing => rong::error::E_NOT_SUPPORTED,
            Self::Failed => rong::error::E_INTERNAL,
        }
    }
}

fn surface_error(code: SurfaceErrorCode, detail: impl std::fmt::Display) -> rong::RongJSError {
    HostError::new(code.host_code(), detail.to_string())
        .with_data(rong::err_data!({ code: (code.as_str()) }))
        .into()
}

/// Every code, in the order the generated union lists them.
#[cfg(test)]
const SURFACE_ERROR_CODES: &[&str] = &[
    SurfaceErrorCode::UnsupportedPlacement.as_str(),
    SurfaceErrorCode::Denied.as_str(),
    SurfaceErrorCode::NotDeclared.as_str(),
    SurfaceErrorCode::InvalidArg.as_str(),
    SurfaceErrorCode::AlreadyOpenOtherRole.as_str(),
    SurfaceErrorCode::Closed.as_str(),
    SurfaceErrorCode::CapabilityMissing.as_str(),
    SurfaceErrorCode::Failed.as_str(),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The public `SurfaceErrorCode` union must list exactly the codes the
    /// runtime raises, so a new code can never ship without a type for it.
    #[test]
    fn declared_error_codes_match_the_enum() {
        let source = include_str!("public_types.rs");
        let block = source
            .split("type SurfaceErrorCode = r###\"")
            .nth(1)
            .and_then(|rest| rest.split("\"###").next())
            .expect("SurfaceErrorCode literal");
        let declared: Vec<&str> = block.split('\'').skip(1).step_by(2).collect();
        assert_eq!(declared, SURFACE_ERROR_CODES);
    }

    #[test]
    fn surface_urls_reject_plain_http() {
        assert!(surface_network_scheme_allowed("https"));
        assert!(!surface_network_scheme_allowed("http"));
    }

    #[test]
    fn external_scheme_allowlist_is_explicit() {
        for allowed in ["https", "mailto", "tel"] {
            assert!(external_scheme_allowed(allowed));
        }
        assert!(!external_scheme_allowed("http"));
        assert!(!external_scheme_allowed("custom+handler"));
    }

    #[test]
    fn builtin_browser_pages_require_exact_public_urls() {
        assert_eq!(
            parse_builtin_browser_page("lingxia://settings"),
            Some(BuiltinBrowserPage::Settings)
        );
        assert_eq!(
            parse_builtin_browser_page("lingxia://downloads"),
            Some(BuiltinBrowserPage::Downloads)
        );
        for rejected in [
            "Lingxia://settings",
            "lingxia://settings/",
            "lingxia://settings?tab=privacy",
            "lingxia://downloads#active",
            "lingxia://history",
        ] {
            assert_eq!(parse_builtin_browser_page(rejected), None, "{rejected}");
        }
    }

    #[test]
    fn file_url_requires_local_absolute_path() {
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            file_url_path("file:///tmp/report.pdf").unwrap(),
            "/tmp/report.pdf"
        );
        #[cfg(target_os = "windows")]
        assert_eq!(
            file_url_path("file:///C:/Temp/report.pdf").unwrap(),
            "C:/Temp/report.pdf"
        );
        assert!(file_url_path("file://server/share/report.pdf").is_err());
        assert!(file_url_path("file://relative").is_err());
        assert!(file_url_path("file:///tmp/report.pdf#fragment").is_err());
    }

    #[test]
    fn stale_lxapp_handle_does_not_match_a_reopened_region_or_session() {
        use lxapp::LxAppOpenRegion::{Aside, Main};

        assert!(lxapp_surface_identity_matches(Main, 7, Some(Main), Some(7)));
        assert!(!lxapp_surface_identity_matches(
            Main,
            7,
            Some(Aside),
            Some(8)
        ));
        assert!(!lxapp_surface_identity_matches(
            Main,
            7,
            Some(Main),
            Some(8)
        ));
    }

    #[test]
    fn declared_surface_orchestration_overrides_are_explicit() {
        assert!(!has_declared_surface_orchestration_override(
            None, None, None
        ));
        assert!(has_declared_surface_orchestration_override(
            Some("project-a"),
            None,
            None
        ));
        assert!(has_declared_surface_orchestration_override(
            None,
            Some(lingxia_surface::Role::Main),
            None
        ));
        assert!(has_declared_surface_orchestration_override(
            None,
            None,
            Some("right")
        ));
    }

    #[test]
    fn external_close_notifies_every_retained_managed_handle_once() {
        let surface_id = "test:managed-handle-external-close";
        let (first_tx, mut first_rx) = oneshot::channel();
        let (second_tx, mut second_rx) = oneshot::channel();
        register_closed_sender(surface_id.to_string(), first_tx);
        register_closed_sender(surface_id.to_string(), second_tx);

        assert!(notify_surface_closed(surface_id, "user"));
        for receiver in [&mut first_rx, &mut second_rx] {
            let event = receiver
                .try_recv()
                .expect("close sender must stay connected")
                .expect("close event must be delivered");
            assert_eq!(event.id, surface_id);
            assert_eq!(event.reason, "user");
        }
        assert!(!notify_surface_closed(surface_id, "user"));
    }

    #[test]
    fn managed_handle_cache_shutdown_removes_only_its_context() {
        let key = |app_id: &str, session_id: u64, surface_id: &str| ManagedHandleKey {
            app_id: app_id.to_string(),
            session_id,
            surface_id: surface_id.to_string(),
        };
        let current = key("home", 7, "terminal:a");
        let sibling = key("home", 7, "terminal:b");
        let restarted = key("home", 8, "terminal:a");
        let other_app = key("chat", 2, "chat");
        let mut cache = HashMap::from([
            (current.clone(), ()),
            (sibling.clone(), ()),
            (restarted.clone(), ()),
            (other_app.clone(), ()),
        ]);

        remove_managed_handles_for_session(&mut cache, "home", 7);

        assert!(!cache.contains_key(&current));
        assert!(!cache.contains_key(&sibling));
        assert!(cache.contains_key(&restarted));
        assert!(cache.contains_key(&other_app));
    }

    #[test]
    fn active_main_transition_notifies_previous_and_current_handles() {
        use futures::FutureExt;

        let previous = "test:managed-main-previous";
        let current = "test:managed-main-current";
        let (previous_tx, mut previous_rx) = mpsc::unbounded();
        let (current_tx, mut current_rx) = mpsc::unbounded();
        register_visibility_sender(previous.to_string(), true, previous_tx);
        register_visibility_sender(current.to_string(), false, current_tx);

        assert!(notify_active_main_changed(Some(previous), Some(current)));
        assert_eq!(previous_rx.next().now_or_never().flatten(), Some(false));
        assert_eq!(current_rx.next().now_or_never().flatten(), Some(true));

        let _ = notify_surface_closed(previous, "user");
        let _ = notify_surface_closed(current, "user");
    }

    #[test]
    fn visibility_publication_skips_an_already_observed_state() {
        use futures::FutureExt;

        let surface_id = "test:managed-visibility-dedup";
        let (tx, mut rx) = mpsc::unbounded();
        register_visibility_sender(surface_id.to_string(), true, tx);

        assert!(!notify_surface_visibility(surface_id, true));
        assert_eq!(rx.next().now_or_never(), None);

        assert!(notify_surface_visibility(surface_id, false));
        assert_eq!(rx.next().now_or_never().flatten(), Some(false));

        record_surface_visibility(surface_id, true);
        assert!(!notify_surface_visibility(surface_id, true));
        assert_eq!(rx.next().now_or_never(), None);

        let _ = notify_surface_closed(surface_id, "user");
    }
}
