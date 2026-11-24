//! One table for what every element kind can do, per way an author makes it act.
use super::frame::{PresentedFrame, clickable, focusable, hit_target};
use super::policy_tests::{node, slider_node};
use crate::scene::node::{DragSnapshot, InputSnapshot, NodeSnapshot, ScrollAxis, SelectSnapshot};
use crate::semantics::ids::SemanticIds;
use crate::semantics::projection::{Action, Projection};
use crate::session::lifecycle::CommitTicket;

#[derive(Clone, Copy, Debug)]
enum Way {
    Bare,
    Handler,
    Disabled,
    Drag,
    Interest,
    Crossing,
}

fn element(tag: &'static str, way: Way) -> NodeSnapshot {
    let mut node = match tag {
        "slider" => slider_node(),
        _ => node("x", tag, [0.0, 0.0, 80.0, 40.0]),
    };
    node.accessibility_label = Some("X".into());
    if matches!(tag, "checkbox" | "switch" | "radio-option") {
        node.checked = Some(false);
    }
    if tag == "select" {
        node.select = Some(SelectSnapshot {
            options: Vec::new(),
            selected: None,
            on_select: None,
        });
    }
    if tag == "input" {
        node.input = Some(InputSnapshot {
            display: None,
            attributes: Default::default(),
            value: String::new(),
            placeholder: String::new(),
            secure: false,
            keyboard: "text".into(),
            auto_focus: false,
            submit_on_enter: false,
            enabled: true,
            on_input: None,
            on_submit: None,
            edit: None,
            edit_token: None,
            geometry: None,
            scroll_offset: [0.0; 2],
            ime: Default::default(),
            raise_keyboard: false,
            show_picker: false,
        });
    }
    if let Some(range) = &mut node.range {
        range.on_change = None;
    }
    match way {
        Way::Bare => {}
        Way::Handler => match tag {
            "select" => node.select.as_mut().unwrap().on_select = Some(5),
            "slider" => node.range.as_mut().unwrap().on_change = Some(5),
            "input" => node.input.as_mut().unwrap().on_input = Some(5),
            _ => node.on_click = Some(5),
        },
        // The scene drops a disabled element's handlers; the table starts from none.
        Way::Disabled => {
            node.disabled = true;
            if let Some(input) = &mut node.input {
                input.enabled = false;
            }
            if let Some(range) = &mut node.range {
                range.enabled = false;
            }
        }
        Way::Drag => {
            node.drag = Some(DragSnapshot {
                axis: ScrollAxis::Vertical,
                points: vec![0.0, 100.0],
                index: 0,
                handle: 7,
                duration: std::time::Duration::from_millis(300),
                bounce: 0.0,
                hides: Vec::new(),
                ghost: false,
            })
        }
        Way::Interest => node.interest_for = Some("tip".into()),
        Way::Crossing => node.on_pointer = [Some(3), None],
    }
    node
}

/// (enabled, focusable, hit target, clickable, accessibility Click)
type Answers = (bool, bool, bool, bool, bool);

#[test]
fn every_kind_and_way_to_act_has_one_decided_answer() {
    use Way::*;
    const T: bool = true;
    const F: bool = false;
    let table: &[(&str, Way, Answers)] = &[
        // A control without a handler is enabled and focusable, but nothing runs.
        ("button", Bare, (T, T, F, F, F)),
        ("button", Handler, (T, T, T, T, T)),
        ("button", Disabled, (F, F, F, F, F)),
        ("button", Drag, (T, T, T, F, F)),
        ("button", Interest, (T, T, F, F, F)),
        ("button", Crossing, (T, T, F, F, F)),
        ("link", Bare, (T, F, F, F, F)),
        ("link", Handler, (T, T, T, T, T)),
        ("link", Disabled, (F, F, F, F, F)),
        ("checkbox", Bare, (T, T, F, F, F)),
        ("checkbox", Handler, (T, T, T, T, T)),
        ("checkbox", Disabled, (F, F, F, F, F)),
        ("switch", Bare, (T, T, F, F, F)),
        ("switch", Handler, (T, T, T, T, T)),
        ("switch", Disabled, (F, F, F, F, F)),
        ("radio-option", Bare, (T, T, F, F, F)),
        ("radio-option", Handler, (T, T, T, T, T)),
        ("radio-option", Disabled, (F, F, F, F, F)),
        ("select", Bare, (T, T, F, F, F)),
        ("select", Handler, (T, T, T, T, T)),
        ("select", Disabled, (F, F, F, F, F)),
        // An editor edits without a handler; a click focuses it.
        ("input", Bare, (T, T, T, F, T)),
        ("input", Handler, (T, T, T, F, T)),
        ("input", Disabled, (F, F, F, F, F)),
        ("slider", Bare, (T, T, F, F, F)),
        ("slider", Handler, (T, T, T, F, F)),
        ("slider", Disabled, (F, F, F, F, F)),
        ("column", Bare, (T, F, F, F, F)),
        ("column", Drag, (T, T, T, F, F)),
        ("column", Crossing, (T, F, F, F, F)),
    ];
    let mut wrong = Vec::new();
    for &(tag, way, expected) in table {
        let node = element(tag, way);
        let presented = PresentedFrame::new(CommitTicket::new(1), 1.0, vec![node.clone()]);
        let mut ids = SemanticIds::default();
        ids.update(presented.nodes.iter().map(|n| n.identity.as_ref()));
        let semantics = Projection {
            frame: &presented,
            ids: &ids,
            keyboard_focus: None,
            inert: false,
            drags: None,
            obscured: [0.0; 4],
            touch_target: 0.0,
        };
        let semantic = semantics.node(ids.id(0).unwrap()).unwrap();
        let got = (
            semantic.enabled,
            focusable(&node),
            hit_target(&node),
            clickable(&node),
            semantic.actions.contains(&Action::Click),
        );
        if got != expected {
            wrong.push(format!("{tag} {way:?}: got {got:?}, want {expected:?}"));
        }
        if tag == "slider" {
            let adjusts = semantic.actions.contains(&Action::SetProgress);
            if adjusts != matches!(way, Handler) {
                wrong.push(format!("slider {way:?}: SetProgress {adjusts}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
