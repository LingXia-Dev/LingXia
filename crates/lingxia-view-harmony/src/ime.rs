//! The OHOS input method connection for the focused editor. The IME queries text
//! around the caret synchronously from its own thread, so this keeps a mirror of the
//! editor (text, selection, preview text) and turns IME calls into the session's
//! input protocol on the main thread; editing policy itself stays in the core.
use crate::{dispatch, pages, sys};
use lingxia_view::session::host::CaretState;
use std::sync::Mutex;

/// What the IME sees of the focused editor, in UTF-16 units like the protocol.
#[derive(Default)]
struct Mirror {
    token: i64,
    /// The editor's frame identity and edit token, which every edit request names.
    identity: String,
    edit_token: u64,
    text: Vec<u16>,
    anchor: i32,
    focus: i32,
    /// The IME's preview (composition) range, if any.
    #[allow(dead_code)]
    preview: Option<(i32, i32)>,
    keyboard: i32,
    enter: i32,
}

/// One connection: the proxy ArkUI calls and the controller handle we notify.
#[allow(dead_code)]
struct Connection {
    editor: *mut sys::InputMethod_TextEditorProxy,
    ime: *mut sys::InputMethod_InputMethodProxy,
}
// SAFETY: both handles are thread-safe IPC proxies.
unsafe impl Send for Connection {}

static MIRROR: Mutex<Option<Mirror>> = Mutex::new(None);
static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

fn mirror<T>(work: impl FnOnce(&mut Mirror) -> T) -> Option<T> {
    MIRROR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_mut()
        .map(work)
}

/// Sends one `edit` request of the input protocol for the mirrored editor, on the main thread.
fn edit(edits: serde_json::Value, submit: bool) {
    let Some((token, request)) = mirror(|m| {
        let request = serde_json::json!({
            "op": "edit", "identity": m.identity, "token": m.edit_token,
            "edits": edits, "submit": submit,
        });
        (m.token, request)
    }) else {
        return;
    };
    dispatch::post_main(Box::new(move || {
        pages::with(token, |host| {
            host.input(request.to_string(), |_reply| {});
        });
    }));
}

unsafe extern "C" fn get_text_config(
    _: *mut sys::InputMethod_TextEditorProxy,
    config: *mut sys::InputMethod_TextConfig,
) {
    let Some((keyboard, enter, anchor, focus)) =
        mirror(|m| (m.keyboard, m.enter, m.anchor, m.focus))
    else {
        return;
    };
    // SAFETY: ArkUI hands a live config for this call.
    unsafe {
        sys::OH_TextConfig_SetInputType(config, keyboard);
        sys::OH_TextConfig_SetEnterKeyType(config, enter);
        sys::OH_TextConfig_SetPreviewTextSupport(config, true);
        sys::OH_TextConfig_SetSelection(config, anchor.min(focus), anchor.max(focus));
    }
}

unsafe extern "C" fn insert_text(
    _: *mut sys::InputMethod_TextEditorProxy,
    text: *const u16,
    length: usize,
) {
    // SAFETY: `length` UTF-16 units at `text` for this call.
    let units = unsafe { std::slice::from_raw_parts(text, length) };
    let Ok(text) = String::from_utf16(units) else {
        return;
    };
    edit(
        serde_json::json!([{"op": "replace", "text": text, "cursor": 1, "composing": false}]),
        false,
    );
}

unsafe extern "C" fn delete_backward(_: *mut sys::InputMethod_TextEditorProxy, length: i32) {
    edit(
        serde_json::json!([{"op": "delete_surrounding", "before": length.max(0), "after": 0, "code_points": false}]),
        false,
    );
}

unsafe extern "C" fn delete_forward(_: *mut sys::InputMethod_TextEditorProxy, length: i32) {
    edit(
        serde_json::json!([{"op": "delete_surrounding", "before": 0, "after": length.max(0), "code_points": false}]),
        false,
    );
}

unsafe extern "C" fn left_of_cursor(
    _: *mut sys::InputMethod_TextEditorProxy,
    count: i32,
    out: *mut u16,
    length: *mut usize,
) {
    let left = mirror(|m| {
        let caret = m.anchor.min(m.focus).clamp(0, m.text.len() as i32) as usize;
        m.text[caret.saturating_sub(count.max(0) as usize)..caret].to_vec()
    })
    .unwrap_or_default();
    // SAFETY: ArkUI's buffer holds `*length` units on entry.
    unsafe {
        let n = left.len().min(*length);
        std::ptr::copy_nonoverlapping(left.as_ptr(), out, n);
        *length = n;
    }
}

unsafe extern "C" fn index_at_cursor(_: *mut sys::InputMethod_TextEditorProxy) -> i32 {
    mirror(|m| m.focus).unwrap_or(0)
}

unsafe extern "C" fn set_preview(
    _: *mut sys::InputMethod_TextEditorProxy,
    text: *const u16,
    length: usize,
    start: i32,
    end: i32,
) -> i32 {
    // SAFETY: `length` UTF-16 units at `text` for this call.
    let units = unsafe { std::slice::from_raw_parts(text, length) };
    let Ok(text) = String::from_utf16(units) else {
        return -1;
    };
    // A range of -1 replaces the current preview (or selection) in place.
    let mut edits = Vec::new();
    if start >= 0 && end >= 0 {
        edits.push(serde_json::json!({"op": "composition", "start": start, "end": end}));
    }
    edits.push(serde_json::json!({"op": "replace", "text": text, "cursor": 1, "composing": true}));
    edit(serde_json::Value::Array(edits), false);
    0
}

unsafe extern "C" fn finish_preview(_: *mut sys::InputMethod_TextEditorProxy) {
    edit(serde_json::json!([{"op": "finish_composition"}]), false);
}

/// Attach the input method to the editor `identity` of page `token` and show the keyboard.
pub(crate) fn connect(token: i64, identity: &str, edit_token: u64, state: Option<CaretState>) {
    let _ = (token, edit_token, state, &CONNECTION);
    let _ = (
        get_text_config as unsafe extern "C" fn(_, _),
        insert_text as unsafe extern "C" fn(_, _, _),
        delete_backward as unsafe extern "C" fn(_, _),
        delete_forward as unsafe extern "C" fn(_, _),
        left_of_cursor as unsafe extern "C" fn(_, _, _, _),
        index_at_cursor as unsafe extern "C" fn(_) -> _,
        set_preview as unsafe extern "C" fn(_, _, _, _, _) -> _,
        finish_preview as unsafe extern "C" fn(_),
    );
    log::warn!(target: "lingxia_view", "UI_INPUT: no input method connection for {identity} yet");
}

/// End text input: detach and hide the keyboard.
pub(crate) fn disconnect(token: i64) {
    let _ = token;
}
