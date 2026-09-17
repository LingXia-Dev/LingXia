//! What the shell shows while an AI assistant drives the app over its control
//! socket, modelled on computer-use agents. Mirrors the macOS
//! `AgentControlIndicator`:
//!
//! - a pulsing orange frame around the app window that ignores the mouse;
//! - a capsule at the bottom centre, "● An AI assistant is in control   Stop";
//! - an orange dot over the taskbar button, for when the app is behind others.
//!
//! Both windows are owned by the app window, so they follow it and hide with
//! it. Stop ends the session through [`crate::request_agent_control_stop`].

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateBitmap, CreateRoundRectRgn, CreateSolidBrush, DT_LEFT, DT_RIGHT,
    DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, Ellipse, EndPaint, FillRect,
    GetStockObject, HFONT, HGDIOBJ, NULL_PEN, PAINTSTRUCT, ScreenToClient, SelectObject, SetBkMode,
    SetTextColor, SetWindowRgn, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use super::update_callout::{dpi_scale, make_font, rgb, to_wide};
use super::update_card::Lang;

static CAPSULE_HWND: AtomicIsize = AtomicIsize::new(0);
/// What the control runtime last asked for.
static WANTED: AtomicBool = AtomicBool::new(false);
/// Bumped on every show/hide; windows for a stale request close themselves.
static GENERATION: AtomicU64 = AtomicU64::new(0);

const CAPSULE_HEIGHT: f32 = 32.0;
const CAPSULE_BOTTOM: f32 = 24.0;
const PAD: f32 = 14.0;
const DOT: f32 = 8.0;
const GAP: f32 = 8.0;
const STOP_GAP: f32 = 14.0;
const FRAME_WIDTH: f32 = 3.0;
const TICK_MS: u32 = 30;
const PULSE_MS: f32 = 2400.0;

const INK: (u8, u8, u8) = (33, 38, 43);
const TINT: (u8, u8, u8) = (255, 149, 0);
/// Colour-keyed out of the frame window, leaving only the frame.
const KEY: (u8, u8, u8) = (255, 0, 255);

pub(super) fn show(locale: &str) {
    if WANTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let lang = super::update_card::lang_of(locale);
    std::thread::Builder::new()
        .name("lingxia-agent-control-indicator".to_string())
        .spawn(move || run_indicator_thread(lang, generation))
        .ok();
}

pub(super) fn hide() {
    if !WANTED.swap(false, Ordering::SeqCst) {
        return;
    }
    GENERATION.fetch_add(1, Ordering::SeqCst);
    let hwnd = CAPSULE_HWND.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

fn t_active(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "An AI assistant is in control",
        Lang::Zh => "AI 助手正在操作",
    }
}

fn t_stop(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Stop",
        Lang::Zh => "停止",
    }
}

/// State owned by the capsule window; the frame window and taskbar overlay
/// live and die with it.
struct Indicator {
    font: HFONT,
    lang: Lang,
    owner: Option<HWND>,
    frame: Option<HWND>,
    taskbar: Option<ITaskbarList3>,
    started: std::time::Instant,
    /// Capsule width at the current DPI, fitted to the text.
    width: i32,
}

fn run_indicator_thread(lang: Lang, generation: u64) {
    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let capsule_class = w!("LxAgentControlCapsuleClass");
        let frame_class = w!("LxAgentControlFrameClass");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(capsule_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: capsule_class,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        });
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(frame_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: frame_class,
            ..Default::default()
        });

        let scale = dpi_scale();
        let owner = super::update_card::find_main_window();
        let font = make_font(scale, 9, true);
        let width = capsule_width(font, lang, scale);

        let frame = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            frame_class,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            owner,
            None,
            Some(hinstance.into()),
            None,
        )
        .ok();
        if let Some(frame) = frame {
            let _ = SetLayeredWindowAttributes(
                frame,
                rgb(KEY.0, KEY.1, KEY.2),
                255,
                LWA_COLORKEY | LWA_ALPHA,
            );
        }

        let Ok(capsule) = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            capsule_class,
            w!("AI assistant"),
            WS_POPUP,
            0,
            0,
            width,
            px(CAPSULE_HEIGHT, scale),
            owner,
            None,
            Some(hinstance.into()),
            None,
        ) else {
            if let Some(frame) = frame {
                let _ = DestroyWindow(frame);
            }
            let _ = DeleteObject(font.into());
            if com {
                CoUninitialize();
            }
            return;
        };
        let height = px(CAPSULE_HEIGHT, scale);
        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, height, height);
        let _ = SetWindowRgn(capsule, Some(region), false);

        let taskbar = owner.and_then(|owner| set_taskbar_overlay(owner, lang));
        let indicator = Box::new(Indicator {
            font,
            lang,
            owner,
            frame,
            taskbar,
            started: std::time::Instant::now(),
            width,
        });
        SetWindowLongPtrW(capsule, GWLP_USERDATA, Box::into_raw(indicator) as isize);
        CAPSULE_HWND.store(capsule.0 as isize, Ordering::SeqCst);

        if GENERATION.load(Ordering::SeqCst) != generation {
            let _ = DestroyWindow(capsule);
        } else {
            follow_owner(capsule);
            if let Some(frame) = frame {
                let _ = ShowWindow(frame, SW_SHOWNOACTIVATE);
            }
            let _ = ShowWindow(capsule, SW_SHOWNOACTIVATE);
            SetTimer(Some(capsule), 1, TICK_MS, None);
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if com {
            CoUninitialize();
        }
    }
}

fn px(value: f32, scale: f32) -> i32 {
    (value * scale) as i32
}

/// Padding, dot, text, gap, Stop, padding.
fn capsule_width(font: HFONT, lang: Lang, scale: f32) -> i32 {
    let fixed = px(PAD * 2.0 + DOT + GAP + STOP_GAP, scale);
    fixed + text_width(font, t_active(lang)) + text_width(font, t_stop(lang))
}

fn text_width(font: HFONT, text: &str) -> i32 {
    use windows::Win32::Graphics::Gdi::{DT_CALCRECT, GetDC, ReleaseDC};
    unsafe {
        let dc = GetDC(None);
        let old = SelectObject(dc, font.into());
        let mut rect = RECT::default();
        let mut wide = to_wide(text);
        let n = wide.len().saturating_sub(1);
        DrawTextW(dc, &mut wide[..n], &mut rect, DT_SINGLELINE | DT_CALCRECT);
        SelectObject(dc, old);
        ReleaseDC(None, dc);
        rect.right - rect.left
    }
}

fn stop_left(indicator: &Indicator, scale: f32) -> i32 {
    indicator.width - px(PAD, scale) - text_width(indicator.font, t_stop(indicator.lang))
}

fn point_in_stop(hwnd: HWND, point: POINT) -> bool {
    let Some(indicator) = (unsafe { indicator_ref(hwnd) }) else {
        return false;
    };
    // The whole right end, not just the glyphs, is the target.
    point.x >= stop_left(indicator, dpi_scale()) - px(STOP_GAP / 2.0, dpi_scale())
}

unsafe extern "system" fn capsule_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint_capsule(hwnd);
                LRESULT(0)
            }
            WM_TIMER => {
                follow_owner(hwnd);
                pulse_frame(hwnd);
                LRESULT(0)
            }
            WM_SETCURSOR => {
                let mut point = POINT::default();
                let _ = GetCursorPos(&mut point);
                let _ = ScreenToClient(hwnd, &mut point);
                let cursor = if point_in_stop(hwnd, point) {
                    IDC_HAND
                } else {
                    IDC_ARROW
                };
                SetCursor(LoadCursorW(None, cursor).ok());
                LRESULT(1)
            }
            WM_LBUTTONUP => {
                let point = POINT {
                    x: (lparam.0 & 0xFFFF) as i16 as i32,
                    y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                };
                if point_in_stop(hwnd, point) {
                    // The session-ended event takes the indicator down.
                    crate::request_agent_control_stop();
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                let _ = CAPSULE_HWND.compare_exchange(
                    hwnd.0 as isize,
                    0,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
                if let Some(indicator) = take_indicator(hwnd) {
                    if let Some(frame) = indicator.frame {
                        let _ = DestroyWindow(frame);
                    }
                    if let (Some(taskbar), Some(owner)) = (&indicator.taskbar, indicator.owner) {
                        let _ = taskbar.SetOverlayIcon(owner, HICON::default(), PCWSTR::null());
                    }
                    let _ = DeleteObject(indicator.font.into());
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe extern "system" fn frame_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint_frame(hwnd);
                LRESULT(0)
            }
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// The owner's visible bounds, without the invisible resize margin.
fn owner_rect(owner: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    unsafe {
        if DwmGetWindowAttribute(
            owner,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
            && rect.right > rect.left
        {
            return Some(rect);
        }
        GetWindowRect(owner, &mut rect).ok().map(|_| rect)
    }
}

fn follow_owner(capsule: HWND) {
    unsafe {
        let Some(indicator) = indicator_ref(capsule) else {
            return;
        };
        let anchor = match indicator.owner.and_then(owner_rect) {
            Some(rect) => rect,
            None => {
                let mut work = RECT::default();
                let _ = SystemParametersInfoW(
                    SPI_GETWORKAREA,
                    0,
                    Some(&mut work as *mut _ as *mut _),
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
                );
                work
            }
        };
        let scale = dpi_scale();
        let height = px(CAPSULE_HEIGHT, scale);
        let x = anchor.left + (anchor.right - anchor.left - indicator.width) / 2;
        let y = anchor.bottom - height - px(CAPSULE_BOTTOM, scale);
        move_if_changed(capsule, x, y, indicator.width, height);

        if let Some(frame) = indicator.frame {
            let hidden = indicator.owner.is_some_and(|owner| {
                IsIconic(owner).as_bool() || !IsWindowVisible(owner).as_bool()
            });
            if hidden || indicator.owner.is_none() {
                let _ = ShowWindow(frame, SW_HIDE);
            } else {
                move_if_changed(
                    frame,
                    anchor.left,
                    anchor.top,
                    anchor.right - anchor.left,
                    anchor.bottom - anchor.top,
                );
                if !IsWindowVisible(frame).as_bool() {
                    let _ = ShowWindow(frame, SW_SHOWNOACTIVATE);
                }
            }
        }
    }
}

fn move_if_changed(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    unsafe {
        let mut current = RECT::default();
        let _ = GetWindowRect(hwnd, &mut current);
        if current.left != x
            || current.top != y
            || current.right - current.left != width
            || current.bottom - current.top != height
        {
            let _ = SetWindowPos(
                hwnd,
                None,
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
        }
    }
}

/// Breathes the frame between full and about half opacity.
fn pulse_frame(capsule: HWND) {
    unsafe {
        let Some(indicator) = indicator_ref(capsule) else {
            return;
        };
        let Some(frame) = indicator.frame else {
            return;
        };
        let phase = indicator.started.elapsed().as_millis() as f32 % PULSE_MS / PULSE_MS;
        let wave = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
        let alpha = (255.0 - wave * 140.0) as u8;
        let _ = SetLayeredWindowAttributes(
            frame,
            rgb(KEY.0, KEY.1, KEY.2),
            alpha,
            LWA_COLORKEY | LWA_ALPHA,
        );
    }
}

fn paint_frame(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let key = CreateSolidBrush(rgb(KEY.0, KEY.1, KEY.2));
        FillRect(hdc, &client, key);
        let _ = DeleteObject(key.into());

        let w = px(FRAME_WIDTH, dpi_scale()).max(2);
        let tint = CreateSolidBrush(rgb(TINT.0, TINT.1, TINT.2));
        for edge in [
            RECT {
                bottom: client.top + w,
                ..client
            },
            RECT {
                top: client.bottom - w,
                ..client
            },
            RECT {
                right: client.left + w,
                ..client
            },
            RECT {
                left: client.right - w,
                ..client
            },
        ] {
            FillRect(hdc, &edge, tint);
        }
        let _ = DeleteObject(tint.into());
        let _ = EndPaint(hwnd, &ps);
    }
}

fn paint_capsule(hwnd: HWND) {
    unsafe {
        let Some(indicator) = indicator_ref(hwnd) else {
            return;
        };
        let scale = dpi_scale();
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);

        let bg = CreateSolidBrush(rgb(INK.0, INK.1, INK.2));
        FillRect(hdc, &client, bg);
        let _ = DeleteObject(bg.into());

        let dot = CreateSolidBrush(rgb(TINT.0, TINT.1, TINT.2));
        let old_brush = SelectObject(hdc, dot.into());
        let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
        let cy = client.bottom / 2;
        let d = px(DOT, scale);
        let left = px(PAD, scale);
        let _ = Ellipse(hdc, left, cy - d / 2, left + d, cy - d / 2 + d);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(dot.into());

        SetBkMode(hdc, TRANSPARENT);
        let old_font = SelectObject(hdc, indicator.font.into());
        let stop_left = stop_left(indicator, scale);
        draw_text(
            hdc,
            t_active(indicator.lang),
            RECT {
                left: left + d + px(GAP, scale),
                right: stop_left - px(STOP_GAP, scale),
                ..client
            },
            rgb(255, 255, 255),
            DT_LEFT,
        );
        draw_text(
            hdc,
            t_stop(indicator.lang),
            RECT {
                left: stop_left,
                right: client.right - px(PAD, scale),
                ..client
            },
            rgb(TINT.0, TINT.1, TINT.2),
            DT_RIGHT,
        );
        SelectObject(hdc, old_font);
        let _ = EndPaint(hwnd, &ps);
    }
}

fn draw_text(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    mut rect: RECT,
    color: COLORREF,
    align: windows::Win32::Graphics::Gdi::DRAW_TEXT_FORMAT,
) {
    unsafe {
        SetTextColor(hdc, color);
        let mut wide = to_wide(text);
        let n = wide.len().saturating_sub(1);
        DrawTextW(
            hdc,
            &mut wide[..n],
            &mut rect,
            align | DT_SINGLELINE | DT_VCENTER,
        );
    }
}

/// An orange dot over the owner's taskbar button.
fn set_taskbar_overlay(owner: HWND, lang: Lang) -> Option<ITaskbarList3> {
    unsafe {
        let taskbar: ITaskbarList3 =
            CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
        taskbar.HrInit().ok()?;
        let icon = dot_icon()?;
        let label = to_wide(t_active(lang));
        let result = taskbar.SetOverlayIcon(owner, icon, PCWSTR(label.as_ptr()));
        let _ = DestroyIcon(icon);
        result.ok().map(|_| taskbar)
    }
}

fn dot_icon() -> Option<HICON> {
    const SIZE: i32 = 16;
    let center = (SIZE as f32 - 1.0) / 2.0;
    let mut bgra = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let distance = ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt();
            // White ring, orange fill, soft edge.
            let (r, g, b) = if distance > 5.5 {
                (255, 255, 255)
            } else {
                TINT
            };
            let alpha = (7.5 - distance).clamp(0.0, 1.0);
            let a = (alpha * 255.0) as u8;
            let premultiply = |c: u8| (c as f32 * alpha) as u8;
            bgra.extend_from_slice(&[premultiply(b), premultiply(g), premultiply(r), a]);
        }
    }
    unsafe {
        let color = CreateBitmap(SIZE, SIZE, 1, 32, Some(bgra.as_ptr().cast()));
        let mask = CreateBitmap(SIZE, SIZE, 1, 1, None);
        let info = ICONINFO {
            fIcon: windows::core::BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        icon.filter(|icon| !icon.is_invalid())
    }
}

unsafe fn indicator_ref(hwnd: HWND) -> Option<&'static Indicator> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Indicator).as_ref() }
}

unsafe fn take_indicator(hwnd: HWND) -> Option<Box<Indicator>> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Indicator;
        if ptr.is_null() {
            return None;
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        Some(Box::from_raw(ptr))
    }
}
