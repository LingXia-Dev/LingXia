//! The focused editor's input connection. `Mirror` is platform-neutral: it holds what an
//! input method sees of the editor (UTF-16 text, selection, composition) and turns edits into
//! the session's input protocol. The OHOS binding below only translates the IME C API into
//! mirror calls; the IME queries text around the caret synchronously from its own thread.
use crate::{dispatch, pages, sys};
use lingxia_view::interaction::ime::{Keyboard, ReturnKey};
use lingxia_view::platform::EditOutcome;
use lingxia_view::session::host::{CaretState, FrameSnapshot};
use serde_json::{Value, json};
use std::sync::Mutex;

/// What the input method sees of the focused editor.
#[derive(Clone, Debug)]
struct Mirror {
    page: i64,
    /// The editor's frame identity and edit token, which every request names.
    identity: String,
    token: u64,
    text: Vec<u16>,
    anchor: u32,
    focus: u32,
    composition: Option<(u32, u32)>,
    /// Edit batches the core has not answered; frames do not overwrite the mirror meanwhile.
    in_flight: u32,
    keyboard: Keyboard,
    return_key: ReturnKey,
    secure: bool,
    multiline: bool,
}

/// The keyboard configuration of a frame node's input, by field (the input type is not exported).
#[derive(Clone, Copy, Debug)]
struct Ime {
    keyboard: Keyboard,
    return_key: ReturnKey,
    secure: bool,
    multiline: bool,
}

macro_rules! ime_of {
    ($input:expr) => {
        Ime {
            keyboard: $input.ime.keyboard,
            return_key: $input.ime.return_key,
            secure: $input.ime.secure,
            multiline: $input.ime.multiline,
        }
    };
}

impl Mirror {
    fn from_state(page: i64, identity: &str, token: u64, state: &Value, ime: Option<Ime>) -> Self {
        let mut mirror = Self {
            page,
            identity: identity.to_owned(),
            token,
            text: Vec::new(),
            anchor: 0,
            focus: 0,
            composition: None,
            in_flight: 0,
            keyboard: Keyboard::Text,
            return_key: ReturnKey::Done,
            secure: false,
            multiline: false,
        };
        mirror.take_state(state);
        if let Some(ime) = ime {
            mirror.configure(ime);
        }
        mirror
    }

    fn configure(&mut self, ime: Ime) {
        self.keyboard = ime.keyboard;
        self.return_key = ime.return_key;
        self.secure = ime.secure;
        self.multiline = ime.multiline;
    }

    /// The core's editor state (`EditorState` of the input protocol).
    fn take_state(&mut self, state: &Value) {
        self.text = state["text"]
            .as_str()
            .unwrap_or_default()
            .encode_utf16()
            .collect();
        let pair = |v: &Value| Some((v[0].as_u64()? as u32, v[1].as_u64()? as u32));
        let (anchor, focus) =
            pair(&state["selection"]).unwrap_or((self.text.len() as u32, self.text.len() as u32));
        self.anchor = anchor;
        self.focus = focus;
        self.composition = pair(&state["composition"]);
    }

    fn caret(&self) -> CaretState {
        let (start, end) = self
            .composition
            .map_or((-1, -1), |(s, e)| (s as i32, e as i32));
        CaretState {
            text: String::from_utf16_lossy(&self.text),
            anchor: self.anchor as i32,
            focus: self.focus as i32,
            composition: (start, end),
        }
    }

    fn caret_at(&self) -> usize {
        (self.anchor.min(self.focus) as usize).min(self.text.len())
    }

    fn boundary(&self, offset: i32) -> bool {
        offset >= 0 && (offset as usize) <= self.text.len()
    }
}

static MIRROR: Mutex<Option<Mirror>> = Mutex::new(None);
/// A focus request is on its way to the core; edits wait for it.
static FOCUSING: Mutex<Option<String>> = Mutex::new(None);
/// The page and edit token this side last blurred: frames drawn before the core saw the blur
/// still show that editor focused, and must not raise the keyboard for it again.
static BLURRED: Mutex<Option<(i64, u64)>> = Mutex::new(None);

fn mirror<T>(work: impl FnOnce(&mut Mirror) -> T) -> Option<T> {
    MIRROR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_mut()
        .map(work)
}

/// Main thread: sends one `edit` batch for the mirrored editor and reports the caret.
fn send(edits: Vec<Value>, submit: bool) {
    let Some((page, request, caret)) = mirror(|m| {
        m.in_flight += 1;
        let request = json!({
            "op": "edit", "identity": m.identity, "token": m.token,
            "edits": edits, "submit": submit,
        });
        (m.page, request, m.caret())
    }) else {
        return;
    };
    pages::with(page, |host| {
        host.caret(true, Some(caret));
        host.input(request.to_string(), |reply| {
            if reply.is_none() {
                log::debug!(target: "lingxia_view", "UI_INPUT: edit batch went stale");
            }
            dispatch::post_main(Box::new(|| {
                mirror(|m| m.in_flight = m.in_flight.saturating_sub(1));
            }));
        });
    });
}

/// Any thread: run `work` on the main thread.
fn on_main(work: impl FnOnce() + Send + 'static) {
    dispatch::post_main(Box::new(work));
}

fn replace_all_edits(m: &mut Mirror, text: &str) -> Vec<Value> {
    let end = m.text.len();
    m.text = text.encode_utf16().collect();
    m.anchor = m.text.len() as u32;
    m.focus = m.anchor;
    m.composition = None;
    vec![
        json!({"op": "finish_composition"}),
        json!({"op": "selection", "anchor": 0, "focus": end}),
        json!({"op": "replace", "text": text, "cursor": 1, "composing": false}),
    ]
}

fn select_edits(m: &mut Mirror, start: u32, end: u32) -> Vec<Value> {
    let mut edits = Vec::new();
    if m.composition.take().is_some() {
        edits.push(json!({"op": "finish_composition"}));
    }
    m.anchor = start;
    m.focus = end;
    edits.push(json!({"op": "selection", "anchor": start, "focus": end}));
    edits
}

/// A presented frame: follow the core's focused editor, connecting, refreshing or dropping.
pub(crate) fn reconcile(page: i64, frame: &FrameSnapshot) {
    if FOCUSING.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
        return;
    }
    let focused = frame.frame.nodes.iter().find_map(|node| {
        let input = node.input.as_ref()?;
        let token = input.edit_token.filter(|t| *t > 0)?;
        (input.enabled && input.ime.picker.is_none()).then(|| {
            let state = input
                .edit
                .as_ref()
                .map(|e| serde_json::to_value(e).unwrap_or_default());
            (
                node.identity.clone(),
                token,
                ime_of!(input),
                state,
                input.value.clone(),
                input.raise_keyboard,
            )
        })
    });
    let current = MIRROR.lock().unwrap_or_else(|p| p.into_inner()).clone();
    match (focused, current) {
        (None, Some(m)) if m.page == page => {
            drop_connection(page);
        }
        (Some((identity, token, ime, state, ..)), Some(m))
            if m.page == page && m.identity == identity && m.token == token =>
        {
            mirror(|m| {
                m.configure(ime);
                if m.in_flight == 0
                    && let Some(state) = &state
                {
                    m.take_state(state);
                }
            });
            os::update();
        }
        (Some((identity, token, ime, state, value, raise)), current)
            if (raise
                && *BLURRED.lock().unwrap_or_else(|p| p.into_inner()) != Some((page, token)))
                || current.is_some() =>
        {
            let state = state.unwrap_or_else(|| json!({"text": value}));
            connect(Mirror::from_state(
                page,
                &identity,
                token,
                &state,
                Some(ime),
            ));
        }
        _ => {}
    }
}

fn connect(mirror: Mirror) {
    let (page, caret) = (mirror.page, mirror.caret());
    *MIRROR.lock().unwrap_or_else(|p| p.into_inner()) = Some(mirror);
    pages::with(page, |host| host.caret(true, Some(caret)));
    os::attach();
}

type AfterFocus = Box<dyn FnOnce(&mut Mirror) -> Vec<Value> + Send>;

/// Focus editor `identity` of frame `generation` for the keyboard (a tap or focus dispatch).
pub(crate) fn focus_editor(page: i64, identity: &str, generation: u64) {
    focus(page, identity, generation, None);
}

/// Ask the core to focus editor `identity` of frame `generation`, then run `then` on the mirror.
fn focus(page: i64, identity: &str, generation: u64, then: Option<AfterFocus>) {
    *FOCUSING.lock().unwrap_or_else(|p| p.into_inner()) = Some(identity.to_owned());
    let request = json!({"op": "focus", "identity": identity, "generation": generation});
    let identity = identity.to_owned();
    pages::with(page, move |host| {
        host.input(request.to_string(), move |reply| {
            on_main(move || {
                *FOCUSING.lock().unwrap_or_else(|p| p.into_inner()) = None;
                let reply: Value = reply
                    .and_then(|r| serde_json::from_str(&r).ok())
                    .unwrap_or_default();
                let (Some(token), Some(state)) = (reply["token"].as_u64(), reply.get("state"))
                else {
                    return;
                };
                let snapshot = pages::with(page, |host| host.snapshot()).flatten();
                let ime = snapshot.as_deref().and_then(|frame| {
                    let input = frame
                        .frame
                        .nodes
                        .iter()
                        .find(|n| n.identity == identity)?
                        .input
                        .as_ref()?;
                    Some(ime_of!(input))
                });
                connect(Mirror::from_state(page, &identity, token, state, ime));
                if let Some(then) = then
                    && let Some(edits) = mirror(then)
                    && !edits.is_empty()
                {
                    send(edits, false);
                }
                pages::with(page, |host| host.redraw());
            });
        });
    });
}

/// An accessibility or automation edit of the editor at frame `node`: replace its text, or with
/// `text` absent select `start..end` (-1, -1: the end).
pub(crate) fn edit_text(
    page: i64,
    frame: &FrameSnapshot,
    node: usize,
    text: Option<&str>,
    start: i32,
    end: i32,
) -> EditOutcome {
    let Some(target) = frame.frame.nodes.get(node) else {
        return EditOutcome::Failed;
    };
    let Some(input) = target
        .input
        .as_ref()
        .filter(|i| i.enabled && i.ime.picker.is_none())
    else {
        return EditOutcome::Failed;
    };
    if FOCUSING.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
        return EditOutcome::NotReady;
    }
    let current = mirror(|m| m.page == page && m.identity == target.identity).unwrap_or(false);
    if current {
        let edits = mirror(|m| match text {
            Some(text) => Some(replace_all_edits(m, text)),
            None => {
                let (start, end) = if start == -1 && end == -1 {
                    (m.focus as i32, m.focus as i32)
                } else {
                    (start, end)
                };
                (m.boundary(start) && m.boundary(end))
                    .then(|| select_edits(m, start as u32, end as u32))
            }
        })
        .flatten();
        return match edits {
            Some(edits) => {
                send(edits, false);
                os::update();
                EditOutcome::Applied
            }
            None => EditOutcome::Failed,
        };
    }
    let length = input.value.encode_utf16().count() as i32;
    if text.is_none() {
        let (s, e) = if start == -1 && end == -1 {
            (length, length)
        } else {
            (start, end)
        };
        if s < 0 || e < 0 || s > length || e > length {
            return EditOutcome::Failed;
        }
    }
    let text = text.map(str::to_owned);
    focus(
        page,
        &target.identity,
        frame.generation.get(),
        Some(Box::new(move |m: &mut Mirror| match &text {
            Some(text) => replace_all_edits(m, text),
            None => {
                let at = m.text.len() as u32;
                let (s, e) = if start == -1 && end == -1 {
                    (at, at)
                } else {
                    (start as u32, end as u32)
                };
                select_edits(m, s, e)
            }
        })),
    );
    EditOutcome::Applied
}

/// An activation ends text input on page `page`: detach the keyboard and blur the editor in the
/// core, so a frame drawn before it saw the blur does not raise the keyboard again.
pub(crate) fn disconnect(page: i64) {
    let Some(was) = drop_connection(page) else {
        return;
    };
    *BLURRED.lock().unwrap_or_else(|p| p.into_inner()) = Some((page, was.token));
    let request = json!({"op": "blur", "identity": was.identity, "token": was.token});
    pages::with(page, move |host| {
        host.input(request.to_string(), move |_| {
            on_main(move || {
                pages::with(page, |host| host.redraw());
            });
        });
    });
}

/// The focused editor went away: detach the keyboard; the core already knows.
fn drop_connection(page: i64) -> Option<Mirror> {
    let was = MIRROR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take_if(|m| m.page == page);
    if was.is_some() {
        os::detach();
        pages::with(page, |host| host.caret(false, None));
    }
    was
}

/// The page ended: drop the connection without telling the core.
pub(crate) fn reset(page: i64) {
    BLURRED
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take_if(|(blurred, _)| *blurred == page);
    if MIRROR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take_if(|m| m.page == page)
        .is_some()
    {
        os::detach();
    }
}

/// The OHOS input method controller over the mirror.
mod os {
    use super::*;

    struct Connection {
        editor: *mut sys::InputMethod_TextEditorProxy,
        proxy: *mut sys::InputMethod_InputMethodProxy,
    }
    // SAFETY: both are IPC proxies the IME service allows from any thread.
    unsafe impl Send for Connection {}

    static CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

    fn text_type(m: &Mirror) -> i32 {
        if m.secure {
            return if m.keyboard == Keyboard::Number { 8 } else { 7 };
        }
        match m.keyboard {
            _ if m.multiline => 1,
            Keyboard::Number => 2,
            Keyboard::Phone => 3,
            Keyboard::Email => 5,
            Keyboard::Text | Keyboard::Search => 0,
        }
    }

    fn enter_key(m: &Mirror) -> i32 {
        match m.return_key {
            ReturnKey::Newline => 8,
            ReturnKey::Done => 6,
            ReturnKey::Go => 2,
            ReturnKey::Next => 5,
            ReturnKey::Previous => 7,
            ReturnKey::Search => 3,
            ReturnKey::Send => 4,
        }
    }

    unsafe extern "C" fn get_text_config(
        _: *mut sys::InputMethod_TextEditorProxy,
        config: *mut sys::InputMethod_TextConfig,
    ) {
        let Some((kind, enter, start, end)) = mirror(|m| {
            (
                text_type(m),
                enter_key(m),
                m.anchor.min(m.focus),
                m.anchor.max(m.focus),
            )
        }) else {
            return;
        };
        // SAFETY: ArkUI's live config for this call.
        unsafe {
            sys::OH_TextConfig_SetInputType(config, kind);
            sys::OH_TextConfig_SetEnterKeyType(config, enter);
            sys::OH_TextConfig_SetPreviewTextSupport(config, true);
            sys::OH_TextConfig_SetSelection(config, start as i32, end as i32);
        }
    }

    unsafe extern "C" fn insert_text(
        _: *mut sys::InputMethod_TextEditorProxy,
        text: *const u16,
        length: usize,
    ) {
        // SAFETY: `length` UTF-16 units at `text` for this call.
        let units = unsafe { std::slice::from_raw_parts(text, length) }.to_vec();
        on_main(move || {
            let Ok(text) = String::from_utf16(&units) else {
                return;
            };
            let edits = mirror(|m| {
                let (start, end) = m
                    .composition
                    .take()
                    .unwrap_or((m.anchor.min(m.focus), m.anchor.max(m.focus)));
                let (start, end) = (start as usize, (end as usize).min(m.text.len()));
                m.text.splice(start.min(end)..end, units.iter().copied());
                let caret = (start + units.len()) as u32;
                m.anchor = caret;
                m.focus = caret;
                vec![json!({"op": "replace", "text": text, "cursor": 1, "composing": false})]
            });
            if let Some(edits) = edits {
                send(edits, false);
            }
        });
    }

    fn delete(before: i32, after: i32) {
        on_main(move || {
            let edits = mirror(|m| {
                let caret = m.caret_at();
                let from = caret.saturating_sub(before.max(0) as usize);
                let to = (caret + after.max(0) as usize).min(m.text.len());
                m.text.drain(from..to);
                m.anchor = from as u32;
                m.focus = from as u32;
                m.composition = None;
                vec![
                    json!({"op": "delete_surrounding", "before": before.max(0), "after": after.max(0), "code_points": false}),
                ]
            });
            if let Some(edits) = edits {
                send(edits, false);
            }
        });
    }

    unsafe extern "C" fn delete_backward(_: *mut sys::InputMethod_TextEditorProxy, length: i32) {
        delete(length, 0);
    }

    unsafe extern "C" fn delete_forward(_: *mut sys::InputMethod_TextEditorProxy, length: i32) {
        delete(0, length);
    }

    unsafe extern "C" fn keyboard_status(_: *mut sys::InputMethod_TextEditorProxy, _: i32) {}

    unsafe extern "C" fn enter(_: *mut sys::InputMethod_TextEditorProxy, _: i32) {
        on_main(|| {
            let multiline =
                mirror(|m| m.multiline && m.return_key == ReturnKey::Newline).unwrap_or(false);
            if multiline {
                mirror(|m| {
                    let caret = m.caret_at();
                    m.text.insert(caret, u16::from(b'\n'));
                    m.anchor = caret as u32 + 1;
                    m.focus = m.anchor;
                });
                send(
                    vec![json!({"op": "replace", "text": "\n", "cursor": 1, "composing": false})],
                    false,
                );
            } else {
                send(Vec::new(), true);
            }
        });
    }

    unsafe extern "C" fn move_cursor(_: *mut sys::InputMethod_TextEditorProxy, direction: i32) {
        on_main(move || {
            let edits = mirror(|m| {
                let at = m.focus as i32
                    + match direction {
                        3 => -1,
                        4 => 1,
                        _ => 0,
                    };
                let at = at.clamp(0, m.text.len() as i32) as u32;
                select_edits(m, at, at)
            });
            if let Some(edits) = edits {
                send(edits, false);
            }
        });
    }

    unsafe extern "C" fn set_selection(
        _: *mut sys::InputMethod_TextEditorProxy,
        start: i32,
        end: i32,
    ) {
        on_main(move || {
            let edits = mirror(|m| {
                (m.boundary(start) && m.boundary(end))
                    .then(|| select_edits(m, start as u32, end as u32))
            })
            .flatten();
            if let Some(edits) = edits {
                send(edits, false);
            }
        });
    }

    unsafe extern "C" fn extend_action(_: *mut sys::InputMethod_TextEditorProxy, action: i32) {
        if action == 0 {
            on_main(|| {
                let edits = mirror(|m| {
                    let end = m.text.len() as u32;
                    select_edits(m, 0, end)
                });
                if let Some(edits) = edits {
                    send(edits, false);
                }
            });
        }
    }

    fn around(count: i32, left: bool, out: *mut u16, length: *mut usize) {
        let units = mirror(|m| {
            let caret = m.caret_at();
            let count = count.max(0) as usize;
            if left {
                m.text[caret.saturating_sub(count)..caret].to_vec()
            } else {
                let end = (m.anchor.max(m.focus) as usize).min(m.text.len());
                m.text[end..(end + count).min(m.text.len())].to_vec()
            }
        })
        .unwrap_or_default();
        // SAFETY: the IME's buffer holds `*length` units on entry.
        unsafe {
            let n = units.len().min(*length);
            std::ptr::copy_nonoverlapping(units.as_ptr(), out, n);
            *length = n;
        }
    }

    unsafe extern "C" fn left_of_cursor(
        _: *mut sys::InputMethod_TextEditorProxy,
        count: i32,
        out: *mut u16,
        length: *mut usize,
    ) {
        around(count, true, out, length);
    }

    unsafe extern "C" fn right_of_cursor(
        _: *mut sys::InputMethod_TextEditorProxy,
        count: i32,
        out: *mut u16,
        length: *mut usize,
    ) {
        around(count, false, out, length);
    }

    unsafe extern "C" fn index_at_cursor(_: *mut sys::InputMethod_TextEditorProxy) -> i32 {
        mirror(|m| m.caret_at() as i32).unwrap_or(0)
    }

    unsafe extern "C" fn private_command(
        _: *mut sys::InputMethod_TextEditorProxy,
        _: *mut *mut sys::InputMethod_PrivateCommand,
        _: usize,
    ) -> i32 {
        0
    }

    unsafe extern "C" fn set_preview(
        _: *mut sys::InputMethod_TextEditorProxy,
        text: *const u16,
        length: usize,
        start: i32,
        end: i32,
    ) -> i32 {
        // SAFETY: `length` UTF-16 units at `text` for this call.
        let units = unsafe { std::slice::from_raw_parts(text, length) }.to_vec();
        on_main(move || {
            let Ok(text) = String::from_utf16(&units) else {
                return;
            };
            let edits = mirror(|m| {
                let mut edits = Vec::new();
                let range = if start >= 0 && end >= start && m.boundary(end) {
                    edits.push(json!({"op": "composition", "start": start, "end": end}));
                    (start as usize, end as usize)
                } else {
                    let (s, e) = m
                        .composition
                        .unwrap_or((m.anchor.min(m.focus), m.anchor.max(m.focus)));
                    (s as usize, (e as usize).min(m.text.len()))
                };
                m.text.splice(range.0..range.1, units.iter().copied());
                m.composition = Some((range.0 as u32, (range.0 + units.len()) as u32));
                m.anchor = (range.0 + units.len()) as u32;
                m.focus = m.anchor;
                edits.push(json!({"op": "replace", "text": text, "cursor": 1, "composing": true}));
                edits
            });
            if let Some(edits) = edits {
                send(edits, false);
            }
        });
        0
    }

    unsafe extern "C" fn finish_preview(_: *mut sys::InputMethod_TextEditorProxy) {
        on_main(|| {
            let edits = mirror(|m| {
                m.composition = None;
                vec![json!({"op": "finish_composition"})]
            });
            if let Some(edits) = edits {
                send(edits, false);
            }
        });
    }

    /// Main thread: attach the IME to the mirrored editor and show the keyboard.
    pub(super) fn attach() {
        let mut connection = CONNECTION.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(existing) = connection.as_ref() {
            // SAFETY: a live controller proxy.
            unsafe { sys::OH_InputMethodProxy_ShowKeyboard(existing.proxy) };
            drop(connection);
            return update();
        }
        // SAFETY: the proxy gets every callback before attaching; both live until `detach`.
        unsafe {
            let editor = sys::OH_TextEditorProxy_Create();
            if editor.is_null() {
                return log::warn!(target: "lingxia_view", "UI_INPUT: no text editor proxy");
            }
            sys::OH_TextEditorProxy_SetGetTextConfigFunc(editor, get_text_config);
            sys::OH_TextEditorProxy_SetInsertTextFunc(editor, insert_text);
            sys::OH_TextEditorProxy_SetDeleteForwardFunc(editor, delete_forward);
            sys::OH_TextEditorProxy_SetDeleteBackwardFunc(editor, delete_backward);
            sys::OH_TextEditorProxy_SetSendKeyboardStatusFunc(editor, keyboard_status);
            sys::OH_TextEditorProxy_SetSendEnterKeyFunc(editor, enter);
            sys::OH_TextEditorProxy_SetMoveCursorFunc(editor, move_cursor);
            sys::OH_TextEditorProxy_SetHandleSetSelectionFunc(editor, set_selection);
            sys::OH_TextEditorProxy_SetHandleExtendActionFunc(editor, extend_action);
            sys::OH_TextEditorProxy_SetGetLeftTextOfCursorFunc(editor, left_of_cursor);
            sys::OH_TextEditorProxy_SetGetRightTextOfCursorFunc(editor, right_of_cursor);
            sys::OH_TextEditorProxy_SetGetTextIndexAtCursorFunc(editor, index_at_cursor);
            sys::OH_TextEditorProxy_SetReceivePrivateCommandFunc(editor, private_command);
            sys::OH_TextEditorProxy_SetSetPreviewTextFunc(editor, set_preview);
            sys::OH_TextEditorProxy_SetFinishTextPreviewFunc(editor, finish_preview);
            let options = sys::OH_AttachOptions_Create(true);
            let mut proxy = std::ptr::null_mut();
            let code = sys::OH_InputMethodController_Attach(editor, options, &mut proxy);
            sys::OH_AttachOptions_Destroy(options);
            if code != 0 || proxy.is_null() {
                sys::OH_TextEditorProxy_Destroy(editor);
                return log::warn!(target: "lingxia_view", "UI_INPUT: input method attach failed ({code})");
            }
            *connection = Some(Connection { editor, proxy });
        }
    }

    /// The mirror changed outside the IME (a frame, an automation edit): tell the IME.
    pub(super) fn update() {
        let connection = CONNECTION.lock().unwrap_or_else(|p| p.into_inner());
        let Some(connection) = connection.as_ref() else {
            return;
        };
        let Some((mut text, start, end)) = mirror(|m| {
            (
                m.text.clone(),
                m.anchor.min(m.focus) as i32,
                m.anchor.max(m.focus) as i32,
            )
        }) else {
            return;
        };
        // SAFETY: a live controller proxy; the text outlives the call.
        unsafe {
            sys::OH_InputMethodProxy_NotifySelectionChange(
                connection.proxy,
                text.as_mut_ptr(),
                text.len(),
                start,
                end,
            );
        }
    }

    pub(super) fn detach() {
        let Some(connection) = CONNECTION.lock().unwrap_or_else(|p| p.into_inner()).take() else {
            return;
        };
        // SAFETY: releases what `attach` created.
        unsafe {
            sys::OH_InputMethodProxy_HideKeyboard(connection.proxy);
            sys::OH_InputMethodController_Detach(connection.proxy);
            sys::OH_TextEditorProxy_Destroy(connection.editor);
        }
    }
}
