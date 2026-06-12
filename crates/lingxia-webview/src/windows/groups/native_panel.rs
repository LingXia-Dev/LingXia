//! Native panel focus and keyboard input dispatch.

use super::*;

pub(crate) static WINDOW_ACTIVE_NATIVE_PANEL: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub(crate) fn active_native_panel() -> Option<String> {
    WINDOW_ACTIVE_NATIVE_PANEL
        .get()
        .and_then(|active| active.lock().ok())
        .and_then(|active| active.clone())
}

pub(crate) fn set_active_native_panel(panel_id: Option<String>) {
    let active = WINDOW_ACTIVE_NATIVE_PANEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut active) = active.lock() {
        *active = panel_id;
    }
}

pub(crate) fn handle_native_panel_char(wparam: WPARAM) -> bool {
    let Some(character) = char::from_u32(wparam.0 as u32) else {
        return false;
    };
    let (ctrl, shift, alt) = keyboard_modifiers();
    invoke_native_panel_input(WindowsPanelKeyEvent {
        vk: 0,
        ctrl,
        shift,
        alt,
        character: Some(character),
    })
}

pub(crate) fn handle_native_panel_keydown(wparam: WPARAM) -> bool {
    let (ctrl, shift, alt) = keyboard_modifiers();
    invoke_native_panel_input(WindowsPanelKeyEvent {
        vk: wparam.0 as u32,
        ctrl,
        shift,
        alt,
        character: None,
    })
}

pub(crate) fn keyboard_modifiers() -> (bool, bool, bool) {
    unsafe {
        (
            (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0,
            (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0,
            (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0,
        )
    }
}

pub(crate) fn invoke_native_panel_input(event: WindowsPanelKeyEvent) -> bool {
    let Some(panel_id) = active_native_panel() else {
        return false;
    };
    let Some(handler) = WINDOW_NATIVE_PANEL_INPUT_HANDLERS
        .get()
        .and_then(|handlers| handlers.lock().ok())
        .and_then(|handlers| handlers.get(&panel_id).cloned())
    else {
        return false;
    };
    handler(event)
}
