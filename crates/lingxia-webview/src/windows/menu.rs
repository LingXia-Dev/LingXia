//! Generic Win32 menu-bar mechanic for top-level main host windows.
//!
//! `lingxia-webview` owns no menu content: a product layer supplies the
//! whole menu model (menu titles, command ids, labels, check marks, and
//! optional plain-key accelerators) through [`set_windows_app_menu`] and
//! receives selections through the handler registered with
//! [`set_windows_app_menu_command_handler`]. The mechanic turns the model
//! into a standard Win32 menu bar (`CreateMenu`/`SetMenu`) on every
//! top-level main host window — both windows that already exist and windows
//! shown later — and forwards menu `WM_COMMAND` messages to the registered
//! handler on a short-lived thread (so handlers may synchronously dispatch
//! further webview commands without deadlocking the UI thread).
//!
//! Keyboard note: key-downs only reach the host window procedure while the
//! native window itself has focus. While the WebView2 content has focus,
//! Chromium processes keys internally and they never arrive here — pair an
//! accelerator like F12 with the matching built-in WebView2 behavior
//! (DevTools stay enabled by default, see
//! [`set_webview_devtools_enabled`](super::set_webview_devtools_enabled))
//! so the shortcut works in both focus states.

use super::*;

/// One pull-down menu of the application menu bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAppMenu {
    /// Menu-bar title (e.g. `"Device"`).
    pub title: String,
    /// Entries in display order.
    pub entries: Vec<WindowsAppMenuEntry>,
}

/// One entry of a pull-down menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsAppMenuEntry {
    /// A selectable command item.
    Item(WindowsAppMenuItem),
    /// A horizontal separator line.
    Separator,
}

/// A selectable menu item. All content and meaning is caller-owned; the
/// mechanic only draws the item and reports the id back on selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsAppMenuItem {
    /// Caller-owned command id, reported through the registered command
    /// handler. Must be non-zero, at most `0xFFFF` (`WM_COMMAND` carries
    /// menu ids in the low word), and unique across all menus.
    pub id: u32,
    /// Item label. A `\t` separates the label from right-aligned shortcut
    /// text (e.g. `"Open DevTools\tF12"`).
    pub label: String,
    /// Draws a check mark in front of the item. To change check marks
    /// later, re-call [`set_windows_app_menu`] with an updated model.
    pub checked: bool,
    /// Optional plain (unmodified) virtual-key code (e.g. `0x7B` for F12).
    /// A matching `WM_KEYDOWN` seen by a webview host window — with no
    /// Ctrl/Shift/Alt held — dispatches this item's command. See the module
    /// docs for the focus caveat.
    pub accelerator_vk: Option<u32>,
}

/// Handler receiving the command id of a selected menu item (or a matched
/// accelerator key). Invoked on a short-lived worker thread.
pub type WindowsAppMenuCommandHandler = Arc<dyn Fn(u32) + Send + Sync>;

static APP_MENU_MODEL: OnceLock<Mutex<Vec<WindowsAppMenu>>> = OnceLock::new();

static APP_MENU_HANDLER: OnceLock<Mutex<Option<WindowsAppMenuCommandHandler>>> = OnceLock::new();

/// Installs (or replaces) the application menu-bar model and applies it to
/// every live top-level main host window; main host windows shown later
/// pick the model up when they show. Re-call with an updated model to
/// change labels or check marks. An empty model is ignored — the mechanic
/// never removes an installed menu bar.
pub fn set_windows_app_menu(menus: Vec<WindowsAppMenu>) {
    if menus.is_empty() {
        log::warn!("ignoring empty Windows app menu model");
        return;
    }
    let model = APP_MENU_MODEL.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut model) = model.lock() {
        *model = menus;
    }
    for host in current_group_host_handles() {
        apply_app_menu_to_window(hwnd_from_handle(host));
    }
}

/// Registers the handler that receives menu command ids. The last
/// registered handler wins; it stays installed for the process lifetime.
pub fn set_windows_app_menu_command_handler(handler: WindowsAppMenuCommandHandler) {
    let slot = APP_MENU_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(handler);
    }
}

fn app_menu_model_snapshot() -> Vec<WindowsAppMenu> {
    APP_MENU_MODEL
        .get()
        .and_then(|model| model.lock().ok())
        .map(|model| model.clone())
        .unwrap_or_default()
}

fn current_group_host_handles() -> Vec<isize> {
    WINDOW_GROUP_HOSTS
        .get()
        .and_then(|hosts| hosts.lock().ok())
        .map(|hosts| {
            hosts
                .values()
                .copied()
                .filter(|handle| is_window_handle_valid(*handle))
                .collect()
        })
        .unwrap_or_default()
}

/// Whether `id` belongs to an item of the installed menu model.
fn menu_defines_command(id: u32) -> bool {
    app_menu_model_snapshot().iter().any(|menu| {
        menu.entries.iter().any(|entry| {
            matches!(entry, WindowsAppMenuEntry::Item(item) if item.id == id)
        })
    })
}

/// Command id bound to plain virtual key `vk`, when the model defines one.
fn accelerator_command(vk: u32) -> Option<u32> {
    app_menu_model_snapshot().iter().find_map(|menu| {
        menu.entries.iter().find_map(|entry| match entry {
            WindowsAppMenuEntry::Item(item) if item.accelerator_vk == Some(vk) => Some(item.id),
            _ => None,
        })
    })
}

/// Builds the Win32 menu bar for `menus`. Returns `None` (destroying any
/// partial menu) when a Win32 call fails.
fn build_app_menu(menus: &[WindowsAppMenu]) -> Option<WindowsAndMessaging::HMENU> {
    let bar = unsafe { WindowsAndMessaging::CreateMenu() }.ok()?;
    let build = (|| -> WinResult<()> {
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
                    bar,
                    WindowsAndMessaging::MF_POPUP,
                    popup.0 as usize,
                    PCWSTR(title.as_ptr()),
                )?;
            }
        }
        Ok(())
    })();
    if let Err(err) = build {
        log::warn!("Windows app menu construction failed: {err}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyMenu(bar);
        }
        return None;
    }
    Some(bar)
}

/// Applies the installed menu model to `hwnd` (a top-level main host
/// window). Menus belong to the thread that owns the window, so a
/// cross-thread call is marshalled onto the owner thread first. A no-op
/// while no model is installed.
pub(crate) fn apply_app_menu_to_window(hwnd: HWND) {
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(hwnd, None) };
    if owner_thread != 0 && owner_thread != unsafe { Threading::GetCurrentThreadId() } {
        let handle = hwnd_handle(hwnd);
        post_to_window_thread(
            handle,
            Box::new(move || apply_app_menu_to_window(hwnd_from_handle(handle))),
        );
        return;
    }

    let model = app_menu_model_snapshot();
    if model.is_empty() {
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
    // Attaching/replacing the menu bar changes the client area; re-sync the
    // WebView2 controller bounds (we are on the window's UI thread here).
    handle_window_geometry_change(hwnd);
}

/// `WM_COMMAND` path of the window procedure: forwards menu selections
/// (and accelerator-sourced commands) whose id belongs to the installed
/// model. Returns `true` when the message was consumed.
pub(crate) fn handle_app_menu_wm_command(wparam: WPARAM) -> bool {
    // HIWORD 0: menu, 1: accelerator. Anything else is a control
    // notification and not ours.
    if (wparam.0 >> 16) & 0xffff > 1 {
        return false;
    }
    let id = (wparam.0 & 0xffff) as u32;
    if id == 0 || !menu_defines_command(id) {
        return false;
    }
    dispatch_app_menu_command(id)
}

/// `WM_KEYDOWN` path of the window procedure: dispatches the command of a
/// plain-key menu accelerator (no Ctrl/Shift/Alt held). Returns `true`
/// when the key was consumed.
pub(crate) fn handle_app_menu_keydown(wparam: WPARAM) -> bool {
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

/// Invokes the registered command handler for `id` on a short-lived thread
/// (handlers may synchronously dispatch webview commands, which would
/// deadlock if run on the UI thread that owns the webview).
fn dispatch_app_menu_command(id: u32) -> bool {
    let handler = APP_MENU_HANDLER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone());
    let Some(handler) = handler else {
        return false;
    };
    let _ = std::thread::Builder::new()
        .name(format!("lingxia-webview-menu-{id}"))
        .spawn(move || handler(id));
    true
}
