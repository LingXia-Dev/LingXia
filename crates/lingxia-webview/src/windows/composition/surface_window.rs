//! The per-webview surface child window hosting the DirectComposition
//! target. Input forwarding to the composition controller attaches here.

use super::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_EX_NOREDIRECTIONBITMAP,
};

const SURFACE_CLASS: PCWSTR = windows::core::w!("LingXiaWebViewSurface");

fn ensure_surface_class() -> bool {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    *REGISTERED.get_or_init(|| unsafe {
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(surface_proc),
            hInstance: GetModuleHandleW(None).map(Into::into).unwrap_or_default(),
            hCursor: WindowsAndMessaging::LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: SURFACE_CLASS,
            ..Default::default()
        };
        WindowsAndMessaging::RegisterClassW(&class) != 0
    })
}

/// Creates the (initially hidden) surface child window on the calling
/// thread — the webview's UI thread, so its messages pump there.
/// `WS_EX_NOREDIRECTIONBITMAP`: the DComp tree is the window's only content,
/// so skip the GDI redirection surface entirely.
pub(crate) fn create_surface_window(parent: HWND, bounds: RECT) -> StdResult<HWND> {
    if !ensure_surface_class() {
        return Err(WebViewError::WebView(
            "RegisterClassW(LingXiaWebViewSurface) failed".to_string(),
        ));
    }
    unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            SURFACE_CLASS,
            PCWSTR::null(),
            WS_CHILD | WS_CLIPSIBLINGS,
            bounds.left,
            bounds.top,
            (bounds.right - bounds.left).max(0),
            (bounds.bottom - bounds.top).max(0),
            Some(parent),
            None,
            GetModuleHandleW(None).map(Into::into).ok(),
            None,
        )
        .map_err(|err| WebViewError::WebView(format!("CreateWindowExW(surface) failed: {err}")))
    }
}

unsafe extern "system" fn surface_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        // No redirection surface to erase.
        WindowsAndMessaging::WM_ERASEBKGND => windows::Win32::Foundation::LRESULT(1),
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
