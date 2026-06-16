//! Layered companion window for a presented Windows device frame.

use super::paint::paint_frame_window;
use super::*;

/// Moves the content window to track a shell-initiated drag.
fn sync_content_to_frame(frame: HWND) {
    let Some((content, offset)) = frame_state_by_frame(frame, |content, state| {
        (content, state.layout.content_offset)
    }) else {
        return;
    };
    if !is_window_handle_valid(content) {
        return;
    }
    let mut frame_rect = RECT::default();
    let mut content_rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(frame, &mut frame_rect);
        let _ = WindowsAndMessaging::GetWindowRect(hwnd_from_handle(content), &mut content_rect);
    }
    let x = frame_rect.left + offset.0;
    let y = frame_rect.top + offset.1;
    if content_rect.left == x && content_rect.top == y {
        return;
    }
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(content),
            None,
            x,
            y,
            0,
            0,
            WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}

fn point_in_rect(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

/// Handles a click on one of the toolbar's interactive rects (shell-local
/// `x`/`y`). Close and minimize act on the content window directly; the
/// selector and the action glyph dispatch device-frame command ids through
/// the registered device-frame command handler.
fn handle_toolbar_click(frame: HWND, x: i32, y: i32) {
    let Some((content, spec, layout)) = frame_state_by_frame(frame, |content, state| {
        (content, state.spec.clone(), state.layout)
    }) else {
        return;
    };
    let content = hwnd_from_handle(content);
    if point_in_rect(&layout.close_rect, x, y) {
        unsafe {
            let _ = WindowsAndMessaging::PostMessageW(
                Some(content),
                WindowsAndMessaging::WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
            );
        }
    } else if point_in_rect(&layout.minimize_rect, x, y) {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(content, WindowsAndMessaging::SW_MINIMIZE);
        }
    } else if point_in_rect(&layout.selector_rect, x, y) {
        let Some(toolbar) = spec.toolbar else {
            return;
        };
        show_selector_menu(frame, content, &layout, &toolbar);
    } else if point_in_rect(&layout.action_rect, x, y)
        && let Some(command) = spec.toolbar.and_then(|toolbar| toolbar.action_command)
    {
        dispatch_device_frame_command(command);
    }
}

/// Drops the selector's item list below the selector rect and dispatches
/// the chosen command id.
fn show_selector_menu(
    frame: HWND,
    content: HWND,
    layout: &FrameLayout,
    toolbar: &WindowsDeviceFrameToolbar,
) {
    let Ok(popup) = (unsafe { WindowsAndMessaging::CreatePopupMenu() }) else {
        return;
    };
    for item in &toolbar.selector_items {
        let mut flags = WindowsAndMessaging::MF_STRING;
        if item.checked {
            flags |= WindowsAndMessaging::MF_CHECKED;
        }
        let label = to_wide(&item.label);
        unsafe {
            let _ = WindowsAndMessaging::AppendMenuW(
                popup,
                flags,
                item.id as usize,
                PCWSTR(label.as_ptr()),
            );
        }
    }
    let mut anchor = POINT {
        x: layout.selector_rect.left,
        y: layout.toolbar.bottom + 2,
    };
    unsafe {
        let _ = ClientToScreen(frame, &mut anchor);
        let _ = WindowsAndMessaging::SetForegroundWindow(content);
    }
    let selected = unsafe {
        WindowsAndMessaging::TrackPopupMenu(
            popup,
            WindowsAndMessaging::TPM_LEFTALIGN
                | WindowsAndMessaging::TPM_TOPALIGN
                | WindowsAndMessaging::TPM_RETURNCMD
                | WindowsAndMessaging::TPM_NONOTIFY,
            anchor.x,
            anchor.y,
            None,
            content,
            None,
        )
    };
    unsafe {
        let _ = WindowsAndMessaging::DestroyMenu(popup);
    }
    let id = selected.0 as u32;
    if id != 0 {
        dispatch_device_frame_command(id);
    }
}

/// Shell window procedure: a layered window supporting dragging the
/// assembly (toolbar background and bezel act as a caption), the toolbar
/// buttons. It never takes
/// activation — focus stays on the screen window.
unsafe extern "system" fn frame_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCHITTEST {
        let screen_x = (lparam.0 & 0xffff) as i16 as i32;
        let screen_y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
        let mut rect = RECT::default();
        unsafe {
            let _ = WindowsAndMessaging::GetWindowRect(hwnd, &mut rect);
        }
        let x = screen_x - rect.left;
        let y = screen_y - rect.top;
        let hit = frame_state_by_frame(hwnd, |_, state| {
            let layout = &state.layout;
            if point_in_rect(&layout.close_rect, x, y)
                || point_in_rect(&layout.minimize_rect, x, y)
                || point_in_rect(&layout.selector_rect, x, y)
                || point_in_rect(&layout.action_rect, x, y)
            {
                WindowsAndMessaging::HTCLIENT
            } else if point_in_rect(&layout.toolbar, x, y) || point_in_rect(&layout.bezel, x, y) {
                WindowsAndMessaging::HTCAPTION
            } else {
                WindowsAndMessaging::HTTRANSPARENT as u32
            }
        })
        .unwrap_or(WindowsAndMessaging::HTTRANSPARENT as u32);
        return LRESULT(hit as isize);
    } else if msg == WindowsAndMessaging::WM_MOUSEACTIVATE {
        return LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize);
    } else if msg == WindowsAndMessaging::WM_LBUTTONDOWN {
        let x = (lparam.0 & 0xffff) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
        handle_toolbar_click(hwnd, x, y);
        return LRESULT(0);
    } else if msg == WindowsAndMessaging::WM_NCLBUTTONDBLCLK {
        // No maximize semantics on a fixed-size device.
        return LRESULT(0);
    } else if msg == WindowsAndMessaging::WM_WINDOWPOSCHANGING {
        // The shell's device face is opaque, so it must never stack above
        // the screen window — but DefWindowProc raises a window dragged by
        // HTCAPTION. Rewrite every pending placement to sit directly below
        // the content window instead.
        let content = frame_state_by_frame(hwnd, |content, _| content);
        if let Some(content) = content.filter(|content| is_window_handle_valid(*content)) {
            let pos = lparam.0 as *mut WindowsAndMessaging::WINDOWPOS;
            if !pos.is_null() {
                unsafe {
                    (*pos).hwndInsertAfter = hwnd_from_handle(content);
                    (*pos).flags &= !WindowsAndMessaging::SWP_NOZORDER;
                }
            }
        }
        // Fall through so DefWindowProc still applies the placement.
    } else if msg == WindowsAndMessaging::WM_WINDOWPOSCHANGED {
        let pos = lparam.0 as *const WindowsAndMessaging::WINDOWPOS;
        if !pos.is_null() && !unsafe { (*pos).flags }.contains(WindowsAndMessaging::SWP_NOMOVE) {
            sync_content_to_frame(hwnd);
        }
        // Fall through for default WM_MOVE generation.
    } else if msg == WindowsAndMessaging::WM_ENTERSIZEMOVE {
        // Dragging grabs the shell, but the assembly should rise as one;
        // raising the content also restacks the shell directly below it
        // (see the z-order sync in the content's WM_WINDOWPOSCHANGED).
        let content = frame_state_by_frame(hwnd, |content, _| content);
        if let Some(content) = content.filter(|content| is_window_handle_valid(*content)) {
            unsafe {
                let _ = WindowsAndMessaging::SetWindowPos(
                    hwnd_from_handle(content),
                    Some(WindowsAndMessaging::HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    WindowsAndMessaging::SWP_NOMOVE
                        | WindowsAndMessaging::SWP_NOSIZE
                        | WindowsAndMessaging::SWP_NOACTIVATE,
                );
            }
        }
        // Track the modal drag loop at timer cadence — WM_WINDOWPOSCHANGED
        // is coalesced inside it (same pattern as the host windows).
        unsafe {
            let _ = WindowsAndMessaging::SetTimer(
                Some(hwnd),
                SIZEMOVE_TIMER_ID,
                SIZEMOVE_TIMER_INTERVAL_MS,
                None,
            );
        }
    } else if msg == WindowsAndMessaging::WM_EXITSIZEMOVE {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(hwnd), SIZEMOVE_TIMER_ID);
        }
        sync_content_to_frame(hwnd);
    } else if msg == WindowsAndMessaging::WM_TIMER && wparam.0 == SIZEMOVE_TIMER_ID {
        sync_content_to_frame(hwnd);
        return LRESULT(0);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn frame_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(frame_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaDeviceFrame"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device frame class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceFrame")
}

/// Creates the layered shell window behind `content` and uploads its
/// per-pixel-alpha bitmap (toolbar + bezel + shadow). Returns the window
/// and the layout completed with the text-dependent toolbar rects, or
/// `None` when creation fails (requires the Win8+ `supportedOS` manifest,
/// like the corner caps).
pub(super) fn create_frame_window(
    content: HWND,
    spec: &WindowsDeviceFrame,
    mut layout: FrameLayout,
) -> Option<(HWND, FrameLayout)> {
    let result = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            frame_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            layout.width,
            layout.height,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            Some(hwnd_handle(content) as *const c_void),
        )
    };
    let frame = match result {
        Ok(frame) => frame,
        Err(err) => {
            log::warn!("device frame window creation failed: {err}");
            return None;
        }
    };
    paint_frame_window(frame, spec, &mut layout);
    Some((frame, layout))
}
