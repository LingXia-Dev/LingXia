//! The text menu the core asks for, as a popup menu at its anchor. Static text's items run
//! in the core, an editor's through its input connection.
use lingxia_view::session::host::{MenuAction, MenuTarget, TextMenu, WeakViewHost};
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, EndMenu, MF_STRING, TPM_LEFTALIGN, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenu,
};
use windows::core::HSTRING;

use crate::WindowsAdapter;

const ACTIONS: [MenuAction; 6] = [
    MenuAction::Cut,
    MenuAction::Copy,
    MenuAction::Paste,
    MenuAction::SelectAll,
    MenuAction::Undo,
    MenuAction::Redo,
];

fn title(action: MenuAction, chinese: bool) -> &'static str {
    match (action, chinese) {
        (MenuAction::Cut, false) => "Cut",
        (MenuAction::Copy, false) => "Copy",
        (MenuAction::Paste, false) => "Paste",
        (MenuAction::SelectAll, false) => "Select All",
        (MenuAction::Undo, false) => "Undo",
        (MenuAction::Redo, false) => "Redo",
        (MenuAction::Cut, true) => "剪切",
        (MenuAction::Copy, true) => "复制",
        (MenuAction::Paste, true) => "粘贴",
        (MenuAction::SelectAll, true) => "全选",
        (MenuAction::Undo, true) => "撤销",
        (MenuAction::Redo, true) => "重做",
    }
}

/// Show `menu` (or close the open one) once the current message returns: the popup runs its
/// own loop. On the page's thread.
pub(crate) fn show(adapter: &WindowsAdapter, menu: Option<&TextMenu>) {
    let id = adapter.menu.fetch_add(1, Ordering::AcqRel) + 1;
    // SAFETY: ends a popup this thread is tracking, if any.
    let _ = unsafe { EndMenu() };
    let Some(menu) = menu else {
        return;
    };
    let editor = matches!(menu.target, MenuTarget::Editor { .. });
    let Some(host) = adapter.host.get().cloned() else {
        return;
    };
    let (scale, locale) = (scale(adapter), locale(adapter));
    let hwnd = adapter.hwnd.load(Ordering::Acquire);
    let actions = menu.actions.clone();
    let [x, y, _, height] = menu.anchor;
    let anchor = ((x * scale) as i32, ((y + height) * scale) as i32);
    adapter.dispatcher.post(Box::new(move || {
        let current = |host: &WeakViewHost<WindowsAdapter>| {
            host.upgrade()
                .is_some_and(|host| host.adapter().menu.load(Ordering::Acquire) == id)
        };
        if !current(&host) {
            return;
        }
        let chosen = popup(hwnd, anchor, &actions, locale.starts_with("zh"));
        if current(&host) {
            run(&host, chosen, editor);
        }
    }));
}

fn run(host: &WeakViewHost<WindowsAdapter>, chosen: Option<MenuAction>, editor: bool) {
    let Some(host) = host.upgrade() else { return };
    match chosen {
        Some(action) if editor => {
            crate::ime::menu_action(&host, action);
        }
        Some(action) => {
            host.text_menu_action(action);
        }
        None => host.text_menu_dismissed(),
    }
}

fn popup(
    hwnd: isize,
    (x, y): (i32, i32),
    actions: &[MenuAction],
    chinese: bool,
) -> Option<MenuAction> {
    let owner = HWND(hwnd as *mut _);
    // SAFETY: a menu made, tracked and destroyed on this thread, owned by its window.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        for action in actions {
            let Some(index) = ACTIONS.iter().position(|a| a == action) else {
                continue;
            };
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                index + 1,
                &HSTRING::from(title(*action, chinese)),
            );
        }
        let mut at = POINT { x, y };
        let _ = ClientToScreen(owner, &mut at);
        let command = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            at.x,
            at.y,
            None,
            owner,
            None,
        );
        let _ = DestroyMenu(menu);
        usize::try_from(command.0)
            .ok()
            .and_then(|command| command.checked_sub(1))
            .and_then(|index| ACTIONS.get(index).copied())
    }
}

fn scale(adapter: &WindowsAdapter) -> f32 {
    adapter
        .facts
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map_or(1.0, |facts| facts.scale)
}

fn locale(adapter: &WindowsAdapter) -> String {
    adapter
        .facts
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map(|facts| facts.locale.clone())
        .unwrap_or_default()
}
