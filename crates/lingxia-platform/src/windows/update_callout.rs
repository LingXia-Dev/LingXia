//! Bottom-left "update available" callout — a small dark bubble, anchored to
//! the app window's bottom-left, that announces a downloaded update without
//! stealing focus. It is owned by the app window, so it tracks the app on drag
//! and hides/restores with it. Clicking it opens the "ready to update" card
//! ([`super::update_card::open_ready_card`]). Mirrors the macOS
//! `UpdateReadyCallout` (a calm reminder, on the top layer).

use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, EndPaint, FONT_QUALITY, FW_NORMAL, FW_SEMIBOLD, FillRect, HFONT, PAINTSTRUCT,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

static CALLOUT_HWND: AtomicIsize = AtomicIsize::new(0);
static PRODUCT_NAME: Mutex<Option<String>> = Mutex::new(None);
static LOCALE: Mutex<Option<String>> = Mutex::new(None);

pub(super) fn set_context(product: &str, locale: &str) {
    if let Ok(mut slot) = PRODUCT_NAME.lock() {
        *slot = Some(product.to_string());
    }
    if let Ok(mut slot) = LOCALE.lock() {
        *slot = Some(locale.to_string());
    }
}

/// Show the "update available — click to install" callout at the app's
/// bottom-left.
pub(super) fn show() {
    hide();
    std::thread::Builder::new()
        .name("lingxia-update-callout".to_string())
        .spawn(run_callout_thread)
        .ok();
}

pub(super) fn hide() {
    let hwnd = CALLOUT_HWND.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

struct Callout {
    title_font: HFONT,
    sub_font: HFONT,
    product: String,
    locale: String,
    owner: Option<HWND>,
}

fn run_callout_thread() {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("LxUpdateCalloutClass");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(callout_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_HAND).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let scale = dpi_scale();
        let px = |v: f32| (v * scale) as i32;
        let width = px(240.0);
        let height = px(56.0);

        // Own the callout to the app window so it pins to the app's bottom-left
        // and hides/restores with the app. Poll in case the window isn't up.
        let mut owner = super::update_card::find_main_window();
        let mut tries = 0;
        while owner.is_none() && tries < 50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            owner = super::update_card::find_main_window();
            tries += 1;
        }

        let mut anchor = RECT::default();
        let have_owner = owner
            .map(|h| GetWindowRect(h, &mut anchor).is_ok())
            .unwrap_or(false);
        if !have_owner {
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut anchor as *mut _ as *mut _),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
        let x = anchor.left + px(16.0);
        let y = anchor.bottom - height - px(16.0);

        let hwnd = match CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!("Update"),
            WS_POPUP,
            x,
            y,
            width,
            height,
            owner,
            None,
            Some(hinstance.into()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };
        round_corners(hwnd);

        let product = PRODUCT_NAME
            .lock()
            .ok()
            .and_then(|s| s.clone())
            .unwrap_or_else(|| "LingXia".to_string());
        let locale = LOCALE
            .lock()
            .ok()
            .and_then(|s| s.clone())
            .unwrap_or_default();

        let callout = Box::new(Callout {
            title_font: make_font(scale, 12, true),
            sub_font: make_font(scale, 11, false),
            product,
            locale,
            owner,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(callout) as isize);

        CALLOUT_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(Some(hwnd), 1, 30, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn callout_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint(hwnd);
                LRESULT(0)
            }
            WM_TIMER => {
                if let Some(c) = callout_ref(hwnd)
                    && let Some(owner) = c.owner
                {
                    let mut or = RECT::default();
                    let mut cr = RECT::default();
                    if GetWindowRect(owner, &mut or).is_ok() && GetWindowRect(hwnd, &mut cr).is_ok()
                    {
                        let scale = dpi_scale();
                        let m = (16.0 * scale) as i32;
                        let h = cr.bottom - cr.top;
                        let x = or.left + m;
                        let y = or.bottom - h - m;
                        if x != cr.left || y != cr.top {
                            let _ = SetWindowPos(
                                hwnd,
                                None,
                                x,
                                y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let _ = DestroyWindow(hwnd);
                // Open the "ready to update" card (release notes + Restart/Later).
                super::update_card::open_ready_card();
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                CALLOUT_HWND.store(0, Ordering::SeqCst);
                if let Some(c) = take_callout(hwnd) {
                    let _ = DeleteObject(c.title_font.into());
                    let _ = DeleteObject(c.sub_font.into());
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

fn paint(hwnd: HWND) {
    unsafe {
        let Some(c) = callout_ref(hwnd) else { return };
        let scale = dpi_scale();
        let px = |v: f32| (v * scale) as i32;

        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);

        // Dark ink background — a calm bubble, not a loud blue.
        let bg = CreateSolidBrush(rgb(33, 38, 43));
        FillRect(hdc, &client, bg);
        let _ = DeleteObject(bg.into());

        SetBkMode(hdc, TRANSPARENT);
        let pad = px(12.0);

        let lang = super::update_card::lang_of(&c.locale);
        let title = super::update_card::t_available_title(lang, &c.product);
        let tf = SelectObject(hdc, c.title_font.into());
        SetTextColor(hdc, rgb(255, 255, 255));
        let mut tr = RECT {
            left: pad,
            top: px(8.0),
            right: client.right - pad,
            bottom: px(26.0),
        };
        let mut tw = to_wide(&title);
        let tn = tw.len().saturating_sub(1);
        DrawTextW(
            hdc,
            &mut tw[..tn],
            &mut tr,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
        SelectObject(hdc, tf);

        let sf = SelectObject(hdc, c.sub_font.into());
        SetTextColor(hdc, rgb(210, 210, 214));
        let mut sr = RECT {
            left: pad,
            top: px(28.0),
            right: client.right - pad,
            bottom: px(48.0),
        };
        let mut sw = to_wide(super::update_card::t_click_to_install(lang));
        let sn = sw.len().saturating_sub(1);
        DrawTextW(
            hdc,
            &mut sw[..sn],
            &mut sr,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );
        SelectObject(hdc, sf);

        let _ = EndPaint(hwnd, &ps);
    }
}

unsafe fn callout_ref(hwnd: HWND) -> Option<&'static Callout> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Callout).as_ref() }
}

unsafe fn take_callout(hwnd: HWND) -> Option<Box<Callout>> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Callout;
        if ptr.is_null() {
            return None;
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        Some(Box::from_raw(ptr))
    }
}

fn make_font(scale: f32, pt: i32, semibold: bool) -> HFONT {
    let height = -((pt as f32 * scale * 96.0 / 72.0) as i32);
    let weight = if semibold { FW_SEMIBOLD } else { FW_NORMAL };
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight.0 as i32,
            0,
            0,
            0,
            windows::Win32::Graphics::Gdi::FONT_CHARSET(0),
            windows::Win32::Graphics::Gdi::FONT_OUTPUT_PRECISION(0),
            windows::Win32::Graphics::Gdi::FONT_CLIP_PRECISION(0),
            FONT_QUALITY(windows::Win32::Graphics::Gdi::CLEARTYPE_QUALITY.0),
            0,
            w!("Segoe UI"),
        )
    }
}

fn dpi_scale() -> f32 {
    unsafe {
        let dc = windows::Win32::Graphics::Gdi::GetDC(None);
        if dc.is_invalid() {
            return 1.0;
        }
        let dpi = windows::Win32::Graphics::Gdi::GetDeviceCaps(
            Some(dc),
            windows::Win32::Graphics::Gdi::LOGPIXELSX,
        );
        windows::Win32::Graphics::Gdi::ReleaseDC(None, dc);
        if dpi <= 0 { 1.0 } else { dpi as f32 / 96.0 }
    }
}

fn round_corners(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute};
    let pref: u32 = 2; // DWMWCP_ROUND
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
