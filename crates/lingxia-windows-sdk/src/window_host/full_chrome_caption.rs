//! Caption overlay for `chrome: 'full'` page windows.
//!
//! The page WebView fills the client (edge-to-edge), so host-painted caption
//! buttons would sit *under* the composition surface and its child HWND would
//! swallow every mouse event — no close box, no drag. This owned layered
//! popup sits above that surface: a 1/255-alpha drag strip plus
//! min/max/close controls, forwarding `HTCAPTION` to the host
//! so the window still moves.

use super::*;
use lingxia_windows_contract::WindowsFrameButton;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BeginPaint,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, EndPaint, GetDC,
    HGDIOBJ, PAINTSTRUCT, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::WindowsAndMessaging::{self, GW_OWNER, WNDCLASSW, WS_POPUP};
use windows::core::{PCWSTR, w};

#[derive(Clone, Copy)]
struct CaptionOverlay {
    window: isize,
}

#[derive(Clone, Copy, Default)]
struct OverlayInteraction {
    hover: Option<WindowsFrameButton>,
    pressed: Option<WindowsFrameButton>,
}

static OVERLAYS: OnceLock<Mutex<HashMap<isize, CaptionOverlay>>> = OnceLock::new();
static INTERACTIONS: OnceLock<Mutex<HashMap<isize, OverlayInteraction>>> = OnceLock::new();

pub(super) fn sync_full_chrome_caption_overlay(hwnd: HWND) {
    if !is_full_chrome_window(hwnd) || !is_window_visible(hwnd) || is_minimized(hwnd) {
        destroy_full_chrome_caption_overlay(hwnd);
        return;
    }
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(hwnd, &mut client).is_err() {
            destroy_full_chrome_caption_overlay(hwnd);
            return;
        }
    }
    let rect = caption_strip_rect(hwnd, client);
    if rect.right <= rect.left || rect.bottom <= rect.top {
        destroy_full_chrome_caption_overlay(hwnd);
        return;
    }

    let overlay = ensure_overlay(hwnd, rect);
    if overlay == 0 {
        return;
    }
    // Positioning can synchronously reenter the owner's layout handler.
    if let Ok(mut overlays) = OVERLAYS.get_or_init(|| Mutex::new(HashMap::new())).lock() {
        overlays.insert(hwnd_handle(hwnd), CaptionOverlay { window: overlay });
    }
    let mut origin = POINT {
        x: rect.left,
        y: rect.top,
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin);
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(overlay),
            Some(WindowsAndMessaging::HWND_TOP),
            origin.x,
            origin.y,
            rect.right - rect.left,
            rect.bottom - rect.top,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
    upload_overlay(hwnd_from_handle(overlay));
}

pub(super) fn destroy_full_chrome_caption_overlay(hwnd: HWND) {
    let overlay = OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|mut overlays| overlays.remove(&hwnd_handle(hwnd)));
    if let Some(overlay) = overlay
        && is_window_handle_valid(overlay.window)
    {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(overlay.window));
        }
    }
    if let Some(interactions) = INTERACTIONS.get()
        && let Ok(mut interactions) = interactions.lock()
    {
        interactions.remove(&hwnd_handle(hwnd));
    }
}

fn caption_strip_height(hwnd: HWND) -> i32 {
    full_chrome_drag_strip_pixels(hwnd)
}

fn caption_strip_rect(hwnd: HWND, client: RECT) -> RECT {
    RECT {
        left: client.left,
        top: client.top,
        right: client.right,
        bottom: (client.top + caption_strip_height(hwnd)).min(client.bottom),
    }
}

fn overlay_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            style: WindowsAndMessaging::CS_DBLCLKS,
            lpfnWndProc: Some(overlay_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaFullChromeCaption"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "full-chrome caption overlay class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaFullChromeCaption")
}

fn ensure_overlay(host: HWND, rect: RECT) -> isize {
    if let Some(existing) = OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|overlays| overlays.get(&hwnd_handle(host)).copied())
        && is_window_handle_valid(existing.window)
    {
        return existing.window;
    }

    let class = overlay_class();
    let instance = unsafe { LibraryLoader::GetModuleHandleW(None) }
        .ok()
        .map(|module| HINSTANCE(module.0));
    let overlay = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            class,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
            Some(host),
            None,
            instance,
            None,
        )
    };
    match overlay {
        Ok(hwnd) => hwnd_handle(hwnd),
        Err(_) => 0,
    }
}

fn overlay_host(hwnd: HWND) -> Option<HWND> {
    let owner = unsafe { WindowsAndMessaging::GetWindow(hwnd, GW_OWNER) }.ok()?;
    (!owner.0.is_null()).then_some(owner)
}

fn overlay_client(hwnd: HWND) -> RECT {
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    client
}

fn overlay_interaction(host: HWND) -> OverlayInteraction {
    INTERACTIONS
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.get(&hwnd_handle(host)).copied())
        .unwrap_or_default()
}

fn update_overlay_interaction(host: HWND, update: impl FnOnce(&mut OverlayInteraction)) {
    let state = INTERACTIONS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut state) = state.lock() {
        update(state.entry(hwnd_handle(host)).or_default());
    }
}

fn button_at(client: RECT, point: (i32, i32)) -> Option<WindowsFrameButton> {
    caption_button_rects(client)
        .into_iter()
        .find(|(_, rect)| {
            point.0 >= rect.left
                && point.0 < rect.right
                && point.1 >= rect.top
                && point.1 < rect.bottom
        })
        .map(|(button, _)| button)
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                let _ = BeginPaint(hwnd, &mut ps);
                let _ = EndPaint(hwnd, &ps);
            }
            upload_overlay(hwnd);
            LRESULT(0)
        }
        WindowsAndMessaging::WM_ERASEBKGND => LRESULT(1),
        WindowsAndMessaging::WM_NCHITTEST => {
            if let Some(host) = overlay_host(hwnd) {
                let mut point = lparam_screen_point(lparam);
                unsafe {
                    let _ = ScreenToClient(host, &mut point);
                }
                if let Some(hit) = resize_hit_test(host, (point.x, point.y)) {
                    return hit;
                }
            }
            LRESULT(WindowsAndMessaging::HTCLIENT as isize)
        }
        WindowsAndMessaging::WM_NCLBUTTONDOWN => {
            if let Some(host) = overlay_host(hwnd) {
                // Resize the owner, never the caption popup itself.
                unsafe {
                    let _ =
                        WindowsAndMessaging::SendMessageW(host, msg, Some(wparam), Some(lparam));
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_MOUSEMOVE => {
            if let Some(host) = overlay_host(hwnd) {
                let point = lparam_client_point(lparam);
                let hover = button_at(overlay_client(hwnd), point);
                if overlay_interaction(host).hover != hover {
                    update_overlay_interaction(host, |state| state.hover = hover);
                    upload_overlay(hwnd);
                }
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe {
                    let _ = TrackMouseEvent(&mut track);
                }
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if let Some(host) = overlay_host(hwnd) {
                update_overlay_interaction(host, |state| {
                    state.hover = None;
                });
                upload_overlay(hwnd);
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONDOWN | WindowsAndMessaging::WM_LBUTTONDBLCLK => {
            if let Some(host) = overlay_host(hwnd) {
                let point = lparam_client_point(lparam);
                if let Some(button) = button_at(overlay_client(hwnd), point) {
                    update_overlay_interaction(host, |state| {
                        state.hover = Some(button);
                        state.pressed = Some(button);
                    });
                    upload_overlay(hwnd);
                    unsafe {
                        let _ = SetCapture(hwnd);
                    }
                } else if msg == WindowsAndMessaging::WM_LBUTTONDBLCLK {
                    handle_frame_button(host, WindowsFrameButton::Maximize);
                } else {
                    begin_host_caption_drag(host, hwnd, lparam);
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            if let Some(host) = overlay_host(hwnd) {
                // ReleaseCapture synchronously sends WM_CAPTURECHANGED, which
                // clears the interaction state before it returns.
                let pressed = overlay_interaction(host).pressed;
                unsafe {
                    let _ = ReleaseCapture();
                }
                let point = lparam_client_point(lparam);
                update_overlay_interaction(host, |state| state.pressed = None);
                upload_overlay(hwnd);
                if let Some(button) = pressed
                    && Some(button) == button_at(overlay_client(hwnd), point)
                {
                    handle_frame_button(host, button);
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_CAPTURECHANGED | WindowsAndMessaging::WM_CANCELMODE => {
            if let Some(host) = overlay_host(hwnd) {
                update_overlay_interaction(host, |state| state.pressed = None);
                upload_overlay(hwnd);
            }
            LRESULT(0)
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn begin_host_caption_drag(host: HWND, overlay: HWND, lparam: LPARAM) {
    let (x, y) = lparam_client_point(lparam);
    let mut screen = POINT { x, y };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(overlay, &mut screen);
        let _ = ReleaseCapture();
        let packed = ((screen.y as u32) << 16) | (screen.x as u32 & 0xffff);
        let _ = WindowsAndMessaging::SendMessageW(
            host,
            WindowsAndMessaging::WM_NCLBUTTONDOWN,
            Some(WPARAM(WindowsAndMessaging::HTCAPTION as usize)),
            Some(LPARAM(packed as isize)),
        );
    }
}

fn caption_button_rects(client: RECT) -> [(WindowsFrameButton, RECT); 3] {
    let scale = (client.bottom - client.top) as f32 / 28.0;
    let width = (46.0 * scale).round() as i32;
    [
        WindowsFrameButton::Minimize,
        WindowsFrameButton::Maximize,
        WindowsFrameButton::Close,
    ]
    .map(|button| {
        let offset = match button {
            WindowsFrameButton::Minimize => 3,
            WindowsFrameButton::Maximize => 2,
            WindowsFrameButton::Close => 1,
        };
        (
            button,
            RECT {
                left: (client.right - offset * width).max(client.left),
                right: (client.right - (offset - 1) * width).max(client.left),
                top: client.top,
                bottom: client.bottom,
            },
        )
    })
}

fn paint_caption(
    client: RECT,
    interaction: OverlayInteraction,
    host: HWND,
) -> Option<tiny_skia::Pixmap> {
    use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

    let mut pixmap = Pixmap::new(
        (client.right - client.left) as u32,
        (client.bottom - client.top) as u32,
    )?;
    // Nonzero alpha keeps the otherwise transparent drag strip hit-testable.
    pixmap.fill(Color::from_rgba8(0, 0, 0, 1));
    let (_, icon, dark) = crate::shell::windows_shell_frame_colors();
    let scale = (client.bottom - client.top) as f32 / 28.0;
    for (button, rect) in caption_button_rects(client) {
        let hovered = interaction.hover == Some(button)
            && (interaction.pressed.is_none() || interaction.pressed == Some(button));
        let pressed = hovered && interaction.pressed == Some(button);
        let close_hover = hovered && button == WindowsFrameButton::Close;
        let mut paint = Paint::default();
        if hovered {
            if close_hover {
                paint.set_color_rgba8(196 + if pressed { 0 } else { 36 }, 43, 43, 255);
            } else {
                let channel = if dark { 255 } else { 0 };
                paint.set_color_rgba8(channel, channel, channel, if pressed { 32 } else { 18 });
            }
            if let Some(bounds) = tiny_skia::Rect::from_ltrb(
                rect.left as f32,
                rect.top as f32,
                rect.right as f32,
                rect.bottom as f32,
            ) {
                pixmap.fill_rect(bounds, &paint, Transform::identity(), None);
            }
        }
        let rgb = if close_hover { 0xffffff } else { icon };
        paint.set_color_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255);
        let cx = (rect.left + rect.right) as f32 / 2.0;
        let cy = (rect.top + rect.bottom) as f32 / 2.0;
        let mut path = PathBuilder::new();
        match button {
            WindowsFrameButton::Minimize => {
                path.move_to(-5.0, 0.0);
                path.line_to(5.0, 0.0);
            }
            WindowsFrameButton::Close => {
                path.move_to(-4.5, -4.5);
                path.line_to(4.5, 4.5);
                path.move_to(4.5, -4.5);
                path.line_to(-4.5, 4.5);
            }
            WindowsFrameButton::Maximize
                if unsafe { WindowsAndMessaging::IsZoomed(host).as_bool() } =>
            {
                path.push_rect(tiny_skia::Rect::from_ltrb(-5.0, -2.5, 2.5, 5.0)?);
                path.move_to(-2.5, -2.5);
                path.line_to(-2.5, -5.0);
                path.line_to(5.0, -5.0);
                path.line_to(5.0, 2.5);
                path.line_to(2.5, 2.5);
            }
            WindowsFrameButton::Maximize => {
                path.push_rect(tiny_skia::Rect::from_ltrb(-4.5, -4.5, 4.5, 4.5)?);
            }
        }
        let stroke = Stroke {
            width: 1.0,
            ..Default::default()
        };
        pixmap.stroke_path(
            &path.finish()?,
            &paint,
            &stroke,
            Transform::from_row(scale, 0.0, 0.0, scale, cx, cy),
            None,
        );
    }
    Some(pixmap)
}

fn upload_overlay(hwnd: HWND) {
    let Some(host) = overlay_host(hwnd) else {
        return;
    };
    let client = overlay_client(hwnd);
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);
    let interaction = overlay_interaction(host);
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return;
        }
        let dc = CreateCompatibleDC(Some(screen));
        if dc.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        }
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (width * height) as usize);
        if let Some(pixmap) = paint_caption(client, interaction, host) {
            // tiny-skia and UpdateLayeredWindow both use premultiplied alpha;
            // only the channel order differs (RGBA versus BGRA).
            for (out, color) in pixels.iter_mut().zip(pixmap.pixels()) {
                *out =
                    u32::from_be_bytes([color.alpha(), color.red(), color.green(), color.blue()]);
            }
        } else {
            pixels.fill(0x0100_0000);
        }
        let size = windows::Win32::Foundation::SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = WindowsAndMessaging::UpdateLayeredWindow(
            hwnd,
            Some(screen),
            None,
            Some(&size),
            Some(dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            WindowsAndMessaging::ULW_ALPHA,
        );
        if !old_bitmap.is_invalid() {
            let _ = SelectObject(dc, old_bitmap);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = ReleaseDC(None, screen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_buttons_do_not_extend_below_the_page_inset() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 480,
            bottom: 28,
        };
        assert_eq!(
            button_at(client, (460, 16)),
            Some(WindowsFrameButton::Close)
        );
        assert_eq!(button_at(client, (460, 28)), None);
        assert_eq!(button_at(client, (80, 12)), None);
    }
}
