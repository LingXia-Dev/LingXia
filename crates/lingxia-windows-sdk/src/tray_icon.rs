//! Windows notification-area tray icon support.
//!
//! The CLI maps `surfaces[].tray` to a `menuBarItem` activator in `ui.json`.
//! This module is the Windows runtime consumer for that activator kind.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NIN_SELECT, NINF_KEY,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    self, AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, GetCursorPos, HICON, MF_STRING, PostMessageW, SetForegroundWindow, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
    WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 0x5b1;
const TRAY_ICON_ID: u32 = 1;
const TRAY_MENU_OPEN: usize = 1;
const TRAY_MENU_QUIT: usize = 2;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

#[derive(Debug, Clone)]
struct TrayItem {
    surface_id: String,
    action_kind: String,
    tooltip: String,
    icon_path: Option<PathBuf>,
}

#[derive(Debug)]
struct TrayState {
    hwnd: isize,
    icon: isize,
    owns_icon: bool,
    item: TrayItem,
}

static TRAY_STATE: OnceLock<Mutex<Option<TrayState>>> = OnceLock::new();

pub(crate) fn install_from_ui(asset_dir: &Path) -> Result<(), String> {
    let Some(item) = tray_item_from_ui(asset_dir)? else {
        return Ok(());
    };

    uninstall();

    let hwnd = create_tray_window()?;
    let (icon, owns_icon) = load_tray_icon(&item)?;
    if icon == 0 {
        unsafe {
            let _ = DestroyWindow(HWND(hwnd as *mut c_void));
        }
        return Err("no usable tray icon handle".to_string());
    }

    let mut data = notify_icon_data(hwnd, icon, &item.tooltip);
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data).as_bool() };
    if !added {
        unsafe {
            let _ = DestroyWindow(HWND(hwnd as *mut c_void));
        }
        if owns_icon {
            destroy_icon(icon);
        }
        return Err("Shell_NotifyIconW(NIM_ADD) failed".to_string());
    }

    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    let _ = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) };

    let state = TRAY_STATE.get_or_init(|| Mutex::new(None));
    let mut state = state
        .lock()
        .map_err(|_| "Windows tray icon state is poisoned".to_string())?;
    *state = Some(TrayState {
        hwnd,
        icon,
        owns_icon,
        item,
    });
    Ok(())
}

pub(crate) fn uninstall() {
    let Some(state) = TRAY_STATE.get() else {
        return;
    };
    let Ok(mut state) = state.lock() else {
        return;
    };
    let Some(state) = state.take() else {
        return;
    };

    let data = notify_icon_data(state.hwnd, state.icon, &state.item.tooltip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        let _ = DestroyWindow(HWND(state.hwnd as *mut c_void));
    }
    if state.owns_icon {
        destroy_icon(state.icon);
    }
}

pub(crate) fn is_installed() -> bool {
    TRAY_STATE
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.as_ref().map(|_| ()))
        .is_some()
}

fn tray_item_from_ui(asset_dir: &Path) -> Result<Option<TrayItem>, String> {
    let ui_path = asset_dir.join("ui.json");
    let text = std::fs::read_to_string(&ui_path)
        .map_err(|err| format!("failed to read {}: {err}", ui_path.display()))?;
    let ui: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse {}: {err}", ui_path.display()))?;

    let Some(activators) = ui.get("activators").and_then(serde_json::Value::as_array) else {
        return Ok(None);
    };

    for activator in activators {
        if activator.get("kind").and_then(serde_json::Value::as_str) != Some("menuBarItem") {
            continue;
        }
        let Some(action) = activator
            .get("action")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        let Some(surface_id) = action
            .get("surface")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let action_kind = action
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("toggleSurface");
        let icon_path = activator
            .get("icon")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| resolve_asset_path(asset_dir, value));
        let tooltip = activator
            .get("label")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                lingxia_app_context::product_name()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or(surface_id)
            .to_string();

        return Ok(Some(TrayItem {
            surface_id: surface_id.to_string(),
            action_kind: action_kind.to_string(),
            tooltip,
            icon_path,
        }));
    }

    Ok(None)
}

fn resolve_asset_path(asset_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        asset_dir.join(path)
    }
}

fn create_tray_window() -> Result<isize, String> {
    let class_name = tray_window_class();
    let hinstance = unsafe { GetModuleHandleW(None) }
        .map(|module| windows::Win32::Foundation::HINSTANCE(module.0))
        .unwrap_or_default();
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance),
            None,
        )
    }
    .map_err(|err| format!("failed to create tray window: {err}"))?;
    Ok(hwnd.0 as isize)
}

fn tray_window_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let class = WNDCLASSW {
            lpfnWndProc: Some(tray_window_proc),
            hInstance: unsafe { GetModuleHandleW(None) }
                .map(|module| windows::Win32::Foundation::HINSTANCE(module.0))
                .unwrap_or_default(),
            lpszClassName: w!("LingXiaTrayIconHost"),
            ..Default::default()
        };
        unsafe {
            WindowsAndMessaging::RegisterClassW(&class);
        }
    });
    w!("LingXiaTrayIconHost")
}

fn load_tray_icon(item: &TrayItem) -> Result<(isize, bool), String> {
    if let Some(path) = item.icon_path.as_ref()
        && path.is_file()
        && let Some(icon) = crate::app_icon::create_icon_handle_from_path(path, 32)
    {
        return Ok((icon, true));
    }
    if let Some(path) = crate::app_icon::current_app_icon_path()
        && path.is_file()
        && let Some(icon) = crate::app_icon::create_icon_handle_from_path(&path, 32)
    {
        return Ok((icon, true));
    }
    Ok((
        crate::app_icon::current_large_icon_handle().unwrap_or(0),
        false,
    ))
}

fn notify_icon_data(hwnd: isize, icon: isize, tooltip: &str) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: HWND(hwnd as *mut c_void),
        uID: TRAY_ICON_ID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: TRAY_CALLBACK_MESSAGE,
        hIcon: HICON(icon as *mut c_void),
        ..Default::default()
    };
    write_tray_tip(&mut data.szTip, tooltip);
    data
}

fn write_tray_tip(target: &mut [u16; 128], tooltip: &str) {
    let max_len = target.len().saturating_sub(1);
    for (slot, ch) in target.iter_mut().take(max_len).zip(tooltip.encode_utf16()) {
        *slot = ch;
    }
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == TRAY_CALLBACK_MESSAGE {
        let notification = (lparam.0 as u32) & 0xffff;
        match notification {
            NIN_SELECT | NIN_KEYSELECT | WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                activate_tray_item();
                return LRESULT(0);
            }
            WM_CONTEXTMENU | WM_RBUTTONUP => {
                show_tray_menu(hwnd);
                return LRESULT(0);
            }
            _ => {}
        }
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn activate_tray_item() {
    let Some(item) = current_item() else {
        return;
    };
    if !crate::shell::handle_menu_bar_surface_action(&item.surface_id, &item.action_kind) {
        log::warn!(
            "Windows tray activator could not handle {} for surface {}",
            item.action_kind,
            item.surface_id
        );
    }
}

fn show_tray_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let open = to_wide("Open");
        let quit = to_wide("Quit");
        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_OPEN, PCWSTR(open.as_ptr()));
        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_QUIT, PCWSTR(quit.as_ptr()));

        let mut point = POINT::default();
        if GetCursorPos(&mut point).is_err() {
            let _ = DestroyMenu(menu);
            return;
        }
        let _ = SetForegroundWindow(hwnd);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_TOPALIGN,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));

        match selected.0 as usize {
            TRAY_MENU_OPEN => open_tray_surface(),
            TRAY_MENU_QUIT => {
                if let Err(err) = lingxia::app::exit() {
                    log::warn!("failed to quit from Windows tray menu: {err}");
                }
            }
            _ => {}
        }
    }
}

fn open_tray_surface() {
    let Some(item) = current_item() else {
        return;
    };
    if !crate::shell::handle_menu_bar_surface_action(&item.surface_id, "openSurface") {
        log::warn!(
            "Windows tray menu could not open surface {}",
            item.surface_id
        );
    }
}

fn current_item() -> Option<TrayItem> {
    TRAY_STATE
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.as_ref().map(|state| state.item.clone()))
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn destroy_icon(handle: isize) {
    if handle == 0 {
        return;
    }
    unsafe {
        let _ = WindowsAndMessaging::DestroyIcon(HICON(handle as *mut c_void));
    }
}
