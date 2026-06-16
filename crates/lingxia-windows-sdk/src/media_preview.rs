//! `lx.previewMedia` host for Windows: a fullscreen topmost viewer for
//! image and video items, owned by the SDK layer like the other native
//! components.
//!
//! Each session runs on its own thread with its own message pump: the
//! preview window, the GDI+ image rendering and the MFPlay-backed
//! [`VideoPlayer`] (whose callbacks marshal to the creating thread) all
//! live there, fully independent of the app's UI thread. Videos reuse the
//! player engine and the [`VideoControls`] bar; images letterbox onto the
//! black backdrop (remote images download through the URL cache first).
//!
//! Session protocol (`PreviewMediaRequest`): `presented_callback_id`
//! fires `{}` once the first item has painted; `callback_id` fires
//! `{reason, lastIndex}` once when the session ends — `manual` (Escape /
//! the bar's restore button), `completed` (advance ran past the last
//! item), `interrupted` (`cancel_preview` or a superseding preview) or
//! `error` (nothing could be shown). Left/Right keys and edge clicks
//! navigate; `advance` Next/Loop moves on video end or after an image's
//! `duration_ms`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_platform::traits::media_interaction::{
    MediaKind, PreviewMediaAdvance, PreviewMediaRequest,
};
use serde_json::json;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, GetMonitorInfoW, GetStockObject, HBRUSH, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
    MonitorFromWindow, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};
use windows::Win32::Graphics::GdiPlus::{
    GdipCreateBitmapFromFile, GdipCreateFromHDC, GdipDeleteGraphics, GdipDisposeImage,
    GdipDrawImageRectI, GdipGetImageHeight, GdipGetImageWidth, GdipSetInterpolationMode,
    GdiplusStartup, GdiplusStartupInput, GdiplusStartupOutput, GpBitmap, GpGraphics, GpImage,
    InterpolationModeHighQualityBicubic,
};
use windows::Win32::System::Com::Urlmon::URLDownloadToCacheFileW;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_LEFT, VK_RIGHT};
use windows::Win32::UI::WindowsAndMessaging::{self, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW};
use windows::core::{PCWSTR, w};

use super::video_controls::{ControlsAction, ControlsState, VideoControls};
use super::video_player::{VideoEventSink, VideoPlayer, VideoPlayerEvent};

/// Posted by the player sink / cancel path to the session window.
const MSG_VIDEO_PLAYING: u32 = WindowsAndMessaging::WM_APP + 1;
const MSG_VIDEO_ENDED: u32 = WindowsAndMessaging::WM_APP + 2;
const MSG_VIDEO_ERROR: u32 = WindowsAndMessaging::WM_APP + 3;
const MSG_CANCEL: u32 = WindowsAndMessaging::WM_APP + 4;

/// Bar refresh / image auto-advance cadence.
const TICK_TIMER_ID: usize = 0x4C58_5056; // "LXPV"
const TICK_INTERVAL_MS: u32 = 250;

/// Live sessions by completion callback id (for cancel and supersede).
static SESSIONS: OnceLock<Mutex<HashMap<u64, isize>>> = OnceLock::new();

fn sessions() -> std::sync::MutexGuard<'static, HashMap<u64, isize>> {
    SESSIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Registers the preview host with the platform layer. Called from
/// `native_components::install`.
pub(crate) fn install() {
    lingxia_platform::register_windows_media_preview_host(
        Arc::new(open_preview),
        Arc::new(cancel_preview),
    );
}

fn open_preview(request: PreviewMediaRequest) -> Result<(), String> {
    if request.items.is_empty() {
        return Err("previewMedia: no items".to_string());
    }
    // A new preview supersedes any running session.
    let running: Vec<isize> = sessions().values().copied().collect();
    for window in running {
        unsafe {
            let _ = WindowsAndMessaging::PostMessageW(
                Some(HWND(window as *mut _)),
                MSG_CANCEL,
                WPARAM::default(),
                LPARAM::default(),
            );
        }
    }
    std::thread::Builder::new()
        .name("lingxia-media-preview".to_string())
        .spawn(move || run_session(request))
        .map_err(|err| format!("failed to spawn preview thread: {err}"))?;
    Ok(())
}

fn cancel_preview(callback_id: u64) -> Result<(), String> {
    let Some(window) = sessions().get(&callback_id).copied() else {
        // Already closed; the completion callback has fired.
        return Ok(());
    };
    unsafe {
        let _ = WindowsAndMessaging::PostMessageW(
            Some(HWND(window as *mut _)),
            MSG_CANCEL,
            WPARAM::default(),
            LPARAM::default(),
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Session (one per thread)
// ---------------------------------------------------------------------------

struct Session {
    request: PreviewMediaRequest,
    index: usize,
    /// First pixel reached the screen; `presented_callback_id` fired.
    presented: bool,
    /// At least one item rendered successfully (a session that never
    /// shows anything completes with reason `error`).
    rendered_any: bool,
    /// Completion fired (close is idempotent).
    completed: bool,
    /// Decoded GDI+ image of the current item, if it is an image.
    image: *mut GpImage,
    /// MFPlay surface child (videos render into it; hidden for images).
    surface: HWND,
    player: Option<VideoPlayer>,
    controls: Option<VideoControls>,
    video_playing: bool,
    /// Milliseconds the current image has been displayed (auto-advance).
    image_elapsed_ms: u32,
}

fn session_mut<'a>(hwnd: HWND) -> Option<&'a mut Session> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) };
    if raw == 0 {
        None
    } else {
        Some(unsafe { &mut *(raw as *mut Session) })
    }
}

pub(crate) fn ensure_gdiplus() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let mut token = 0usize;
        let mut output = GdiplusStartupOutput::default();
        unsafe {
            let _ = GdiplusStartup(&mut token, &input, &mut output);
        }
    });
}

fn run_session(request: PreviewMediaRequest) {
    ensure_gdiplus();
    let callback_id = request.callback_id;

    // Cover the monitor the app currently sits on.
    let monitor = unsafe {
        MonitorFromWindow(
            WindowsAndMessaging::GetForegroundWindow(),
            MONITOR_DEFAULTTOPRIMARY,
        )
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        let _ = GetMonitorInfoW(monitor, &mut info);
    }
    let area = info.rcMonitor;

    let Ok(window) = (unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_TOPMOST | WindowsAndMessaging::WS_EX_NOACTIVATE,
            preview_class(),
            PCWSTR::null(),
            WINDOW_STYLE(
                WindowsAndMessaging::WS_POPUP.0
                    | WindowsAndMessaging::WS_VISIBLE.0
                    | WindowsAndMessaging::WS_CLIPCHILDREN.0,
            ),
            area.left,
            area.top,
            area.right - area.left,
            area.bottom - area.top,
            None,
            None,
            GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    }) else {
        complete_request(&request, "error");
        return;
    };

    let Ok(surface) = (unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            preview_surface_class(),
            PCWSTR::null(),
            WINDOW_STYLE(WindowsAndMessaging::WS_CHILD.0 | WindowsAndMessaging::WS_CLIPSIBLINGS.0),
            0,
            0,
            area.right - area.left,
            area.bottom - area.top,
            Some(window),
            None,
            GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    }) else {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(window);
        }
        complete_request(&request, "error");
        return;
    };

    let start = (request.start_index.max(0) as usize).min(request.items.len() - 1);
    let session = Box::new(Session {
        request,
        index: start,
        presented: false,
        rendered_any: false,
        completed: false,
        image: std::ptr::null_mut(),
        surface,
        player: None,
        controls: None,
        video_playing: false,
        image_elapsed_ms: 0,
    });
    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            window,
            WindowsAndMessaging::GWLP_USERDATA,
            Box::into_raw(session) as isize,
        );
        let _ = WindowsAndMessaging::SetTimer(Some(window), TICK_TIMER_ID, TICK_INTERVAL_MS, None);
    }
    sessions().insert(callback_id, window.0 as isize);

    show_item(window);

    // The session's message pump; ends when the window is destroyed.
    unsafe {
        let mut message = WindowsAndMessaging::MSG::default();
        while WindowsAndMessaging::GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = WindowsAndMessaging::TranslateMessage(&message);
            WindowsAndMessaging::DispatchMessageW(&message);
        }
    }
    sessions().remove(&callback_id);
}

fn complete_request(request: &PreviewMediaRequest, reason: &str) {
    let payload = json!({ "reason": reason, "lastIndex": 0 }).to_string();
    lingxia_messaging::invoke_callback(request.callback_id, Ok(payload));
}

/// Resolves an item path: remote URLs go through the URL cache, file URIs
/// strip their scheme, everything else is a local path already.
pub(crate) fn resolve_media_path(path: &str) -> Option<String> {
    if path.starts_with("http://") || path.starts_with("https://") {
        let wide_url: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buffer = vec![0u16; 1024];
        let downloaded = unsafe {
            URLDownloadToCacheFileW(
                None::<&windows::core::IUnknown>,
                PCWSTR(wide_url.as_ptr()),
                &mut buffer,
                0,
                None,
            )
        };
        if downloaded.is_err() {
            log::warn!("previewMedia: failed to download {path}");
            return None;
        }
        let length = buffer.iter().position(|&c| c == 0).unwrap_or(0);
        Some(String::from_utf16_lossy(&buffer[..length]))
    } else if let Some(stripped) = path.strip_prefix("file://") {
        Some(stripped.to_string())
    } else {
        Some(path.to_string())
    }
}

/// Tears down the current item's renderer and shows `session.index`.
fn show_item(window: HWND) {
    let Some(session) = session_mut(window) else {
        return;
    };
    // Teardown.
    if !session.image.is_null() {
        unsafe {
            let _ = GdipDisposeImage(session.image);
        }
        session.image = std::ptr::null_mut();
    }
    if let Some(player) = session.player.take() {
        player.stop();
        drop(player);
    }
    if let Some(controls) = session.controls.take() {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(HWND(controls.hwnd as *mut _));
        }
    }
    session.video_playing = false;
    session.image_elapsed_ms = 0;
    unsafe {
        let _ = WindowsAndMessaging::ShowWindow(session.surface, WindowsAndMessaging::SW_HIDE);
    }

    let item = session.request.items[session.index].clone();
    match item.media_type {
        MediaKind::Video => {
            let Some(path) = resolve_local_or_remote_video(&item.path) else {
                advance_or_fail(window);
                return;
            };
            unsafe {
                let _ = WindowsAndMessaging::ShowWindow(
                    session.surface,
                    WindowsAndMessaging::SW_SHOWNA,
                );
            }
            let sink = preview_video_sink(window.0 as isize);
            let Some(player) = VideoPlayer::new(session.surface, sink) else {
                advance_or_fail(window);
                return;
            };
            player.set_source(&path);
            player.play();
            let controls = VideoControls::create(window, preview_controls_sink(window.0 as isize));
            session.player = Some(player);
            session.controls = controls;
            layout_children(window);
        }
        MediaKind::Image | MediaKind::Unknown => {
            let Some(path) = resolve_media_path(&item.path) else {
                advance_or_fail(window);
                return;
            };
            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut bitmap: *mut GpBitmap = std::ptr::null_mut();
            let status = unsafe { GdipCreateBitmapFromFile(PCWSTR(wide.as_ptr()), &mut bitmap) };
            if status.0 != 0 || bitmap.is_null() {
                log::warn!("previewMedia: failed to decode image {path}");
                advance_or_fail(window);
                return;
            }
            session.image = bitmap as *mut GpImage;
        }
    }
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(window), None, true);
    }
}

/// Videos: MFPlay streams remote URLs natively; only file URIs need the
/// scheme stripped.
fn resolve_local_or_remote_video(path: &str) -> Option<String> {
    if let Some(stripped) = path.strip_prefix("file://") {
        Some(stripped.to_string())
    } else {
        Some(path.to_string())
    }
}

/// A current item that cannot render: move on (Next/Loop) or end the
/// session with `error` when nothing was ever shown.
fn advance_or_fail(window: HWND) {
    let Some(session) = session_mut(window) else {
        return;
    };
    match session.request.advance {
        PreviewMediaAdvance::Next | PreviewMediaAdvance::Loop => advance(window, 1),
        PreviewMediaAdvance::Manual => {
            if !session.rendered_any {
                close_session(window, "error");
            }
        }
    }
}

/// Steps the item index by `delta` honoring the advance mode; closes with
/// `completed` when a Next sequence runs past its end.
fn advance(window: HWND, delta: i32) {
    let Some(session) = session_mut(window) else {
        return;
    };
    let count = session.request.items.len() as i32;
    let next = session.index as i32 + delta;
    let next = if next < 0 {
        count - 1
    } else if next >= count {
        match session.request.advance {
            PreviewMediaAdvance::Next => {
                close_session(window, "completed");
                return;
            }
            _ => 0,
        }
    } else {
        next
    };
    session.index = next as usize;
    show_item(window);
}

/// Ends the session once: completion callback, then window teardown.
fn close_session(window: HWND, reason: &str) {
    let Some(session) = session_mut(window) else {
        return;
    };
    if session.completed {
        return;
    }
    session.completed = true;
    let payload = json!({ "reason": reason, "lastIndex": session.index as u32 }).to_string();
    lingxia_messaging::invoke_callback(session.request.callback_id, Ok(payload));
    if !session.presented {
        // Degenerate path: never painted; release the presented waiter.
        lingxia_messaging::invoke_callback(
            session.request.presented_callback_id,
            Ok("{}".to_string()),
        );
        session.presented = true;
    }
    unsafe {
        let _ = WindowsAndMessaging::DestroyWindow(window);
    }
}

fn mark_presented(window: HWND) {
    let Some(session) = session_mut(window) else {
        return;
    };
    session.rendered_any = true;
    if !session.presented {
        session.presented = true;
        lingxia_messaging::invoke_callback(
            session.request.presented_callback_id,
            Ok("{}".to_string()),
        );
    }
}

fn layout_children(window: HWND) {
    let Some(session) = session_mut(window) else {
        return;
    };
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(window, &mut rect);
        let _ =
            WindowsAndMessaging::MoveWindow(session.surface, 0, 0, rect.right, rect.bottom, true);
    }
    if let Some(player) = session.player.as_ref() {
        player.update_video();
    }
    if let Some(controls) = session.controls.as_ref() {
        controls.layout(rect.right, rect.bottom);
        controls.poke();
    }
}

fn update_preview_controls(window: HWND) {
    let Some(session) = session_mut(window) else {
        return;
    };
    let (Some(player), Some(controls)) = (session.player.as_ref(), session.controls.as_ref())
    else {
        return;
    };
    controls.update(ControlsState {
        playing: session.video_playing,
        muted: false,
        // The restore glyph: leaving "fullscreen" closes the preview.
        fullscreen: true,
        position: player.position(),
        duration: player.duration(),
        show_progress: true,
        quality: None,
        rate: None,
        volume: 1.0,
    });
}

// ---------------------------------------------------------------------------
// Sinks (delivered on this session's thread)
// ---------------------------------------------------------------------------

fn preview_video_sink(window: isize) -> VideoEventSink {
    Arc::new(move |event| {
        let window = HWND(window as *mut _);
        let message = match event {
            VideoPlayerEvent::Play => MSG_VIDEO_PLAYING,
            VideoPlayerEvent::Ended => MSG_VIDEO_ENDED,
            VideoPlayerEvent::Error { message } => {
                log::warn!("previewMedia video error: {message}");
                MSG_VIDEO_ERROR
            }
            VideoPlayerEvent::Pause | VideoPlayerEvent::Stop => {
                if let Some(session) = session_mut(window) {
                    session.video_playing = false;
                }
                update_preview_controls(window);
                return;
            }
            VideoPlayerEvent::MediaLoaded { .. } => {
                update_preview_controls(window);
                return;
            }
        };
        unsafe {
            let _ = WindowsAndMessaging::PostMessageW(
                Some(window),
                message,
                WPARAM::default(),
                LPARAM::default(),
            );
        }
    })
}

fn preview_controls_sink(window: isize) -> super::video_controls::ControlsActionSink {
    Arc::new(move |action| {
        let window = HWND(window as *mut _);
        let Some(session) = session_mut(window) else {
            return;
        };
        let Some(player) = session.player.as_ref() else {
            return;
        };
        match action {
            ControlsAction::TogglePlay => {
                if session.video_playing {
                    player.pause();
                } else {
                    player.play();
                }
            }
            ControlsAction::ToggleMute => {}
            ControlsAction::ToggleFullscreen => close_session(window, "manual"),
            ControlsAction::Seek(position) => player.seek(position),
            ControlsAction::SetVolume(volume) => player.set_volume(volume),
            ControlsAction::QualityMenu { .. } | ControlsAction::RateMenu { .. } => {}
        }
    })
}

// ---------------------------------------------------------------------------
// Window procedures
// ---------------------------------------------------------------------------

fn preview_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(preview_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaMediaPreview"),
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            // Without an explicit cursor the freshly-shown top-level preview
            // window keeps the system "app-starting" (busy/spinning) cursor.
            hCursor: unsafe {
                WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW)
            }
            .unwrap_or_default(),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaMediaPreview")
}

fn preview_surface_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(preview_surface_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaMediaPreviewSurface"),
            hbrBackground: HBRUSH(unsafe { GetStockObject(BLACK_BRUSH) }.0),
            hCursor: unsafe {
                WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW)
            }
            .unwrap_or_default(),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaMediaPreviewSurface")
}

/// The video surface forwards interaction to the session window.
unsafe extern "system" fn preview_surface_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_LBUTTONDOWN
        | WindowsAndMessaging::WM_KEYDOWN
        | WindowsAndMessaging::WM_MOUSEMOVE => {
            let parent = unsafe { WindowsAndMessaging::GetParent(hwnd) }.unwrap_or_default();
            unsafe { WindowsAndMessaging::SendMessageW(parent, msg, Some(wparam), Some(lparam)) }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn preview_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_PAINT => {
            paint_preview(hwnd);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_KEYDOWN => {
            match wparam.0 as u16 {
                code if code == VK_ESCAPE.0 => close_session(hwnd, "manual"),
                code if code == VK_LEFT.0 => advance(hwnd, -1),
                code if code == VK_RIGHT.0 => advance(hwnd, 1),
                _ => {}
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let mut rect = RECT::default();
            unsafe {
                let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
            }
            let multiple = session_mut(hwnd)
                .map(|session| session.request.items.len() > 1)
                .unwrap_or(false);
            if multiple && x < rect.right / 5 {
                advance(hwnd, -1);
            } else if multiple && x > rect.right * 4 / 5 {
                advance(hwnd, 1);
            } else if session_mut(hwnd).is_some_and(|session| session.player.is_none()) {
                // Plain click on a single image closes, like the mobile
                // viewers; videos keep their controls instead.
                close_session(hwnd, "manual");
            } else if let Some(session) = session_mut(hwnd) {
                if let Some(controls) = session.controls.as_ref() {
                    controls.poke();
                    update_preview_controls(hwnd);
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_MOUSEMOVE => {
            if let Some(session) = session_mut(hwnd)
                && let Some(controls) = session.controls.as_ref()
            {
                controls.poke();
                update_preview_controls(hwnd);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_TIMER if wparam.0 == TICK_TIMER_ID => {
            update_preview_controls(hwnd);
            // Image auto-advance.
            let advance_now = session_mut(hwnd).is_some_and(|session| {
                if session.player.is_some() || session.completed {
                    return false;
                }
                if !matches!(
                    session.request.advance,
                    PreviewMediaAdvance::Next | PreviewMediaAdvance::Loop
                ) {
                    return false;
                }
                let duration = session.request.items[session.index]
                    .duration_ms
                    .unwrap_or(3000) as u32;
                session.image_elapsed_ms += TICK_INTERVAL_MS;
                session.image_elapsed_ms >= duration
            });
            if advance_now {
                advance(hwnd, 1);
            }
            LRESULT(0)
        }
        msg if msg == MSG_VIDEO_PLAYING => {
            if let Some(session) = session_mut(hwnd) {
                session.video_playing = true;
            }
            mark_presented(hwnd);
            update_preview_controls(hwnd);
            LRESULT(0)
        }
        msg if msg == MSG_VIDEO_ENDED => {
            if let Some(session) = session_mut(hwnd) {
                session.video_playing = false;
                match session.request.advance {
                    PreviewMediaAdvance::Next | PreviewMediaAdvance::Loop => advance(hwnd, 1),
                    PreviewMediaAdvance::Manual => update_preview_controls(hwnd),
                }
            }
            LRESULT(0)
        }
        msg if msg == MSG_VIDEO_ERROR => {
            advance_or_fail(hwnd);
            LRESULT(0)
        }
        msg if msg == MSG_CANCEL => {
            close_session(hwnd, "interrupted");
            LRESULT(0)
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let raw = unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0)
            };
            if raw != 0 {
                let session = unsafe { Box::from_raw(raw as *mut Session) };
                if !session.image.is_null() {
                    unsafe {
                        let _ = GdipDisposeImage(session.image);
                    }
                }
                drop(session);
            }
            unsafe {
                WindowsAndMessaging::PostQuitMessage(0);
                WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Paints the current image letterboxed on the black backdrop, plus the
/// `i/N` indicator; the first successful draw fires `presented`.
fn paint_preview(hwnd: HWND) {
    use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, FillRect, PAINTSTRUCT};
    let mut paint = PAINTSTRUCT::default();
    let dc = unsafe { BeginPaint(hwnd, &mut paint) };
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
        let _ = FillRect(dc, &client, HBRUSH(GetStockObject(BLACK_BRUSH).0));
    }

    let (image, index, count, indicator) = match session_mut(hwnd) {
        Some(session) => (
            session.image,
            session.index,
            session.request.items.len(),
            session.request.show_index_indicator,
        ),
        None => {
            unsafe {
                let _ = EndPaint(hwnd, &paint);
            }
            return;
        }
    };

    let mut drew = false;
    if !image.is_null() {
        let mut width = 0u32;
        let mut height = 0u32;
        unsafe {
            let _ = GdipGetImageWidth(image, &mut width);
            let _ = GdipGetImageHeight(image, &mut height);
        }
        if width > 0 && height > 0 {
            // Letterbox (object-fit: contain).
            let client_w = client.right as f64;
            let client_h = client.bottom as f64;
            let scale = (client_w / width as f64).min(client_h / height as f64);
            let draw_w = (width as f64 * scale) as i32;
            let draw_h = (height as f64 * scale) as i32;
            let x = (client.right - draw_w) / 2;
            let y = (client.bottom - draw_h) / 2;
            let mut graphics: *mut GpGraphics = std::ptr::null_mut();
            unsafe {
                if GdipCreateFromHDC(dc, &mut graphics).0 == 0 && !graphics.is_null() {
                    let _ = GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic);
                    let status = GdipDrawImageRectI(graphics, image, x, y, draw_w, draw_h);
                    drew = status.0 == 0;
                    let _ = GdipDeleteGraphics(graphics);
                }
            }
        }
    }

    if indicator && count > 1 {
        let text = format!("{} / {}", index + 1, count);
        let wide: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, COLORREF(0x00ffffff));
            let _ = TextOutW(dc, client.right / 2 - 20, 24, &wide);
        }
    }

    unsafe {
        let _ = EndPaint(hwnd, &paint);
    }
    if drew {
        mark_presented(hwnd);
    }
}
