use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{self, WNDPROC};
use windows::core::{PCWSTR, Result as WinResult};

use crate::webview_host::post_to_window_thread;
use crate::webview_host::{
    WindowsWebViewHostWindow, add_webview_host_window_created_handler,
    request_webview_host_window_layout,
};

use super::{WindowsAppMenu, WindowsAppMenuCommandHandler, WindowsAppMenuEntry};

static APP_MENU_MODEL: OnceLock<Mutex<Vec<WindowsAppMenu>>> = OnceLock::new();
static APP_MENU_HANDLER: OnceLock<Mutex<Option<WindowsAppMenuCommandHandler>>> = OnceLock::new();
static HOST_WINDOWS: OnceLock<Mutex<Vec<isize>>> = OnceLock::new();
static WINDOW_STATES: OnceLock<Mutex<HashMap<isize, MenuWindowState>>> = OnceLock::new();
static MENU_SUPPORT_INSTALLED: OnceLock<()> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct MenuWindowState {
    original_proc: isize,
}

pub(crate) fn install_host_window_menu_support() {
    MENU_SUPPORT_INSTALLED.get_or_init(|| {
        add_webview_host_window_created_handler(Arc::new(|window| {
            track_host_window(window);
            let hwnd = hwnd_from_handle(window);
            install_menu_subclass(hwnd);
            apply_app_menu_to_window(hwnd);
        }));
    });
}

/// Installs (or replaces) the Windows menu-bar model.
pub fn set_windows_app_menu(menus: Vec<WindowsAppMenu>) {
    install_host_window_menu_support();
    if menus.is_empty() {
        log::warn!("ignoring empty Windows app menu model");
        return;
    }
    let model = APP_MENU_MODEL.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut model) = model.lock() {
        *model = menus;
    }
    apply_menu_to_tracked_windows();
}

/// Registers the handler for Windows menu command ids.
pub fn set_windows_app_menu_command_handler(handler: WindowsAppMenuCommandHandler) {
    install_host_window_menu_support();
    let slot = APP_MENU_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(handler.clone());
    }
    crate::device_frame::set_device_frame_command_handler(handler);
}

pub(crate) fn refresh_host_window_menu(window: isize) {
    let _ = post_to_window_thread(
        window,
        Box::new(move || {
            let hwnd = hwnd_from_handle(window);
            install_menu_subclass(hwnd);
            apply_app_menu_to_window(hwnd);
        }),
    );
}

fn track_host_window(window: isize) {
    let windows = HOST_WINDOWS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut windows) = windows.lock()
        && !windows.contains(&window)
    {
        windows.push(window);
    }
}

fn tracked_host_windows() -> Vec<isize> {
    HOST_WINDOWS
        .get()
        .and_then(|windows| windows.lock().ok())
        .map(|windows| {
            windows
                .iter()
                .copied()
                .filter(|window| is_window_valid(*window))
                .collect()
        })
        .unwrap_or_default()
}

fn apply_menu_to_tracked_windows() {
    for window in tracked_host_windows() {
        let _ = post_to_window_thread(
            window,
            Box::new(move || {
                let hwnd = hwnd_from_handle(window);
                install_menu_subclass(hwnd);
                apply_app_menu_to_window(hwnd);
            }),
        );
    }
}

fn app_menu_model_snapshot() -> Vec<WindowsAppMenu> {
    APP_MENU_MODEL
        .get()
        .and_then(|model| model.lock().ok())
        .map(|model| model.clone())
        .unwrap_or_default()
}

fn menu_defines_command(id: u32) -> bool {
    app_menu_model_snapshot().iter().any(|menu| {
        menu.entries
            .iter()
            .any(|entry| matches!(entry, WindowsAppMenuEntry::Item(item) if item.id == id))
    })
}

fn accelerator_command(vk: u32) -> Option<u32> {
    app_menu_model_snapshot().iter().find_map(|menu| {
        menu.entries.iter().find_map(|entry| match entry {
            WindowsAppMenuEntry::Item(item) if item.accelerator_vk == Some(vk) => Some(item.id),
            _ => None,
        })
    })
}

fn append_app_menus(parent: WindowsAndMessaging::HMENU, menus: &[WindowsAppMenu]) -> WinResult<()> {
    for menu in menus {
        let popup = unsafe { WindowsAndMessaging::CreateMenu() }?;
        for entry in &menu.entries {
            match entry {
                WindowsAppMenuEntry::Separator => unsafe {
                    WindowsAndMessaging::AppendMenuW(
                        popup,
                        WindowsAndMessaging::MF_SEPARATOR,
                        0,
                        PCWSTR::null(),
                    )?;
                },
                WindowsAppMenuEntry::Item(item) => {
                    let mut flags = WindowsAndMessaging::MF_STRING;
                    if item.checked {
                        flags |= WindowsAndMessaging::MF_CHECKED;
                    }
                    let label = to_wide(&item.label);
                    unsafe {
                        WindowsAndMessaging::AppendMenuW(
                            popup,
                            flags,
                            item.id as usize,
                            PCWSTR(label.as_ptr()),
                        )?;
                    }
                }
            }
        }
        let title = to_wide(&menu.title);
        unsafe {
            WindowsAndMessaging::AppendMenuW(
                parent,
                WindowsAndMessaging::MF_POPUP,
                popup.0 as usize,
                PCWSTR(title.as_ptr()),
            )?;
        }
    }
    Ok(())
}

fn build_app_menu(menus: &[WindowsAppMenu]) -> Option<WindowsAndMessaging::HMENU> {
    let bar = unsafe { WindowsAndMessaging::CreateMenu() }.ok()?;
    if let Err(err) = append_app_menus(bar, menus) {
        log::warn!("Windows app menu construction failed: {err}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyMenu(bar);
        }
        return None;
    }
    Some(bar)
}

fn apply_app_menu_to_window(hwnd: HWND) {
    if !is_hwnd_valid(hwnd) {
        return;
    }
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(hwnd, None) };
    if owner_thread != 0 && owner_thread != unsafe { Threading::GetCurrentThreadId() } {
        let handle = hwnd_handle(hwnd);
        let _ = post_to_window_thread(
            handle,
            Box::new(move || apply_app_menu_to_window(hwnd_from_handle(handle))),
        );
        return;
    }

    let model = app_menu_model_snapshot();
    if model.is_empty() || looks_like_device_frame_screen(hwnd) {
        return;
    }
    let Some(menu) = build_app_menu(&model) else {
        return;
    };
    let previous = unsafe { WindowsAndMessaging::GetMenu(hwnd) };
    if unsafe { WindowsAndMessaging::SetMenu(hwnd, Some(menu)) }.is_err() {
        unsafe {
            let _ = WindowsAndMessaging::DestroyMenu(menu);
        }
        return;
    }
    if !previous.is_invalid() {
        unsafe {
            let _ = WindowsAndMessaging::DestroyMenu(previous);
        }
    }
    unsafe {
        let _ = WindowsAndMessaging::DrawMenuBar(hwnd);
    }
    request_webview_host_window_layout(WindowsWebViewHostWindow {
        window: hwnd_handle(hwnd),
    });
}

fn install_menu_subclass(hwnd: HWND) {
    if !is_hwnd_valid(hwnd) {
        return;
    }
    let key = hwnd_handle(hwnd);
    let states = WINDOW_STATES.get_or_init(|| Mutex::new(HashMap::new()));
    if states
        .lock()
        .ok()
        .is_some_and(|states| states.contains_key(&key))
    {
        return;
    }

    let original_proc = unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            WindowsAndMessaging::GWLP_WNDPROC,
            menu_window_proc as *const () as usize as isize,
        )
    };
    if original_proc == 0 {
        log::warn!("failed to subclass Windows app menu host window");
        return;
    }
    if let Ok(mut states) = states.lock() {
        states.insert(key, MenuWindowState { original_proc });
    }
}

unsafe extern "system" fn menu_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let original = menu_window_state(hwnd).map(|state| state.original_proc);
    if msg == WindowsAndMessaging::WM_COMMAND {
        if handle_app_menu_wm_command(wparam) {
            return LRESULT(0);
        }
    } else if msg == WindowsAndMessaging::WM_KEYDOWN {
        if handle_app_menu_keydown(wparam) {
            return LRESULT(0);
        }
    } else if msg == WindowsAndMessaging::WM_NCDESTROY {
        if let Some(state) = remove_menu_window_state(hwnd) {
            unsafe {
                let _ = WindowsAndMessaging::SetWindowLongPtrW(
                    hwnd,
                    WindowsAndMessaging::GWLP_WNDPROC,
                    state.original_proc,
                );
            }
            return unsafe { call_original(state.original_proc, hwnd, msg, wparam, lparam) };
        }
    }

    match original {
        Some(original) => unsafe { call_original(original, hwnd, msg, wparam, lparam) },
        None => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn handle_app_menu_wm_command(wparam: WPARAM) -> bool {
    if (wparam.0 >> 16) & 0xffff > 1 {
        return false;
    }
    let id = (wparam.0 & 0xffff) as u32;
    if id == 0 || !menu_defines_command(id) {
        return false;
    }
    dispatch_app_menu_command(id)
}

fn handle_app_menu_keydown(wparam: WPARAM) -> bool {
    let modifier_down = unsafe {
        GetKeyState(VK_CONTROL.0 as i32) < 0
            || GetKeyState(VK_SHIFT.0 as i32) < 0
            || GetKeyState(VK_MENU.0 as i32) < 0
    };
    if modifier_down {
        return false;
    }
    let Some(id) = accelerator_command(wparam.0 as u32) else {
        return false;
    };
    dispatch_app_menu_command(id)
}

fn dispatch_app_menu_command(id: u32) -> bool {
    let handler = APP_MENU_HANDLER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone());
    let Some(handler) = handler else {
        return false;
    };
    let _ = std::thread::Builder::new()
        .name(format!("lingxia-windows-menu-{id}"))
        .spawn(move || handler(id));
    true
}

fn menu_window_state(hwnd: HWND) -> Option<MenuWindowState> {
    WINDOW_STATES
        .get()
        .and_then(|states| states.lock().ok())
        .and_then(|states| states.get(&hwnd_handle(hwnd)).copied())
}

fn remove_menu_window_state(hwnd: HWND) -> Option<MenuWindowState> {
    WINDOW_STATES
        .get()
        .and_then(|states| states.lock().ok())
        .and_then(|mut states| states.remove(&hwnd_handle(hwnd)))
}

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

fn looks_like_device_frame_screen(hwnd: HWND) -> bool {
    let style =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_STYLE) }
            as u32;
    style & WindowsAndMessaging::WS_POPUP.0 != 0
        && style & WindowsAndMessaging::WS_CAPTION.0 == 0
        && style & WindowsAndMessaging::WS_THICKFRAME.0 == 0
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

fn is_window_valid(window: isize) -> bool {
    is_hwnd_valid(hwnd_from_handle(window))
}

fn is_hwnd_valid(hwnd: HWND) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd)).as_bool() }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
