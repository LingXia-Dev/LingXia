//! Keyboard focus: dialogs trap it and give it back; pointers never move it.
use super::policy_tests::node;
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{Effect, Key, ViewInteraction};
use crate::scene::node::NodeSnapshot;
use crate::session::lifecycle::{CommitTicket, PageTicket};
use std::sync::Arc;

fn button(identity: &str, y: f32) -> NodeSnapshot {
    let mut node = node(identity, "button", [0.0, y, 100.0, 40.0]);
    node.on_click = Some(1);
    node
}

fn dialog(identity: &str) -> NodeSnapshot {
    let mut node = node(identity, "column", [0.0, 200.0, 300.0, 200.0]);
    node.role = Some("dialog".into());
    node
}

fn present(
    interaction: &mut ViewInteraction,
    nodes: Vec<NodeSnapshot>,
    commit: u64,
) -> Vec<Effect> {
    interaction
        .presented(
            Arc::new(PresentedFrame::new(CommitTicket::new(commit), 1.0, nodes)),
            0,
            &Default::default(),
            PageTicket::new(1),
        )
        .effects
}

fn ring(effects: &[Effect]) -> Option<usize> {
    effects.iter().rev().find_map(|effect| match effect {
        Effect::FocusRing { node } => Some(*node),
        _ => None,
    })
}

fn tab(interaction: &mut ViewInteraction) -> Option<usize> {
    ring(
        &interaction
            .key(Key::Tab, false, false, PageTicket::new(1))
            .1,
    )
}

#[test]
fn an_open_dialog_keeps_tab_inside_and_returns_focus_when_it_closes() {
    let page = || vec![button("/a", 0.0), button("/open", 50.0)];
    let with_dialog = || {
        let mut nodes = page();
        nodes.extend([
            dialog("/d"),
            button("/d/ok", 220.0),
            button("/d/cancel", 270.0),
        ]);
        nodes
    };
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, page(), 1);
    assert_eq!(tab(&mut interaction), Some(0));
    assert_eq!(tab(&mut interaction), Some(1));
    // Enter on "open" shows the dialog: focus moves to its first control.
    assert_eq!(ring(&present(&mut interaction, with_dialog(), 2)), Some(3));
    assert_eq!(tab(&mut interaction), Some(4));
    assert_eq!(
        tab(&mut interaction),
        Some(3),
        "Tab wraps inside the dialog"
    );
    // Closing it returns focus to "open".
    assert_eq!(ring(&present(&mut interaction, page(), 3)), Some(1));
}

#[test]
fn a_dialog_opened_by_pointer_leaves_focus_and_its_ring_alone() {
    use crate::interaction::view::{Pointer, PointerPhase};
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, vec![button("/open", 0.0)], 1);
    let page = PageTicket::new(1);
    let down = |phase| Pointer {
        phase,
        x: 10.0,
        y: 10.0,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    };
    interaction.pointer(down(PointerPhase::Down), page);
    interaction.pointer(down(PointerPhase::Up), page);
    let effects = present(
        &mut interaction,
        vec![button("/open", 0.0), dialog("/d"), button("/d/ok", 220.0)],
        2,
    );
    assert_eq!(ring(&effects), None);
    // The keyboard still lands inside the dialog first.
    assert_eq!(tab(&mut interaction), Some(2));
}
