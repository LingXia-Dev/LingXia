use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lingxia_surface::{Edge, LayoutPresentationPlan};
use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{WindowsWebViewHandler, find_webview_handler};
use lingxia_webview::runtime as webview_runtime;
use lingxia_windows_host::{
    WindowsPanelPosition, hide_webview_window, navigate_webview_window, present_webview_as_overlay,
    present_webview_in_active_group, set_webview_close_handler, show_webview_as_adaptive_panel,
    show_webview_as_panel, show_webview_window, show_webview_window_with_content_size,
};

use super::request_windows_app_exit;
use crate::error::PlatformError;
use crate::traits::app_runtime::LxAppOpenMode;
use crate::traits::ui::{SurfaceContent, SurfaceKind, SurfaceRequest, SurfaceRole};

static WINDOWS_SHOW_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static WINDOWS_SHOW_REQUESTS: LazyLock<Mutex<HashMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsCloseAction {
    ExitApp,
    HideWindow,
}

pub(super) fn show_webtag_window(
    webtag: WebTag,
    title: String,
    activate: bool,
    open_mode: LxAppOpenMode,
    panel_id: String,
) {
    let request_key = show_request_key(&webtag, open_mode, &panel_id);
    let request_id = remember_show_request(&request_key);
    if let Some(handler) = find_webview_handler(&webtag) {
        if show_request_is_current(&request_key, request_id) {
            install_close_handler(&webtag, close_action_for_mode(open_mode));
            show_webview_handler_for_mode(handler, &title, activate, open_mode, &panel_id);
        }
        return;
    }

    let _ = thread::Builder::new()
        .name(format!("lingxia-windows-show-{}", webtag.key()))
        .spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if !show_request_is_current(&request_key, request_id) {
                    return;
                }
                if let Some(handler) = find_webview_handler(&webtag) {
                    install_close_handler(&webtag, close_action_for_mode(open_mode));
                    show_webview_handler_for_mode(handler, &title, activate, open_mode, &panel_id);
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            log::error!("Timed out waiting for Windows WebView {}", webtag.key());
        });
}

pub(super) fn navigate_webtag_window(webtag: WebTag, title: String) {
    let request_key = show_request_key(&webtag, LxAppOpenMode::Normal, "");
    let request_id = remember_show_request(&request_key);
    if let Some(handler) = find_webview_handler(&webtag) {
        if show_request_is_current(&request_key, request_id) {
            show_webview_handler_navigate(handler, &title);
        }
        return;
    }

    let _ = thread::Builder::new()
        .name(format!("lingxia-windows-navigate-{}", webtag.key()))
        .spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if !show_request_is_current(&request_key, request_id) {
                    return;
                }
                if let Some(handler) = find_webview_handler(&webtag) {
                    show_webview_handler_navigate(handler, &title);
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            log::error!(
                "Timed out waiting for Windows navigation WebView {}",
                webtag.key()
            );
        });
}

pub(super) fn hide_lxapp_window(appid: &str, session_id: u64) {
    // Invalidate any pending show request first so the polling waiter thread
    // cannot re-show the window after this hide.
    invalidate_show_request(&format!("main:{appid}#{session_id}"));
    for webtag in webview_runtime::list_webviews() {
        if webtag.extract_appid() == appid && webtag.session_id() == Some(session_id) {
            let _ = hide_webview_window(&webtag);
        }
    }
}

fn show_webview_handler_for_mode(
    handler: WindowsWebViewHandler,
    title: &str,
    activate: bool,
    open_mode: LxAppOpenMode,
    panel_id: &str,
) {
    let result = match open_mode {
        LxAppOpenMode::Panel => show_webview_as_panel(&handler.webtag(), title, panel_id),
        LxAppOpenMode::Normal => show_webview_window(&handler.webtag(), title, activate),
    };
    if let Err(err) = result {
        log::warn!(
            "Failed to show Windows WebView window {}: {}",
            handler.webtag().key(),
            err
        );
    }
}

fn show_webview_handler_navigate(handler: WindowsWebViewHandler, title: &str) {
    let webtag = handler.webtag();
    if let Err(err) = navigate_webview_window(&webtag, title, false) {
        log::warn!(
            "Failed to navigate Windows WebView window {}: {}",
            webtag.key(),
            err
        );
    }
}

fn show_request_key(webtag: &WebTag, open_mode: LxAppOpenMode, panel_id: &str) -> String {
    match open_mode {
        LxAppOpenMode::Normal => {
            format!(
                "main:{}#{}",
                webtag.extract_appid(),
                webtag
                    .session_id()
                    .map(|session| session.to_string())
                    .unwrap_or_else(|| "0".to_string())
            )
        }
        LxAppOpenMode::Panel => format!("panel:{panel_id}"),
    }
}

fn remember_show_request(key: &str) -> u64 {
    let request_id = WINDOWS_SHOW_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut requests) = WINDOWS_SHOW_REQUESTS.lock() {
        requests.insert(key.to_string(), request_id);
    }
    request_id
}

fn show_request_is_current(key: &str, request_id: u64) -> bool {
    WINDOWS_SHOW_REQUESTS
        .lock()
        .ok()
        .and_then(|requests| requests.get(key).copied())
        == Some(request_id)
}

fn invalidate_show_request(key: &str) {
    if let Ok(mut requests) = WINDOWS_SHOW_REQUESTS.lock() {
        requests.remove(key);
    }
}

fn close_action_for_mode(open_mode: LxAppOpenMode) -> WindowsCloseAction {
    match open_mode {
        LxAppOpenMode::Normal => WindowsCloseAction::ExitApp,
        LxAppOpenMode::Panel => WindowsCloseAction::HideWindow,
    }
}

fn install_close_handler(webtag: &WebTag, action: WindowsCloseAction) {
    let webtag_for_close = webtag.clone();
    set_webview_close_handler(
        webtag,
        Arc::new(move || match action {
            WindowsCloseAction::ExitApp => request_windows_app_exit(),
            WindowsCloseAction::HideWindow => {
                let _ = hide_webview_window(&webtag_for_close);
            }
        }),
    );
}

// === lx.surface (SurfacePresenter) ===
//
// A surface's content webview is created by lxapp (a page instance); the
// presenter shows it as a desktop window and reports closes back to the logic
// layer. The platform layer cannot depend on lingxia-logic, so the close
// notification is delivered through a callback the `lingxia` facade registers
// through the same inversion the apple/android/harmony FFI layers use to call
// `lingxia_logic::notify_surface_closed`.

type SurfaceClosedHandler = Arc<dyn Fn(&str, &str) + Send + Sync>;
static SURFACE_CLOSED_HANDLER: Mutex<Option<SurfaceClosedHandler>> = Mutex::new(None);

/// Registers the callback invoked when a surface closes (user-initiated window
/// close or programmatic). The `lingxia` facade wires this to
/// `lingxia_logic::notify_surface_closed` so the JS `onClose` event fires.
pub fn set_windows_surface_closed_handler(handler: SurfaceClosedHandler) {
    if let Ok(mut slot) = SURFACE_CLOSED_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn notify_surface_closed(id: &str, reason: &str) {
    let handler = SURFACE_CLOSED_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler(id, reason);
    }
}

/// Reports a surface page-instance's visibility to the logic layer. The
/// callback returns whether the page instance accepted the transition; Windows
/// may present the WebView before lxapp has marked the page instance mounted.
type PageVisibilityHandler = Arc<dyn Fn(&str, bool) -> bool + Send + Sync>;
static PAGE_VISIBILITY_HANDLER: Mutex<Option<PageVisibilityHandler>> = Mutex::new(None);

pub fn set_windows_page_visibility_handler(handler: PageVisibilityHandler) {
    if let Ok(mut slot) = PAGE_VISIBILITY_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn notify_page_visibility(page_instance_id: &str, visible: bool) -> bool {
    let handler = PAGE_VISIBILITY_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        return handler(page_instance_id, visible);
    }
    false
}

fn notify_page_visibility_when_ready(page_instance_id: String, visible: bool) {
    if notify_page_visibility(&page_instance_id, visible) {
        return;
    }

    let _ = thread::Builder::new()
        .name(format!("lingxia-surface-visible-{page_instance_id}"))
        .spawn(move || {
            for _ in 0..100 {
                thread::sleep(Duration::from_millis(50));
                if notify_page_visibility(&page_instance_id, visible) {
                    return;
                }
            }
        });
}

/// Disposes a surface's content page instance in the logic layer. Disposing
/// detaches and destroys the page's webview. Closing a window/overlay through
/// plain `destroy_webview` cannot, because the page instance still holds a
/// webview reference) and fires onClose. The `lingxia` facade wires this to
/// `lxapp::dispose_page_instance_by_id`.
type SurfaceDisposeHandler = Arc<dyn Fn(&str, &str) + Send + Sync>;
static SURFACE_DISPOSE_HANDLER: Mutex<Option<SurfaceDisposeHandler>> = Mutex::new(None);

pub fn set_windows_surface_dispose_handler(handler: SurfaceDisposeHandler) {
    if let Ok(mut slot) = SURFACE_DISPOSE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn dispose_surface_page(page_instance_id: &str, reason: &str) {
    let handler = SURFACE_DISPOSE_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler(page_instance_id, reason);
    }
}

type ManagedSurfaceVisibleHandler = Arc<dyn Fn(&str, bool) -> bool + Send + Sync>;
static MANAGED_SURFACE_VISIBLE_HANDLER: Mutex<Option<ManagedSurfaceVisibleHandler>> =
    Mutex::new(None);

pub fn set_windows_managed_surface_visible_handler(handler: ManagedSurfaceVisibleHandler) {
    if let Ok(mut slot) = MANAGED_SURFACE_VISIBLE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

pub(super) fn set_managed_surface_visible(id: &str, visible: bool) -> Result<(), PlatformError> {
    let handler = MANAGED_SURFACE_VISIBLE_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .ok_or_else(|| {
            PlatformError::NotSupported(
                "managed surfaces are not supported on this Windows host".to_string(),
            )
        })?;
    if handler(id, visible) {
        Ok(())
    } else {
        Err(PlatformError::InvalidParameter(format!(
            "unknown managed surface: {id}"
        )))
    }
}

type ManagedSurfaceToggleHandler = Arc<dyn Fn(&str) -> bool + Send + Sync>;
static MANAGED_SURFACE_TOGGLE_HANDLER: Mutex<Option<ManagedSurfaceToggleHandler>> =
    Mutex::new(None);

pub fn set_windows_managed_surface_toggle_handler(handler: ManagedSurfaceToggleHandler) {
    if let Ok(mut slot) = MANAGED_SURFACE_TOGGLE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

pub(super) fn toggle_managed_surface(id: &str) -> Result<(), PlatformError> {
    let handler = MANAGED_SURFACE_TOGGLE_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .ok_or_else(|| {
            PlatformError::NotSupported(
                "managed surfaces are not supported on this Windows host".to_string(),
            )
        })?;
    if handler(id) {
        Ok(())
    } else {
        Err(PlatformError::InvalidParameter(format!(
            "unknown managed surface: {id}"
        )))
    }
}

#[derive(Clone)]
pub struct WindowsUrlSurfaceWebTag {
    pub app_id: String,
    pub path: String,
    pub session_id: u64,
    pub cleanup: Option<Arc<dyn Fn() + Send + Sync>>,
}

type UrlSurfaceHandler =
    Arc<dyn Fn(&SurfaceRequest) -> Option<WindowsUrlSurfaceWebTag> + Send + Sync>;
static URL_SURFACE_HANDLER: Mutex<Option<UrlSurfaceHandler>> = Mutex::new(None);

pub fn set_windows_url_surface_handler(handler: UrlSurfaceHandler) {
    if let Ok(mut slot) = URL_SURFACE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn resolve_url_surface(request: &SurfaceRequest) -> Option<WindowsUrlSurfaceWebTag> {
    let handler = URL_SURFACE_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone())?;
    handler(request)
}

/// Tears down a surface: clears window-mode flags, fires onClose, and disposes
/// the content page instance (which closes the window/overlay). Used by both
/// programmatic close and the native close button.
fn teardown_surface(entry: &SurfaceEntry, id: &str, reason: &str) {
    notify_surface_closed(id, reason);
    match &entry.page_instance_id {
        // Disposing the page instance detaches + destroys its webview (closing
        // the window; a presented overlay is restored by cleanup_window_state).
        Some(page_instance_id) => dispose_surface_page(page_instance_id, reason),
        // A Url-content surface has no page instance; destroy its webview.
        None => {
            if let Some(cleanup) = &entry.cleanup {
                cleanup();
            }
            webview_runtime::destroy_webview(&entry.webtag);
        }
    }
}

/// Geometry for an `overlay`-kind popup. `width`/`height` are logical pixels
/// (0 = derive from the ratio or a default); `width_ratio`/`height_ratio` are
/// fractions of the monitor work area (0 = none); `position` mirrors the
/// `SurfacePosition` discriminants (0 center, 1 bottom, 2 left, 3 right, 4 top).
#[derive(Debug, Clone, Copy, Default)]
struct OverlayPlacement {
    width: f64,
    height: f64,
    width_ratio: f64,
    height_ratio: f64,
    position: u8,
}

#[derive(Clone)]
struct SurfaceEntry {
    webtag: WebTag,
    kind: SurfaceKind,
    role: SurfaceRole,
    title: String,
    page_instance_id: Option<String>,
    cleanup: Option<Arc<dyn Fn() + Send + Sync>>,
    placement: OverlayPlacement,
}

#[derive(Clone, Copy)]
enum PresentationTarget {
    Stored,
    Aside {
        edge: Option<Edge>,
        preferred_size: Option<f64>,
    },
}

static SURFACES: LazyLock<Mutex<HashMap<String, SurfaceEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn surface_entry(id: &str) -> Option<SurfaceEntry> {
    SURFACES.lock().ok().and_then(|map| map.get(id).cloned())
}

/// A finite, positive dimension, else 0 (meaning "unset"; the host derives a
/// default). The logic layer passes NaN for unset surface dimensions.
fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

/// Shows a surface's webview according to the core-arbitrated role.
fn present_entry(id: &str, entry: &SurfaceEntry, target: PresentationTarget) -> Result<(), String> {
    let result = match (entry.role, target) {
        (
            SurfaceRole::Aside,
            PresentationTarget::Aside {
                edge,
                preferred_size,
            },
        ) => show_webview_as_adaptive_panel(
            &entry.webtag,
            &entry.title,
            id,
            panel_position_for(edge, entry.placement.position),
            preferred_panel_size(preferred_size),
        ),
        (SurfaceRole::Aside, PresentationTarget::Stored) => show_webview_as_adaptive_panel(
            &entry.webtag,
            &entry.title,
            id,
            panel_position_for(None, entry.placement.position),
            None,
        ),
        (SurfaceRole::Float, _) => {
            let p = &entry.placement;
            present_webview_as_overlay(
                &entry.webtag,
                p.width,
                p.height,
                p.width_ratio,
                p.height_ratio,
                p.position,
            )
        }
        (SurfaceRole::Main, _) => match entry.kind {
            SurfaceKind::Overlay => present_webview_in_active_group(&entry.webtag),
            SurfaceKind::Window => show_webview_window_with_content_size(
                &entry.webtag,
                &entry.title,
                true,
                window_dimension(entry.placement.width),
                window_dimension(entry.placement.height),
            ),
        },
    };
    result.map_err(|err| err.to_string())
}

fn window_dimension(value: f64) -> Option<i32> {
    (value.is_finite() && value > 0.0).then(|| value.round().clamp(1.0, i32::MAX as f64) as i32)
}

fn panel_position_for(edge: Option<Edge>, fallback_position: u8) -> WindowsPanelPosition {
    match edge {
        Some(Edge::Left) => WindowsPanelPosition::Left,
        Some(Edge::Right) => WindowsPanelPosition::Right,
        Some(Edge::Top) => WindowsPanelPosition::Top,
        Some(Edge::Bottom) => WindowsPanelPosition::Bottom,
        None => match fallback_position {
            2 => WindowsPanelPosition::Left,
            4 => WindowsPanelPosition::Top,
            1 => WindowsPanelPosition::Bottom,
            _ => WindowsPanelPosition::Right,
        },
    }
}

fn preferred_panel_size(value: Option<f64>) -> Option<i32> {
    value
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.round().clamp(1.0, i32::MAX as f64) as i32)
}

fn hide_entry(entry: &SurfaceEntry) {
    let _ = hide_webview_window(&entry.webtag);
    if let Some(page_instance_id) = &entry.page_instance_id {
        notify_page_visibility_when_ready(page_instance_id.clone(), false);
    };
}

pub(super) fn present_surface(
    request: SurfaceRequest,
    product_name: &str,
) -> Result<(), PlatformError> {
    // A page-content surface's webview is created with a per-instance webtag
    // (`{path}#{page_instance_id}`, see lxapp create_page_instance), so the
    // plain path would never match. Url-content surfaces carry no instance id.
    let mut cleanup = None;
    let webtag = if request.content == SurfaceContent::Url {
        if let Some(resolved) = resolve_url_surface(&request) {
            cleanup = resolved.cleanup;
            WebTag::new(&resolved.app_id, &resolved.path, Some(resolved.session_id))
        } else {
            WebTag::new(&request.app_id, &request.path, Some(request.session_id))
        }
    } else if request.page_instance_id.is_empty() {
        WebTag::new(&request.app_id, &request.path, Some(request.session_id))
    } else {
        WebTag::new(
            &request.app_id,
            &format!("{}#{}", request.path, request.page_instance_id),
            Some(request.session_id),
        )
    };
    let id = request.id.clone();
    let kind = request.kind;
    let title = if request.content == SurfaceContent::Url {
        request.path.clone()
    } else {
        product_name.to_string()
    };
    let page_instance_id =
        (!request.page_instance_id.is_empty()).then(|| request.page_instance_id.clone());
    let placement = OverlayPlacement {
        width: finite_or_zero(request.width),
        height: finite_or_zero(request.height),
        width_ratio: finite_or_zero(request.width_ratio),
        height_ratio: finite_or_zero(request.height_ratio),
        position: request.position as u8,
    };
    // A surface page instance has its own WebView parent. `Window` presents
    // that parent as a standalone top-level window; `Overlay` positions it
    // relative to the active app content.
    if let Ok(mut map) = SURFACES.lock() {
        map.insert(
            id.clone(),
            SurfaceEntry {
                webtag: webtag.clone(),
                kind,
                role: request.role,
                title: title.clone(),
                page_instance_id,
                cleanup,
                placement,
            },
        );
    }
    // Asides are placed by the window-global LayoutPresentationPlan. Presenting
    // them immediately here races the later `present_layout` commit and can
    // show the page-instance parent as a standalone window before it is docked.
    // Store the entry now; the layout reconciler is the first presenter.
    if request.role != SurfaceRole::Aside {
        present_surface_when_ready(webtag, id, PresentationTarget::Stored);
    }
    Ok(())
}

pub(super) fn present_layout(
    _window_id: &str,
    plan: &LayoutPresentationPlan,
    _product_name: &str,
) -> Result<(), PlatformError> {
    let known = SURFACES
        .lock()
        .ok()
        .map(|map| map.clone())
        .unwrap_or_default();

    if let Some(active_main_id) = plan.active_main_id.as_deref()
        && let Some(entry) = known.get(active_main_id)
    {
        present_surface_when_ready(
            entry.webtag.clone(),
            active_main_id.to_string(),
            PresentationTarget::Stored,
        );
    }

    let mut planned_asides = HashSet::new();
    for aside in &plan.asides {
        planned_asides.insert(aside.id.clone());
        if let Some(entry) = known.get(&aside.id) {
            present_surface_when_ready(
                entry.webtag.clone(),
                aside.id.clone(),
                PresentationTarget::Aside {
                    edge: aside.edge,
                    preferred_size: aside.preferred_size,
                },
            );
        }
    }

    let planned_floats: HashSet<_> = plan.floats.iter().map(|float| float.id.clone()).collect();
    for float_id in &planned_floats {
        if let Some(entry) = known.get(float_id) {
            present_surface_when_ready(
                entry.webtag.clone(),
                float_id.clone(),
                PresentationTarget::Stored,
            );
        }
    }

    for (id, entry) in known {
        let still_planned = match entry.role {
            SurfaceRole::Aside => planned_asides.contains(&id),
            SurfaceRole::Float => planned_floats.contains(&id),
            SurfaceRole::Main => true,
        };
        if !still_planned {
            hide_entry(&entry);
        }
    }

    Ok(())
}

pub(super) fn close_surface(_app_id: &str, id: &str, reason: &str) -> Result<(), PlatformError> {
    if let Some(entry) = SURFACES.lock().ok().and_then(|mut map| map.remove(id)) {
        teardown_surface(&entry, id, reason);
    } else {
        // The webview is already gone; just resolve the JS handle. The logic
        // notifier is idempotent, so a later user-close no-ops.
        notify_surface_closed(id, reason);
    }
    Ok(())
}

pub(super) fn show_surface(_app_id: &str, id: &str) -> Result<(), PlatformError> {
    let Some(entry) = surface_entry(id) else {
        return Err(PlatformError::InvalidParameter(format!(
            "unknown surface: {id}"
        )));
    };
    if find_webview_handler(&entry.webtag).is_none() {
        present_surface_when_ready(entry.webtag, id.to_string(), PresentationTarget::Stored);
        return Ok(());
    }
    let result = present_entry(id, &entry, PresentationTarget::Stored);
    if result.is_ok()
        && let Some(page_instance_id) = &entry.page_instance_id
    {
        notify_page_visibility_when_ready(page_instance_id.clone(), true);
    }
    result.map_err(|err| PlatformError::Platform(format!("failed to show surface {id}: {err}")))
}

pub(super) fn hide_surface(_app_id: &str, id: &str) -> Result<(), PlatformError> {
    let Some(entry) = surface_entry(id) else {
        return Err(PlatformError::InvalidParameter(format!(
            "unknown surface: {id}"
        )));
    };
    // Both kinds own their host window; hiding it keeps the webview alive so a
    // later show re-presents it.
    hide_entry(&entry);
    Ok(())
}

fn present_surface_when_ready(webtag: WebTag, id: String, target: PresentationTarget) {
    if find_webview_handler(&webtag).is_some() {
        present_surface_with_handler(&webtag, &id, target);
        return;
    }
    // The surface's page-instance webview is created asynchronously; poll for
    // it like the lxapp window show path, bailing if the surface is closed
    // before it appears.
    let _ = thread::Builder::new()
        .name(format!("lingxia-surface-{}", webtag.key()))
        .spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if surface_entry(&id).is_none() {
                    return;
                }
                if find_webview_handler(&webtag).is_some() {
                    present_surface_with_handler(&webtag, &id, target);
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            log::error!("Timed out waiting for surface WebView {}", webtag.key());
        });
}

fn present_surface_with_handler(webtag: &WebTag, id: &str, target: PresentationTarget) {
    let id_for_close = id.to_string();
    set_webview_close_handler(
        webtag,
        // The native close button (or the WM_CLOSE that the OS frame sends)
        // must actually close the surface: the WM_CLOSE handler suppresses the
        // default DestroyWindow once a close handler is registered, so we tear
        // the surface down ourselves (fire onClose + dispose the page, which
        // destroys the webview and its window).
        Arc::new(move || {
            if let Some(entry) = SURFACES
                .lock()
                .ok()
                .and_then(|mut map| map.remove(&id_for_close))
            {
                teardown_surface(&entry, &id_for_close, "user");
            } else {
                notify_surface_closed(&id_for_close, "user");
            }
        }),
    );
    let Some(entry) = surface_entry(id) else {
        return;
    };
    if let Err(err) = present_entry(id, &entry, target) {
        log::warn!("Failed to present surface {}: {}", webtag.key(), err);
        return;
    }
    // Mark the surface page visible: cancels the page-instance dispose timer
    // (which would otherwise reclaim the surface and close its window) and
    // fires the page's onShow.
    if let Some(page_instance_id) = surface_entry(id).and_then(|entry| entry.page_instance_id) {
        notify_page_visibility_when_ready(page_instance_id, true);
    }
}
