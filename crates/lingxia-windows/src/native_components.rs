//! Embedded native components for Windows — Input / Textarea / Video.
//!
//! The page view mounts `<lx-input>` / `<lx-textarea>` / `<lx-video>` over
//! the webview by sending component messages (`component.mount` /
//! `component.update` / `component.unmount`, plus `component.ready` and
//! `page.scroll`) through the native-component bridge channel.
//! `lingxia-lxapp` routes those messages to the host registered by
//! [`install`]; this module owns all component policy: it places a
//! borderless Win32 child over the webview content at the reported
//! coverage rect — an `EDIT` control (multiline for textarea) or an MFPlay
//! video surface (`video_player::VideoPlayer`) — keeps it aligned while
//! the page scrolls/relays out, and emits the component's events back to
//! the page view and to page-function bindings (`input`/`focus`/`blur`/
//! `confirm` for text, the media transitions plus a timer-driven
//! `timeupdate` for video). Video control commands from the logic layer
//! (`lx.createVideoContext`) arrive through the dispatcher registered with
//! `lingxia_platform::register_windows_video_command_dispatcher`.
//!
//! Mirrors the manager contract of
//! `lingxia-sdk/apple/Sources/macOS/NativeComponents/MacNativeComponentManager.swift`:
//! a per-page component registry keyed by component id, document-space
//! rects converted to viewport space with a natively tracked scroll
//! offset, a ready handshake that queues events until the view handler is
//! registered, and graceful no-ops (log only) for component kinds this
//! phase does not support (picker/video/media-swiper are deferred).
//!
//! Threading: every Win32 mutation runs on the UI thread that owns the
//! webview window (component controls are its children). Messages already
//! arrive on that thread (WebView2 `WebMessageReceived`); calls from other
//! threads (page teardown) are marshalled with
//! `lingxia_webview::platform::windows::post_to_window_thread`. The state
//! registry is guarded by a mutex that is never held across Win32 calls
//! that can re-enter the window procedures (e.g. `SetWindowTextW` →
//! `EN_CHANGE`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_platform::traits::video_player::VideoPlayerCommand;
use lingxia_webview::WebViewController;
use lingxia_webview::platform::windows::{
    WindowsWebViewContentWindow, post_to_window_thread, webview_content_window,
};
use serde_json::{Value, json};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CreateFontW, CreateRectRgn, DEFAULT_CHARSET,
    DEFAULT_PITCH, DeleteObject, FF_SWISS, GetStockObject, HBRUSH, HDC, HGDIOBJ,
    InvalidateRect, OUT_DEFAULT_PRECIS, SetBkColor, SetTextColor, SetWindowRgn, WHITE_BRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, SetFocus, VK_CONTROL, VK_RETURN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    self, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_PASSWORD, ES_WANTRETURN, GW_CHILD,
    WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW, WNDPROC,
};
use windows::core::{PCWSTR, w};

use super::video_player::{VideoEventSink, VideoPlayer, VideoPlayerEvent};

/// Edit-control messages and notification codes, defined locally (they live
/// in `Win32::UI::Controls`; the constants are stable and tiny, matching
/// the `text_input` module's approach).
const EM_GETSEL: u32 = 0x00b0;
const EM_SETSEL: u32 = 0x00b1;
const EM_SETLIMITTEXT: u32 = 0x00c5;
const EM_SETCUEBANNER: u32 = 0x1501;
const EN_SETFOCUS: u32 = 0x0100;
const EN_KILLFOCUS: u32 = 0x0200;
const EN_CHANGE: u32 = 0x0300;

/// Default text size (CSS px) when the view does not report one.
const DEFAULT_FONT_SIZE: f64 = 14.0;
/// Default text color (CSS `#111111`-ish dark gray) as 0x00BBGGRR.
const DEFAULT_TEXT_COLOR: u32 = 0x0011_1111;
/// Horizontal inset of the EDIT inside its container, CSS px.
const EDIT_PADDING_X: f64 = 8.0;
/// Vertical inset of a multiline EDIT inside its container, CSS px.
const EDIT_PADDING_Y: f64 = 6.0;
/// Events queued per component until the view sends `component.ready`.
const MAX_PENDING_EVENTS: usize = 8;

/// `WM_TIMER` id driving `timeupdate` while a video component plays ("LXVT").
const VIDEO_TIMER_ID: usize = 0x4C58_5654;
/// Video `timeupdate` cadence, matching the HTML media-event ballpark.
const VIDEO_TIMER_INTERVAL_MS: u32 = 250;

/// Registers this module as the process-wide native-component host and as
/// the video-command dispatcher of the platform layer.
/// Called from the shell `register_runtime()` path (`windows::install`).
pub(crate) fn install() {
    if !lxapp::register_native_component_host(Arc::new(ShellNativeComponentHost)) {
        log::warn!("a native-component host was already registered; Windows manager inactive");
    }
    lingxia_platform::register_windows_video_command_dispatcher(Arc::new(dispatch_video_command));
}

struct ShellNativeComponentHost;

impl lxapp::NativeComponentHost for ShellNativeComponentHost {
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
        let target = webview_content_window(&webtag);
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

#[derive(Clone, Copy, PartialEq, Default)]
struct DocRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Component props this host applies. Each field is `Some` only when the
/// view supplied it; updates merge field-wise into the stored state. Text
/// and video components read disjoint subsets of one props bag — the parse
/// is shared and unknown fields are simply never consulted.
#[derive(Clone, Default)]
struct ComponentProps {
    value: Option<String>,
    placeholder: Option<String>,
    text_color: Option<u32>,
    font_size: Option<f64>,
    disabled: Option<bool>,
    password: Option<bool>,
    maxlength: Option<u32>,
    focus: Option<bool>,
    // — video —
    src: Option<String>,
    autoplay: Option<bool>,
    looping: Option<bool>,
    muted: Option<bool>,
    volume: Option<f64>,
    bindings_json: Option<String>,
    dataset_json: Option<String>,
}

impl ComponentProps {
    fn merge_from(&mut self, other: &ComponentProps) {
        macro_rules! take {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field.clone();
                }
            };
        }
        take!(value);
        take!(placeholder);
        take!(text_color);
        take!(font_size);
        take!(disabled);
        take!(password);
        take!(maxlength);
        take!(focus);
        take!(src);
        take!(autoplay);
        take!(looping);
        take!(muted);
        take!(volume);
        take!(bindings_json);
        take!(dataset_json);
    }
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
    doc_rect: DocRect,
    state: ComponentProps,
    last_value: String,
    ready: bool,
    pending: Vec<(String, Value)>,
}

struct VideoComponent {
    player: Arc<VideoPlayer>,
    /// Covers the whole content area instead of its document rect
    /// (`requestFullScreen` from the video context).
    fullscreen: bool,
}

/// Per-page view state: latest scroll offset (CSS px) and content-window
/// geometry, refreshed on every message that carries a target.
#[derive(Clone, Copy)]
struct PageView {
    scroll_x: f64,
    scroll_y: f64,
    target: WindowsWebViewContentWindow,
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
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn component_key(page_key: &str, component_id: &str) -> String {
    format!("{page_key}\u{1}{component_id}")
}

// ---------------------------------------------------------------------------
// Message dispatch
// ---------------------------------------------------------------------------

fn handle_message(context: PageContext, target: Option<WindowsWebViewContentWindow>, message: &Value) {
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
    let props = parse_props(message.get("props"));

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

fn handle_update(context: &PageContext, message: &Value) {
    let Some(component_id) = message_component_id(message) else {
        return;
    };
    let doc_rect = parse_rect(message.get("rect"));
    let props = message.get("props").map(|raw| parse_props(Some(raw)));

    let key = component_key(&context.page_key, &component_id);
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

/// Destroys every component mounted by `page_key` and drops its view state.
fn teardown_page(page_key: &str) {
    page_views().remove(page_key);
    {
        let mut ready = ready_keys();
        ready.retain(|key| !key.starts_with(page_key) || !key[page_key.len()..].starts_with('\u{1}'));
    }
    let targets: Vec<(String, isize)> = components()
        .iter()
        .filter(|(_, entry)| entry.context.page_key == page_key)
        .map(|(key, entry)| (key.clone(), entry.parent))
        .collect();
    for (key, parent) in targets {
        let posted = is_window(parent)
            && {
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
// Parsing helpers
// ---------------------------------------------------------------------------

fn parse_rect(raw: Option<&Value>) -> Option<DocRect> {
    let rect = raw?;
    Some(DocRect {
        x: rect.get("x").and_then(Value::as_f64)?,
        y: rect.get("y").and_then(Value::as_f64)?,
        width: rect.get("width").and_then(Value::as_f64)?,
        height: rect.get("height").and_then(Value::as_f64)?,
    })
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" | "" => Some(false),
            _ => None,
        },
        Value::Number(number) => number.as_f64().map(|n| n != 0.0),
        _ => None,
    }
}

fn parse_props(raw: Option<&Value>) -> ComponentProps {
    let mut props = ComponentProps::default();
    let Some(raw) = raw.and_then(Value::as_object) else {
        return props;
    };

    props.value = raw.get("value").and_then(Value::as_str).map(str::to_string);
    props.placeholder = raw
        .get("placeholder")
        .and_then(Value::as_str)
        .map(str::to_string);
    props.text_color = raw
        .get("textColor")
        .and_then(Value::as_str)
        .and_then(parse_css_color);
    props.font_size = raw
        .get("fontSize")
        .and_then(Value::as_f64)
        .filter(|size| *size > 1.0);
    props.disabled = raw.get("disabled").and_then(value_as_bool);
    props.password = raw.get("password").and_then(value_as_bool);
    props.maxlength = raw
        .get("maxlength")
        .and_then(Value::as_f64)
        .filter(|n| *n >= 0.0)
        .map(|n| n as u32);
    props.focus = raw.get("focus").and_then(value_as_bool);
    props.src = raw.get("src").and_then(Value::as_str).map(str::to_string);
    props.autoplay = raw.get("autoplay").and_then(value_as_bool);
    props.looping = raw.get("loop").and_then(value_as_bool);
    props.muted = raw.get("muted").and_then(value_as_bool);
    props.volume = raw
        .get("volume")
        .and_then(Value::as_f64)
        .filter(|volume| volume.is_finite());
    props.bindings_json = raw
        .get("pageFuncBindingsJson")
        .and_then(Value::as_str)
        .filter(|json| !json.is_empty() && *json != "{}")
        .map(str::to_string);
    props.dataset_json = raw
        .get("datasetJson")
        .and_then(Value::as_str)
        .filter(|json| !json.is_empty())
        .map(str::to_string);
    props
}

/// Parses a CSS color (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()/rgba()`)
/// into a `COLORREF` value (0x00BBGGRR). Returns `None` for anything else.
fn parse_css_color(raw: &str) -> Option<u32> {
    let value = raw.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let rgb = match hex.len() {
            3 => {
                let expanded: String = hex.chars().flat_map(|ch| [ch, ch]).collect();
                u32::from_str_radix(&expanded, 16).ok()?
            }
            6 => u32::from_str_radix(hex, 16).ok()?,
            8 => u32::from_str_radix(&hex[..6], 16).ok()?,
            _ => return None,
        };
        let (r, g, b) = ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
        return Some((b << 16) | (g << 8) | r);
    }

    let inner = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let mut parts = inner.split(',').map(str::trim);
    let r: u32 = parts.next()?.parse().ok()?;
    let g: u32 = parts.next()?.parse().ok()?;
    let b: u32 = parts.next()?.parse().ok()?;
    Some(((b & 0xff) << 16) | ((g & 0xff) << 8) | (r & 0xff))
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// EDIT controls use CRLF line endings; the page view uses LF.
fn to_edit_text(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn from_edit_text(value: &str) -> String {
    value.replace("\r\n", "\n")
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
        MountKind::Edit { multiline } => {
            mount_edit_on_ui(context, component_id, multiline, parent, container, doc_rect, props)
        }
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

/// Mounts a `video.native` component: an MFPlay player rendering into the
/// container window. Playback transitions and the play-timer drive the
/// element's media events; the document rect only positions the surface.
fn mount_video_on_ui(
    context: PageContext,
    component_id: String,
    parent: isize,
    container: HWND,
    doc_rect: DocRect,
    props: ComponentProps,
) {
    let key = component_key(&context.page_key, &component_id);
    let sink = video_event_sink(key.clone(), container.0 as isize);
    let Some(player) = VideoPlayer::new(container, sink) else {
        log::warn!("failed to create video player for {component_id}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(container);
        }
        return;
    };

    let entry = ComponentEntry {
        context,
        component_id,
        multiline: false,
        parent,
        container: container.0 as isize,
        edit: 0,
        font: 0,
        video: Some(VideoComponent {
            player: Arc::new(player),
            fullscreen: false,
        }),
        doc_rect,
        state: ComponentProps::default(),
        last_value: String::new(),
        ready: ready_keys().contains(&key),
        pending: Vec::new(),
    };
    components().insert(key.clone(), entry);
    containers().insert(container.0 as isize, key.clone());

    apply_video_props(&key, &props);
    apply_layout(&key);
}

fn mount_edit_on_ui(
    context: PageContext,
    component_id: String,
    multiline: bool,
    parent: isize,
    container: HWND,
    doc_rect: DocRect,
    props: ComponentProps,
) {
    let key = component_key(&context.page_key, &component_id);
    let mut edit_style = WindowsAndMessaging::WS_CHILD.0
        | WindowsAndMessaging::WS_VISIBLE.0
        | WindowsAndMessaging::WS_CLIPSIBLINGS.0;
    if multiline {
        edit_style |= (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32;
    } else {
        edit_style |= ES_AUTOHSCROLL as u32;
    }
    if props.password == Some(true) {
        edit_style |= ES_PASSWORD as u32;
    }
    let initial_text = to_wide(&to_edit_text(props.value.as_deref().unwrap_or("")));
    let edit = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            PCWSTR(initial_text.as_ptr()),
            WINDOW_STYLE(edit_style),
            0,
            0,
            16,
            16,
            Some(container),
            None,
            None,
            None,
        )
    };
    let Ok(edit) = edit else {
        log::warn!("failed to create native-component EDIT for {component_id}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(container);
        }
        return;
    };

    // Subclass the EDIT for confirm (Enter) handling.
    let original_proc = unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            edit,
            WindowsAndMessaging::GWLP_WNDPROC,
            edit_proc as *const () as usize as isize,
        )
    };
    let edit_state = Box::new(EditState {
        original_proc,
        component_key: key.clone(),
        multiline,
    });
    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            edit,
            WindowsAndMessaging::GWLP_USERDATA,
            Box::into_raw(edit_state) as isize,
        );
    }

    let entry = ComponentEntry {
        context,
        component_id: component_id.clone(),
        multiline,
        parent,
        container: container.0 as isize,
        edit: edit.0 as isize,
        font: 0,
        video: None,
        doc_rect,
        state: ComponentProps::default(),
        last_value: props.value.clone().unwrap_or_default(),
        ready: ready_keys().contains(&key),
        pending: Vec::new(),
    };
    components().insert(key.clone(), entry);
    containers().insert(container.0 as isize, key.clone());

    apply_props(&key, &props);
    apply_layout(&key);
}

/// Registers (once) and returns the container window class. The container
/// receives the EDIT's `WM_COMMAND` notifications and colors its text.
fn container_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
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
/// clips it to the content area (chrome regions stay clean) and keeps it
/// above the WebView2 child windows.
fn apply_layout(key: &str) {
    let (container, edit, doc_rect, page_key, font_size, multiline, video) = {
        let components = components();
        let Some(entry) = components.get(key) else {
            return;
        };
        (
            entry.container,
            entry.edit,
            entry.doc_rect,
            entry.context.page_key.clone(),
            entry.state.font_size.unwrap_or(DEFAULT_FONT_SIZE),
            entry.multiline,
            entry
                .video
                .as_ref()
                .map(|video| (video.player.clone(), video.fullscreen)),
        )
    };
    let Some(view) = page_views().get(&page_key).copied() else {
        return;
    };
    let target = view.target;
    let scale = if target.scale > 0.0 { target.scale } else { 1.0 };

    let fullscreen = matches!(video, Some((_, true)));
    let (x, y, width, height) = if fullscreen {
        // A fullscreen video covers the whole content area of its window.
        (
            target.content_left,
            target.content_top,
            target.content_width,
            target.content_height,
        )
    } else {
        (
            ((doc_rect.x - view.scroll_x) * scale).round() as i32 + target.content_left,
            ((doc_rect.y - view.scroll_y) * scale).round() as i32 + target.content_top,
            (doc_rect.width * scale).round().max(0.0) as i32,
            (doc_rect.height * scale).round().max(0.0) as i32,
        )
    };

    let content_right = target.content_left + target.content_width;
    let content_bottom = target.content_top + target.content_height;
    let visible_left = x.max(target.content_left);
    let visible_top = y.max(target.content_top);
    let visible_right = (x + width).min(content_right);
    let visible_bottom = (y + height).min(content_bottom);

    let container_hwnd = HWND(container as *mut _);
    if width <= 0 || height <= 0 || visible_right <= visible_left || visible_bottom <= visible_top {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(container_hwnd, WindowsAndMessaging::SW_HIDE);
        }
        return;
    }

    unsafe {
        let _ = WindowsAndMessaging::MoveWindow(container_hwnd, x, y, width, height, true);
        // Clip to the content area when the component pokes out of it
        // (scrolled partially under the chrome).
        let fully_visible = visible_left == x
            && visible_top == y
            && visible_right == x + width
            && visible_bottom == y + height;
        if fully_visible {
            SetWindowRgn(container_hwnd, None, true);
        } else {
            let region = CreateRectRgn(
                visible_left - x,
                visible_top - y,
                visible_right - x,
                visible_bottom - y,
            );
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

        // The video surface fills the container; MFPlay just needs a
        // repaint nudge after the window moved or resized.
        if let Some((player, _)) = video {
            player.update_video();
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
        components.get(key).is_some_and(|entry| entry.video.is_some())
    };
    if is_video {
        apply_video_props(key, props);
        return;
    }
    apply_edit_props(key, props);
}

fn apply_edit_props(key: &str, props: &ComponentProps) {
    struct Pending {
        edit: isize,
        container: isize,
        parent: isize,
        multiline: bool,
        old_font: isize,
        font_size: Option<f64>,
        scale: f64,
        placeholder: Option<String>,
        maxlength: Option<u32>,
        disabled: Option<bool>,
        value: Option<String>,
        focus: Option<bool>,
        color_changed: bool,
    }

    let pending = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        let font_changed = props.font_size.is_some() && props.font_size != entry.state.font_size;
        let color_changed = props.text_color.is_some() && props.text_color != entry.state.text_color;
        let placeholder_changed =
            props.placeholder.is_some() && props.placeholder != entry.state.placeholder;
        // Focus is asserted only when the prop actually flips: the view
        // resends the whole prop set on unrelated changes, and re-applying
        // `focus:"false"` would yank focus from a control the user just
        // clicked into.
        let focus_changed = props.focus.is_some() && props.focus != entry.state.focus;
        let first_apply = entry.font == 0;
        entry.state.merge_from(props);

        let scale = page_views()
            .get(&entry.context.page_key)
            .map(|view| view.target.scale)
            .filter(|scale| *scale > 0.0)
            .unwrap_or(1.0);

        Pending {
            edit: entry.edit,
            container: entry.container,
            parent: entry.parent,
            multiline: entry.multiline,
            old_font: entry.font,
            font_size: (font_changed || first_apply)
                .then_some(entry.state.font_size.unwrap_or(DEFAULT_FONT_SIZE)),
            scale,
            placeholder: (placeholder_changed || first_apply)
                .then(|| entry.state.placeholder.clone().unwrap_or_default()),
            maxlength: props.maxlength,
            disabled: if first_apply {
                Some(entry.state.disabled.unwrap_or(false))
            } else {
                props.disabled
            },
            value: props.value.clone(),
            focus: if first_apply {
                entry.state.focus.filter(|focus| *focus)
            } else if focus_changed {
                props.focus
            } else {
                None
            },
            color_changed,
        }
    };

    let edit = HWND(pending.edit as *mut _);
    unsafe {
        if let Some(font_size) = pending.font_size {
            let height = -((font_size * pending.scale).round() as i32);
            let font = CreateFontW(
                height,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
                w!("Segoe UI"),
            );
            if !font.is_invalid() {
                let _ = WindowsAndMessaging::SendMessageW(
                    edit,
                    WindowsAndMessaging::WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
                {
                    let mut components = components();
                    if let Some(entry) = components.get_mut(key) {
                        entry.font = font.0 as isize;
                    }
                }
                if pending.old_font != 0 {
                    let _ = DeleteObject(HGDIOBJ(pending.old_font as *mut _));
                }
            }
        }

        if let Some(placeholder) = pending.placeholder
            && !pending.multiline
        {
            // Multiline EDIT controls do not support cue banners; textarea
            // placeholders are deferred.
            let text = to_wide(&placeholder);
            let _ = WindowsAndMessaging::SendMessageW(
                edit,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }

        if let Some(maxlength) = pending.maxlength {
            let _ = WindowsAndMessaging::SendMessageW(
                edit,
                EM_SETLIMITTEXT,
                Some(WPARAM(maxlength as usize)),
                Some(LPARAM(0)),
            );
        }

        if let Some(disabled) = pending.disabled {
            let _ = EnableWindow(edit, !disabled);
        }

        if let Some(value) = pending.value {
            let current = from_edit_text(&read_window_text(edit));
            if current != value {
                suppressed_edits().insert(pending.edit);
                let edit_text = to_edit_text(&value);
                let text = to_wide(&edit_text);
                let _ = WindowsAndMessaging::SetWindowTextW(edit, PCWSTR(text.as_ptr()));
                // Caret to the end of the synced text.
                let end = edit_text.encode_utf16().count();
                let _ = WindowsAndMessaging::SendMessageW(
                    edit,
                    EM_SETSEL,
                    Some(WPARAM(end)),
                    Some(LPARAM(end as isize)),
                );
                suppressed_edits().remove(&pending.edit);
                let mut components = components();
                if let Some(entry) = components.get_mut(key) {
                    entry.last_value = value;
                }
            }
        }

        if pending.color_changed {
            let _ = InvalidateRect(Some(HWND(pending.container as *mut _)), None, true);
        }

        if let Some(focus) = pending.focus {
            set_edit_focus_with_parent(pending.edit, pending.parent, focus);
        }
    }
}

fn set_edit_focus_with_parent(edit: isize, parent: isize, focus: bool) {
    let edit_hwnd = HWND(edit as *mut _);
    unsafe {
        let focused = GetFocus() == edit_hwnd;
        if focus && !focused {
            let _ = SetFocus(Some(edit_hwnd));
        } else if !focus && focused {
            let _ = SetFocus(Some(HWND(parent as *mut _)));
        }
    }
}

// ---------------------------------------------------------------------------
// Video components (UI thread only)
// ---------------------------------------------------------------------------

/// Merges `props` into a video component's stored state and applies the
/// changes to its player. The player calls run after the registry lock is
/// dropped (they are COM calls into MFPlay).
fn apply_video_props(key: &str, props: &ComponentProps) {
    let pending = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        let Some(video) = entry.video.as_ref() else {
            return;
        };
        let src_changed = props.src.is_some() && props.src != entry.state.src;
        entry.state.merge_from(props);
        (
            video.player.clone(),
            src_changed.then(|| entry.state.src.clone().unwrap_or_default()),
            src_changed && entry.state.autoplay == Some(true),
        )
    };
    let (player, source, autoplay) = pending;

    if let Some(looping) = props.looping {
        player.set_looping(looping);
    }
    if let Some(volume) = props.volume {
        player.set_volume(volume);
    }
    if let Some(muted) = props.muted {
        player.set_muted(muted);
    }
    if let Some(source) = source {
        if source.is_empty() {
            player.stop();
        } else {
            player.set_source(&source);
            if autoplay {
                player.play();
            }
        }
    }
}

/// Builds the sink translating player transitions into the element's media
/// events and driving the `timeupdate` timer. MFPlay delivers these on the
/// UI thread that owns the container window.
fn video_event_sink(key: String, container: isize) -> VideoEventSink {
    Arc::new(move |event| {
        let container_hwnd = HWND(container as *mut _);
        match event {
            VideoPlayerEvent::MediaLoaded { duration } => {
                emit_event(&key, "loadedmetadata", json!({ "duration": duration }));
            }
            VideoPlayerEvent::Play => {
                unsafe {
                    let _ = WindowsAndMessaging::SetTimer(
                        Some(container_hwnd),
                        VIDEO_TIMER_ID,
                        VIDEO_TIMER_INTERVAL_MS,
                        None,
                    );
                }
                emit_event(&key, "play", json!({}));
                emit_event(&key, "playing", json!({}));
            }
            VideoPlayerEvent::Pause => {
                stop_video_timer(container_hwnd);
                emit_event(&key, "pause", json!({}));
            }
            VideoPlayerEvent::Stop => {
                stop_video_timer(container_hwnd);
                emit_event(&key, "stop", json!({}));
            }
            VideoPlayerEvent::Ended => {
                stop_video_timer(container_hwnd);
                emit_event(&key, "ended", json!({}));
            }
            VideoPlayerEvent::Error { message } => {
                stop_video_timer(container_hwnd);
                log::warn!("native video component {key}: {message}");
                emit_event(&key, "error", json!({ "errMsg": message }));
            }
        }
    })
}

fn stop_video_timer(container: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::KillTimer(Some(container), VIDEO_TIMER_ID);
    }
}

/// Emits `timeupdate` while a video plays (container `WM_TIMER` tick).
fn on_video_timer(container: HWND) {
    let Some(key) = component_key_for_container(container) else {
        stop_video_timer(container);
        return;
    };
    let player = {
        let components = components();
        components
            .get(&key)
            .and_then(|entry| entry.video.as_ref())
            .map(|video| video.player.clone())
    };
    let Some(player) = player else {
        stop_video_timer(container);
        return;
    };
    let current_time = player.position();
    let duration = player.duration();
    emit_event(
        &key,
        "timeupdate",
        json!({ "currentTime": current_time, "duration": duration }),
    );
}

/// Routes a video-context command (`lx.createVideoContext`) to the mounted
/// `video.native` component with that id. Registered with the platform
/// layer at [`install`]; called from logic threads.
fn dispatch_video_command(component_id: &str, command: &VideoPlayerCommand) -> Result<(), String> {
    let target = {
        let components = components();
        components
            .iter()
            .find(|(_, entry)| entry.video.is_some() && entry.component_id == component_id)
            .map(|(key, entry)| (key.clone(), entry.parent))
    };
    let Some((key, parent)) = target else {
        return Err(format!("no native video component '{component_id}'"));
    };
    let command = command.clone();
    if run_on_window_thread(parent, move || apply_video_command(&key, &command)) {
        Ok(())
    } else {
        Err(format!("window of video component '{component_id}' is gone"))
    }
}

fn apply_video_command(key: &str, command: &VideoPlayerCommand) {
    let player = {
        let components = components();
        let Some(video) = components.get(key).and_then(|entry| entry.video.as_ref()) else {
            return;
        };
        video.player.clone()
    };
    match command {
        VideoPlayerCommand::Play => player.play(),
        VideoPlayerCommand::Pause => player.pause(),
        VideoPlayerCommand::Stop => player.stop(),
        VideoPlayerCommand::Seek { position } => player.seek(*position),
        VideoPlayerCommand::NotifyEnded => {
            // Stream providers surface an authoritative end-of-stream.
            player.stop();
            emit_event(key, "ended", json!({}));
        }
        VideoPlayerCommand::SetDuration { .. } => {
            // Stream-piped duration; file/URL playback reads it from the
            // media item instead.
        }
        VideoPlayerCommand::EnterFullscreen => set_video_fullscreen(key, true),
        VideoPlayerCommand::ExitFullscreen => set_video_fullscreen(key, false),
    }
}

fn set_video_fullscreen(key: &str, fullscreen: bool) {
    {
        let mut components = components();
        let Some(video) = components
            .get_mut(key)
            .and_then(|entry| entry.video.as_mut())
        else {
            return;
        };
        if video.fullscreen == fullscreen {
            return;
        }
        video.fullscreen = fullscreen;
    }
    apply_layout(key);
    emit_event(key, "fullscreenchange", json!({ "fullScreen": fullscreen }));
}

/// Destroys a component's windows and removes all its bookkeeping. Runs on
/// the owning UI thread (window destruction requirement).
fn destroy_component(key: &str) {
    let Some((container, font)) = ({
        let components = components();
        components.get(key).map(|entry| (entry.container, entry.font))
    }) else {
        return;
    };
    unsafe {
        // Destroys the EDIT child too; WM_NCDESTROY unsubclasses it and the
        // container removes itself from the lookup map.
        let _ = WindowsAndMessaging::DestroyWindow(HWND(container as *mut _));
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

fn read_window_text(hwnd: HWND) -> String {
    unsafe {
        let length = WindowsAndMessaging::GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buffer = vec![0u16; length + 1];
        let copied = WindowsAndMessaging::GetWindowTextW(hwnd, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied.min(length)])
    }
}

// ---------------------------------------------------------------------------
// Events back to the page
// ---------------------------------------------------------------------------

/// Emits a component event to the page: queued until the view announces
/// `component.ready`, then delivered to the view's component handler and —
/// when the component declared page-function bindings — to the page's
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
            deliver_event(&context, &component_id, &event, detail, bindings_json, dataset_json);
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

fn edit_caret_position(edit: HWND) -> u32 {
    let selection =
        unsafe { WindowsAndMessaging::SendMessageW(edit, EM_GETSEL, None, None) }.0 as u64;
    ((selection >> 16) & 0xffff) as u32
}

fn current_edit_value(key: &str) -> Option<(HWND, String)> {
    let edit = {
        let components = components();
        components.get(key).map(|entry| entry.edit)?
    };
    let edit = HWND(edit as *mut _);
    Some((edit, from_edit_text(&read_window_text(edit))))
}

fn on_edit_changed(container: HWND) {
    let Some(key) = component_key_for_container(container) else {
        return;
    };
    let Some((edit, value)) = current_edit_value(&key) else {
        return;
    };
    let cursor = edit_caret_position(edit);

    {
        let suppressed = suppressed_edits().contains(&(edit.0 as isize));
        let mut components = components();
        let Some(entry) = components.get_mut(&key) else {
            return;
        };
        if entry.last_value == value {
            return;
        }
        entry.last_value = value.clone();
        if suppressed {
            return;
        }
    }
    emit_event(&key, "input", json!({ "value": value, "cursor": cursor }));
}

fn on_edit_focus_changed(container: HWND, focused: bool) {
    let Some(key) = component_key_for_container(container) else {
        return;
    };
    let Some((_, value)) = current_edit_value(&key) else {
        return;
    };
    let event = if focused { "focus" } else { "blur" };
    emit_event(&key, event, json!({ "value": value }));
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
                    components.get(&key).and_then(|entry| entry.state.text_color)
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
        // Clicks on the container padding focus the inner EDIT.
        WindowsAndMessaging::WM_LBUTTONDOWN => {
            unsafe {
                if let Ok(edit) = WindowsAndMessaging::GetWindow(hwnd, GW_CHILD) {
                    let _ = SetFocus(Some(edit));
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            containers().remove(&(hwnd.0 as isize));
            unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Per-EDIT subclass state stashed in `GWLP_USERDATA`.
struct EditState {
    original_proc: isize,
    component_key: String,
    multiline: bool,
}

fn edit_state(hwnd: HWND) -> *mut EditState {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) };
    raw as *mut EditState
}

fn emit_confirm(key: &str, edit: HWND) {
    let value = from_edit_text(&read_window_text(edit));
    emit_event(key, "confirm", json!({ "value": value }));
}

/// Subclass procedure of component EDIT controls: Enter confirms
/// (Ctrl+Enter for multiline, where plain Enter inserts a newline);
/// `WM_NCDESTROY` unsubclasses and frees the state.
unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let state = edit_state(hwnd);
    if state.is_null() {
        return unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let original = unsafe { (*state).original_proc };
    let multiline = unsafe { (*state).multiline };

    match msg {
        WindowsAndMessaging::WM_KEYDOWN if wparam.0 == VK_RETURN.0 as usize => {
            let ctrl_down = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
            if !multiline || ctrl_down {
                let key = unsafe { (*state).component_key.clone() };
                emit_confirm(&key, hwnd);
                if !multiline {
                    return LRESULT(0);
                }
            }
        }
        // Swallow the translated Enter character on single-line controls
        // (message beep).
        WindowsAndMessaging::WM_CHAR if wparam.0 == 0x0d && !multiline => {
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let state = unsafe { Box::from_raw(state) };
            unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0);
                WindowsAndMessaging::SetWindowLongPtrW(
                    hwnd,
                    WindowsAndMessaging::GWLP_WNDPROC,
                    state.original_proc,
                );
            }
            suppressed_edits().remove(&(hwnd.0 as isize));
            return unsafe { call_original(state.original_proc, hwnd, msg, wparam, lparam) };
        }
        _ => {}
    }
    unsafe { call_original(original, hwnd, msg, wparam, lparam) }
}

/// Calls the EDIT class procedure captured at subclass time.
unsafe fn call_original(
    original: isize,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let proc: WNDPROC = unsafe { std::mem::transmute(original) };
    unsafe { WindowsAndMessaging::CallWindowProcW(proc, hwnd, msg, wparam, lparam) }
}
