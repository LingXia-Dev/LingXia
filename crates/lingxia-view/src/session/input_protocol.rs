//! Bounded host input messages. Called only on the owning View worker.
use crate::{
    ViewSession,
    text::input::{Edit, MAX_BATCH_EDITS, OffsetUnit},
};
use serde::Deserialize;

pub const MAX_REQUEST_BYTES: usize = crate::protocol::limits::INPUT_REQUEST_BYTES;
/// A paste carries the clipboard's text and HTML, so it alone may be larger.
pub const MAX_PASTE_REQUEST_BYTES: usize = crate::protocol::limits::PASTE_REQUEST_BYTES;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    DragHitTest {
        identity: String,
        generation: crate::session::lifecycle::CommitTicket,
        token: u64,
        point: [f32; 2],
        edge: [i8; 2],
    },
    HitTest {
        identity: String,
        generation: crate::session::lifecycle::CommitTicket,
        point: [f32; 2],
        word: bool,
    },
    Focus {
        identity: String,
        generation: crate::session::lifecycle::CommitTicket,
    },
    Query {
        identity: String,
        token: u64,
    },
    Blur {
        identity: String,
        token: u64,
    },
    Edit {
        identity: String,
        token: u64,
        edits: Vec<HostEdit>,
        submit: bool,
    },
    /// A paste into an editor with `onPaste`; the core inserts unless the page cancels.
    Paste {
        identity: String,
        token: u64,
        text: String,
        #[serde(default)]
        html: Option<String>,
        plain: bool,
        /// Flavours the platform left out to keep the request in budget: `"html"`.
        #[serde(default)]
        omitted: Option<Vec<String>>,
    },
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum HostEdit {
    Selection {
        anchor: u32,
        focus: u32,
    },
    SelectAll,
    Composition {
        start: u32,
        end: u32,
    },
    Replace {
        text: String,
        cursor: i32,
        composing: bool,
    },
    FinishComposition,
    Undo,
    Redo,
    DeleteSurrounding {
        before: u32,
        after: u32,
        code_points: bool,
    },
}

impl From<HostEdit> for Edit {
    fn from(edit: HostEdit) -> Self {
        match edit {
            HostEdit::Selection { anchor, focus } => Self::Selection { anchor, focus },
            HostEdit::SelectAll => Self::SelectAll,
            HostEdit::Composition { start, end } => Self::Composition { start, end },
            HostEdit::Replace {
                text,
                cursor,
                composing,
            } => Self::Replace {
                text,
                cursor,
                composing,
            },
            HostEdit::FinishComposition => Self::FinishComposition,
            HostEdit::Undo => Self::Undo,
            HostEdit::Redo => Self::Redo,
            HostEdit::DeleteSurrounding {
                before,
                after,
                code_points,
            } => Self::DeleteSurrounding {
                before,
                after,
                unit: if code_points {
                    OffsetUnit::CodePoint
                } else {
                    OffsetUnit::Utf16
                },
            },
        }
    }
}

/// Whether `request` is a well-formed host input request (the platforms' builders are tested against it).
#[cfg(test)]
pub(crate) fn parses(request: &str) -> bool {
    serde_json::from_str::<Request>(request).is_ok()
}

impl ViewSession {
    /// Returns JSON null for an expired connection or stale focus request.
    pub fn input_request(&mut self, request: &str) -> Result<String, String> {
        if request.len() > MAX_PASTE_REQUEST_BYTES {
            return Err("UI_INPUT: request budget exceeded".into());
        }
        let size = request.len();
        let request: Request =
            serde_json::from_str(request).map_err(|_| "UI_INPUT: invalid host request")?;
        if size > MAX_REQUEST_BYTES && !matches!(request, Request::Paste { .. }) {
            return Err("UI_INPUT: request budget exceeded".into());
        }
        if matches!(
            request,
            Request::Focus { .. }
                | Request::Blur { .. }
                | Request::Edit { .. }
                | Request::Paste { .. }
        ) {
            self.input_serial += 1;
        }
        let reply = match request {
            Request::DragHitTest {
                identity,
                generation,
                token,
                point,
                edge,
            } => {
                let selection =
                    self.drag_hit_test_input(generation, &identity, token, point, edge)?;
                return serde_json::to_string(
                    &selection.map(|selection| serde_json::json!({"selection":selection})),
                )
                .map_err(|e| e.to_string());
            }
            Request::HitTest {
                identity,
                generation,
                point,
                word,
            } => {
                let selection = self.hit_test_input(generation, &identity, point, word)?;
                return serde_json::to_string(
                    &selection.map(|selection| serde_json::json!({"selection":selection})),
                )
                .map_err(|e| e.to_string());
            }
            Request::Focus {
                identity,
                generation,
            } => self.focus_input(generation, &identity)?,
            Request::Query { identity, token } => self.input_state(token, &identity),
            Request::Blur { identity, token } => {
                return Ok(self.blur_input(token, &identity)?.to_string());
            }
            Request::Edit {
                identity,
                token,
                edits,
                submit,
            } => {
                if edits.len() + usize::from(submit) > MAX_BATCH_EDITS {
                    return Err("UI_INPUT: too many edits".into());
                }
                let edits: Vec<_> = edits.into_iter().map(Edit::from).collect();
                self.edit_input(token, &identity, &edits, submit)?
            }
            Request::Paste {
                identity,
                token,
                text,
                html,
                plain,
                omitted,
            } => {
                let omitted = omitted.unwrap_or_default();
                if omitted.iter().any(|flavour| flavour != "html") {
                    return Err("UI_INPUT: a paste omits only html".into());
                }
                let html_omitted = !omitted.is_empty();
                self.paste_input(token, &identity, text, html, plain, html_omitted)?
            }
        };
        serde_json::to_string(&reply).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_queries_reject_stale_frames_and_unpresented_edits() {
        let source = crate::session::tests::source(
            "function render(){return h('input',{value:'hello world',style:{height:50,fontSize:20,padding:8}})}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let frame = page.prepare_native(300, 100, 1.0, true, None, Ok).unwrap();
        let id = &frame.nodes[0].identity;
        let generation = page.generation();
        let request=serde_json::json!({"op":"hit_test","identity":id,"generation":generation,"point":[12,20],"word":true}).to_string();
        let reply: serde_json::Value =
            serde_json::from_str(&page.input_request(&request).unwrap()).unwrap();
        assert_eq!(reply["selection"], serde_json::json!([0, 5]));
        let focused = page.focus_input(generation, id).unwrap().unwrap();
        assert_eq!(page.input_request(&request).unwrap(), "null");
        page.prepare_native(300, 100, 1.0, false, None, Ok).unwrap();
        // The focus redecorated the field but moved none of its text: the older frame still answers.
        let reply: serde_json::Value =
            serde_json::from_str(&page.input_request(&request).unwrap()).unwrap();
        assert_eq!(reply["selection"], serde_json::json!([0, 5]));
        let current = page.generation();
        assert!(
            page.hit_test_input(current, id, [12.0, 20.0], false)
                .unwrap()
                .is_some()
        );
        page.edit_input(
            focused.token,
            id,
            &[Edit::Replace {
                text: "changed".into(),
                cursor: 1,
                composing: false,
            }],
            false,
        )
        .unwrap();
        assert!(
            page.hit_test_input(current, id, [12.0, 20.0], false)
                .unwrap()
                .is_none()
        );
        assert!(
            page.prepare_native::<()>(300, 100, 1.0, false, None, |_| Err(
                "failed presentation".into()
            ))
            .is_err()
        );
        assert!(
            page.hit_test_input(current, id, [12.0, 20.0], false)
                .unwrap()
                .is_none()
        );
    }

    /// A paste reaches `onPaste` with its flavours; a handler that cancels inserts its own
    /// result, otherwise the core inserts the plain text (line breaks as spaces on one line).
    #[test]
    fn a_paste_reaches_on_paste_and_inserts_unless_cancelled() {
        use serde_json::{Value, json};
        let source = crate::session::tests::source(
            "function render(){const doc=useLocal('doc','ab'),line=useLocal('line',''),seen=useLocal('seen','');return h('view',{},h('textarea',{bind:doc,'aria-label':'Doc',style:{height:60},onPaste:e=>{seen.set(JSON.stringify([e.text,e.html??null,e.selection.start,e.selection.end,e.plain,e.omitted]));if(e.html){e.preventDefault();doc.replaceRange(e.selection.start,e.selection.end,'<'+e.text+'>')}}}),h('input',{bind:line,'aria-label':'Line',style:{height:30}}),h('text',{},seen.value))}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let frame = page.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
        let editors: Vec<String> = frame
            .nodes
            .iter()
            .filter(|node| node.input.is_some())
            .map(|node| node.identity.to_string())
            .collect();
        let (doc, line) = (&editors[0], &editors[1]);
        let focus = |page: &mut ViewSession, id: &str| {
            let generation = page.generation();
            page.focus_input(generation, id).unwrap().unwrap().token
        };
        let request = |page: &mut ViewSession, value: Value| -> Value {
            serde_json::from_str(&page.input_request(&value.to_string()).unwrap()).unwrap()
        };
        let seen = |page: &mut ViewSession| -> Value {
            let frame = page.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
            let text = frame
                .nodes
                .iter()
                .rev()
                .find(|n| n.tag == "text")
                .unwrap()
                .text
                .clone();
            serde_json::from_str(&text).unwrap()
        };
        let token = focus(&mut page, doc);
        request(
            &mut page,
            json!({"op":"edit","identity":doc,"token":token,"edits":[{"op":"selection","anchor":1,"focus":1}],"submit":false}),
        );
        let reply = request(
            &mut page,
            json!({"op":"paste","identity":doc,"token":token,"text":"3*4","html":"<b>3*4</b>","plain":false}),
        );
        assert_eq!(reply["needs_view_render"], true);
        assert_eq!(
            seen(&mut page),
            json!(["3*4", "<b>3*4</b>", 1, 1, false, []])
        );
        let state = request(
            &mut page,
            json!({"op":"query","identity":doc,"token":token}),
        );
        assert_eq!(
            state["state"]["text"], "a<3*4>b",
            "the handler's own insert, not the default"
        );
        // A plain paste carries no html, so the handler lets the default insert it.
        let reply = request(
            &mut page,
            json!({"op":"paste","identity":doc,"token":token,"text":"x\r\ny","html":"<i>x</i>","plain":true}),
        );
        assert_eq!(reply["state"]["text"], "a<3*4>x\nyb");
        assert_eq!(seen(&mut page)[1], Value::Null);
        // Too large an html is left out and named.
        let big = "<p>".repeat(crate::protocol::limits::PASTE_HTML_BYTES / 3 + 1);
        request(
            &mut page,
            json!({"op":"paste","identity":doc,"token":token,"text":"","html":big,"plain":false}),
        );
        assert_eq!(seen(&mut page)[5], json!(["html"]));
        // No handler: the core inserts, a single line taking breaks as spaces.
        let token = focus(&mut page, line);
        let reply = request(
            &mut page,
            json!({"op":"paste","identity":line,"token":token,"text":"one\ntwo","plain":false}),
        );
        assert_eq!(reply["state"]["text"], "one two");
    }

    /// A platform that left the html out to stay in budget names it; the page hears that, and
    /// the plain text still goes in.
    #[test]
    fn a_paste_whose_html_the_platform_left_out_names_it_omitted() {
        use serde_json::{Value, json};
        let source = crate::session::tests::source(
            "function render(){const doc=useLocal('doc','ab'),seen=useLocal('seen','');return h('view',{},h('textarea',{bind:doc,'aria-label':'Doc',style:{height:60},onPaste:e=>seen.set(JSON.stringify([e.html??null,e.omitted]))}),h('text',{},seen.value))}",
        );
        let mut page = ViewSession::new(&source).unwrap();
        let frame = page.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
        let doc = frame
            .nodes
            .iter()
            .find(|node| node.input.is_some())
            .unwrap()
            .identity
            .to_string();
        let generation = page.generation();
        let token = page.focus_input(generation, &doc).unwrap().unwrap().token;
        let request = |page: &mut ViewSession, value: Value| page.input_request(&value.to_string());
        request(
            &mut page,
            json!({"op":"edit","identity":doc,"token":token,"edits":[{"op":"selection","anchor":2,"focus":2}],"submit":false}),
        )
        .unwrap();
        let reply: Value = serde_json::from_str(
            &request(
                &mut page,
                json!({"op":"paste","identity":doc,"token":token,"text":"x","plain":false,"omitted":["html"]}),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(reply["state"]["text"], "abx");
        let frame = page.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
        let seen = &frame
            .nodes
            .iter()
            .rev()
            .find(|n| n.tag == "text")
            .unwrap()
            .text;
        assert_eq!(
            serde_json::from_str::<Value>(seen).unwrap(),
            json!([null, ["html"]])
        );
        assert!(
            request(
                &mut page,
                json!({"op":"paste","identity":doc,"token":token,"text":"x","plain":false,"omitted":["files"]}),
            )
            .is_err()
        );
    }

    #[test]
    fn protocol_rejects_control_edits_and_ambiguous_offsets() {
        for edit in [
            r#"{"op":"focus"}"#,
            r#"{"op":"value","text":"override"}"#,
            r#"{"op":"selection","anchor":-1,"focus":0}"#,
            r#"{"op":"selection","anchor":0.5,"focus":0}"#,
            r#"{"op":"selection","anchor":0,"focus":0,"extra":true}"#,
            r#"{"op":"replace","text":"\ud800","cursor":1,"composing":true}"#,
        ] {
            assert!(serde_json::from_str::<HostEdit>(edit).is_err(), "{edit}");
        }
        let edit: HostEdit = serde_json::from_str(
            r#"{"op":"replace","text":"\u4f60\ud83d\ude00","cursor":-1,"composing":true}"#,
        )
        .unwrap();
        assert!(
            matches!(Edit::from(edit), Edit::Replace { text, cursor: -1, composing: true } if text == "\u{4f60}\u{1f600}")
        );
    }
}
