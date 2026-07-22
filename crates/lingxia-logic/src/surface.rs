use futures::channel::oneshot;
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_platform::traits::ui::{SurfaceKind, SurfacePosition};
use lxapp::{
    LxApp, LxAppError, PageQueryInput, PageSurfaceRequest, PageSurfaceTarget, PageTarget,
    publish_app_event, register_app_handler, try_get, unregister_app_handler,
};
use rong::{
    Class, HostError, IntoJSObject, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise,
    function::{Rest, This},
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
    kind: String,
    sender: oneshot::Sender<JSSurfaceClosed>,
}

static SURFACE_CLOSED: OnceLock<Mutex<HashMap<String, Vec<ClosedRegistration>>>> = OnceLock::new();

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSSurfaceClosed {
    id: String,
    kind: String,
    reason: String,
}

#[derive(Debug, Clone, IntoJSObject)]
#[ts_skip]
struct JSSurfaceVisibility {
    id: String,
    kind: String,
    /// Which side initiated the visibility change. "opener" when the caller
    /// holds the opener-side surface, "page" when the page-side surface drove
    /// it. Lets analytics / logging distinguish without having to wire extra
    /// state through the caller.
    source: String,
}

#[js_class(clone)]
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

    #[js_method(rename = "close", ts_return = "Promise<void>")]
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
    register_surface_api(ctx)
}

rong::js_api! {
    fn register_surface_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        // Precise correlated overloads remain in the curated Lx augmentation.
        fn openSurface(ts_params = "spec: never", ts_return = "never") = open_surface_spec;
        fn openExternal = open_external;
        fn onSurfaceContext(
            ts_params = "handler: (context: SurfaceContext) => void",
            ts_return = "() => void"
        ) = surface_on_change;
    }
}

/// Event name on the per-app bus carrying `{ sizeClass, width, height }`.
const SURFACE_CONTEXT_EVENT: &str = "SurfaceContextChange";

/// `lx.onSurfaceContext(handler)` — register a JS callback (scoped to this
/// lxapp's JS context), invoke it immediately, then again whenever that
/// presentation's actual viewport changes. Returns an unsubscribe fn.
fn surface_on_change(ctx: JSContext, handler: JSFunc) -> JSResult<JSFunc> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let initial = surface_context_for(&lxapp);
    register_app_handler(&ctx, SURFACE_CONTEXT_EVENT, handler.clone())?;
    let payload = JSValue::from_rust(&ctx, initial);
    if let Err(err) = handler.call::<_, ()>(None, (payload,)) {
        unregister_app_handler(&ctx, SURFACE_CONTEXT_EVENT, Some(handler.clone()));
        return Err(err);
    }
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

/// `lx.openSurface(spec)` — unified surface entry point. The spec is a
/// discriminated union keyed by exactly one of `page`, `surface`, or `url`:
///
/// - `{ page, as, position?, size?, query? }` opens one of this lxapp's own
///   pages as a `float` (overlay popup) or a `window` (bare standalone desktop
///   window). Pages cannot be docked as an `aside` — an aside shows external
///   content only.
/// - `{ surface, edge?, query? }` shows a host-declared surface by its `ui` id.
/// - `{ url }` opens an authorized HTTPS/file URL in the in-app chromed browser.
async fn open_surface_spec(ctx: JSContext, spec: JSValue) -> JSResult<JSValue> {
    let Some(obj) = spec.clone().into_object() else {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            "lx.openSurface expects a spec object",
        ));
    };
    let keys = [
        get_property(&obj, "page").is_some(),
        get_property(&obj, "url").is_some(),
        get_property(&obj, "lxapp").is_some(),
        get_property(&obj, "native").is_some(),
        get_property(&obj, "surface").is_some(),
    ];
    if keys.iter().filter(|set| **set).count() != 1 {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            "spec must contain exactly one content key: lxapp, page, url, native, or surface",
        ));
    }
    match keys {
        [true, ..] => open_page_spec(ctx, &obj).await.map(JSObject::into_js_value),
        [_, true, ..] => open_url_spec(ctx, &obj).await,
        [_, _, true, ..] => open_lxapp_spec(ctx, &obj)
            .await
            .map(JSObject::into_js_value),
        [_, _, _, true, _] => open_native_spec(&ctx, &obj).map(JSObject::into_js_value),
        _ => open_declared_surface_spec(&ctx, &obj).map(JSObject::into_js_value),
    }
}

/// Reject callers other than the home lxapp for the privileged content keys
/// (`lxapp` / `native`) — the same single-writer model as `lx.shell`. Gates on
/// the configured home appId (like `ensure_home_lxapp`), not the instance
/// flag, which a dev-mode reinstall can recreate without.
fn require_home_caller(lxapp: &LxApp, key: &str) -> JSResult<()> {
    if lingxia_app_context::home_app_id().is_some_and(|home| lxapp.appid == home) {
        return Ok(());
    }
    Err(surface_error(
        rong::error::E_PERMISSION_DENIED,
        "denied",
        format!("openSurface({{ {key} }}) is restricted to the home lxapp"),
    ))
}

/// `{ lxapp, as?, edge? }` branch of `lx.openSurface`. Opens another lxapp by
/// appId — the bundle is ensured first (installed from the configured cloud
/// when missing); declared surfaces then toggle their shell presentation, and
/// an undeclared lxapp opens as a main tab, or docks as an aside panel with
/// `as: 'aside'`.
async fn open_lxapp_spec(ctx: JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let app_id = read_required_string(spec, "lxapp")?;
    let app_id = app_id.trim().to_string();
    if app_id.is_empty() {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            "lxapp must be a non-empty appId",
        ));
    }
    let edge = read_validated_edge(spec)?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    require_home_caller(&lxapp, "lxapp")?;

    let as_role = read_optional_string(spec, "as")?;
    let as_role = as_role.as_deref().map(str::trim);
    if let Some(other) = as_role
        && !matches!(other, "main" | "aside" | "float")
    {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            format!("an lxapp surface supports as: 'main' | 'aside' | 'float'; got {other}"),
        ));
    }

    // Ensure the bundle exists before any presentation — this is what pulls
    // the lxapp from the cloud when it is not bundled/installed.
    lxapp::prepare_lxapp_open(&app_id, lxapp::ReleaseType::Release)
        .await
        .map_err(|err| {
            surface_error(rong::error::E_NOT_FOUND, "lxapp_not_found", err.to_string())
        })?;

    let declared_aside = declared_lxapp_aside(&app_id);
    let (region, shell_surface_id) = match as_role {
        // No override: preserve a live app's current region; otherwise let a
        // declaration supply its role, falling back to main when undeclared.
        None => match lxapp::open_region(&app_id) {
            Some(region) => {
                let surface_id = if region == lxapp::LxAppOpenRegion::Aside {
                    declared_aside
                        .as_ref()
                        .map(|aside| aside.surface_id.as_str())
                        .unwrap_or(&app_id)
                } else {
                    &app_id
                };
                show_lxapp_region(&lxapp, &app_id, surface_id, region, edge.as_deref())?;
                (region, surface_id.to_string())
            }
            None => {
                if let Some(aside) = declared_aside.as_ref() {
                    open_lxapp_region(
                        &app_id,
                        lxapp::LxAppOpenRegion::Aside,
                        &aside.surface_id,
                        &aside.path,
                    )?;
                    lxapp.register_host_aside(
                        &aside.surface_id,
                        edge.as_deref().unwrap_or(&aside.edge),
                    );
                    (lxapp::LxAppOpenRegion::Aside, aside.surface_id.clone())
                } else {
                    open_lxapp_region(&app_id, lxapp::LxAppOpenRegion::Main, &app_id, "")?;
                    (lxapp::LxAppOpenRegion::Main, app_id.clone())
                }
            }
        },
        Some("main") => {
            open_lxapp_region(&app_id, lxapp::LxAppOpenRegion::Main, &app_id, "")?;
            (lxapp::LxAppOpenRegion::Main, app_id.clone())
        }
        Some("aside") => {
            let surface_id = declared_aside
                .as_ref()
                .map(|aside| aside.surface_id.as_str())
                .unwrap_or(&app_id);
            let path = declared_aside
                .as_ref()
                .map(|aside| aside.path.as_str())
                .unwrap_or("");
            let default_edge = declared_aside
                .as_ref()
                .map(|aside| aside.edge.as_str())
                .unwrap_or("right");
            open_lxapp_region(&app_id, lxapp::LxAppOpenRegion::Aside, surface_id, path)?;
            // Runtime lxapps have no declaration for the host's managed-
            // surface lookup. Register the child directly in the shared graph;
            // declared hosts may already have done so and replacement is
            // intentionally idempotent.
            lxapp.register_host_aside(surface_id, edge.as_deref().unwrap_or(default_edge));
            (lxapp::LxAppOpenRegion::Aside, surface_id.to_string())
        }
        Some("float") => {
            lxapp
                .set_shell_surface_visible(&app_id, true, edge.as_deref())
                .map_err(|_| {
                    surface_error(
                        rong::error::E_NOT_SUPPORTED,
                        "float_undeclared",
                        "as: 'float' requires a declared float surface",
                    )
                })?;
            let region = lxapp::open_region(&app_id).ok_or_else(|| {
                surface_error(
                    rong::error::E_INTERNAL,
                    "shell_surface_failed",
                    format!("float lxapp '{app_id}' opened without a runtime region"),
                )
            })?;
            (region, app_id.clone())
        }
        Some(_) => unreachable!("validated above"),
    };
    lxapp::schedule_lxapp_update_check(&app_id, lxapp::ReleaseType::Release);
    lxapp_surface_handle(&ctx, lxapp, app_id, shell_surface_id, region)
}

struct DeclaredLxappAside {
    surface_id: String,
    path: String,
    edge: String,
}

fn declared_lxapp_aside(app_id: &str) -> Option<DeclaredLxappAside> {
    let item = lingxia_app_context::app_config()?
        .panels
        .as_ref()?
        .items
        .iter()
        .find(|item| item.content.kind.is_lxapp() && item.content.app_id == app_id)?;
    let edge = match item.position {
        lingxia_app_context::PanelPosition::Left => "left",
        lingxia_app_context::PanelPosition::Right => "right",
        lingxia_app_context::PanelPosition::Top => "top",
        lingxia_app_context::PanelPosition::Bottom => "bottom",
    };
    Some(DeclaredLxappAside {
        surface_id: item.id.clone(),
        path: item.content.path.clone().unwrap_or_default(),
        edge: edge.to_string(),
    })
}

fn open_lxapp_region(
    app_id: &str,
    region: lxapp::LxAppOpenRegion,
    shell_surface_id: &str,
    path: &str,
) -> JSResult<()> {
    let options = match region {
        lxapp::LxAppOpenRegion::Main => lxapp::LxAppStartupOptions::new(path),
        lxapp::LxAppOpenRegion::Aside => lxapp::LxAppStartupOptions {
            path: path.to_string(),
            open_mode: lingxia_platform::traits::app_runtime::LxAppOpenMode::Panel,
            panel_id: shell_surface_id.to_string(),
            ..Default::default()
        },
    };
    lxapp::open_lxapp(app_id, options)
        .map(|_| ())
        .map_err(lxapp_open_error)
}

fn show_lxapp_region(
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
                    rong::error::E_NOT_FOUND,
                    "lxapp_not_found",
                    format!("lxapp is not active: {app_id}"),
                )
            })?;
            app.set_active_main();
            Ok(())
        }
        lxapp::LxAppOpenRegion::Aside => {
            if shell
                .set_shell_surface_visible(shell_surface_id, true, edge)
                .is_ok()
            {
                Ok(())
            } else {
                open_lxapp_region(app_id, region, shell_surface_id, "")?;
                shell.register_host_aside(shell_surface_id, edge.unwrap_or("right"));
                Ok(())
            }
        }
    }
}

fn lxapp_open_error(err: LxAppError) -> rong::RongJSError {
    match err {
        LxAppError::SurfaceConflict(message) => {
            surface_error("E_SURFACE_CONFLICT", "surface_conflict", message)
        }
        other => surface_error(
            rong::error::E_NOT_FOUND,
            "lxapp_not_found",
            other.to_string(),
        ),
    }
}

/// `{ native, edge? }` branch of `lx.openSurface`. Shows a host-registered
/// native capability (declared in lingxia.yaml surfaces, e.g. the terminal).
fn open_native_spec(ctx: &JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let name = read_required_string(spec, "native")?;
    let edge = read_validated_edge(spec)?;
    let lxapp = LxApp::from_ctx(ctx)?;
    require_home_caller(&lxapp, "native")?;
    lxapp
        .set_shell_surface_visible(name.trim(), true, edge.as_deref())
        .map_err(|err| surface_error(rong::error::E_NOT_FOUND, "native_not_found", err))?;
    managed_surface_handle(ctx, lxapp, name.trim().to_string())
}

/// Published declaration-id form. It remains available to non-home lxapps;
/// only the newer product-level `{ lxapp }` and `{ native }` selectors are
/// restricted to the home app.
fn open_declared_surface_spec(ctx: &JSContext, spec: &JSObject) -> JSResult<JSObject> {
    let id = read_required_string(spec, "surface")?;
    let edge = read_validated_edge(spec)?;
    let lxapp = LxApp::from_ctx(ctx)?;
    lxapp
        .set_shell_surface_visible(id.trim(), true, edge.as_deref())
        .map_err(|err| surface_error(rong::error::E_NOT_FOUND, "surface_not_found", err))?;
    managed_surface_handle(ctx, lxapp, id.trim().to_string())
}

fn read_validated_edge(spec: &JSObject) -> JSResult<Option<String>> {
    let edge = read_optional_string(spec, "edge")?;
    if let Some(edge) = edge.as_deref()
        && !matches!(edge.trim(), "left" | "right" | "top" | "bottom")
    {
        return Err(surface_error(
            rong::error::E_INVALID_ARG,
            "invalid_surface_spec",
            format!("edge must be left, right, top, or bottom; got {edge}"),
        ));
    }
    Ok(edge.map(|edge| edge.trim().to_string()))
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
        "float" => {
            let position = position.or(edge).unwrap_or_else(|| "center".to_string());
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
                    rong::error::E_NOT_SUPPORTED,
                    "window_unsupported_platform",
                    "lx.surface window is not supported on this platform",
                ));
            }
            #[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
            {
                // A compact (phone-shaped) layout — e.g. the macOS runner emulating
                // an iPhone — has no room for a separate desktop window, so reject
                // it exactly as a real phone does instead of spawning one.
                if is_compact_layout(&lxapp) {
                    return Err(surface_error(
                        rong::error::E_NOT_SUPPORTED,
                        "window_unsupported_platform",
                        "as: 'window' opens a separate desktop window and is not available on this platform",
                    ));
                }
                build_window_options(&ctx, &path_value, size.as_ref())?
            }
        }
        other => {
            return Err(surface_error(
                rong::error::E_INVALID_ARG,
                "invalid_surface_spec",
                format!(
                    "as must be 'float' or 'window' (a page cannot be an aside — asides are external content only); got {other}"
                ),
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

/// `{ url, as?, edge? }` branch of `lx.openSurface`. Without `as`, the url opens
/// as a full in-app browser tab in the main content (host-owned chrome, no
/// handle), in contrast to `lx.openExternal` which hands off to the OS browser.
/// With `as: 'aside'` the url is docked beside the main as a closable browser
/// tab strip on desktop. Compact hosts project the same request into the
/// full-screen in-app browser with aside chrome.
async fn open_url_spec(ctx: JSContext, spec: &JSObject) -> JSResult<JSValue> {
    let raw_url = read_required_string(spec, "url")?;
    let lxapp = LxApp::from_ctx(&ctx)?;

    match read_optional_string(spec, "as")?.as_deref().map(str::trim) {
        Some("aside") => {
            let position = read_validated_edge(spec)?.unwrap_or_else(|| "right".to_string());
            let _ = parse_size(spec, SurfaceKind::Overlay)?;
            if url_aside_uses_compact_browser(&lxapp) {
                return open_url_in_browser(&ctx, &lxapp, raw_url.trim(), true);
            }
            let url = validate_url_target(&lxapp, raw_url.trim())?;
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
        None => open_url_in_browser(&ctx, &lxapp, raw_url.trim(), false),
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
    let url = validate_external_url(&lxapp, &url)?;
    lxapp
        .runtime
        .open_url(OpenUrlRequest {
            owner_appid: lxapp.appid.clone(),
            owner_session_id: lxapp.session_id(),
            url,
            target: OpenUrlTarget::External,
        })
        .map_err(|err| surface_error(rong::error::E_INTERNAL, "open_url_failed", err))
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
        kind: "overlay".to_string(),
        message_port,
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
        visible: Cell::new(true),
        alive: Cell::new(true),
    });
    handle.set("id", app_id.clone())?;
    handle.set("kind", "overlay")?;
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
        JSFunc::new(ctx, move || -> JSResult<()> {
            ensure_lxapp_surface_open(&show_handle, &show_id, region, show_session_id)?;
            show_lxapp_region(&show_shell, &show_id, &show_surface_id, region, None)?;
            mark_visible(&show_handle, true, "opener")
        })?,
    )?;

    let hide_shell = shell.clone();
    let hide_id = app_id.clone();
    let hide_surface_id = shell_surface_id.clone();
    let hide_session_id = session_id;
    let hide_handle = handle.clone();
    handle.set(
        "hide",
        JSFunc::new(ctx, move || -> JSResult<()> {
            ensure_lxapp_surface_open(&hide_handle, &hide_id, region, hide_session_id)?;
            match region {
                lxapp::LxAppOpenRegion::Main => Err(surface_error(
                    rong::error::E_NOT_SUPPORTED,
                    "main_hide_unsupported",
                    "a main surface cannot be hidden; select another main or close it",
                )),
                lxapp::LxAppOpenRegion::Aside => {
                    hide_lxapp_aside(&hide_shell, &hide_id, &hide_surface_id)?;
                    mark_visible(&hide_handle, false, "opener")
                }
            }
        })?,
    )?;

    let close_shell = shell;
    let close_id = app_id;
    let close_surface_id = shell_surface_id;
    let close_session_id = session_id;
    let close_handle = handle.clone();
    handle.set(
        "close",
        JSFunc::new(ctx, move || -> JSResult<()> {
            if !close_handle.borrow::<JSSurface>()?.alive.get() {
                return Ok(());
            }
            if !lxapp_surface_session_is_current(&close_id, region, close_session_id) {
                return emit_lxapp_handle_close(&close_handle, &close_id, "app_closed");
            }
            if region == lxapp::LxAppOpenRegion::Aside {
                hide_lxapp_aside(&close_shell, &close_id, &close_surface_id)?;
            }
            lxapp::close_lxapp(&close_id).map_err(|err| {
                surface_error(
                    rong::error::E_INTERNAL,
                    "surface_close_failed",
                    err.to_string(),
                )
            })?;
            emit_lxapp_handle_close(&close_handle, &close_id, "programmatic")
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
            kind: "overlay".to_string(),
            reason: reason.to_string(),
        },
    )
}

fn hide_lxapp_aside(shell: &LxApp, app_id: &str, shell_surface_id: &str) -> JSResult<()> {
    if shell
        .set_shell_surface_visible(shell_surface_id, false, None)
        .is_ok()
    {
        return Ok(());
    }
    let app = lxapp::try_get(app_id).ok_or_else(|| {
        surface_error(
            rong::error::E_NOT_FOUND,
            "lxapp_not_found",
            format!("lxapp is not active: {app_id}"),
        )
    })?;
    app.runtime
        .hide_lxapp(app_id.to_string(), app.session_id())
        .map_err(|err| {
            surface_error(
                rong::error::E_INTERNAL,
                "shell_surface_failed",
                err.to_string(),
            )
        })?;
    shell.unregister_host_aside(shell_surface_id);
    Ok(())
}

/// Lifecycle-complete handle for a host-managed native surface. Native
/// capabilities without messaging omit only postMessage/onMessage.
fn managed_surface_handle(ctx: &JSContext, lxapp: Arc<LxApp>, id: String) -> JSResult<JSObject> {
    let (message_port, _) = crate::message_port::pair(ctx)?;
    let handle = Class::lookup::<JSSurface>(ctx)?.instance(JSSurface {
        id: id.clone(),
        kind: "overlay".to_string(),
        message_port,
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
        visible: Cell::new(true),
        alive: Cell::new(true),
    });
    handle.set("id", id.clone())?;
    handle.set("kind", "overlay")?;
    handle.set("role", "aside")?;
    handle.set(
        "presentation",
        lxapp.shell_surface_presentation(&id).unwrap_or("dock"),
    )?;
    handle.set("visible", true)?;
    handle.set("alive", true)?;

    let show_lxapp = lxapp.clone();
    let show_id = id.clone();
    let show_handle = handle.clone();
    handle.set(
        "show",
        JSFunc::new(ctx, move || -> JSResult<()> {
            ensure_surface_object_open(&show_handle)?;
            show_lxapp
                .set_shell_surface_visible(&show_id, true, None)
                .map_err(|err| {
                    surface_error(rong::error::E_INTERNAL, "shell_surface_failed", err)
                })?;
            mark_visible(&show_handle, true, "opener")
        })?,
    )?;

    let hide_lxapp = lxapp.clone();
    let hide_id = id.clone();
    let hide_handle = handle.clone();
    handle.set(
        "hide",
        JSFunc::new(ctx, move || -> JSResult<()> {
            ensure_surface_object_open(&hide_handle)?;
            hide_lxapp
                .set_shell_surface_visible(&hide_id, false, None)
                .map_err(|err| {
                    surface_error(rong::error::E_INTERNAL, "shell_surface_failed", err)
                })?;
            mark_visible(&hide_handle, false, "opener")
        })?,
    )?;

    let close_lxapp = lxapp;
    let close_id = id;
    let close_handle = handle.clone();
    handle.set(
        "close",
        JSFunc::new(ctx, move || -> JSResult<()> {
            if !close_handle.borrow::<JSSurface>()?.alive.get() {
                return Ok(());
            }
            close_lxapp
                .set_shell_surface_visible(&close_id, false, None)
                .map_err(|err| {
                    surface_error(rong::error::E_INTERNAL, "shell_surface_failed", err)
                })?;
            close_lxapp.unregister_host_aside(&close_id);
            emit_close(
                &close_handle,
                &JSSurfaceClosed {
                    id: close_id.clone(),
                    kind: "overlay".to_string(),
                    reason: "programmatic".to_string(),
                },
            )
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

    Ok(handle)
}

/// Validate a `{ url }` target for the in-app browser surface policy.
fn lxapp_url(lxapp: &LxApp, raw: &str) -> JSResult<String> {
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

/// Whether the lxapp's window currently renders at a compact (phone) width.
/// Drives the desktop `as: 'window'` gate so the macOS runner's iPhone shape
/// rejects windows like a real phone. Only the desktop build consults this.
#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
fn is_compact_layout(lxapp: &LxApp) -> bool {
    use lingxia_surface::SizeClass;
    matches!(
        lxapp
            .surface_derived_layout()
            .as_ref()
            .map(|l| l.size_class),
        Some(SizeClass::Compact)
    )
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
/// Returns null because compact browser tabs are owned by browser chrome rather
/// than the generic surface presenter.
fn open_url_in_browser(
    ctx: &JSContext,
    lxapp: &LxApp,
    raw_url: &str,
    aside: bool,
) -> JSResult<JSValue> {
    let url = lxapp_url(lxapp, raw_url)?;
    lxapp
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
        })
        .map_err(|err| surface_error(rong::error::E_INTERNAL, "open_url_failed", err))?;
    Ok(JSValue::null(ctx))
}

async fn open_surface(ctx: JSContext, options: JSValue) -> JSResult<JSObject> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let mut request = parse_surface_options(&lxapp, &options)?;
    request.id = format!("surface-{}", Uuid::new_v4().simple());
    let requested_surface_id = request.id.clone();
    let kind = surface_kind_label(request.kind).to_string();

    let (closed_tx, closed_rx) = oneshot::channel::<JSSurfaceClosed>();
    register_closed_sender(requested_surface_id.clone(), kind.clone(), closed_tx);
    let opened_surface = lxapp.open_surface(request).map_err(|err| {
        unregister_closed_sender(&requested_surface_id);
        match err {
            LxAppError::UnsupportedOperation(detail) => surface_error(
                rong::error::E_NOT_SUPPORTED,
                "surface_not_supported",
                detail,
            ),
            other => surface_error(rong::error::E_INTERNAL, "surface_open_failed", other),
        }
    })?;
    let surface_id = opened_surface.id.clone();
    if surface_id != requested_surface_id {
        move_closed_senders(&requested_surface_id, &surface_id);
    }
    let surface_id_for_closed = surface_id.clone();
    let kind_for_closed = kind.clone();
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
    surface.set("id", opened_surface.id.clone())?;
    surface.set("kind", kind.clone())?;
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
    surface_error(
        "E_SURFACE_CLOSED",
        "surface_closed",
        "surface handle is closed",
    )
}

fn surface_lifecycle_error(operation: &str, error: LxAppError) -> rong::RongJSError {
    match error {
        LxAppError::UnsupportedOperation(detail) => surface_error(
            rong::error::E_NOT_SUPPORTED,
            "surface_not_supported",
            detail,
        ),
        other => surface_error(
            rong::error::E_INTERNAL,
            match operation {
                "show" => "surface_show_failed",
                "hide" => "surface_hide_failed",
                _ => "surface_operation_failed",
            },
            other,
        ),
    }
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
            kind: registration.kind,
            reason: reason.clone(),
        });
    }
    true
}

fn register_closed_sender(id: String, kind: String, sender: oneshot::Sender<JSSurfaceClosed>) {
    if let Ok(mut guard) = SURFACE_CLOSED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        guard
            .entry(id)
            .or_default()
            .push(ClosedRegistration { kind, sender });
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
    if url
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        let path = file_url_path(url)?;
        lxapp.resolve_accessible_path(&path).map_err(|_| {
            surface_error(
                "E_DENIED",
                "security_denied",
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
            rong::error::E_INVALID_ARG,
            "security_denied",
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
            "E_DENIED",
            "security_denied",
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
            "E_DENIED",
            "security_denied",
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

fn surface_kind_label(kind: SurfaceKind) -> &'static str {
    match kind {
        SurfaceKind::Overlay => "overlay",
        SurfaceKind::Window => "window",
    }
}

fn surface_role_label(role: lingxia_platform::traits::ui::SurfaceRole) -> &'static str {
    use lingxia_platform::traits::ui::SurfaceRole;
    match role {
        SurfaceRole::Main => "main",
        SurfaceRole::Aside => "aside",
        SurfaceRole::Float => "float",
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
