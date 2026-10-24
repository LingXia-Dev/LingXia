//! Bounded host input messages. Called only on the owning View worker.
use crate::{
    ViewSession,
    text_input::{Edit, MAX_BATCH_EDITS, OffsetUnit},
};
use serde::Deserialize;

pub(crate) const MAX_REQUEST_BYTES: usize = 512 * 1024;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    DragHitTest {
        identity: String,
        generation: u64,
        token: u64,
        point: [f32; 2],
        edge: [i8; 2],
    },
    HitTest {
        identity: String,
        generation: u64,
        point: [f32; 2],
        word: bool,
    },
    Focus {
        identity: String,
        generation: u64,
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
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum HostEdit {
    Selection {
        anchor: u32,
        focus: u32,
    },
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

impl ViewSession {
    /// Returns JSON null for an expired connection or stale focus request.
    pub fn input_request(&mut self, request: &str) -> Result<String, String> {
        if request.len() > MAX_REQUEST_BYTES {
            return Err("UI_INPUT: request budget exceeded".into());
        }
        let request: Request =
            serde_json::from_str(request).map_err(|_| "UI_INPUT: invalid host request")?;
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
        assert_eq!(page.input_request(&request).unwrap(), "null");
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
