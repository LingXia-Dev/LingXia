//! Native text input and textarea component handling.

use super::*;

/// Edit-control messages and notification codes, defined locally (they live
/// in `Win32::UI::Controls`; the constants are stable and tiny, matching
/// the `text_input` module's approach).
const EM_GETSEL: u32 = 0x00b0;
const EM_SETSEL: u32 = 0x00b1;
const EM_SETLIMITTEXT: u32 = 0x00c5;
const EM_SETCUEBANNER: u32 = 0x1501;
pub(super) const EN_SETFOCUS: u32 = 0x0100;
pub(super) const EN_KILLFOCUS: u32 = 0x0200;
pub(super) const EN_CHANGE: u32 = 0x0300;

/// Default text size (CSS px) when the view does not report one.
pub(super) const DEFAULT_FONT_SIZE: f64 = 14.0;
/// Default text color (CSS `#111111`-ish dark gray) as 0x00BBGGRR.
pub(super) const DEFAULT_TEXT_COLOR: u32 = 0x0011_1111;
/// Horizontal inset of the EDIT inside its container, CSS px.
pub(super) const EDIT_PADDING_X: f64 = 8.0;
/// Vertical inset of a multiline EDIT inside its container, CSS px.
pub(super) const EDIT_PADDING_Y: f64 = 6.0;
pub(super) fn mount_edit_on_ui(
    context: PageContext,
    component_id: String,
    multiline: bool,
    parent: isize,
    container: HWND,
    doc_rect: DocRect,
    props: ComponentProps,
) {
    let key = component_key(&context.page_key, &component_id);
    let mut edit_style = WindowsAndMessaging::WS_CHILD.0
        | WindowsAndMessaging::WS_VISIBLE.0
        | WindowsAndMessaging::WS_CLIPSIBLINGS.0;
    if multiline {
        edit_style |= (ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32;
    } else {
        edit_style |= ES_AUTOHSCROLL as u32;
    }
    if props.password == Some(true) {
        edit_style |= ES_PASSWORD as u32;
    }
    let initial_text = to_wide(&to_edit_text(props.value.as_deref().unwrap_or("")));
    let edit = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            PCWSTR(initial_text.as_ptr()),
            WINDOW_STYLE(edit_style),
            0,
            0,
            16,
            16,
            Some(container),
            None,
            None,
            None,
        )
    };
    let Ok(edit) = edit else {
        log::warn!("failed to create native-component EDIT for {component_id}");
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(container);
        }
        return;
    };

    // Subclass the EDIT for confirm (Enter) handling.
    let original_proc = unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            edit,
            WindowsAndMessaging::GWLP_WNDPROC,
            edit_proc as *const () as usize as isize,
        )
    };
    let edit_state = Box::new(EditState {
        original_proc,
        component_key: key.clone(),
        multiline,
    });
    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            edit,
            WindowsAndMessaging::GWLP_USERDATA,
            Box::into_raw(edit_state) as isize,
        );
    }

    let entry = ComponentEntry {
        context,
        component_id: component_id.clone(),
        multiline,
        parent,
        container: container.0 as isize,
        edit: edit.0 as isize,
        font: 0,
        video: None,
        doc_rect,
        state: ComponentProps::default(),
        last_value: props.value.clone().unwrap_or_default(),
        ready: ready_keys().contains(&key),
        pending: Vec::new(),
    };
    components().insert(key.clone(), entry);
    containers().insert(container.0 as isize, key.clone());

    apply_props(&key, &props);
    apply_layout(&key);
}

pub(super) fn apply_edit_props(key: &str, props: &ComponentProps) {
    struct Pending {
        edit: isize,
        container: isize,
        parent: isize,
        multiline: bool,
        old_font: isize,
        font_size: Option<f64>,
        scale: f64,
        placeholder: Option<String>,
        maxlength: Option<u32>,
        disabled: Option<bool>,
        value: Option<String>,
        focus: Option<bool>,
        color_changed: bool,
    }

    let pending = {
        let mut components = components();
        let Some(entry) = components.get_mut(key) else {
            return;
        };
        let font_changed = props.font_size.is_some() && props.font_size != entry.state.font_size;
        let color_changed =
            props.text_color.is_some() && props.text_color != entry.state.text_color;
        let placeholder_changed =
            props.placeholder.is_some() && props.placeholder != entry.state.placeholder;
        // Focus is asserted only when the prop actually flips: the view
        // resends the whole prop set on unrelated changes, and re-applying
        // `focus:"false"` would yank focus from a control the user just
        // clicked into.
        let focus_changed = props.focus.is_some() && props.focus != entry.state.focus;
        let first_apply = entry.font == 0;
        entry.state.merge_from(props);

        let scale = page_views()
            .get(&entry.context.page_key)
            .map(|view| view.target.scale)
            .filter(|scale| *scale > 0.0)
            .unwrap_or(1.0);

        Pending {
            edit: entry.edit,
            container: entry.container,
            parent: entry.parent,
            multiline: entry.multiline,
            old_font: entry.font,
            font_size: (font_changed || first_apply)
                .then_some(entry.state.font_size.unwrap_or(DEFAULT_FONT_SIZE)),
            scale,
            placeholder: (placeholder_changed || first_apply)
                .then(|| entry.state.placeholder.clone().unwrap_or_default()),
            maxlength: props.maxlength,
            disabled: if first_apply {
                Some(entry.state.disabled.unwrap_or(false))
            } else {
                props.disabled
            },
            value: props.value.clone(),
            focus: if first_apply {
                entry.state.focus.filter(|focus| *focus)
            } else if focus_changed {
                props.focus
            } else {
                None
            },
            color_changed,
        }
    };

    let edit = HWND(pending.edit as *mut _);
    unsafe {
        if let Some(font_size) = pending.font_size {
            let height = -((font_size * pending.scale).round() as i32);
            let font = CreateFontW(
                height,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
                w!("Segoe UI"),
            );
            if !font.is_invalid() {
                let _ = WindowsAndMessaging::SendMessageW(
                    edit,
                    WindowsAndMessaging::WM_SETFONT,
                    Some(WPARAM(font.0 as usize)),
                    Some(LPARAM(1)),
                );
                {
                    let mut components = components();
                    if let Some(entry) = components.get_mut(key) {
                        entry.font = font.0 as isize;
                    }
                }
                if pending.old_font != 0 {
                    let _ = DeleteObject(HGDIOBJ(pending.old_font as *mut _));
                }
            }
        }

        if let Some(placeholder) = pending.placeholder
            && !pending.multiline
        {
            // Multiline EDIT controls do not support cue banners; textarea
            // placeholders are deferred.
            let text = to_wide(&placeholder);
            let _ = WindowsAndMessaging::SendMessageW(
                edit,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(text.as_ptr() as isize)),
            );
        }

        if let Some(maxlength) = pending.maxlength {
            let _ = WindowsAndMessaging::SendMessageW(
                edit,
                EM_SETLIMITTEXT,
                Some(WPARAM(maxlength as usize)),
                Some(LPARAM(0)),
            );
        }

        if let Some(disabled) = pending.disabled {
            let _ = EnableWindow(edit, !disabled);
        }

        if let Some(value) = pending.value {
            let current = from_edit_text(&read_window_text(edit));
            if current != value {
                suppressed_edits().insert(pending.edit);
                let edit_text = to_edit_text(&value);
                let text = to_wide(&edit_text);
                let _ = WindowsAndMessaging::SetWindowTextW(edit, PCWSTR(text.as_ptr()));
                // Caret to the end of the synced text.
                let end = edit_text.encode_utf16().count();
                let _ = WindowsAndMessaging::SendMessageW(
                    edit,
                    EM_SETSEL,
                    Some(WPARAM(end)),
                    Some(LPARAM(end as isize)),
                );
                suppressed_edits().remove(&pending.edit);
                let mut components = components();
                if let Some(entry) = components.get_mut(key) {
                    entry.last_value = value;
                }
            }
        }

        if pending.color_changed {
            let _ = InvalidateRect(Some(HWND(pending.container as *mut _)), None, true);
        }

        if let Some(focus) = pending.focus {
            set_edit_focus_with_parent(pending.edit, pending.parent, focus);
        }
    }
}

pub(super) fn set_edit_focus_with_parent(edit: isize, parent: isize, focus: bool) {
    let edit_hwnd = HWND(edit as *mut _);
    unsafe {
        let focused = GetFocus() == edit_hwnd;
        if focus && !focused {
            let _ = SetFocus(Some(edit_hwnd));
        } else if !focus && focused {
            let _ = SetFocus(Some(HWND(parent as *mut _)));
        }
    }
}

fn read_window_text(hwnd: HWND) -> String {
    unsafe {
        let length = WindowsAndMessaging::GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut buffer = vec![0u16; length + 1];
        let copied = WindowsAndMessaging::GetWindowTextW(hwnd, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied.min(length)])
    }
}

fn edit_caret_position(edit: HWND) -> u32 {
    let selection =
        unsafe { WindowsAndMessaging::SendMessageW(edit, EM_GETSEL, None, None) }.0 as u64;
    ((selection >> 16) & 0xffff) as u32
}

fn current_edit_value(key: &str) -> Option<(HWND, String)> {
    let edit = {
        let components = components();
        components.get(key).map(|entry| entry.edit)?
    };
    let edit = HWND(edit as *mut _);
    Some((edit, from_edit_text(&read_window_text(edit))))
}

pub(super) fn on_edit_changed(container: HWND) {
    let Some(key) = component_key_for_container(container) else {
        return;
    };
    let Some((edit, value)) = current_edit_value(&key) else {
        return;
    };
    let cursor = edit_caret_position(edit);

    {
        let suppressed = suppressed_edits().contains(&(edit.0 as isize));
        let mut components = components();
        let Some(entry) = components.get_mut(&key) else {
            return;
        };
        if entry.last_value == value {
            return;
        }
        entry.last_value = value.clone();
        if suppressed {
            return;
        }
    }
    emit_event(&key, "input", json!({ "value": value, "cursor": cursor }));
}

pub(super) fn on_edit_focus_changed(container: HWND, focused: bool) {
    let Some(key) = component_key_for_container(container) else {
        return;
    };
    let Some((_, value)) = current_edit_value(&key) else {
        return;
    };
    let event = if focused { "focus" } else { "blur" };
    emit_event(&key, event, json!({ "value": value }));
}

/// Per-EDIT subclass state stashed in `GWLP_USERDATA`.
struct EditState {
    original_proc: isize,
    component_key: String,
    multiline: bool,
}

fn edit_state(hwnd: HWND) -> *mut EditState {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) };
    raw as *mut EditState
}

fn emit_confirm(key: &str, edit: HWND) {
    let value = from_edit_text(&read_window_text(edit));
    emit_event(key, "confirm", json!({ "value": value }));
}

/// Subclass procedure of component EDIT controls: Enter confirms
/// (Ctrl+Enter for multiline, where plain Enter inserts a newline);
/// `WM_NCDESTROY` unsubclasses and frees the state.
unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = edit_state(hwnd);
    if state.is_null() {
        return unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let original = unsafe { (*state).original_proc };
    let multiline = unsafe { (*state).multiline };

    match msg {
        WindowsAndMessaging::WM_KEYDOWN if wparam.0 == VK_RETURN.0 as usize => {
            let ctrl_down = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
            if !multiline || ctrl_down {
                let key = unsafe { (*state).component_key.clone() };
                emit_confirm(&key, hwnd);
                if !multiline {
                    return LRESULT(0);
                }
            }
        }
        // Swallow the translated Enter character on single-line controls
        // (message beep).
        WindowsAndMessaging::WM_CHAR if wparam.0 == 0x0d && !multiline => {
            return LRESULT(0);
        }
        WindowsAndMessaging::WM_NCDESTROY => {
            let state = unsafe { Box::from_raw(state) };
            unsafe {
                WindowsAndMessaging::SetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA, 0);
                WindowsAndMessaging::SetWindowLongPtrW(
                    hwnd,
                    WindowsAndMessaging::GWLP_WNDPROC,
                    state.original_proc,
                );
            }
            suppressed_edits().remove(&(hwnd.0 as isize));
            return unsafe { call_original(state.original_proc, hwnd, msg, wparam, lparam) };
        }
        _ => {}
    }
    unsafe { call_original(original, hwnd, msg, wparam, lparam) }
}

/// Calls the EDIT class procedure captured at subclass time.
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
