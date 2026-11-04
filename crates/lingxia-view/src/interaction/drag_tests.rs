//! `drag` elements: the finger, its release, keys and assistive input against presented frames.
use super::drag::DragModel;
use super::frame::PresentedFrame;
use super::policy_tests::node;
use super::view::*;
use crate::scene::node::{DragSnapshot, NodeSnapshot, ScrollAxis};
use crate::semantics::projection::{Action, Role};
use crate::session::lifecycle::{CommitTicket, PageTicket};
use std::sync::Arc;

/// A 100×200 panel that rests at 0 and also snaps to 100 and 180 below it.
fn panel(index: usize) -> NodeSnapshot {
    let mut panel = node("sheet", "column", [0.0, 0.0, 100.0, 200.0]);
    panel.accessibility_label = Some("Sheet".into());
    panel.drag = Some(DragSnapshot {
        axis: ScrollAxis::Vertical,
        points: vec![0.0, 100.0, 180.0],
        index,
        handle: 7,
        duration: std::time::Duration::from_millis(300),
        bounce: 0.0,
        hides: Vec::new(),
    });
    panel
}

fn present(interaction: &mut ViewInteraction, nodes: Vec<NodeSnapshot>, commit: u64) {
    let offsets = interaction.offsets();
    let revision = interaction.scroll().revision();
    interaction.presented(
        Arc::new(PresentedFrame::new(CommitTicket::new(commit), 1.0, nodes)),
        revision,
        &offsets,
        PageTicket::new(1),
    );
}

fn pointer(phase: PointerPhase, y: f32, velocity: f32) -> Pointer {
    Pointer {
        phase,
        x: 50.0,
        y,
        slop: 8.0,
        velocity: [0.0, velocity],
        min_fling: 50.0,
    }
}

fn events(interaction: &mut ViewInteraction) -> Vec<serde_json::Value> {
    interaction
        .take_dispatch()
        .into_iter()
        .filter_map(|dispatch| match dispatch {
            Dispatch::DragEvent {
                handle, payload, ..
            } => {
                assert_eq!(handle, 7);
                Some(serde_json::from_str(&payload).unwrap())
            }
            _ => None,
        })
        .collect()
}

/// Run every pending release to its end, as the host's spring does.
fn settle(interaction: &mut ViewInteraction) -> Vec<usize> {
    let mut landed = Vec::new();
    for release in interaction.take_drag_releases() {
        interaction.set_drag_offset(&release.identity, release.to, true);
        interaction.drag_settled(&release.identity, release.index);
        landed.push(release.index);
    }
    landed
}

fn offset(interaction: &ViewInteraction) -> f32 {
    interaction.offsets()[&DragSnapshot::key("sheet")]
}

#[test]
fn a_finger_moves_the_element_past_slop_and_its_release_springs_to_the_nearest_point() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, vec![panel(0)], 1);
    assert_eq!(offset(&interaction), 0.0);
    let (consumed, _) = interaction.pointer(pointer(PointerPhase::Down, 50.0, 0.0), page);
    assert!(consumed, "a press on a drag element is the View's");
    let (_, effects) = interaction.pointer(pointer(PointerPhase::Move, 70.0, 0.0), page);
    assert!(effects.contains(&Effect::Claim { exclusive: true }));
    assert_eq!(events(&mut interaction)[0]["phase"], "start");
    assert_eq!(
        offset(&interaction),
        12.0,
        "only distance past slop moves it"
    );
    interaction.pointer(pointer(PointerPhase::Move, 128.0, 0.0), page);
    assert_eq!(offset(&interaction), 70.0);
    interaction.pointer(pointer(PointerPhase::Up, 128.0, 0.0), page);
    let end = events(&mut interaction);
    assert_eq!(
        (end[0]["phase"].as_str(), end[0]["index"].as_u64()),
        (Some("end"), Some(1))
    );
    assert_eq!(settle(&mut interaction), [1]);
    assert_eq!(offset(&interaction), 100.0);
    let settled = events(&mut interaction);
    assert_eq!(
        (settled[0]["phase"].as_str(), settled[0]["index"].as_u64()),
        (Some("settle"), Some(1))
    );
}

#[test]
fn a_flick_carries_on_to_the_next_point_and_past_the_ends_the_element_resists() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, vec![panel(0)], 1);
    interaction.pointer(pointer(PointerPhase::Down, 50.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 68.0, 0.0), page);
    assert_eq!(offset(&interaction), 10.0);
    interaction.pointer(pointer(PointerPhase::Up, 68.0, 400.0), page);
    assert_eq!(
        settle(&mut interaction),
        [1],
        "a short flick still moves one point on"
    );
    // Upward past the most open point it follows only part of the finger.
    interaction.pointer(pointer(PointerPhase::Down, 150.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 130.0, 0.0), page);
    assert_eq!(offset(&interaction), 88.0);
    interaction.pointer(pointer(PointerPhase::Move, 30.0, 0.0), page);
    assert!(
        (offset(&interaction) + 4.2).abs() < 0.01,
        "only the part past 0 resists"
    );
    interaction.pointer(pointer(PointerPhase::Move, 62.0, 0.0), page);
    assert_eq!(
        offset(&interaction),
        20.0,
        "coming back follows the finger exactly"
    );
    interaction.pointer(pointer(PointerPhase::Cancel, 10.0, 0.0), page);
    let end = events(&mut interaction);
    assert_eq!(
        end.last().unwrap()["phase"],
        "end",
        "a cancelled drag still ends"
    );
    assert_eq!(settle(&mut interaction), [0]);
}

#[test]
fn a_finger_catches_a_spring_and_letting_go_without_moving_lets_it_finish() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, vec![panel(0)], 1);
    interaction.pointer(pointer(PointerPhase::Down, 50.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 128.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Up, 128.0, 0.0), page);
    let release = interaction.take_drag_releases().pop().unwrap();
    interaction.set_drag_offset("sheet", 85.0, false);
    interaction.pointer(pointer(PointerPhase::Down, 150.0, 0.0), page);
    assert!(
        interaction.drag_held("sheet"),
        "the spring stops under the finger"
    );
    interaction.pointer(pointer(PointerPhase::Up, 150.0, 0.0), page);
    let resumed = interaction.take_drag_releases();
    assert_eq!((resumed[0].from, resumed[0].to), (85.0, release.to));
}

#[test]
fn keys_on_its_axis_and_assistive_input_step_through_the_points_in_order() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, vec![panel(0)], 1);
    let (handled, _) = interaction.key(Key::Tab, false, false, page);
    assert!(
        handled && interaction.keyboard_focus() == Some(0),
        "a drag element takes focus"
    );
    let (handled, effects) = interaction.key(Key::Down, false, false, page);
    assert!(handled && effects.contains(&Effect::Redraw));
    assert_eq!(settle(&mut interaction), [1]);
    interaction.key(Key::End, false, false, page);
    assert_eq!(settle(&mut interaction), [2]);
    let (handled, _) = interaction.key(Key::Down, false, false, page);
    assert!(handled, "an arrow at the last point is still the element's");
    assert!(interaction.take_drag_releases().is_empty());
    let (handled, _) = interaction.key(Key::Left, false, false, page);
    assert!(!handled, "an arrow across its axis is not");

    let id = interaction.semantics().unwrap().ids.id(0).unwrap();
    let semantic = interaction.semantics().unwrap().node(id).unwrap();
    assert_eq!(semantic.role, Role::Slider);
    assert_eq!(semantic.name, "Sheet");
    let range = semantic.range.unwrap();
    assert_eq!(
        (range.min, range.max, range.value),
        (1.0, 3.0, 3.0),
        "the live point, not the frame's"
    );
    assert!(semantic.focusable && semantic.actions.contains(&Action::ScrollBackward));
    assert!(!semantic.actions.contains(&Action::ScrollForward));
    let tree = interaction.automation_tree().unwrap().to_string();
    assert!(
        tree.contains(r#""drag":{"axis":"y","count":3,"index":2}"#),
        "{tree}"
    );
    let (handled, _) = interaction.accessibility(id, AccessibilityAction::ScrollForward, page);
    assert!(!handled);
    let (handled, _) = interaction.accessibility(id, AccessibilityAction::ScrollBackward, page);
    assert!(handled);
    assert_eq!(settle(&mut interaction), [1]);
    let settled = events(&mut interaction);
    assert_eq!(settled.last().unwrap()["phase"], "settle");
}

#[test]
fn a_view_that_moves_the_snap_index_springs_the_element_there_and_resized_points_carry_it() {
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, vec![panel(0)], 1);
    present(&mut interaction, vec![panel(2)], 2);
    let release = interaction.take_drag_releases();
    assert_eq!(
        (release[0].from, release[0].to, release[0].index),
        (0.0, 180.0, 2)
    );
    interaction.set_drag_offset("sheet", release[0].to, true);
    // A finger on it holds it where it is: an index the View sets meanwhile waits.
    let page = PageTicket::new(1);
    interaction.pointer(pointer(PointerPhase::Down, 150.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 130.0, 0.0), page);
    present(&mut interaction, vec![panel(0)], 3);
    assert!(interaction.take_drag_releases().is_empty());
    interaction.pointer(pointer(PointerPhase::Up, 130.0, 0.0), page);
    assert_eq!(settle(&mut interaction), [2], "the finger's choice wins");
    // A bound index that echoes the settle does not spring again.
    present(&mut interaction, vec![panel(2)], 4);
    assert!(interaction.take_drag_releases().is_empty());
    let mut taller = panel(2);
    taller.drag.as_mut().unwrap().points = vec![0.0, 150.0, 270.0];
    present(&mut interaction, vec![taller], 5);
    assert!(interaction.take_drag_releases().is_empty());
    assert_eq!(offset(&interaction), 270.0);
}

#[test]
fn an_element_that_enters_rests_on_its_point_and_one_point_is_not_steppable() {
    let mut model = DragModel::default();
    let mut only = panel(0);
    only.drag.as_mut().unwrap().points = vec![40.0];
    model.accept(&PresentedFrame::new(
        CommitTicket::new(1),
        1.0,
        vec![only.clone()],
    ));
    assert_eq!(model.offset("sheet"), 40.0);
    assert!(model.step("sheet", 1).is_none() && !super::frame::draggable(&only));
}

/// The panel at its middle point, holding a list that scrolls 300 on the same axis.
fn sheet_with_list(contain: bool) -> Vec<NodeSnapshot> {
    let mut list = node("sheet/list", "scroll", [0.0, 0.0, 100.0, 200.0]);
    let mut region = super::policy_tests::region("list", None, 0.0, 300.0);
    region.contain = contain;
    list.scroll = Some(region);
    vec![panel(1), list]
}

fn scrolled(interaction: &ViewInteraction, id: &str) -> f32 {
    interaction.scroll().offset(id)
}

#[test]
fn expanding_moves_the_element_first_and_collapsing_the_inner_scroll_first_in_one_gesture() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, sheet_with_list(false), 1);
    interaction.pointer(pointer(PointerPhase::Down, 150.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 130.0, 0.0), page);
    assert_eq!(
        (offset(&interaction), scrolled(&interaction, "list")),
        (88.0, 0.0)
    );
    assert_eq!(events(&mut interaction)[0]["phase"], "start");
    // Fully open, the rest of the same move scrolls the list.
    interaction.pointer(pointer(PointerPhase::Move, 30.0, 0.0), page);
    assert_eq!(
        (offset(&interaction), scrolled(&interaction, "list")),
        (0.0, 12.0)
    );
    // Back down: the list returns to its top before the element follows.
    interaction.pointer(pointer(PointerPhase::Move, 60.0, 0.0), page);
    assert_eq!(
        (offset(&interaction), scrolled(&interaction, "list")),
        (18.0, 0.0)
    );
    interaction.pointer(pointer(PointerPhase::Up, 60.0, 0.0), page);
    assert_eq!(settle(&mut interaction), [0]);
}

#[test]
fn a_release_while_the_inner_scroll_moves_flings_the_scroll_and_rests_the_element() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, sheet_with_list(false), 1);
    interaction.pointer(pointer(PointerPhase::Down, 150.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 130.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 20.0, 0.0), page);
    let (_, effects) = interaction.pointer(pointer(PointerPhase::Up, 20.0, -900.0), page);
    assert!(effects.contains(&Effect::Fling {
        region: "list".into(),
        horizontal: false,
        velocity: 900.0
    }));
    assert_eq!(
        settle(&mut interaction),
        [0],
        "the flick went to the list, not the element"
    );
}

#[test]
fn a_contained_inner_scroll_keeps_the_gesture() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, sheet_with_list(true), 1);
    interaction.pointer(pointer(PointerPhase::Down, 150.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 100.0, 0.0), page);
    assert_eq!(
        (offset(&interaction), scrolled(&interaction, "list")),
        (100.0, 42.0)
    );
    assert!(events(&mut interaction).is_empty());
}

#[test]
fn distance_past_the_points_passes_to_an_enclosing_scroll_and_across_its_axis_the_element_stays() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    let mut outer = node("page", "scroll", [0.0, 0.0, 100.0, 400.0]);
    outer.scroll = Some(super::policy_tests::region("page", None, 0.0, 500.0));
    let mut inside = panel(1);
    inside.identity = "page/sheet".into();
    inside.scroll_parent = Some("page".into());
    present(&mut interaction, vec![outer.clone(), inside.clone()], 1);
    interaction.pointer(pointer(PointerPhase::Down, 150.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 130.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 30.0, 0.0), page);
    let at = interaction.offsets()[&DragSnapshot::key("page/sheet")];
    assert_eq!((at, scrolled(&interaction, "page")), (0.0, 12.0));
    interaction.pointer(pointer(PointerPhase::Up, 30.0, 0.0), page);
    interaction.take_drag_releases();
    interaction.take_dispatch();

    // A row that drags sideways inside a vertical list: a vertical finger scrolls the list.
    let mut row = node("page/row", "button", [0.0, 0.0, 100.0, 60.0]);
    row.on_click = Some(3);
    row.scroll_parent = Some("page".into());
    row.accessibility_label = Some("Row".into());
    row.drag = Some(DragSnapshot {
        axis: ScrollAxis::Horizontal,
        ..panel(0).drag.unwrap()
    });
    present(&mut interaction, vec![outer, row], 2);
    interaction.pointer(pointer(PointerPhase::Down, 50.0, 0.0), page);
    interaction.pointer(pointer(PointerPhase::Move, 10.0, 0.0), page);
    assert_eq!(
        scrolled(&interaction, "page"),
        32.0,
        "only distance past slop"
    );
    assert!(events(&mut interaction).is_empty());
    assert!(interaction.can_scroll_vertically(-1));
}

#[test]
fn a_settle_on_a_point_that_hides_the_element_says_so() {
    let mut interaction = ViewInteraction::default();
    let mut hidden = panel(0);
    hidden.drag.as_mut().unwrap().hides = vec![false, false, true];
    present(&mut interaction, vec![hidden], 1);
    interaction.drag_settled("sheet", 2);
    interaction.drag_settled("sheet", 1);
    let settled = events(&mut interaction);
    assert_eq!(
        (
            settled[0]["hidden"].as_bool(),
            settled[1]["hidden"].as_bool()
        ),
        (Some(true), Some(false))
    );
}

/// A row that swipes open to reveal its Delete button, which `aria-actions` names.
fn swipe_row(index: usize) -> Vec<NodeSnapshot> {
    let mut delete = node("list/delete", "button", [0.0, 0.0, 100.0, 40.0]);
    delete.on_click = Some(5);
    delete.text = "Delete".into();
    delete.element_id = Some("del-1".into());
    let mut row = node("list/row", "button", [0.0, 0.0, 100.0, 40.0]);
    row.on_click = Some(6);
    row.text = "Groceries".into();
    row.test_id = "row".into();
    row.aria_actions = vec!["del-1".into()];
    row.drag = Some(DragSnapshot {
        axis: ScrollAxis::Horizontal,
        points: vec![0.0, -88.0],
        ..panel(index).drag.unwrap()
    });
    vec![delete, row]
}

#[test]
fn a_swipe_row_offers_the_controls_behind_it_as_its_own_actions() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, swipe_row(0), 1);
    let semantics = interaction.semantics().unwrap();
    let row = semantics.ids.id(1).unwrap();
    let actions = semantics.node(row).unwrap().actions;
    assert!(
        actions.contains(&Action::Custom {
            index: 0,
            label: "Delete".into()
        }),
        "{actions:?}"
    );
    let roots = semantics.roots();
    assert!(
        !roots.contains(&semantics.ids.id(0).unwrap()),
        "a covered control leaves linear navigation"
    );
    assert!(semantics.node(semantics.ids.id(0).unwrap()).is_none());
    assert_eq!(
        crate::semantics::query::select(&semantics, "[role=\"button\"]").unwrap(),
        [1]
    );
    let (handled, _) = interaction.accessibility(row, AccessibilityAction::Custom(0), page);
    assert!(handled);
    assert_eq!(
        interaction.take_dispatch(),
        [Dispatch::Click {
            commit: CommitTicket::new(1),
            handle: 5
        }]
    );
    // Automation names the action as a screen reader reads it.
    let effects = interaction
        .automate(
            "[data-testid=\"row\"]",
            None,
            AutomationAct::Action("Delete".into()),
            false,
            page,
        )
        .unwrap();
    assert!(effects.contains(&Effect::Dispatch));
    assert_eq!(interaction.take_dispatch().len(), 1);
    assert!(
        interaction
            .automate(
                "[data-testid=\"row\"]",
                None,
                AutomationAct::Action("Archive".into()),
                false,
                page
            )
            .unwrap_err()
            .contains("has no action")
    );
    // Swiped open, the control is in reach again.
    present(&mut interaction, swipe_row(1), 2);
    let semantics = interaction.semantics().unwrap();
    assert!(semantics.roots().contains(&semantics.ids.id(0).unwrap()));
}

#[test]
fn an_open_dialog_can_be_dismissed_as_escape_would() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    let mut layer = node("layer", "column", [0.0, 0.0, 100.0, 200.0]);
    layer.close_request = true;
    let mut dialog = node("layer/panel", "column", [0.0, 100.0, 100.0, 100.0]);
    dialog.role = Some("dialog".into());
    dialog.accessibility_label = Some("Filters".into());
    present(&mut interaction, vec![layer, dialog], 1);
    let id = interaction.semantics().unwrap().ids.id(1).unwrap();
    assert!(
        interaction
            .semantics()
            .unwrap()
            .node(id)
            .unwrap()
            .actions
            .contains(&Action::Dismiss)
    );
    let (handled, _) = interaction.accessibility(id, AccessibilityAction::Dismiss, page);
    assert!(handled);
    assert_eq!(interaction.take_dispatch(), [Dispatch::Escape]);
}

/// A 100-wide pager of three pages that stops at every one and reports where it rests.
fn pager(index: usize) -> Vec<NodeSnapshot> {
    let mut pages = node("pager", "scroll", [0.0, 0.0, 100.0, 100.0]);
    let mut region = super::policy_tests::region("pager", None, 0.0, 200.0);
    region.axis = ScrollAxis::Horizontal;
    region.snap = vec![0.0, 100.0, 200.0];
    region.snap_stop = true;
    region.snap_index = Some(index);
    region.on_snap = Some(9);
    pages.scroll = Some(region);
    vec![pages]
}

fn pages_settle(interaction: &mut ViewInteraction) -> Vec<usize> {
    let mut landed = Vec::new();
    for id in interaction.take_moved_snaps() {
        if let Some(to) = interaction.scroll().snap_target(&id) {
            interaction.set_scroll_offset(&id, to);
        }
        interaction.snap_settled(&id);
    }
    for dispatch in interaction.take_dispatch() {
        if let Dispatch::SnapChange {
            handle: 9, index, ..
        } = dispatch
        {
            landed.push(index);
        }
    }
    landed
}

#[test]
fn a_pager_flick_stops_on_the_next_page_and_bind_follows_both_ways() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, pager(0), 1);
    let horizontal = |phase, x: f32, velocity: f32| Pointer {
        phase,
        x,
        y: 50.0,
        slop: 8.0,
        velocity: [velocity, 0.0],
        min_fling: 50.0,
    };
    interaction.pointer(horizontal(PointerPhase::Down, 90.0, 0.0), page);
    interaction.pointer(horizontal(PointerPhase::Move, 60.0, 0.0), page);
    let (_, effects) = interaction.pointer(horizontal(PointerPhase::Up, 60.0, -3000.0), page);
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Fling { .. })),
        "snap-always releases into the spring, not a fling"
    );
    assert_eq!(
        interaction.scroll().snap_target("pager"),
        Some(100.0),
        "one page, however fast"
    );
    assert_eq!(pages_settle(&mut interaction), [1]);
    // The View moves it: it springs there and does not report the index back.
    present(&mut interaction, pager(2), 2);
    assert_eq!(interaction.scroll().snap_target("pager"), Some(200.0));
    pages_settle(&mut interaction);
    // Assistive scrolling steps one page.
    let id = interaction.semantics().unwrap().ids.id(0).unwrap();
    let (handled, _) = interaction.accessibility(id, AccessibilityAction::ScrollBackward, page);
    assert!(handled);
    assert_eq!(pages_settle(&mut interaction), [1]);
}

#[test]
fn a_trackpad_swipe_pages_a_snap_always_region_by_one() {
    let mut interaction = ViewInteraction::default();
    present(&mut interaction, pager(0), 1);
    let wheel = |phase, dx: f32| Wheel {
        phase,
        x: 50.0,
        y: 50.0,
        dx,
        dy: 0.0,
        precise: true,
    };
    interaction.wheel(wheel(WheelPhase::Began, -10.0));
    interaction.wheel(wheel(WheelPhase::Changed, -10.0));
    let moved = interaction.scroll().offset("pager");
    assert!(moved > 12.5, "{moved}");
    assert_eq!(
        interaction.scroll().snap_target("pager"),
        Some(100.0),
        "an eighth of a page goes one on"
    );
    assert_eq!(pages_settle(&mut interaction), [1]);
}

#[test]
fn an_assistive_step_wins_over_a_page_index_requested_before_it() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, pager(1), 1);
    interaction.set_scroll_offset("pager", 100.0);
    pages_settle(&mut interaction);
    // The View asks for page 2; before that frame arrives, a screen reader steps back.
    let requested = interaction.scroll().revision();
    let id = interaction.semantics().unwrap().ids.id(0).unwrap();
    interaction.accessibility(id, AccessibilityAction::ScrollBackward, page);
    let offsets = interaction.offsets();
    let mut late = pager(2);
    late[0].scroll.as_mut().unwrap().offset = 100.0;
    interaction.presented(
        Arc::new(PresentedFrame::new(CommitTicket::new(2), 1.0, late)),
        requested,
        &offsets,
        page,
    );
    assert_eq!(
        interaction.scroll().snap_target("pager"),
        Some(0.0),
        "the later input wins"
    );
    assert_eq!(pages_settle(&mut interaction), [0]);
}

#[test]
fn a_step_before_a_view_set_page_lands_reports_where_it_rests() {
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    present(&mut interaction, pager(1), 1);
    interaction.set_scroll_offset("pager", 100.0);
    pages_settle(&mut interaction);
    // The View moves to page 2; mid-spring a screen reader steps back to page 1.
    present(&mut interaction, pager(2), 2);
    let id = interaction.semantics().unwrap().ids.id(0).unwrap();
    interaction.accessibility(id, AccessibilityAction::ScrollBackward, page);
    assert_eq!(
        pages_settle(&mut interaction),
        [1],
        "the View heard page 2 last, so 1 is news"
    );
}
