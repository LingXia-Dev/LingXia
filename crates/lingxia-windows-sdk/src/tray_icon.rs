//! Windows notification-area tray icon support.
//!
//! The CLI maps `surfaces[].tray` to a `menuBarItem` activator in `ui.json`.
//! This module is the Windows runtime consumer for that activator kind.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NIN_SELECT, NINF_KEY,
    NOTIFYICON_VERSION_4, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER, Shell_NotifyIconGetRect,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    self, AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
    DestroyWindow, GetCursorPos, HICON, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    PostMessageW, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL,
    WM_RBUTTONUP, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 0x5b1;
const TRAY_ICON_ID: u32 = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

/// One entry in the JS-registered tray dropdown (`lx.tray.setMenu`). The native
/// menu carries no default items; the developer supplies them (and their
/// click handlers) — mirroring the macOS tray contract.
#[derive(Debug, Clone, Default)]
struct TrayMenuItem {
    separator: bool,
    label: String,
    enabled: bool,
    checked: bool,
}

/// The current JS-registered menu spec. Empty => a right-click shows nothing
/// (no default Open/Quit). Replaced wholesale on each `lx.tray.setMenu`.
static TRAY_MENU: Mutex<Vec<TrayMenuItem>> = Mutex::new(Vec::new());

/// While set, a tray left-click is delivered to JS (`lx.tray.onClick`) instead
/// of running the tray's configured surface action.
static TRAY_CLICK_INTERCEPT: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
struct TrayItem {
    surface_id: String,
    app_id: String,
    page: Option<String>,
    query: Option<serde_json::Value>,
    action_kind: String,
    tooltip: String,
    icon_path: Option<PathBuf>,
    window_size: Option<(i32, i32)>,
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

/// Replace the JS-registered tray dropdown. `items_json` is the array
/// `lx.tray.setMenu` produced: each entry is `{ "separator": true }` or
/// `{ "label", "enabled", "checked" }`. Invoked via the platform tray-menu
/// handler; the menu is rebuilt on demand when the icon is right-clicked.
pub(crate) fn set_menu(items_json: &str) {
    let parsed: Vec<TrayMenuItem> = serde_json::from_str::<Vec<serde_json::Value>>(items_json)
        .unwrap_or_default()
        .into_iter()
        .map(|value| {
            if value
                .get("separator")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                return TrayMenuItem {
                    separator: true,
                    ..Default::default()
                };
            }
            TrayMenuItem {
                separator: false,
                label: value
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                enabled: value
                    .get("enabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
                checked: value
                    .get("checked")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            }
        })
        .collect();
    if let Ok(mut menu) = TRAY_MENU.lock() {
        *menu = parsed;
    }
}

/// Toggle whether a tray left-click is delivered to JS (`lx.tray.onClick`)
/// instead of running the tray's configured surface action.
pub(crate) fn set_click_intercept(intercept: bool) {
    TRAY_CLICK_INTERCEPT.store(intercept, Ordering::Relaxed);
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
        // Menu / left-click events route to the lxapp backing the tray's
        // surface. Resolve it from `surfaces[].content.appId`, falling back to
        // the surface id (they coincide for a single-app tray surface).
        let target = resolve_surface_lxapp_target(&ui, surface_id);
        let app_id = target
            .as_ref()
            .map(|target| target.app_id.clone())
            .unwrap_or_else(|| surface_id.to_string());
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
            .or_else(|| product_name_tooltip())
            .unwrap_or(surface_id)
            .to_string();
        let window_size = surface_window_size(&ui, surface_id);

        return Ok(Some(TrayItem {
            surface_id: surface_id.to_string(),
            app_id,
            page: target.as_ref().and_then(|target| target.page.clone()),
            query: target.and_then(|target| target.query),
            action_kind: action_kind.to_string(),
            tooltip,
            icon_path,
            window_size,
        }));
    }

    Ok(None)
}

#[derive(Debug, Clone, PartialEq)]
struct SurfaceLxappTarget {
    app_id: String,
    page: Option<String>,
    query: Option<serde_json::Value>,
}

/// Resolve the lxapp target that backs `surface_id` from `ui.surfaces[].content`.
fn resolve_surface_lxapp_target(
    ui: &serde_json::Value,
    surface_id: &str,
) -> Option<SurfaceLxappTarget> {
    let content = ui
        .get("surfaces")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|surface| surface.get("id").and_then(serde_json::Value::as_str) == Some(surface_id))
        .and_then(|surface| surface.get("content"))
        .and_then(serde_json::Value::as_object)?;
    let app_id = content
        .get("appId")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;
    let page = content
        .get("page")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let query = content.get("query").cloned();
    Some(SurfaceLxappTarget {
        app_id,
        page,
        query,
    })
}

fn product_name_tooltip() -> Option<&'static str> {
    #[cfg(feature = "shell-chrome")]
    {
        lingxia_app_context::product_name()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
    #[cfg(not(feature = "shell-chrome"))]
    {
        None
    }
}

fn surface_window_size(ui: &serde_json::Value, surface_id: &str) -> Option<(i32, i32)> {
    let size = ui
        .get("surfaces")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .find(|surface| surface.get("id").and_then(serde_json::Value::as_str) == Some(surface_id))?
        .get("size")?;
    let width = size.get("width").and_then(serde_json::Value::as_i64)? as i32;
    let height = size.get("height").and_then(serde_json::Value::as_i64)? as i32;
    if width > 0 && height > 0 {
        Some((width, height))
    } else {
        None
    }
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
        && let Some(icon) = crate::app_icon::create_tray_icon_handle_from_path(path, 32)
    {
        return Ok((icon, true));
    }
    if let Some(path) = crate::app_icon::current_app_icon_path()
        && path.is_file()
        && let Some(icon) = crate::app_icon::create_tray_icon_handle_from_path(&path, 32)
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
    // While a JS `lx.tray.onClick` handler is registered, the left-click is the
    // app's to handle: deliver it to that app and suppress the surface action.
    if TRAY_CLICK_INTERCEPT.load(Ordering::Relaxed) {
        lxapp::publish_app_event(&item.app_id, "lx.tray.click", None);
        return;
    }
    if !activate_tray_surface(&item) {
        log::warn!(
            "Windows tray activator could not handle {} for surface {}",
            item.action_kind,
            item.surface_id
        );
    }
}

fn activate_tray_surface(item: &TrayItem) -> bool {
    #[cfg(feature = "browser-shell")]
    {
        crate::shell::handle_menu_bar_surface_action(
            &item.surface_id,
            &item.action_kind,
            item.page.as_deref(),
            item.query.as_ref(),
        )
    }
    #[cfg(not(feature = "browser-shell"))]
    {
        toggle_runtime_tray_window(item)
    }
}

#[cfg(not(feature = "browser-shell"))]
fn toggle_runtime_tray_window(item: &TrayItem) -> bool {
    let window = crate::window_host::tray_popover_window_handle()
        .or_else(crate::window_host::primary_host_window_handle);
    if let Some(window) = window {
        if item.action_kind == "closeSurface" {
            crate::window_host::hide_exclusive_tray_popover();
            return crate::window_host::hide_host_window(window);
        }
        if item.action_kind == "toggleSurface"
            && crate::window_host::host_window_is_visible(window)
        {
            crate::window_host::hide_exclusive_tray_popover();
            return true;
        }
        position_window_near_icon(window);
        return crate::window_host::restore_and_focus_host_window(window);
    }
    if let Some((width, height)) = item.window_size {
        lingxia::windows::set_default_window_size(width, height);
    }
    let options = match lxapp::LxAppStartupOptions::for_page(item.page.as_deref(), item.query.as_ref())
    {
        Ok(options) => options,
        Err(error) => {
            log::warn!("Windows tray could not build startup options: {error}");
            return false;
        }
    };
    match lxapp::open_lxapp(&item.app_id, options) {
        Ok(_) => {
            if let Some(window) = crate::window_host::tray_popover_window_handle()
                .or_else(crate::window_host::primary_host_window_handle)
            {
                position_window_near_icon(window);
                crate::window_host::restore_and_focus_host_window(window);
            }
            true
        }
        Err(error) => {
            log::warn!("Windows tray could not open {}: {error}", item.app_id);
            false
        }
    }
}

const DEFAULT_QUIT_COMMAND: usize = 0x7001;

fn show_tray_menu(hwnd: HWND) {
    // A context menu replaces the flyout; leave only one surface up.
    crate::window_host::hide_exclusive_tray_popover();
    // JS `lx.tray.setMenu` owns the dropdown when the app registered one.
    // Exclusive tray hosts still need a way out: an empty menu gets Quit.
    let items = TRAY_MENU
        .lock()
        .map(|menu| menu.clone())
        .unwrap_or_default();
    let default_quit = items.is_empty();
    let Some(item) = current_item() else {
        return;
    };

    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        for (index, entry) in items.iter().enumerate() {
            if entry.separator {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                continue;
            }
            // Command id is the 1-based JS array index so the selection maps
            // back to the handler registered at `lx.tray.menu:{index}`.
            // AppendMenuW copies the label, so a local buffer is fine.
            let mut flags = MF_STRING;
            if entry.checked {
                flags |= MF_CHECKED;
            }
            if !entry.enabled {
                flags |= MF_GRAYED;
            }
            let label = to_wide(&entry.label);
            let _ = AppendMenuW(menu, flags, index + 1, PCWSTR(label.as_ptr()));
        }
        if default_quit {
            let label = to_wide("Quit");
            let _ = AppendMenuW(menu, MF_STRING, DEFAULT_QUIT_COMMAND, PCWSTR(label.as_ptr()));
        }

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

        let command = selected.0 as usize;
        if default_quit && command == DEFAULT_QUIT_COMMAND {
            #[cfg(feature = "runtime")]
            {
                let _ = lingxia::app::exit();
            }
            return;
        }
        if command >= 1 && command != DEFAULT_QUIT_COMMAND {
            // Deliver to the owning app's handler for that menu index.
            let event = format!("lx.tray.menu:{}", command - 1);
            lxapp::publish_app_event(&item.app_id, &event, None);
        }
    }
}

fn position_window_near_icon(window: isize) {
    let Some(icon) = icon_screen_rect().or_else(cursor_anchor_rect) else {
        return;
    };
    crate::window_host::place_host_window_near_rect(window, icon);
}

/// Place and show the exclusive-tray flyout next to the notify icon.
pub(crate) fn show_tray_popover() -> bool {
    let Some(window) = crate::window_host::tray_popover_window_handle() else {
        return false;
    };
    position_window_near_icon(window);
    crate::window_host::restore_and_focus_host_window(window)
}

/// True when the cursor is on (or a few pixels around) the notify icon.
/// The pad covers DPI rounding so a tray click is not treated as click-outside.
pub(crate) fn cursor_over_icon() -> bool {
    let Some(rect) = icon_screen_rect() else {
        return false;
    };
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_err() {
        return false;
    }
    point_in_padded_rect(point, rect, 4)
}

fn point_in_padded_rect(point: POINT, rect: RECT, pad: i32) -> bool {
    point.x >= rect.left - pad
        && point.x < rect.right + pad
        && point.y >= rect.top - pad
        && point.y < rect.bottom + pad
}

fn icon_screen_rect() -> Option<RECT> {
    let hwnd = current_tray_hwnd()?;
    let ident = NOTIFYICONIDENTIFIER {
        cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
        hWnd: HWND(hwnd as *mut c_void),
        uID: TRAY_ICON_ID,
        guidItem: windows::core::GUID::default(),
    };
    let rect = unsafe { Shell_NotifyIconGetRect(&ident).ok()? };
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return None;
    }
    Some(rect)
}

fn cursor_anchor_rect() -> Option<RECT> {
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point).ok()? };
    Some(RECT {
        left: point.x - 8,
        top: point.y - 8,
        right: point.x + 8,
        bottom: point.y + 8,
    })
}

fn current_tray_hwnd() -> Option<isize> {
    TRAY_STATE
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.as_ref().map(|state| state.hwnd))
}

pub(crate) fn fit_popover_size(work: RECT, width: i32, height: i32) -> (i32, i32) {
    (
        width.min((work.right - work.left).max(1)).max(1),
        height.min((work.bottom - work.top).max(1)).max(1),
    )
}

/// Place a popover next to `anchor` (the tray icon), clamped to that
/// monitor's work area — the same rule Steam / Discord / Clash use.
pub(crate) fn popover_origin(anchor: RECT, work: RECT, width: i32, height: i32, gap: i32) -> (i32, i32) {
    let space_above = anchor.top - work.top;
    let space_below = work.bottom - anchor.bottom;
    let y = if space_above >= height + gap || space_above >= space_below {
        anchor.top - height - gap
    } else {
        anchor.bottom + gap
    };
    let icon_width = (anchor.right - anchor.left).max(1);
    let x = anchor.left + (icon_width - width) / 2;
    let max_x = (work.right - width).max(work.left);
    let max_y = (work.bottom - height).max(work.top);
    (x.clamp(work.left, max_x), y.clamp(work.top, max_y))
}

pub(crate) fn work_area_for_rect(rect: RECT) -> Option<RECT> {
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info).as_bool() } {
        return None;
    }
    Some(info.rcWork)
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

#[cfg(test)]
mod tests {
    use super::{SurfaceLxappTarget, resolve_surface_lxapp_target};
    use serde_json::json;
    use windows::Win32::Foundation::{POINT, RECT};

    #[test]
    fn tray_surface_reads_declared_popover_size() {
        let ui = json!({
            "surfaces": [{
                "id": "home",
                "size": { "width": 320, "height": 420 }
            }]
        });
        assert_eq!(super::surface_window_size(&ui, "home"), Some((320, 420)));
    }

    #[test]
    fn padded_icon_rect_counts_as_over_icon() {
        let icon = RECT {
            left: 100,
            top: 100,
            right: 140,
            bottom: 140,
        };
        assert!(super::point_in_padded_rect(
            POINT { x: 138, y: 102 },
            icon,
            4
        ));
        assert!(super::point_in_padded_rect(
            POINT { x: 97, y: 138 },
            icon,
            4
        ));
        assert!(!super::point_in_padded_rect(
            POINT { x: 90, y: 90 },
            icon,
            4
        ));
    }

    #[test]
    fn popover_shrinks_to_a_short_work_area() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 800,
            bottom: 400,
        };
        assert_eq!(super::fit_popover_size(work, 320, 520), (320, 400));
    }

    #[test]
    fn popover_sits_above_a_bottom_taskbar_icon() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        let icon = RECT {
            left: 1860,
            top: 1048,
            right: 1900,
            bottom: 1080,
        };
        let (x, y) = super::popover_origin(icon, work, 320, 420, 8);
        assert_eq!(y, 1048 - 420 - 8);
        assert!(x + 320 <= work.right);
        assert!(x >= work.left);
    }

    #[test]
    fn tray_target_keeps_page_name_and_structured_query() {
        let ui = json!({
            "surfaces": [{
                "id": "home",
                "content": {
                    "kind": "lxapp",
                    "appId": "home",
                    "page": "tray",
                    "query": { "source": "tray", "compact": true }
                }
            }]
        });

        assert_eq!(
            resolve_surface_lxapp_target(&ui, "home"),
            Some(SurfaceLxappTarget {
                app_id: "home".to_string(),
                page: Some("tray".to_string()),
                query: Some(json!({ "source": "tray", "compact": true })),
            })
        );
    }
}
