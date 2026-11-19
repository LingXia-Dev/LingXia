//! The focused editor's input connection. `Mirror` holds what an input method sees of the
//! editor (UTF-16 text, selection, composition) and turns keys, characters and IMM32
//! composition into the session's input protocol. The core may call in from any thread;
//! window and IMM work runs on the page's thread.
use crate::WindowsAdapter;
use lingxia_view::interaction::ime::ReturnKey;
use lingxia_view::platform::EditOutcome;
use lingxia_view::session::host::{CaretState, FrameSnapshot, ViewHost, WeakViewHost};
use serde_json::{Value, json};
use std::sync::Mutex;

type Host = ViewHost<WindowsAdapter>;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Mirror {
    identity: String,
    token: u64,
    text: Vec<u16>,
    anchor: u32,
    focus: u32,
    composition: Option<(u32, u32)>,
    /// Edit batches the core has not answered; frames do not overwrite the mirror meanwhile.
    in_flight: u32,
    multiline: bool,
    /// Return inserts a line break instead of submitting.
    newline: bool,
    /// A password: no input method composes into it.
    secure: bool,
}

impl Mirror {
    fn new(identity: &str, token: u64, state: &Value) -> Self {
        let mut mirror = Self {
            identity: identity.to_owned(),
            token,
            ..Default::default()
        };
        mirror.take_state(state);
        mirror
    }

    /// The core's editor state (`EditorState` of the input protocol).
    fn take_state(&mut self, state: &Value) {
        self.text = state["text"]
            .as_str()
            .unwrap_or_default()
            .encode_utf16()
            .collect();
        let pair = |v: &Value| Some((v[0].as_u64()? as u32, v[1].as_u64()? as u32));
        let end = self.text.len() as u32;
        let (anchor, focus) = pair(&state["selection"]).unwrap_or((end, end));
        self.anchor = anchor.min(end);
        self.focus = focus.min(end);
        self.composition = pair(&state["composition"]);
    }

    fn configure(&mut self, frame: &FrameSnapshot) {
        let Some(input) = frame
            .frame
            .nodes
            .iter()
            .find(|node| *node.identity == *self.identity)
            .and_then(|node| node.input.as_ref())
        else {
            return;
        };
        self.multiline = input.ime.multiline;
        self.newline = input.ime.multiline && input.ime.return_key == ReturnKey::Newline;
        self.secure = input.ime.secure;
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

    fn selection(&self) -> (usize, usize) {
        let len = self.text.len();
        (
            (self.anchor.min(self.focus) as usize).min(len),
            (self.anchor.max(self.focus) as usize).min(len),
        )
    }

    fn selected(&self) -> String {
        let (start, end) = self.selection();
        String::from_utf16_lossy(&self.text[start..end])
    }

    fn boundary(&self, offset: i32) -> bool {
        offset >= 0 && (offset as usize) <= self.text.len()
    }

    fn collapse(&mut self, at: usize) {
        self.anchor = at as u32;
        self.focus = at as u32;
    }

    fn replace_all(&mut self, text: &str) -> Vec<Value> {
        let end = self.text.len();
        self.text = text.encode_utf16().collect();
        self.collapse(self.text.len());
        self.composition = None;
        vec![
            json!({"op": "finish_composition"}),
            json!({"op": "selection", "anchor": 0, "focus": end}),
            json!({"op": "replace", "text": text, "cursor": 1, "composing": false}),
        ]
    }

    fn select(&mut self, anchor: u32, focus: u32) -> Vec<Value> {
        let mut edits = Vec::new();
        if self.composition.take().is_some() {
            edits.push(json!({"op": "finish_composition"}));
        }
        self.anchor = anchor;
        self.focus = focus;
        edits.push(json!({"op": "selection", "anchor": anchor, "focus": focus}));
        edits
    }

    /// Committed text replaces the composition, else the selection.
    fn insert(&mut self, text: &str) -> Vec<Value> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let (start, end) = self
            .composition
            .take()
            .map(|(s, e)| (s as usize, (e as usize).min(self.text.len())))
            .unwrap_or_else(|| self.selection());
        self.text.splice(start..end, units.iter().copied());
        self.collapse(start + units.len());
        vec![json!({"op": "replace", "text": text, "cursor": 1, "composing": false})]
    }

    /// The input method's in-progress text replaces the composition, else the selection.
    fn compose(&mut self, text: &str) -> Vec<Value> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let (start, end) = self
            .composition
            .map(|(s, e)| (s as usize, (e as usize).min(self.text.len())))
            .unwrap_or_else(|| self.selection());
        self.text.splice(start..end, units.iter().copied());
        self.composition = Some((start as u32, (start + units.len()) as u32));
        self.collapse(start + units.len());
        vec![json!({"op": "replace", "text": text, "cursor": 1, "composing": true})]
    }

    /// The input method dropped its text: remove what it had composed.
    fn cancel_composition(&mut self) -> Option<Vec<Value>> {
        self.composition?;
        let mut edits = self.compose("");
        self.composition = None;
        edits.push(json!({"op": "finish_composition"}));
        Some(edits)
    }

    /// Backspace (`forward` false) or Delete: the selection, else one code point.
    fn delete(&mut self, forward: bool) -> Option<Vec<Value>> {
        let (start, end) = self.selection();
        if start != end {
            self.text.drain(start..end);
            self.collapse(start);
            self.composition = None;
            return Some(vec![
                json!({"op": "replace", "text": "", "cursor": 1, "composing": false}),
            ]);
        }
        let surrogate = |unit: u16| (0xDC00..0xE000).contains(&unit);
        let (from, to) = if forward {
            let next = *self.text.get(start)?;
            let wide = (0xD800..0xDC00).contains(&next)
                && self.text.get(start + 1).is_some_and(|&u| surrogate(u));
            (start, start + 1 + usize::from(wide))
        } else {
            let previous = *self.text.get(start.checked_sub(1)?)?;
            let wide = surrogate(previous) && start >= 2;
            (start - 1 - usize::from(wide), start)
        };
        self.text.drain(from..to);
        self.collapse(from);
        self.composition = None;
        let (before, after) = if forward { (0, 1) } else { (1, 0) };
        Some(vec![json!({
            "op": "delete_surrounding", "before": before, "after": after, "code_points": true,
        })])
    }

    /// Move the focus to offset `to`; `extend` keeps the anchor.
    fn move_caret(&mut self, to: usize, extend: bool) -> Vec<Value> {
        let to = to.min(self.text.len()) as u32;
        let anchor = if extend { self.anchor } else { to };
        self.select(anchor, to)
    }

    /// The offset one code point from the focus.
    fn step(&self, forward: bool) -> usize {
        let focus = (self.focus as usize).min(self.text.len());
        let low = |i: usize| {
            self.text
                .get(i)
                .is_some_and(|u| (0xDC00..0xE000).contains(u))
        };
        if forward {
            (focus + 1 + usize::from(low(focus + 1))).min(self.text.len())
        } else {
            focus.saturating_sub(1 + usize::from(focus >= 2 && low(focus - 1)))
        }
    }
}

#[derive(Default)]
pub(crate) struct Ime {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    mirror: Option<Mirror>,
    /// A focus request is on its way to the core; edits wait for it.
    focusing: Option<String>,
    /// The edit token this side last blurred: frames drawn before the core saw the blur
    /// still show that editor focused, and must not connect it again.
    blurred: Option<u64>,
    /// The high half of a character arriving as two `WM_CHAR`s.
    high: Option<u16>,
}

/// What a key press does while an editor is connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyUse {
    /// The editor took it.
    Taken,
    /// Its character edits: leave the key to `WM_CHAR`.
    Character,
    /// Not the editor's: the page's key handling gets it.
    Page,
}

impl Ime {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn mirror<T>(&self, work: impl FnOnce(&mut Mirror) -> T) -> Option<T> {
        self.state().mirror.as_mut().map(work)
    }

    pub(crate) fn connected(&self) -> bool {
        self.state().mirror.is_some()
    }
}

fn host_of(adapter: &WindowsAdapter) -> Option<(Host, WeakViewHost<WindowsAdapter>)> {
    let weak = adapter.host.get()?.clone();
    Some((weak.upgrade()?, weak))
}

/// Send one `edit` batch for the connected editor and show its caret.
fn send(host: &Host, edits: Vec<Value>, submit: bool) {
    let adapter = host.adapter();
    let Some((request, caret)) = adapter.ime.mirror(|m| {
        m.in_flight += 1;
        let request = json!({
            "op": "edit", "identity": m.identity, "token": m.token,
            "edits": edits, "submit": submit,
        });
        (request, m.caret())
    }) else {
        return;
    };
    host.caret(true, Some(caret));
    let Some((_, weak)) = host_of(adapter) else {
        return;
    };
    let dispatcher = adapter.dispatcher;
    host.input(request.to_string(), move |reply| {
        dispatcher.post(Box::new(move || {
            if let Some(host) = weak.upgrade() {
                answered(&host, reply);
            }
        }));
    });
    sync_os(host);
}

/// The core answered an edit batch: once all are answered its state is the editor's.
fn answered(host: &Host, reply: Option<String>) {
    let reply: Value = reply
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    let ime = &host.adapter().ime;
    if let (Some(moved), Some(token)) = (reply["moved_to"].as_str(), reply["token"].as_u64()) {
        // Next/previous moved focus: the reply is the new editor's.
        let mut mirror = Mirror::new(moved, token, &reply["state"]);
        if let Some(frame) = host.snapshot() {
            mirror.configure(&frame);
        }
        connect(host, mirror);
        host.redraw();
        return;
    }
    let settled = ime.mirror(|m| {
        m.in_flight = m.in_flight.saturating_sub(1);
        if m.in_flight == 0
            && reply["token"].as_u64() == Some(m.token)
            && reply["state"].is_object()
        {
            m.take_state(&reply["state"]);
            return Some(m.caret());
        }
        None
    });
    if let Some(Some(caret)) = settled {
        host.caret(true, Some(caret));
    }
    if reply["needs_view_render"].as_bool() == Some(true) {
        host.redraw();
    }
}

fn connect(host: &Host, mirror: Mirror) {
    let caret = mirror.caret();
    host.adapter().ime.state().mirror = Some(mirror);
    host.caret(true, Some(caret));
    sync_os(host);
}

/// A presented frame: follow the core's focused editor, connecting, refreshing or dropping.
pub(crate) fn reconcile(adapter: &WindowsAdapter, frame: &FrameSnapshot) {
    let Some((host, _)) = host_of(adapter) else {
        return;
    };
    let ime = &adapter.ime;
    if ime.state().focusing.is_some() {
        return;
    }
    let focused = frame.frame.nodes.iter().find_map(|node| {
        let input = node.input.as_ref()?;
        let token = input.edit_token.filter(|t| *t > 0)?;
        (input.enabled && input.ime.picker.is_none()).then(|| {
            let state = input
                .edit
                .as_ref()
                .map(|e| serde_json::to_value(e).unwrap_or_default())
                .unwrap_or_else(|| json!({"text": input.value}));
            (node.identity.to_string(), token, state)
        })
    });
    let current = ime
        .state()
        .mirror
        .as_ref()
        .map(|m| (m.identity.clone(), m.token));
    match (focused, current) {
        (None, Some(_)) => {
            drop_connection(&host);
        }
        (Some((identity, token, state)), Some(current)) if current == (identity.clone(), token) => {
            ime.mirror(|m| {
                m.configure(frame);
                if m.in_flight == 0 {
                    m.take_state(&state);
                }
            });
            sync_os(&host);
        }
        (Some((identity, token, state)), _) => {
            if ime.state().blurred == Some(token) {
                return;
            }
            let mut mirror = Mirror::new(&identity, token, &state);
            mirror.configure(frame);
            connect(&host, mirror);
        }
        (None, None) => {}
    }
}

type AfterFocus = Box<dyn FnOnce(&mut Mirror) -> Vec<Value> + Send>;

/// Ask the core to focus editor `identity` of frame `generation`, then run `then` on the mirror.
pub(crate) fn focus(
    adapter: &WindowsAdapter,
    identity: &str,
    generation: u64,
    then: Option<AfterFocus>,
) {
    let Some((host, weak)) = host_of(adapter) else {
        return;
    };
    {
        let mut state = adapter.ime.state();
        state.focusing = Some(identity.to_owned());
        state.blurred = None;
    }
    let request = json!({"op": "focus", "identity": identity, "generation": generation});
    let identity = identity.to_owned();
    let dispatcher = adapter.dispatcher;
    host.input(request.to_string(), move |reply| {
        dispatcher.post(Box::new(move || {
            let Some(host) = weak.upgrade() else { return };
            host.adapter().ime.state().focusing = None;
            let reply: Value = reply
                .and_then(|r| serde_json::from_str(&r).ok())
                .unwrap_or_default();
            let (Some(token), Some(state)) = (reply["token"].as_u64(), reply.get("state")) else {
                return;
            };
            let mut mirror = Mirror::new(&identity, token, state);
            if let Some(frame) = host.snapshot() {
                mirror.configure(&frame);
            }
            connect(&host, mirror);
            if let Some(then) = then
                && let Some(edits) = host.adapter().ime.mirror(then)
                && !edits.is_empty()
            {
                send(&host, edits, false);
            }
            host.redraw();
        }));
    });
}

/// An accessibility or automation edit of the editor at frame `node`: replace its text, or with
/// `text` absent select `start..end` (-1, -1: the end).
pub(crate) fn edit_text(
    adapter: &WindowsAdapter,
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
    if adapter.ime.state().focusing.is_some() {
        return EditOutcome::NotReady;
    }
    let range = |length: i32| {
        let (s, e) = if start == -1 && end == -1 {
            (length, length)
        } else {
            (start, end)
        };
        (s >= 0 && e >= 0 && s <= length && e <= length).then_some((s as u32, e as u32))
    };
    let current = adapter
        .ime
        .mirror(|m| *m.identity == *target.identity)
        .unwrap_or(false);
    if current {
        let edits = adapter
            .ime
            .mirror(|m| match text {
                Some(text) => Some(m.replace_all(text)),
                None => {
                    let length = m.text.len() as i32;
                    let (s, e) = if start == -1 && end == -1 {
                        (m.focus as i32, m.focus as i32)
                    } else {
                        (start, end)
                    };
                    (m.boundary(s) && m.boundary(e) && s <= length && e <= length)
                        .then(|| m.select(s as u32, e as u32))
                }
            })
            .flatten();
        let Some(edits) = edits else {
            return EditOutcome::Failed;
        };
        let Some((_, weak)) = host_of(adapter) else {
            return EditOutcome::Failed;
        };
        adapter.dispatcher.post(Box::new(move || {
            if let Some(host) = weak.upgrade() {
                send(&host, edits, false);
            }
        }));
        return EditOutcome::Applied;
    }
    let length = input.value.encode_utf16().count() as i32;
    if text.is_none() && range(length).is_none() {
        return EditOutcome::Failed;
    }
    let text = text.map(str::to_owned);
    focus(
        adapter,
        &target.identity,
        frame.generation.get(),
        Some(Box::new(move |m: &mut Mirror| match &text {
            Some(text) => m.replace_all(text),
            None => {
                let at = m.text.len() as u32;
                let (s, e) = if start == -1 && end == -1 {
                    (at, at)
                } else {
                    (start as u32, end as u32)
                };
                m.select(s, e)
            }
        })),
    );
    EditOutcome::Applied
}

/// An activation ends text input: blur the editor in the core, so a frame drawn before it saw
/// the blur does not connect it again.
pub(crate) fn disconnect(adapter: &WindowsAdapter) {
    let Some((host, _)) = host_of(adapter) else {
        return;
    };
    let Some(was) = drop_connection(&host) else {
        return;
    };
    adapter.ime.state().blurred = Some(was.token);
    let request = json!({"op": "blur", "identity": was.identity, "token": was.token});
    let weak = adapter.host.get().cloned();
    let dispatcher = adapter.dispatcher;
    host.input(request.to_string(), move |_| {
        dispatcher.post(Box::new(move || {
            if let Some(host) = weak.and_then(|w| w.upgrade()) {
                host.redraw();
            }
        }));
    });
}

fn drop_connection(host: &Host) -> Option<Mirror> {
    let was = host.adapter().ime.state().mirror.take();
    if was.is_some() {
        host.caret(false, None);
        sync_os(host);
    }
    was
}

/// The page ended: drop the connection without telling the core.
pub(crate) fn reset(adapter: &WindowsAdapter) {
    let mut state = adapter.ime.state();
    state.mirror = None;
    state.blurred = None;
    state.focusing = None;
    drop(state);
    if let Some((host, _)) = host_of(adapter) {
        sync_os(&host);
    }
}

/// Run `work` on the connected mirror and send what it returns.
fn edit(host: &Host, work: impl FnOnce(&mut Mirror) -> Option<Vec<Value>>) -> bool {
    match host.adapter().ime.mirror(work) {
        Some(Some(edits)) => {
            send(host, edits, false);
            true
        }
        Some(None) => true,
        None => false,
    }
}

/// Page thread: a `WM_CHAR` unit; false when no editor takes it.
pub(crate) fn character(host: &Host, unit: u16) -> bool {
    let ime = &host.adapter().ime;
    if !ime.connected() {
        return false;
    }
    match unit {
        // Tab and Escape stay the page's: focus moves, overlays close.
        0x09 | 0x1B => false,
        0x08 => edit(host, |m| m.delete(false)),
        0x0D => {
            let newline = ime.mirror(|m| m.newline).unwrap_or(false);
            if newline {
                edit(host, |m| Some(m.insert("\n")))
            } else {
                send(host, Vec::new(), true);
                true
            }
        }
        // Control characters of Ctrl chords: the key handler already ran them.
        unit if unit < 0x20 || unit == 0x7F => true,
        0xD800..=0xDBFF => {
            ime.state().high = Some(unit);
            true
        }
        0xDC00..=0xDFFF => {
            let Some(high) = ime.state().high.take() else {
                return true;
            };
            let text = String::from_utf16_lossy(&[high, unit]);
            edit(host, |m| Some(m.insert(&text)))
        }
        unit => {
            let text = String::from_utf16_lossy(&[unit]);
            edit(host, |m| Some(m.insert(&text)))
        }
    }
}

/// Page thread: a key press, by virtual key.
pub(crate) fn key(host: &Host, vk: u16, shift: bool, ctrl: bool) -> KeyUse {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_BACK, VK_DELETE, VK_END, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE,
    };
    let ime = &host.adapter().ime;
    if !ime.connected() {
        return KeyUse::Page;
    }
    let taken = |done: bool| if done { KeyUse::Taken } else { KeyUse::Page };
    if ctrl {
        return match vk {
            0x41 => taken(edit(host, |m| Some(m.select(0, m.text.len() as u32)))),
            0x43 => taken(menu_action(
                host,
                lingxia_view::session::host::MenuAction::Copy,
            )),
            0x58 => taken(menu_action(
                host,
                lingxia_view::session::host::MenuAction::Cut,
            )),
            0x56 => taken(menu_action(
                host,
                lingxia_view::session::host::MenuAction::Paste,
            )),
            0x5A if shift => taken(menu_action(
                host,
                lingxia_view::session::host::MenuAction::Redo,
            )),
            0x5A => taken(menu_action(
                host,
                lingxia_view::session::host::MenuAction::Undo,
            )),
            0x59 => taken(menu_action(
                host,
                lingxia_view::session::host::MenuAction::Redo,
            )),
            _ => KeyUse::Page,
        };
    }
    match vk {
        v if v == VK_BACK.0 || v == VK_RETURN.0 || v == VK_SPACE.0 => KeyUse::Character,
        v if v == VK_DELETE.0 => taken(edit(host, |m| m.delete(true))),
        v if v == VK_LEFT.0 || v == VK_RIGHT.0 => {
            let forward = v == VK_RIGHT.0;
            taken(edit(host, |m| {
                let (start, end) = m.selection();
                let to = if !shift && start != end {
                    if forward { end } else { start }
                } else {
                    m.step(forward)
                };
                Some(m.move_caret(to, shift))
            }))
        }
        v if v == VK_HOME.0 => taken(edit(host, |m| Some(m.move_caret(0, shift)))),
        v if v == VK_END.0 => taken(edit(host, |m| Some(m.move_caret(m.text.len(), shift)))),
        _ => KeyUse::Page,
    }
}

/// An editor menu or shortcut action; false when no editor is connected.
pub(crate) fn menu_action(host: &Host, action: lingxia_view::session::host::MenuAction) -> bool {
    use lingxia_view::session::host::MenuAction;
    let ime = &host.adapter().ime;
    let Some((selected, secure)) = ime.mirror(|m| (m.selected(), m.secure)) else {
        return false;
    };
    match action {
        MenuAction::Copy | MenuAction::Cut => {
            if selected.is_empty() || secure {
                return true;
            }
            if let Err(error) = crate::clipboard::set_text(&selected) {
                log::warn!(target: "lingxia_view", "clipboard: {error}");
                return true;
            }
            if action == MenuAction::Cut {
                edit(host, |m| m.delete(false));
            }
            true
        }
        MenuAction::Paste => {
            let Some(text) = crate::clipboard::text().filter(|t| !t.is_empty()) else {
                return true;
            };
            edit(host, |m| {
                let text = if m.multiline {
                    text.replace("\r\n", "\n")
                } else {
                    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
                };
                Some(m.insert(&text))
            })
        }
        MenuAction::SelectAll => edit(host, |m| Some(m.select(0, m.text.len() as u32))),
        MenuAction::Undo => edit(host, |_| Some(vec![json!({"op": "undo"})])),
        MenuAction::Redo => edit(host, |_| Some(vec![json!({"op": "redo"})])),
    }
}

/// Page thread: the input method's composition changed (`result` committed text, `composing`
/// its in-progress text; empty composing text with no result cancels it).
pub(crate) fn composition(host: &Host, result: Option<String>, composing: Option<String>) -> bool {
    if !host.adapter().ime.connected() {
        return false;
    }
    if let Some(result) = result {
        edit(host, |m| Some(m.insert(&result)));
    }
    match composing {
        Some(text) if !text.is_empty() => {
            edit(host, |m| Some(m.compose(&text)));
        }
        Some(_) => {
            edit(host, Mirror::cancel_composition);
        }
        None => {}
    }
    true
}

/// Page thread: the input method closed its composition without committing what remains.
pub(crate) fn end_composition(host: &Host) {
    edit(host, Mirror::cancel_composition);
}

/// Bind the window's input method context to the connection: on for an editor (at its caret),
/// off otherwise, so keys typed on the page do not start a composition.
fn sync_os(host: &Host) {
    let adapter = host.adapter();
    let Some(weak) = adapter.host.get().cloned() else {
        return;
    };
    adapter.dispatcher.post(Box::new(move || {
        let Some(host) = weak.upgrade() else { return };
        let adapter = host.adapter();
        let hwnd = adapter.hwnd.load(std::sync::atomic::Ordering::Acquire);
        if hwnd == 0 {
            return;
        }
        let editor = adapter
            .ime
            .mirror(|m| (!m.secure).then(|| m.identity.clone()))
            .flatten();
        let caret = editor.and_then(|identity| {
            let frame = host.snapshot()?;
            let node = frame
                .frame
                .nodes
                .iter()
                .find(|node| *node.identity == *identity)?;
            let geometry = node.input.as_ref()?.geometry.as_ref()?;
            Some((geometry.caret, adapter.shown_scale()))
        });
        os::bind(hwnd, adapter.ime.connected(), caret);
    }));
}

mod os {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::UI::Input::Ime::HIMC;
    use windows::Win32::UI::Input::Ime::{
        CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, IACE_DEFAULT,
        ImmAssociateContextEx, ImmGetContext, ImmReleaseContext, ImmSetCandidateWindow,
        ImmSetCompositionWindow,
    };

    pub(super) fn bind(hwnd: isize, enabled: bool, caret: Option<([f32; 4], f32)>) {
        let hwnd = HWND(hwnd as *mut _);
        // SAFETY: this thread's window and its own input context.
        unsafe {
            if !enabled {
                let _ = ImmAssociateContextEx(hwnd, HIMC::default(), 0);
                return;
            }
            let _ = ImmAssociateContextEx(hwnd, HIMC::default(), IACE_DEFAULT);
            let Some(([x, y, w, h], scale)) = caret else {
                return;
            };
            let scale = if scale > 0.0 { scale } else { 1.0 };
            let (left, top) = ((x * scale) as i32, (y * scale) as i32);
            let (right, bottom) = (((x + w.max(1.0)) * scale) as i32, ((y + h) * scale) as i32);
            let himc = ImmGetContext(hwnd);
            if himc.is_invalid() {
                return;
            }
            let _ = ImmSetCompositionWindow(
                himc,
                &COMPOSITIONFORM {
                    dwStyle: CFS_POINT,
                    ptCurrentPos: POINT { x: left, y: top },
                    rcArea: RECT::default(),
                },
            );
            let _ = ImmSetCandidateWindow(
                himc,
                &CANDIDATEFORM {
                    dwIndex: 0,
                    dwStyle: CFS_EXCLUDE,
                    ptCurrentPos: POINT { x: left, y: bottom },
                    rcArea: RECT {
                        left,
                        top,
                        right,
                        bottom,
                    },
                },
            );
            let _ = ImmReleaseContext(hwnd, himc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mirror;
    use serde_json::json;

    fn mirror(text: &str, at: u32) -> Mirror {
        Mirror::new("field", 1, &json!({"text": text, "selection": [at, at]}))
    }

    #[test]
    fn backspace_after_an_astral_character_removes_both_of_its_units() {
        let mut m = mirror("a😀", 3);
        let edits = m.delete(false).unwrap();
        assert_eq!(String::from_utf16_lossy(&m.text), "a");
        assert_eq!((m.anchor, m.focus), (1, 1));
        // One code point, so the core deletes the same span it holds.
        assert_eq!(edits[0]["code_points"], json!(true));
        assert_eq!(edits[0]["before"], json!(1));
    }

    #[test]
    fn a_committed_result_replaces_the_composition_not_the_selection() {
        let mut m = mirror("ab", 2);
        m.compose("ni");
        m.compose("你");
        assert_eq!(m.composition, Some((2, 3)));
        m.insert("你好");
        assert_eq!(String::from_utf16_lossy(&m.text), "ab你好");
        assert_eq!(m.composition, None);
        assert_eq!((m.anchor, m.focus), (4, 4));
    }

    #[test]
    fn a_cancelled_composition_leaves_the_text_it_found() {
        let mut m = mirror("ab", 1);
        m.compose("xyz");
        assert_eq!(String::from_utf16_lossy(&m.text), "axyzb");
        let edits = m.cancel_composition().unwrap();
        assert_eq!(String::from_utf16_lossy(&m.text), "ab");
        assert_eq!(edits.last().unwrap()["op"], json!("finish_composition"));
    }
}
