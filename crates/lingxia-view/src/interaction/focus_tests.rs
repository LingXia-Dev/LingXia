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
    node.modal = true;
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

/// The last focus move: the node and whether its ring shows.
fn focused(effects: &[Effect]) -> Option<(usize, bool)> {
    effects.iter().rev().find_map(|effect| match effect {
        Effect::FocusRing { node, visible } => Some((*node, *visible)),
        _ => None,
    })
}

fn ring(effects: &[Effect]) -> Option<usize> {
    focused(effects)
        .filter(|(_, visible)| *visible)
        .map(|(node, _)| node)
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
fn a_dialog_opened_by_pointer_focuses_inside_without_a_ring_and_returns_focus_on_close() {
    let page = || vec![button("/open", 0.0)];
    let with_dialog = || vec![button("/open", 0.0), dialog("/d"), button("/d/ok", 220.0)];
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, page(), 1);
    tap(&mut interaction, 10.0);
    let effects = present(&mut interaction, with_dialog(), 2);
    assert_eq!(focused(&effects), Some((2, false)), "focused, no ring");
    assert!(
        effects.iter().any(|effect| matches!(
            effect,
            Effect::Accessibility(
                crate::interaction::view::AccessibilityEffect::InputFocused { .. }
            )
        )),
        "assistive tech hears the focus move"
    );
    // A pointer close hands focus back to the control that opened it, still without a ring.
    tap(&mut interaction, 230.0);
    assert_eq!(
        focused(&present(&mut interaction, page(), 3)),
        Some((0, false))
    );
    // Opened and closed from the keyboard, the ring shows.
    let mut keyed = ViewInteraction::default();
    present(&mut keyed, page(), 1);
    assert_eq!(tab(&mut keyed), Some(0));
    assert_eq!(ring(&present(&mut keyed, with_dialog(), 2)), Some(2));
    keyed.key(Key::Escape, false, false, PageTicket::new(1));
    assert_eq!(ring(&present(&mut keyed, page(), 3)), Some(0));
}

#[test]
fn a_closed_popover_returns_focus_to_its_invoker_only_when_focus_was_inside_it() {
    let page = || vec![button("/a", 0.0), button("/more", 50.0)];
    let with_popover = || {
        let mut nodes = page();
        let mut panel = node("/layer/panel", "column", [0.0, 100.0, 200.0, 80.0]);
        panel.panel = true;
        nodes.extend([panel, button("/layer/panel/item", 110.0)]);
        nodes
    };
    // Enter opens it: focus stays on the invoker (HTML popovers take none); Tab goes in;
    // Escape closes it and focus returns to the invoker, visibly.
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, page(), 1);
    tab(&mut interaction);
    assert_eq!(tab(&mut interaction), Some(1));
    assert_eq!(ring(&present(&mut interaction, with_popover(), 2)), Some(1));
    assert_eq!(tab(&mut interaction), Some(3));
    interaction.key(Key::Escape, false, false, PageTicket::new(1));
    assert_eq!(ring(&present(&mut interaction, page(), 3)), Some(1));
    // Focus outside the popover when it closes stays where it is.
    let mut outside = ViewInteraction::default();
    present(&mut outside, page(), 1);
    assert_eq!(tab(&mut outside), Some(0));
    present(&mut outside, with_popover(), 2);
    assert_eq!(ring(&present(&mut outside, page(), 3)), Some(0));
}

#[test]
fn a_menu_opened_from_the_keyboard_focuses_its_first_item_and_returns_focus_on_close() {
    let page = || vec![button("/a", 0.0), button("/more", 50.0)];
    let with_menu = || {
        let mut nodes = page();
        // The popover's dismiss backdrop comes first in the layer and is hidden from assistive tech.
        let mut backdrop = button("/layer/backdrop", 0.0);
        backdrop.accessibility_hidden = Some(true);
        let mut menu = node("/layer/panel", "column", [0.0, 100.0, 100.0, 80.0]);
        menu.role = Some("menu".into());
        let mut rename = button("/layer/panel/rename", 100.0);
        rename.role = Some("menuitem".into());
        let mut delete = button("/layer/panel/delete", 140.0);
        delete.role = Some("menuitem".into());
        nodes.extend([backdrop, menu, rename, delete]);
        nodes
    };
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, page(), 1);
    assert_eq!(tab(&mut interaction), Some(0));
    assert_eq!(tab(&mut interaction), Some(1));
    // Enter on "more" opens the menu: focus lands on its first item, not the backdrop.
    assert_eq!(ring(&present(&mut interaction, with_menu(), 2)), Some(4));
    assert_eq!(tab(&mut interaction), Some(5));
    // Choosing an item closes the menu; focus returns to "more".
    assert_eq!(ring(&present(&mut interaction, page(), 3)), Some(1));
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

fn composite(identity: &str, role: &str, y: f32) -> NodeSnapshot {
    let mut node = node(identity, "column", [0.0, y, 300.0, 40.0]);
    node.role = Some(role.into());
    node
}

/// Item `i` of `container`: a button with `role`, test id `<container>-<i>`, handle 100 + i.
fn item(container: &str, i: usize, role: &str, y: f32) -> NodeSnapshot {
    let mut node = button(&format!("{container}/{i}"), y);
    node.bounds = [40.0 * i as f32, y, 40.0, 40.0];
    node.visible_bounds = node.bounds;
    node.role = Some(role.into());
    node.test_id = format!("{container}-{i}");
    node.text = format!("{container} {i}");
    node.on_click = Some(100 + i as u64);
    node
}

fn press(interaction: &mut ViewInteraction, on: &str, key: &str) -> Option<usize> {
    use crate::interaction::view::AutomationAct;
    interaction
        .automate(
            &format!("[data-testid=\"{on}\"]"),
            None,
            AutomationAct::Press(key.into()),
            false,
            PageTicket::new(1),
        )
        .unwrap();
    interaction.keyboard_focus()
}

fn clicks(interaction: &mut ViewInteraction) -> Vec<u64> {
    interaction
        .take_dispatch()
        .into_iter()
        .filter_map(|dispatch| match dispatch {
            crate::interaction::view::Dispatch::Click { handle, .. } => Some(handle),
            _ => None,
        })
        .collect()
}

#[test]
fn a_tab_list_takes_one_stop_on_its_selected_tab_and_arrows_activate_mirrored_in_rtl() {
    let tabs = |rtl: bool| {
        let mut list = composite("t", "tablist", 0.0);
        list.rtl = rtl;
        let mut nodes = vec![button("/before", 100.0), list];
        for i in 0..3 {
            let mut tab = item("t", i, "tab", 0.0);
            tab.states[1] = Some(i == 1);
            nodes.push(tab);
        }
        nodes.push(button("/after", 200.0));
        nodes
    };
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, tabs(false), 1);
    assert_eq!(tab(&mut interaction), Some(0));
    assert_eq!(
        tab(&mut interaction),
        Some(3),
        "the selected tab is the stop"
    );
    assert_eq!(tab(&mut interaction), Some(5), "Tab leaves the list");
    clicks(&mut interaction);
    assert_eq!(press(&mut interaction, "t-1", "ArrowRight"), Some(4));
    assert_eq!(
        clicks(&mut interaction),
        [102],
        "arriving activates the tab"
    );
    assert_eq!(
        press(&mut interaction, "t-2", "ArrowRight"),
        Some(2),
        "arrows wrap"
    );
    assert_eq!(press(&mut interaction, "t-0", "End"), Some(4));
    present(&mut interaction, tabs(true), 2);
    assert_eq!(
        press(&mut interaction, "t-1", "ArrowRight"),
        Some(2),
        "right goes back in RTL"
    );
}

#[test]
fn a_radio_group_stops_on_its_checked_radio_and_a_toolbar_on_its_last_focused_control() {
    let nodes = || {
        let mut nodes = vec![composite("r", "radiogroup", 0.0)];
        for i in 0..3 {
            let mut radio = item("r", i, "radio", 0.0);
            radio.states[6] = Some(i == 2);
            nodes.push(radio);
        }
        nodes.push(composite("b", "toolbar", 100.0));
        nodes.extend((0..3).map(|i| item("b", i, "", 100.0)));
        nodes
            .iter_mut()
            .filter(|n| n.role.as_deref() == Some(""))
            .for_each(|n| n.role = None);
        nodes.push(button("/after", 200.0));
        nodes
    };
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, nodes(), 1);
    assert_eq!(
        tab(&mut interaction),
        Some(3),
        "the checked radio is the stop"
    );
    assert_eq!(
        press(&mut interaction, "r-2", "ArrowDown"),
        Some(1),
        "down wraps to the first"
    );
    assert_eq!(
        tab(&mut interaction),
        Some(5),
        "the toolbar's first control"
    );
    assert_eq!(press(&mut interaction, "b-0", "ArrowRight"), Some(6));
    clicks(&mut interaction);
    assert_eq!(press(&mut interaction, "b-1", "Home"), Some(5));
    assert!(
        clicks(&mut interaction).is_empty(),
        "a toolbar only moves focus"
    );
    assert_eq!(press(&mut interaction, "b-0", "ArrowRight"), Some(6));
    assert_eq!(tab(&mut interaction), Some(8));
    let back = interaction.key(Key::Tab, true, false, PageTicket::new(1)).1;
    assert_eq!(
        ring(&back),
        Some(6),
        "Shift+Tab returns to the control focused last"
    );
}

#[test]
fn a_list_box_stops_at_its_ends_and_a_tree_walks_levels() {
    let nodes = || {
        let mut nodes = vec![composite("l", "listbox", 0.0)];
        nodes.extend((0..3).map(|i| item("l", i, "option", 0.0)));
        nodes.push(composite("t", "tree", 100.0));
        // Fruit (expanded) > Apple, Pear; Nuts (collapsed).
        for (i, level, expanded) in [
            (0, 1, Some(true)),
            (1, 2, None),
            (2, 2, None),
            (3, 1, Some(false)),
        ] {
            let mut tree_item = item("t", i, "treeitem", 100.0 + 40.0 * i as f32);
            tree_item.heading_level = Some(level);
            tree_item.states[2] = expanded;
            nodes.push(tree_item);
        }
        nodes
    };
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, nodes(), 1);
    assert_eq!(press(&mut interaction, "l-0", "End"), Some(3));
    assert_eq!(
        press(&mut interaction, "l-2", "ArrowDown"),
        Some(3),
        "a list box does not wrap"
    );
    assert_eq!(
        press(&mut interaction, "t-0", "ArrowRight"),
        Some(6),
        "into an expanded item"
    );
    assert_eq!(
        press(&mut interaction, "t-2", "ArrowLeft"),
        Some(5),
        "up to the parent"
    );
    assert!(
        clicks(&mut interaction).is_empty(),
        "moving presses nothing"
    );
    assert_eq!(
        press(&mut interaction, "t-3", "ArrowRight"),
        Some(8),
        "a closed branch keeps focus"
    );
    assert_eq!(clicks(&mut interaction), [103], "and is pressed open");
    assert_eq!(press(&mut interaction, "t-0", "ArrowLeft"), Some(5));
    assert_eq!(
        clicks(&mut interaction),
        [100],
        "an open branch is pressed shut"
    );
    assert_eq!(
        press(&mut interaction, "t-3", "ArrowLeft"),
        Some(8),
        "a closed top-level branch has nowhere to go"
    );
    assert_eq!(
        press(&mut interaction, "t-1", "ArrowRight"),
        Some(6),
        "a leaf stays"
    );
    assert!(clicks(&mut interaction).is_empty());
    assert_eq!(press(&mut interaction, "t-3", "ArrowUp"), Some(7));
    // Right to left the arrows swap.
    let mut mirrored = nodes();
    mirrored[4].rtl = true;
    present(&mut interaction, mirrored, 2);
    press(&mut interaction, "t-3", "ArrowLeft");
    assert_eq!(clicks(&mut interaction), [103], "Left opens");
    press(&mut interaction, "t-0", "ArrowRight");
    assert_eq!(clicks(&mut interaction), [100], "Right closes");
    assert_eq!(
        press(&mut interaction, "t-0", "ArrowLeft"),
        Some(6),
        "Left moves into an open branch"
    );
    assert_eq!(
        press(&mut interaction, "t-2", "ArrowRight"),
        Some(5),
        "Right moves to the parent"
    );
}

fn tap(interaction: &mut ViewInteraction, y: f32) {
    use crate::interaction::view::{Pointer, PointerPhase};
    let at = |phase| Pointer {
        phase,
        x: 10.0,
        y,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    };
    interaction.pointer(at(PointerPhase::Down), PageTicket::new(1));
    interaction.pointer(at(PointerPhase::Up), PageTicket::new(1));
}

#[test]
fn focus_origin_follows_the_last_input_and_only_a_pointer_hides_focus() {
    use crate::interaction::view::AccessibilityAction;
    use crate::interaction::view::FocusOrigin;
    let mut interaction = ViewInteraction::default();
    present(
        &mut interaction,
        vec![button("/a", 0.0), button("/b", 50.0)],
        1,
    );
    assert_eq!(interaction.focus_origin(), FocusOrigin::Pointer);
    assert_eq!(tab(&mut interaction), Some(0));
    assert_eq!(interaction.focus_origin(), FocusOrigin::Keyboard);
    tap(&mut interaction, 60.0);
    assert_eq!(interaction.focus_origin(), FocusOrigin::Pointer);
    let id = interaction.semantics().unwrap().ids.id(0).unwrap();
    interaction.accessibility(id, AccessibilityAction::Focus, PageTicket::new(1));
    assert_eq!(interaction.focus_origin(), FocusOrigin::Assistive);
}

#[test]
fn focus_the_page_moves_itself_keeps_the_last_origin_visible_after_keys_hidden_after_a_pointer() {
    use crate::interaction::view::AccessibilityAction;
    let page = || vec![button("/open", 0.0)];
    let with_dialog = || vec![button("/open", 0.0), dialog("/d"), button("/d/ok", 220.0)];
    // After a key: the dialog's first control takes focus, visibly.
    let mut keyed = ViewInteraction::default();
    present(&mut keyed, page(), 1);
    tab(&mut keyed);
    assert_eq!(ring(&present(&mut keyed, with_dialog(), 2)), Some(2));
    // After a pointer: focus moves in, but nothing shows.
    let mut tapped = ViewInteraction::default();
    present(&mut tapped, page(), 1);
    tap(&mut tapped, 10.0);
    assert_eq!(
        focused(&present(&mut tapped, with_dialog(), 2)),
        Some((2, false))
    );
    // An activation through assistive tech or automation is a tap: no ring either.
    let mut clicked = ViewInteraction::default();
    present(&mut clicked, page(), 1);
    let id = clicked.semantics().unwrap().ids.id(0).unwrap();
    clicked.accessibility(id, AccessibilityAction::Click, PageTicket::new(1));
    assert_eq!(
        focused(&present(&mut clicked, with_dialog(), 2)),
        Some((2, false))
    );
    // After an assistive action: visible, as after a key.
    let mut assisted = ViewInteraction::default();
    present(&mut assisted, page(), 1);
    let id = assisted.semantics().unwrap().ids.id(0).unwrap();
    assisted.accessibility(id, AccessibilityAction::Focus, PageTicket::new(1));
    assert_eq!(ring(&present(&mut assisted, with_dialog(), 2)), Some(2));
}

#[test]
fn a_node_with_a_focus_visible_style_paints_its_own_ring() {
    let mut styled = button("/a", 0.0);
    styled.focus_visible_style = true;
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, vec![styled, button("/b", 50.0)], 1);
    assert!(interaction.rings_itself(0) && !interaction.rings_itself(1));
    assert_eq!(interaction.styled_focus(0).as_deref(), Some("/a"));
    assert_eq!(interaction.styled_focus(1), None);
}

#[test]
fn a_dragged_slider_stays_pressed_until_release() {
    use crate::interaction::view::{Pointer, PointerPhase};
    let mut volume = super::policy_tests::slider_node();
    volume.pressed_style = true;
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, vec![volume], 1);
    let mut at = |phase, x| {
        interaction
            .pointer(
                Pointer {
                    phase,
                    x,
                    y: 44.0,
                    slop: 8.0,
                    velocity: [0.0; 2],
                    min_fling: 50.0,
                },
                PageTicket::new(1),
            )
            .1
    };
    let down = at(PointerPhase::Down, 60.0);
    assert!(down.iter().any(|e| matches!(e, Effect::Press { .. })));
    let moved = at(PointerPhase::Move, 120.0);
    assert!(!moved.contains(&Effect::ClearPress), "{moved:?}");
    assert!(at(PointerPhase::Up, 120.0).contains(&Effect::ClearPress));
}
