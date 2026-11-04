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

#[test]
fn a_menu_takes_home_end_wrapping_arrows_and_type_ahead() {
    use super::policy_tests::node;
    use super::view::*;
    use crate::session::lifecycle::{CommitTicket, PageTicket};
    let mut menu = node("m", "column", [0.0, 0.0, 100.0, 120.0]);
    menu.role = Some("menu".into());
    menu.accessibility_label = Some("Actions".into());
    let items = ["Rename", "Share", "Delete"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut item = node(
                &format!("m/{i}"),
                "button",
                [0.0, 40.0 * i as f32, 100.0, 40.0],
            );
            item.role = Some("menuitem".into());
            item.text = (*name).into();
            item.test_id = format!("item-{i}");
            item.on_click = Some(10 + i as u64);
            item
        });
    let nodes: Vec<_> = std::iter::once(menu).chain(items).collect();
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    let offsets = interaction.offsets();
    interaction.presented(
        std::sync::Arc::new(super::frame::PresentedFrame::new(
            CommitTicket::new(1),
            1.0,
            nodes,
        )),
        interaction.scroll().revision(),
        &offsets,
        page,
    );
    let press = |interaction: &mut ViewInteraction, on: &str, key: &str| {
        interaction
            .automate(
                &format!("[data-testid=\"{on}\"]"),
                None,
                AutomationAct::Press(key.into()),
                false,
                page,
            )
            .unwrap();
        interaction.keyboard_focus()
    };
    assert_eq!(press(&mut interaction, "item-0", "End"), Some(3));
    assert_eq!(
        press(&mut interaction, "item-2", "ArrowDown"),
        Some(1),
        "arrows wrap inside a menu"
    );
    assert_eq!(press(&mut interaction, "item-0", "Home"), Some(1));
    assert_eq!(
        press(&mut interaction, "item-0", "d"),
        Some(3),
        "a letter jumps to the next item it starts"
    );
    assert_eq!(press(&mut interaction, "item-2", "S"), Some(2));
    assert_eq!(press(&mut interaction, "item-1", "ArrowUp"), Some(1));
}
