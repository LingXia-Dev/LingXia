use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lingxia_webview::WebTag;
use lingxia_webview::platform::windows::{WindowsWebViewHandler, find_webview_handler};
use lingxia_webview::runtime as webview_runtime;
use lingxia_windows_host::{
    clear_webview_group_override, clear_webview_os_frame, hide_webview_window,
    navigate_webview_window, present_webview_as_overlay, set_webview_close_handler,
    set_webview_group_override, set_webview_os_frame, show_webview_as_panel, show_webview_window,
};

use super::request_windows_app_exit;
use crate::error::PlatformError;
use crate::traits::app_runtime::LxAppOpenMode;
use crate::traits::ui::{SurfaceKind, SurfaceRequest};

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

/// Tears down a surface: clears window-mode flags, fires onClose, and disposes
/// the content page instance (which closes the window/overlay). Used by both
/// programmatic close and the native close button.
fn teardown_surface(entry: &SurfaceEntry, id: &str, reason: &str) {
    // Clear the window-mode marks; each clear is a no-op when unset, so this is
    // safe regardless of kind. (An overlay's child attachment + placement are
    // cleaned up by the webview when its window is destroyed.)
    clear_webview_group_override(&entry.webtag);
    clear_webview_os_frame(&entry.webtag);
    notify_surface_closed(id, reason);
    match &entry.page_instance_id {
        // Disposing the page instance detaches + destroys its webview (closing
        // the window; a presented overlay is restored by cleanup_window_state).
        Some(page_instance_id) => dispose_surface_page(page_instance_id, reason),
        // A Url-content surface has no page instance; destroy its webview.
        None => webview_runtime::destroy_webview(&entry.webtag),
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
    title: String,
    page_instance_id: Option<String>,
    placement: OverlayPlacement,
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

/// Shows a surface's webview per its kind: a standalone window for `window`,
/// a floating top-most popup for `overlay`.
fn present_for_kind(_handler: &WindowsWebViewHandler, entry: &SurfaceEntry) -> Result<(), String> {
    let result = match entry.kind {
        SurfaceKind::Window => show_webview_window(&entry.webtag, &entry.title, true),
        SurfaceKind::Overlay => {
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
    };
    result.map_err(|err| err.to_string())
}

pub(super) fn present_surface(
    request: SurfaceRequest,
    product_name: &str,
) -> Result<(), PlatformError> {
    // A page-content surface's webview is created with a per-instance webtag
    // (`{path}#{page_instance_id}`, see lxapp create_page_instance), so the
    // plain path would never match. Url-content surfaces carry no instance id.
    let webtag = if request.page_instance_id.is_empty() {
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
    let title = product_name.to_string();
    let page_instance_id =
        (!request.page_instance_id.is_empty()).then(|| request.page_instance_id.clone());
    let placement = OverlayPlacement {
        width: finite_or_zero(request.width),
        height: finite_or_zero(request.height),
        width_ratio: finite_or_zero(request.width_ratio),
        height_ratio: finite_or_zero(request.height_ratio),
        position: request.position as u8,
    };
    // A `window` surface gets its own host group so it opens as a separate
    // top-level window with native controls. An `overlay` joins the app's main
    // group and is layered over its content as a child card.
    if matches!(kind, SurfaceKind::Window) {
        set_webview_group_override(&webtag, &format!("surface:{id}"));
        set_webview_os_frame(&webtag);
    }
    if let Ok(mut map) = SURFACES.lock() {
        map.insert(
            id.clone(),
            SurfaceEntry {
                webtag: webtag.clone(),
                kind,
                title: title.clone(),
                page_instance_id,
                placement,
            },
        );
    }
    present_surface_when_ready(webtag, id);
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
    let Some(handler) = find_webview_handler(&entry.webtag) else {
        return Ok(());
    };
    let result = present_for_kind(&handler, &entry);
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
    let _ = hide_webview_window(&entry.webtag);
    // Mark hidden so the page fires onHide and the dispose timer can reclaim it
    // after the hidden TTL.
    if let Some(page_instance_id) = &entry.page_instance_id {
        notify_page_visibility_when_ready(page_instance_id.clone(), false);
    }
    Ok(())
}

fn present_surface_when_ready(webtag: WebTag, id: String) {
    if let Some(handler) = find_webview_handler(&webtag) {
        present_surface_with_handler(handler, &webtag, &id);
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
                if let Some(handler) = find_webview_handler(&webtag) {
                    present_surface_with_handler(handler, &webtag, &id);
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            log::error!("Timed out waiting for surface WebView {}", webtag.key());
        });
}

fn present_surface_with_handler(handler: WindowsWebViewHandler, webtag: &WebTag, id: &str) {
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
    if let Err(err) = present_for_kind(&handler, &entry) {
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
