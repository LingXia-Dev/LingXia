//! Embedded native components for Windows - Input / Textarea / Video / MediaSwiper.
//!
//! The page view mounts `<lx-input>` / `<lx-textarea>` / `<lx-video>` /
//! `<lx-media-swiper>` over the webview by sending component messages
//! (`component.mount` / `component.update` / `component.unmount`, plus
//! `component.ready` and `page.scroll`) through the native-component bridge
//! channel. `lingxia-lxapp` routes those messages to the host registered by
//! [`install`]. This module owns component policy: it places a borderless
//! Win32 child over the webview content at the reported coverage rect, keeps
//! it aligned while the page scrolls or relays out, and emits component events
//! back to the page view and page-function bindings.
//!
//! Mirrors the manager contract of
//! `lingxia-sdk/apple/Sources/macOS/NativeComponents/MacNativeComponentManager.swift`:
//! a per-page component registry keyed by component id, document-space rects
//! converted to viewport space with a natively tracked scroll offset, a ready
//! handshake that queues events until the view handler is registered, and
//! graceful no-ops (log only) for component kinds this phase does not support
//! (picker is deferred).
//!
//! Threading: every Win32 mutation runs on the UI thread that owns the webview
//! window. Messages already arrive on that thread (WebView2
//! `WebMessageReceived`); calls from other threads are marshalled with
//! `crate::window_host::post_to_window_thread`. The state registry is guarded
//! by a mutex that is never held across Win32 calls that can re-enter the
//! window procedures, such as `SetWindowTextW` causing `EN_CHANGE`.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use crate::window_host::{
    WindowsWebViewContentWindow, find_webview_content_window, post_to_window_thread,
};
use lingxia_platform::traits::video_player::VideoPlayerCommand;
use lingxia_webview::WebViewController;
use serde_json::{Value, json};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CombineRgn, CreateFontW,
    CreateRectRgn, CreateRoundRectRgn, DEFAULT_CHARSET, DEFAULT_PITCH, DeleteObject, FF_SWISS,
    GetMonitorInfoW, GetStockObject, HBRUSH, HDC, HGDIOBJ, InvalidateRect,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, OUT_DEFAULT_PRECIS, RGN_AND,
    SetBkColor, SetTextColor, SetWindowRgn, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    self, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_PASSWORD, ES_WANTRETURN, GW_CHILD,
    WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW, WNDPROC,
};
use windows::core::{PCWSTR, w};

use super::video_controls::{ControlsAction, ControlsState, VideoControls};
use super::video_player::{VideoEventSink, VideoPlayer, VideoPlayerEvent};

mod model;
mod swiper;
mod text;
mod video;

use model::*;
use swiper::*;
use text::*;
use video::*;

/// Events queued per component until the view sends `component.ready`.
const MAX_PENDING_EVENTS: usize = 8;

/// Registers this module as the process-wide native-component host and as
/// the video-command dispatcher of the platform layer.
/// Called from the shell `register_runtime()` path (`windows::install`).
pub(crate) fn install() {
    if !lxapp::register_native_component_host(Arc::new(ShellNativeComponentHost)) {
        log::warn!("a native-component host was already registered; Windows manager inactive");
    }
    lingxia_platform::register_windows_video_command_dispatcher(Arc::new(dispatch_video_command));
    super::media_preview::install();
}
struct ShellNativeComponentHost;

impl lxapp::NativeComponentHost for ShellNativeComponentHost {
    fn on_page_visibility(&self, page_key: &str, visible: bool) {
        set_page_components_visible(page_key, visible);
    }

    fn on_component_message(&self, page: &lxapp::PageInstance, message_json: &str) {
        let message: Value = match serde_json::from_str(message_json) {
            Ok(message) => message,
            Err(err) => {
                log::warn!("invalid native-component message: {err}");
                return;
            }
        };
        let webtag = page.webtag();
        let context = PageContext {
            page_key: webtag.key().to_string(),
            appid: page.appid(),
            path: page.path(),
        };
        let target = find_webview_content_window(&webtag);
        handle_message(context, target, &message);
    }

    fn on_page_destroyed(&self, page_key: &str) {
        teardown_page(page_key);
    }
}

#[derive(Clone)]
struct PageContext {
    page_key: String,
    appid: String,
    path: String,
}

struct ComponentEntry {
    context: PageContext,
    component_id: String,
    multiline: bool,
    parent: isize,
    container: isize,
    /// `0` for components without an EDIT control (video).
    edit: isize,
    font: isize,
    /// Playback engine of a `video.native` component, `None` for text.
    /// `Arc` so command/timer paths can call it after dropping the
    /// registry lock.
    video: Option<VideoComponent>,
    /// Paged carousel of a `media-swiper.native` component, `None` otherwise.
    swiper: Option<MediaSwiperComponent>,
    doc_rect: DocRect,
    state: ComponentProps,
    last_value: String,
    ready: bool,
    pending: Vec<(String, Value)>,
}

/// Per-page view state: latest scroll offset (CSS px) and content-window
/// geometry, refreshed on every message that carries a target.
#[derive(Clone, Copy)]
struct PageView {
    scroll_x: f64,
    scroll_y: f64,
    target: WindowsWebViewContentWindow,
}

/// A page's overlays show only while it is its app's current page.
/// Derived live from the page stack on every layout pass. A cached flag
/// wedges as soon as any navigation path forgets to dispatch a lifecycle
/// event (the pause/resume hooks remain event-driven).
fn page_is_foreground(context: &PageContext) -> bool {
    lxapp::try_get(&context.appid)
        .and_then(|app| app.peek_current_page())
        .is_some_and(|current| current == context.path)
}

static COMPONENTS: OnceLock<Mutex<HashMap<String, ComponentEntry>>> = OnceLock::new();
static CONTAINERS: OnceLock<Mutex<HashMap<isize, String>>> = OnceLock::new();
static PAGE_VIEWS: OnceLock<Mutex<HashMap<String, PageView>>> = OnceLock::new();
/// Component keys whose view handler announced `component.ready` (the
/// handshake may arrive before `component.mount`).
static READY_KEYS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
/// EDIT handles whose `EN_CHANGE` stems from a programmatic value sync;
/// those changes update the cache without echoing an `input` event.
static SUPPRESSED_EDITS: OnceLock<Mutex<HashSet<isize>>> = OnceLock::new();

fn components() -> std::sync::MutexGuard<'static, HashMap<String, ComponentEntry>> {
    lock_registry(COMPONENTS.get_or_init(|| Mutex::new(HashMap::new())))
}

fn containers() -> std::sync::MutexGuard<'static, HashMap<isize, String>> {
    lock_registry(CONTAINERS.get_or_init(|| Mutex::new(HashMap::new())))
}

fn page_views() -> std::sync::MutexGuard<'static, HashMap<String, PageView>> {
    lock_registry(PAGE_VIEWS.get_or_init(|| Mutex::new(HashMap::new())))
}

fn ready_keys() -> std::sync::MutexGuard<'static, HashSet<String>> {
    lock_registry(READY_KEYS.get_or_init(|| Mutex::new(HashSet::new())))
}

fn suppressed_edits() -> std::sync::MutexGuard<'static, HashSet<isize>> {
    lock_registry(SUPPRESSED_EDITS.get_or_init(|| Mutex::new(HashSet::new())))
}

fn lock_registry<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // The registries hold no invariants that poisoning can break.
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn component_key(page_key: &str, component_id: &str) -> String {
    format!("{page_key}\u{1}{component_id}")
}

fn resolve_native_media_source(appid: &str, src: &str) -> Option<String> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    if let Some(stripped) = trimmed.strip_prefix("file://") {
        return Some(stripped.to_string());
    }
    if let Some(app) = lxapp::try_get(appid)
        && let Ok(path) = app.resolve_accessible_path(trimmed)
    {
        return Some(path.to_string_lossy().into_owned());
    }
    Some(trimmed.to_string())
}

// ---------------------------------------------------------------------------
// Message dispatch
// ---------------------------------------------------------------------------

fn handle_message(
    context: PageContext,
    target: Option<WindowsWebViewContentWindow>,
    message: &Value,
) {
    let Some(action) = message.get("action").and_then(Value::as_str) else {
        log::debug!("native-component message without action; ignoring");
        return;
    };

    if let Some(target) = target {
        let mut views = page_views();
        let view = views.entry(context.page_key.clone()).or_insert(PageView {
            scroll_x: 0.0,
            scroll_y: 0.0,
            target,
        });
        view.target = target;
    }

    match action {
        "component.mount" => handle_mount(&context, message),
        "component.update" => handle_update(&context, message),
        "component.unmount" => handle_unmount(&context, message),
        "component.ready" => handle_ready(&context, message),
        "component.command" => handle_command(&context, message),
        "component.focus" => handle_focus_action(&context, message, true),
        "component.blur" => handle_focus_action(&context, message, false),
        "page.scroll" => handle_page_scroll(&context, message),
        other => {
            // Unknown actions (e.g. component kinds outside this phase) must
            // never break the page; they are logged and dropped.
            log::debug!("unsupported native-component action '{other}'; ignoring");
        }
    }
}

fn message_component_id(message: &Value) -> Option<String> {
    message
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// Kinds of components this host can mount.
#[derive(Clone, Copy)]
enum MountKind {
    Edit { multiline: bool },
    Video,
}

fn handle_mount(context: &PageContext, message: &Value) {
    let Some(component_id) = message_component_id(message) else {
        return;
    };
    let kind = match message.get("type").and_then(Value::as_str) {
        Some("input.native") => MountKind::Edit { multiline: false },
        Some("textarea.native") => MountKind::Edit { multiline: true },
        Some("video.native") => MountKind::Video,
        Some("media-swiper.native") => {
            handle_swiper_mount(context, message, component_id);
            return;
        }
        Some(other) => {
            log::info!("native component type '{other}' is not supported on Windows yet; ignoring");
            return;
        }
        None => return,
    };
    let Some(doc_rect) = parse_rect(message.get("rect")) else {
        log::warn!("component.mount without rect for {component_id}; ignoring");
        return;
    };
    let mut props = parse_props(message.get("props"));
    if props.corner_radius.is_none() {
        props.corner_radius = corner_radius_value(message.get("cornerRadius"));
    }

    let Some(parent) = parent_window_for_page(&context.page_key) else {
        log::warn!(
            "no webview window for page {}; dropping mount of {component_id}",
            context.page_key
        );
        return;
    };

    let context = context.clone();
    run_on_window_thread(parent, move || {
        mount_on_ui(context, component_id, kind, parent, doc_rect, props);
    });
}

fn handle_swiper_mount(context: &PageContext, message: &Value, component_id: String) {
    let Some(doc_rect) = parse_rect(message.get("rect")) else {
        log::warn!("media-swiper mount without rect for {component_id}; ignoring");
        return;
    };
    let config = SwiperConfig::parse(
        message.get("props").unwrap_or(&Value::Null),
        &SwiperConfig::default(),
    );
    let corner = corner_radius_value(message.get("cornerRadius"));
    let Some(parent) = parent_window_for_page(&context.page_key) else {
        log::warn!(
            "no webview window for page {}; dropping mount of {component_id}",
            context.page_key
        );
        return;
    };
    let context = context.clone();
    run_on_window_thread(parent, move || {
        mount_swiper_on_ui(context, component_id, parent, doc_rect, config, corner);
    });
}

fn handle_command(context: &PageContext, message: &Value) {
    let Some(component_id) = message_component_id(message) else {
        return;
    };
    let key = component_key(&context.page_key, &component_id);
    let Some(parent) = components().get(&key).map(|entry| entry.parent) else {
        return;
    };
    let name = message
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let params = message.get("params").cloned();
    run_on_window_thread(parent, move || {
        handle_swiper_command(&key, &name, params.as_ref());
    });
}

fn handle_update(context: &PageContext, message: &Value) {
    let Some(component_id) = message_component_id(message) else {
        return;
    };
    let key = component_key(&context.page_key, &component_id);

    if components()
        .get(&key)
        .is_some_and(|entry| entry.swiper.is_some())
    {
        let doc_rect = parse_rect(message.get("rect"));
        let props = message.get("props").cloned();
        let corner = corner_radius_value(message.get("cornerRadius"));
        let Some(parent) = components().get(&key).map(|entry| entry.parent) else {
            return;
        };
        run_on_window_thread(parent, move || {
            apply_swiper_update(&key, doc_rect, props, corner);
        });
        return;
    }

    let doc_rect = parse_rect(message.get("rect"));
    let mut props = message.get("props").map(|raw| parse_props(Some(raw)));
    // Geometry-only updates carry the measured radius at the top level.
    if let Some(radius) = corner_radius_value(message.get("cornerRadius")) {
        let merged = props.get_or_insert_with(ComponentProps::default);
        merged.corner_radius.get_or_insert(radius);
    }

    let Some(parent) = components().get(&key).map(|entry| entry.parent) else {
        log::debug!("component.update for unknown component {component_id}; ignoring");
        return;
    };

    run_on_window_thread(parent, move || {
        {
            let mut components = components();
            let Some(entry) = components.get_mut(&key) else {
                return;
            };
            if let Some(rect) = doc_rect {
                entry.doc_rect = rect;
            }
        }
        if let Some(props) = props {
            apply_props(&key, &props);
        }
        apply_layout(&key);
    });
}

fn handle_unmount(context: &PageContext, message: &Value) {
    let Some(component_id) = message_component_id(message) else {
        return;
    };
    let key = component_key(&context.page_key, &component_id);
    ready_keys().remove(&key);
    let Some(parent) = components().get(&key).map(|entry| entry.parent) else {
        return;
    };
    run_on_window_thread(parent, move || {
        destroy_component(&key);
    });
}

fn handle_ready(context: &PageContext, message: &Value) {
    let Some(component_id) = message_component_id(message) else {
        return;
    };
    let key = component_key(&context.page_key, &component_id);
    ready_keys().insert(key.clone());

    let pending = {
        let mut components = components();
        let Some(entry) = components.get_mut(&key) else {
            return;
        };
        entry.ready = true;
        std::mem::take(&mut entry.pending)
    };
    for (event, detail) in pending {
        emit_event(&key, &event, detail);
    }
}

fn handle_focus_action(context: &PageContext, message: &Value, focus: bool) {
    let Some(component_id) = message_component_id(message) else {
        return;
    };
    let key = component_key(&context.page_key, &component_id);
    let Some(parent) = components().get(&key).map(|entry| entry.parent) else {
        return;
    };
    run_on_window_thread(parent, move || {
        let (edit, parent) = {
            let components = components();
            let Some(entry) = components.get(&key) else {
                return;
            };
            (entry.edit, entry.parent)
        };
        if edit == 0 {
            // Components without an EDIT control (video) do not take focus.
            return;
        }
        set_edit_focus_with_parent(edit, parent, focus);
    });
}

fn handle_page_scroll(context: &PageContext, message: &Value) {
    let x = message.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let y = message.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    {
        let mut views = page_views();
        let Some(view) = views.get_mut(&context.page_key) else {
            return;
        };
        if view.scroll_x == x && view.scroll_y == y {
            return;
        }
        view.scroll_x = x;
        view.scroll_y = y;
    }

    let Some(parent) = parent_window_for_page(&context.page_key) else {
        return;
    };
    let page_key = context.page_key.clone();
    run_on_window_thread(parent, move || {
        let keys: Vec<String> = components()
            .iter()
            .filter(|(_, entry)| entry.context.page_key == page_key)
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            apply_layout(&key);
        }
    });
}

/// Hides or re-shows every component of `page_key` when it leaves or
/// re-enters the foreground (page navigation), mirroring the macOS
/// manager's inactive/active handling: hiding pauses a playing video
/// immediately and remembers to resume it when the page returns.
fn set_page_components_visible(page_key: &str, visible: bool) {
    let targets: Vec<(String, isize)> = components()
        .iter()
        .filter(|(_, entry)| entry.context.page_key == page_key)
        .map(|(key, entry)| (key.clone(), entry.parent))
        .collect();
    for (key, parent) in targets {
        run_on_window_thread(parent, move || apply_component_visibility(&key, visible));
    }
}

/// Applies one component's page-visibility change on its UI thread.
fn apply_component_visibility(key: &str, visible: bool) {
    if components()
        .get(key)
        .is_some_and(|entry| entry.swiper.is_some())
    {
        if visible {
            apply_layout(key);
            swiper_on_visible(key, true);
        } else {
            swiper_on_visible(key, false);
            if let Some(container) = components().get(key).map(|entry| entry.container) {
                unsafe {
                    let _ = WindowsAndMessaging::ShowWindow(
                        HWND(container as *mut _),
                        WindowsAndMessaging::SW_HIDE,
                    );
                }
            }
        }
        return;
    }

    if !visible {
        // A fullscreen video must leave its screen-sized window before the
        // page hides, or the black host would stay covering the monitor.
        let fullscreen = {
            let components = components();
            components
                .get(key)
                .and_then(|entry| entry.video.as_ref())
                .is_some_and(|video| video.fullscreen)
        };
        if fullscreen {
            set_video_fullscreen(key, false);
        }
    }
    let (container, edit, parent, video) = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        let video = entry.video.as_mut().map(|video| {
            if visible {
                (
                    video.player.clone(),
                    std::mem::take(&mut video.resume_on_show),
                )
            } else {
                video.resume_on_show = video.playing;
                (video.player.clone(), false)
            }
        });
        (entry.container, entry.edit, entry.parent, video)
    };

    if visible {
        // Re-position and re-show through the normal layout pass, then
        // resume a video the hide had paused.
        apply_layout(key);
        if let Some((player, resume)) = video {
            if resume {
                player.play();
            }
        }
        return;
    }

    if edit != 0 {
        set_edit_focus_with_parent(edit, parent, false);
    }
    if let Some((player, _)) = video {
        player.pause();
    }
    unsafe {
        let _ = WindowsAndMessaging::ShowWindow(
            HWND(container as *mut _),
            WindowsAndMessaging::SW_HIDE,
        );
    }
}

/// Destroys every component mounted by `page_key` and drops its view state.
fn teardown_page(page_key: &str) {
    page_views().remove(page_key);
    {
        let mut ready = ready_keys();
        ready.retain(|key| {
            !key.starts_with(page_key) || !key[page_key.len()..].starts_with('\u{1}')
        });
    }
    let targets: Vec<(String, isize)> = components()
        .iter()
        .filter(|(_, entry)| entry.context.page_key == page_key)
        .map(|(key, entry)| (key.clone(), entry.parent))
        .collect();
    for (key, parent) in targets {
        let posted = is_window(parent) && {
            let key = key.clone();
            run_on_window_thread(parent, move || destroy_component(&key))
        };
        if !posted {
            // Window (and its children) are already gone; purge bookkeeping.
            purge_component_state(&key);
        }
    }
}

fn parent_window_for_page(page_key: &str) -> Option<isize> {
    page_views()
        .get(page_key)
        .map(|view| view.target.window)
        .filter(|window| is_window(*window))
}

/// Runs `callback` on the UI thread that owns `window`: directly when the
/// caller is already on it (component messages arrive on the webview UI
/// thread), otherwise marshalled through the window's message queue.
/// Returns `false` when the window is gone and the callback was dropped.
fn run_on_window_thread(window: isize, callback: impl FnOnce() + Send + 'static) -> bool {
    let hwnd = HWND(window as *mut _);
    let owner = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(hwnd, None) };
    if owner != 0 && owner == unsafe { GetCurrentThreadId() } {
        callback();
        return true;
    }
    post_to_window_thread(window, Box::new(callback))
}

fn is_window(window: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(HWND(window as *mut _))).as_bool() }
}

// ---------------------------------------------------------------------------
// Window creation / layout / props (UI thread only)
// ---------------------------------------------------------------------------

fn mount_on_ui(
    context: PageContext,
    component_id: String,
    kind: MountKind,
    parent: isize,
    doc_rect: DocRect,
    props: ComponentProps,
) {
    let key = component_key(&context.page_key, &component_id);
    // Remount of a live id replaces the previous control.
    destroy_component(&key);

    if !is_window(parent) {
        return;
    }
    let Some(container) = create_container(parent, &component_id) else {
        return;
    };

    match kind {
        MountKind::Edit { multiline } => mount_edit_on_ui(
            context,
            component_id,
            multiline,
            parent,
            container,
            doc_rect,
            props,
        ),
        MountKind::Video => {
            mount_video_on_ui(context, component_id, parent, container, doc_rect, props)
        }
    }
}

/// Creates the (hidden) component container as a child of the webview
/// window; `apply_layout` positions and shows it.
fn create_container(parent: isize, component_id: &str) -> Option<HWND> {
    let container_style = WINDOW_STYLE(
        WindowsAndMessaging::WS_CHILD.0
            | WindowsAndMessaging::WS_CLIPCHILDREN.0
            | WindowsAndMessaging::WS_CLIPSIBLINGS.0,
    );
    let container = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            container_class(),
            PCWSTR::null(),
            container_style,
            0,
            0,
            16,
            16,
            Some(HWND(parent as *mut _)),
            None,
            GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    match container {
        Ok(container) => Some(container),
        Err(_) => {
            log::warn!("failed to create native-component container for {component_id}");
            None
        }
    }
}

/// Registers (once) and returns the container window class. The container
/// receives the EDIT's `WM_COMMAND` notifications and colors its text.
fn container_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            // Double clicks toggle video fullscreen.
            style: WindowsAndMessaging::CS_DBLCLKS,
            lpfnWndProc: Some(container_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaNativeComponentHost"),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as *mut _),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaNativeComponentHost")
}

/// Repositions a component's container over the webview content from its
/// document rect, the page scroll offset, and the content-window geometry;
/// clips it to the content area (chrome regions stay clean) and to the
/// element's measured corner radius, and keeps it above the WebView2 child
/// windows.
fn apply_layout(key: &str) {
    let (container, edit, doc_rect, context, font_size, multiline, corner_radius, video, is_swiper) = {
        let components = components();
        let Some(entry) = components.get(key) else {
            return;
        };
        (
            entry.container,
            entry.edit,
            entry.doc_rect,
            entry.context.clone(),
            entry.state.font_size.unwrap_or(DEFAULT_FONT_SIZE),
            entry.multiline,
            entry.state.corner_radius.unwrap_or(0.0),
            entry.video.as_ref().map(|video| VideoLayout {
                player: video.player.clone(),
                surface: video.surface,
                stopped: video.stopped,
                fullscreen_host: if video.fullscreen {
                    video.fullscreen_host
                } else {
                    0
                },
                controls: video.controls.as_ref().map(|controls| controls.hwnd),
            }),
            entry.swiper.is_some(),
        )
    };
    let Some(view) = page_views().get(&context.page_key).copied() else {
        return;
    };
    let container_hwnd = HWND(container as *mut _);
    if !page_is_foreground(&context) || video.as_ref().is_some_and(|video| video.stopped) {
        // Background pages keep their overlays hidden; a stopped video
        // hides too, letting the element's DOM placeholder/poster show.
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(container_hwnd, WindowsAndMessaging::SW_HIDE);
        }
        return;
    }

    // Fullscreen: the container fills its dedicated screen-sized host
    // window instead of tracking the document rect.
    if let Some(video) = video
        .as_ref()
        .filter(|video| video.fullscreen_host != 0 && is_window(video.fullscreen_host))
    {
        let host = HWND(video.fullscreen_host as *mut _);
        let mut rect = RECT::default();
        unsafe {
            let _ = WindowsAndMessaging::GetClientRect(host, &mut rect);
            let _ = WindowsAndMessaging::MoveWindow(
                container_hwnd,
                0,
                0,
                rect.right,
                rect.bottom,
                true,
            );
            SetWindowRgn(container_hwnd, None, true);
            let _ = WindowsAndMessaging::ShowWindow(container_hwnd, WindowsAndMessaging::SW_SHOWNA);
        }
        video.layout_children(rect.right, rect.bottom);
        return;
    }

    let target = view.target;
    let scale = if target.scale > 0.0 {
        target.scale
    } else {
        1.0
    };

    let (x, y, width, height) = (
        ((doc_rect.x - view.scroll_x) * scale).round() as i32 + target.content_left,
        ((doc_rect.y - view.scroll_y) * scale).round() as i32 + target.content_top,
        (doc_rect.width * scale).round().max(0.0) as i32,
        (doc_rect.height * scale).round().max(0.0) as i32,
    );

    let content_right = target.content_left + target.content_width;
    let content_bottom = target.content_top + target.content_height;
    let visible_left = x.max(target.content_left);
    let visible_top = y.max(target.content_top);
    let visible_right = (x + width).min(content_right);
    let visible_bottom = (y + height).min(content_bottom);

    if width <= 0 || height <= 0 || visible_right <= visible_left || visible_bottom <= visible_top {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(container_hwnd, WindowsAndMessaging::SW_HIDE);
        }
        return;
    }

    unsafe {
        let _ = WindowsAndMessaging::MoveWindow(container_hwnd, x, y, width, height, true);
        // Clip to the content area when the component pokes out of it
        // (scrolled partially under the chrome), and round the corners to
        // the element's measured border-radius. Both clips compose into
        // one region.
        let radius = (corner_radius * scale).round() as i32;
        let fully_visible = visible_left == x
            && visible_top == y
            && visible_right == x + width
            && visible_bottom == y + height;
        if fully_visible && radius <= 0 {
            SetWindowRgn(container_hwnd, None, true);
        } else {
            let region = if radius > 0 {
                // End coordinates are exclusive, hence the +1.
                CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2)
            } else {
                CreateRectRgn(0, 0, width, height)
            };
            if !fully_visible {
                let clip = CreateRectRgn(
                    visible_left - x,
                    visible_top - y,
                    visible_right - x,
                    visible_bottom - y,
                );
                let _ = CombineRgn(Some(region), Some(region), Some(clip), RGN_AND);
                let _ = DeleteObject(HGDIOBJ(clip.0));
            }
            // The system owns the region after SetWindowRgn succeeds.
            if SetWindowRgn(container_hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(HGDIOBJ(region.0));
            }
        }
        let _ = WindowsAndMessaging::ShowWindow(container_hwnd, WindowsAndMessaging::SW_SHOWNA);
        let _ = WindowsAndMessaging::SetWindowPos(
            container_hwnd,
            Some(WindowsAndMessaging::HWND_TOP),
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER,
        );

        if is_swiper {
            layout_swiper_children(key, width, height);
            return;
        }

        // The video surface fills the container; MFPlay just needs a
        // repaint nudge after the window moved or resized.
        if let Some(video) = video {
            video.layout_children(width, height);
            return;
        }

        // Lay the EDIT out inside the container: multiline fills it (with a
        // small inset); single-line is vertically centered on its font.
        let pad_x = (EDIT_PADDING_X * scale).round() as i32;
        let edit_hwnd = HWND(edit as *mut _);
        if multiline {
            let pad_y = (EDIT_PADDING_Y * scale).round() as i32;
            let _ = WindowsAndMessaging::MoveWindow(
                edit_hwnd,
                pad_x,
                pad_y,
                (width - pad_x * 2).max(8),
                (height - pad_y * 2).max(8),
                true,
            );
        } else {
            let font_px = (font_size * scale).round() as i32;
            let edit_height = ((font_px as f64) * 1.45).round() as i32;
            let edit_height = edit_height.clamp(8, height.max(8));
            let edit_y = ((height - edit_height) / 2).max(0);
            let _ = WindowsAndMessaging::MoveWindow(
                edit_hwnd,
                pad_x,
                edit_y,
                (width - pad_x * 2).max(8),
                edit_height,
                true,
            );
        }
    }
}

/// Merges `props` into a component's stored state and applies the visible
/// changes to its EDIT control or video player. Runs on the owning UI
/// thread.
fn apply_props(key: &str, props: &ComponentProps) {
    let is_video = {
        let components = components();
        components
            .get(key)
            .is_some_and(|entry| entry.video.is_some())
    };
    if is_video {
        apply_video_props(key, props);
        return;
    }
    apply_edit_props(key, props);
}

// ---------------------------------------------------------------------------
// Video components (UI thread only)
// ---------------------------------------------------------------------------

/// Destroys a component's windows and removes all its bookkeeping. Runs on
/// the owning UI thread (window destruction requirement).
fn destroy_component(key: &str) {
    let Some((container, font, fullscreen_host)) = ({
        let components = components();
        components.get(key).map(|entry| {
            (
                entry.container,
                entry.font,
                entry
                    .video
                    .as_ref()
                    .map(|video| video.fullscreen_host)
                    .unwrap_or(0),
            )
        })
    }) else {
        return;
    };
    unsafe {
        // Destroys the EDIT/surface child too; WM_NCDESTROY unsubclasses
        // it and the container removes itself from the lookup map. A
        // fullscreen host owns the container while fullscreen, so
        // destroying it tears both down.
        if fullscreen_host != 0 && is_window(fullscreen_host) {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(fullscreen_host as *mut _));
        } else {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(container as *mut _));
        }
        if font != 0 {
            let _ = DeleteObject(HGDIOBJ(font as *mut _));
        }
    }
    purge_component_state(key);
}

fn purge_component_state(key: &str) {
    let entry = components().remove(key);
    if let Some(entry) = entry {
        containers().remove(&entry.container);
        suppressed_edits().remove(&entry.edit);
    }
    ready_keys().remove(key);
}

// ---------------------------------------------------------------------------
// Events back to the page
// ---------------------------------------------------------------------------

/// Emits a component event to the page: queued until the view announces
/// `component.ready`, then delivered to the view's component handler and,
/// when the component declared page-function bindings, to the page's
/// logic service through `lxapp::on_native_component_event`.
fn emit_event(key: &str, event: &str, detail: Value) {
    let snapshot = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        if !entry.ready {
            if entry.pending.len() >= MAX_PENDING_EVENTS {
                entry.pending.remove(0);
            }
            entry.pending.push((event.to_string(), detail));
            return;
        }
        (
            entry.context.clone(),
            entry.component_id.clone(),
            entry.state.bindings_json.clone(),
            entry.state.dataset_json.clone(),
        )
    };
    let (context, component_id, bindings_json, dataset_json) = snapshot;
    let event = event.to_string();

    // Hop off the UI thread: posting to the view dispatches a synchronous
    // command to this very webview's UI thread, which would self-deadlock.
    let spawned = std::thread::Builder::new()
        .name(format!("lingxia-nc-event-{}", component_id))
        .spawn(move || {
            deliver_event(
                &context,
                &component_id,
                &event,
                detail,
                bindings_json,
                dataset_json,
            );
        });
    if let Err(err) = spawned {
        log::warn!("failed to spawn native-component event thread: {err}");
    }
}

fn deliver_event(
    context: &PageContext,
    component_id: &str,
    event: &str,
    detail: Value,
    bindings_json: Option<String>,
    dataset_json: Option<String>,
) {
    let payload = json!({
        "action": "component.event",
        "id": component_id,
        "componentId": component_id,
        "event": event,
        "detail": detail,
        "pageId": format!("{}:{}", context.appid, context.path),
    });

    // 1) The view's registered component handler (drives the DOM events the
    //    page sees: input/focus/blur/confirm).
    let view_message = json!({
        "type": "event",
        "name": "nativecomponent",
        "payload": payload,
    })
    .to_string();
    let page = lxapp::try_get(&context.appid).and_then(|app| app.get_page(&context.path));
    if let Some(page) = page.as_ref()
        && let Some(webview) = page.webview()
        && let Err(err) = webview.post_message(&view_message)
    {
        log::debug!("failed to post native-component event to view: {err}");
    }

    // 2) Page-function bindings (lx-input page-bindings attribute), same
    //    enriched event shape the macOS manager builds.
    let Some(bindings_json) = bindings_json else {
        return;
    };
    let dataset: Value = dataset_json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_else(|| json!({}));
    let target = json!({ "id": component_id, "dataset": dataset });
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0);
    let page_event = json!({
        "type": event,
        "detail": payload.get("detail").cloned().unwrap_or_else(|| json!({})),
        "target": target,
        "currentTarget": target,
        "timeStamp": timestamp,
    })
    .to_string();
    let _ = lxapp::on_native_component_event(
        &context.appid,
        &context.path,
        component_id,
        event,
        &page_event,
        &bindings_json,
    );
}

// ---------------------------------------------------------------------------
// Window procedures (UI thread)
// ---------------------------------------------------------------------------

fn component_key_for_container(container: HWND) -> Option<String> {
    containers().get(&(container.0 as isize)).cloned()
}

fn container_is_video(container: HWND) -> bool {
    let Some(key) = component_key_for_container(container) else {
        return false;
    };
    let components = components();
    components
        .get(&key)
        .is_some_and(|entry| entry.video.is_some())
}

unsafe extern "system" fn container_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_COMMAND => {
            let code = ((wparam.0 >> 16) & 0xffff) as u32;
            match code {
                EN_CHANGE => on_edit_changed(hwnd),
                EN_SETFOCUS => on_edit_focus_changed(hwnd, true),
                EN_KILLFOCUS => on_edit_focus_changed(hwnd, false),
                _ => {}
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_CTLCOLOREDIT | WindowsAndMessaging::WM_CTLCOLORSTATIC => {
            let color = component_key_for_container(hwnd)
                .and_then(|key| {
                    let components = components();
                    components
                        .get(&key)
                        .and_then(|entry| entry.state.text_color)
                })
                .unwrap_or(DEFAULT_TEXT_COLOR);
            let hdc = HDC(wparam.0 as *mut _);
            unsafe {
                SetTextColor(hdc, COLORREF(color));
                SetBkColor(hdc, COLORREF(0x00ff_ffff));
                LRESULT(GetStockObject(WHITE_BRUSH).0 as isize)
            }
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == VIDEO_TIMER_ID => {
            on_video_timer(hwnd);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == SWIPER_AUTOPLAY_TIMER_ID => {
            on_swiper_autoplay_timer(hwnd);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == SWIPER_ANIM_TIMER_ID => {
            on_swiper_anim_timer(hwnd);
            LRESULT(0)
        }
        // A video or swiper container paints black while its media children
        // decide their own visible content.
        WindowsAndMessaging::WM_ERASEBKGND
            if container_is_video(hwnd) || container_is_swiper(hwnd) =>
        {
            let hdc = HDC(wparam.0 as *mut _);
            let mut rect = RECT::default();
            unsafe {
                let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                let _ = windows::Win32::Graphics::Gdi::FillRect(
                    hdc,
                    &rect,
                    HBRUSH(GetStockObject(BLACK_BRUSH).0),
                );
            }
            LRESULT(1)
        }
        WindowsAndMessaging::WM_PAINT if container_is_video(hwnd) || container_is_swiper(hwnd) => {
            let mut paint = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            unsafe {
                let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut paint);
                let mut rect = RECT::default();
                let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                let _ = windows::Win32::Graphics::Gdi::FillRect(
                    hdc,
                    &rect,
                    HBRUSH(GetStockObject(BLACK_BRUSH).0),
                );
                let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &paint);
            }
            LRESULT(0)
        }
        // Clicks on the container padding focus the inner EDIT; clicking a
        // video focuses the surface itself so Escape reaches it.
        WindowsAndMessaging::WM_LBUTTONDOWN => {
            unsafe {
                if container_is_video(hwnd) {
                    let _ = SetFocus(Some(hwnd));
                } else if !container_is_swiper(hwnd) {
                    if let Ok(edit) = WindowsAndMessaging::GetWindow(hwnd, GW_CHILD) {
                        let _ = SetFocus(Some(edit));
                    }
                }
            }
            LRESULT(0)
        }
        // Double click toggles video fullscreen. Escape leaves it because the
        // fullscreen surface covers the page's own controls, so it must be
        // dismissible from the surface itself.
        WindowsAndMessaging::WM_LBUTTONDBLCLK => {
            if let Some(key) = component_key_for_container(hwnd) {
                let fullscreen = {
                    let components = components();
                    components
                        .get(&key)
                        .and_then(|entry| entry.video.as_ref())
                        .map(|video| video.fullscreen)
                };
                if let Some(fullscreen) = fullscreen {
                    set_video_fullscreen(&key, !fullscreen);
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
            if let Some(key) = component_key_for_container(hwnd) {
                let fullscreen = {
                    let components = components();
                    components
                        .get(&key)
                        .and_then(|entry| entry.video.as_ref())
                        .is_some_and(|video| video.fullscreen)
                };
                if fullscreen {
                    set_video_fullscreen(&key, false);
                    return LRESULT(0);
                }
            }
            unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            containers().remove(&(hwnd.0 as isize));
            unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
