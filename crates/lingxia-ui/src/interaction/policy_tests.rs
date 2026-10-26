//! Input, scroll, reach, keyboard and accessibility policy against presented frames.
use super::frame::PresentedFrame;
use super::keyboard;
use super::reach::ReachTracker;
use super::scroll::ScrollModel;
use super::slider::{self, SliderInput};
use super::view::*;
use crate::SliderSnapshot;
use crate::scene::node::{ClipSnapshot, InputSnapshot, NodeSnapshot, ScrollAxis, ScrollSnapshot};
use crate::scene::plot::{PieLayout, PlotSnapshot};
use crate::semantics::ids::SemanticIds;
use crate::semantics::projection::{self, Action, Event, Projection, Role};
use crate::session::lifecycle::{CommitTicket, PageTicket};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) fn node(identity: &str, tag: &'static str, bounds: [f32; 4]) -> NodeSnapshot {
    NodeSnapshot {
        identity: identity.into(),
        tag,
        test_id: String::new(),
        text: String::new(),
        bounds,
        visible_bounds: bounds,
        clips: Vec::new(),
        scroll: None,
        on_click: None,
        checked: None,
        range: None,
        plot: None,
        radio_group: None,
        scroll_parent: None,
        accessibility_label: None,
        accessibility_hidden: None,
        button_content: None,
        radii: None,
        radius: 0.0,
        content_clip: None,
        control_bounds: None,
        input: None,
        clips_children: false,
        pressed_style: false,
        order: 0,
        plot_local: None,
    }
}

fn region(id: &str, parent: Option<&str>, offset: f32, max: f32) -> ScrollSnapshot {
    ScrollSnapshot {
        content_insets: [0.0; 4],
        content_size: [0.0; 2],
        axis: ScrollAxis::Vertical,
        on_reach_start: None,
        on_reach_end: None,
        id: id.into(),
        parent: parent.map(str::to_owned),
        offset,
        max_offset: max,
    }
}

fn frame(nodes: Vec<NodeSnapshot>) -> PresentedFrame {
    PresentedFrame::new(CommitTicket::new(1), 1.0, nodes)
}

fn scrolls(inner_offset: f32, inner_max: f32) -> PresentedFrame {
    let mut outer = node("o", "scroll", [0.0, 0.0, 100.0, 100.0]);
    outer.scroll = Some(region("outer", None, 0.0, 200.0));
    let mut inner = node("i", "scroll", [0.0, 0.0, 100.0, 100.0]);
    inner.scroll = Some(region("inner", Some("outer"), inner_offset, inner_max));
    frame(vec![outer, inner])
}

#[test]
fn nested_scrolling_transfers_only_remaining_distance_on_one_axis() {
    let mut scroll = ScrollModel::default();
    scroll.accept(&scrolls(0.0, 50.0), 0, &HashMap::new());
    assert!(scroll.scroll_by("inner", 75.0));
    assert_eq!(
        (scroll.offset("inner"), scroll.offset("outer")),
        (50.0, 25.0)
    );
    assert!(scroll.scroll_by("inner", -100.0));
    assert_eq!((scroll.offset("inner"), scroll.offset("outer")), (0.0, 0.0));
    assert!(!scroll.scroll_by("inner", -1.0));
    assert!(!scroll.scroll_by("inner", f32::NAN));
    let mut horizontal = scrolls(0.0, 50.0);
    horizontal.nodes[1].scroll.as_mut().unwrap().axis = ScrollAxis::Horizontal;
    scroll.accept(&horizontal, scroll.revision(), &scroll.snapshot());
    assert_eq!(
        scroll
            .target_for_axis(Some("inner"), ScrollAxis::Horizontal)
            .as_deref(),
        Some("inner")
    );
    assert_eq!(
        scroll
            .target_for_axis(Some("inner"), ScrollAxis::Vertical)
            .as_deref(),
        Some("outer")
    );
    assert!(scroll.scroll_by("inner", 75.0));
    assert_eq!(
        (scroll.offset("inner"), scroll.offset("outer")),
        (50.0, 0.0)
    );
    assert!(scroll.can_scroll(Some("inner"), 1, ScrollAxis::Vertical));
    assert!(!scroll.can_scroll(Some("missing"), -1, ScrollAxis::Vertical));
}

#[test]
fn late_frames_keep_newer_finger_movement_and_prune_removed_regions() {
    let mut scroll = ScrollModel::default();
    scroll.accept(&scrolls(0.0, 200.0), 0, &HashMap::new());
    scroll.scroll_by("inner", 25.0);
    let (revision, requested) = (scroll.revision(), scroll.snapshot());
    scroll.scroll_by("inner", 30.0);
    // Measurement moved the origin by 50 while 30 of finger movement was pending.
    scroll.accept(&scrolls(75.0, 200.0), revision, &requested);
    assert_eq!(scroll.offset("inner"), 105.0);
    scroll.accept(&scrolls(105.0, 40.0), revision, &requested);
    assert_eq!(scroll.offset("inner"), 40.0);
    scroll.accept(&frame(Vec::new()), scroll.revision(), &scroll.snapshot());
    assert!(scroll.snapshot().is_empty());
    assert!(!scroll.scroll_by("inner", 30.0));
}

#[test]
fn reveal_moves_every_ancestor_once_and_rejects_unreachable_targets_atomically() {
    let mut outer = node("o", "scroll", [0.0, 0.0, 100.0, 100.0]);
    outer.scroll = Some(region("outer", None, 0.0, 200.0));
    let mut inner = node("i", "scroll", [0.0, 120.0, 100.0, 60.0]);
    inner.visible_bounds = [0.0, 120.0, 100.0, 0.0];
    inner.scroll = Some(region("inner", Some("outer"), 0.0, 200.0));
    let presented = frame(vec![outer, inner]);
    let mut scroll = ScrollModel::default();
    scroll.accept(&presented, 0, &HashMap::new());
    let mut target = node("t", "button", [0.0, 320.0, 100.0, 40.0]);
    target.scroll_parent = Some("inner".into());
    assert!(scroll.reveal(&presented, &target));
    assert_eq!(
        (scroll.offset("inner"), scroll.offset("outer")),
        (180.0, 80.0)
    );
    let revision = scroll.revision();
    assert!(
        !scroll.reveal(&presented, &target),
        "pending offsets are not applied twice"
    );
    assert_eq!(revision, scroll.revision());
    let mut missing = target.clone();
    missing.scroll_parent = Some("missing".into());
    assert!(!scroll.can_reveal(&presented, &missing));
}

#[test]
fn reach_edges_fire_once_per_arrival_in_bounded_batches() {
    let edge = |offset, max, visible: bool, start, end| {
        let mut feed = node("feed", "scroll", [0.0, 0.0, 100.0, 100.0]);
        if !visible {
            feed.visible_bounds[3] = 0.0;
        }
        let mut snapshot = region("feed", None, offset, max);
        snapshot.on_reach_start = Some(start);
        snapshot.on_reach_end = Some(end);
        feed.scroll = Some(snapshot);
        feed
    };
    let mut reach = ReachTracker::default();
    assert_eq!(
        reach
            .presented(&frame(vec![edge(0.0, 100.0, true, 11, 12)]))
            .handles,
        [11]
    );
    assert!(
        reach
            .presented(&frame(vec![edge(0.0, 100.0, true, 21, 22)]))
            .handles
            .is_empty()
    );
    assert_eq!(
        reach
            .presented(&frame(vec![edge(100.0, 100.0, true, 21, 22)]))
            .handles,
        [22]
    );
    assert!(
        reach
            .presented(&frame(vec![edge(0.0, 0.0, false, 1, 2)]))
            .handles
            .is_empty()
    );
    reach.clear();
    assert!(
        reach
            .presented(&frame(vec![edge(0.0, 0.0, false, 1, 2)]))
            .handles
            .is_empty()
    );
    assert_eq!(
        reach
            .presented(&frame(vec![edge(0.0, 0.0, true, 1, 2)]))
            .handles,
        [1, 2]
    );
    let many = frame(
        (0..40)
            .map(|i| {
                let mut feed = edge(0.0, 0.0, true, 2 * i + 1, 2 * i + 2);
                feed.scroll.as_mut().unwrap().id = format!("feed-{i}");
                feed
            })
            .collect(),
    );
    let mut reach = ReachTracker::default();
    let mut delivered = std::collections::HashSet::new();
    for expected in [32, 32, 16] {
        let reached = reach.presented(&many);
        assert_eq!(
            reached
                .handles
                .iter()
                .filter(|h| delivered.insert(**h))
                .count(),
            expected
        );
    }
    assert_eq!(delivered.len(), 80);
    assert!(!reach.presented(&many).more);
}

#[test]
fn hit_testing_obeys_rounded_ancestor_clips_and_own_shape() {
    let mut clipped = node("c", "button", [0.0, 0.0, 100.0, 200.0]);
    clipped.visible_bounds = [0.0, 0.0, 100.0, 100.0];
    clipped.clips = vec![ClipSnapshot {
        bounds: [0.0, 0.0, 100.0, 100.0],
        radius: 30.0,
        radii: None,
        transform: None,
    }];
    clipped.on_click = Some(4);
    let presented = frame(vec![clipped.clone()]);
    assert_eq!(
        presented.hit([1.0, 1.0]),
        None,
        "rounded corner masks the child"
    );
    assert_eq!(presented.hit([50.0, 1.0]), Some(0));
    assert_eq!(presented.hit([50.0, 150.0]), None, "clipped away");
    let mut capsule = node("s", "scroll", [10.0, 20.0, 100.0, 40.0]);
    capsule.radius = 1000.0;
    capsule.scroll = Some(region("s", None, 0.0, 10.0));
    let presented = frame(vec![capsule]);
    assert_eq!(
        presented.scroll_hit([11.0, 21.0]),
        None,
        "scroll hits use the own radius"
    );
    assert_eq!(presented.scroll_hit([60.0, 21.0]).as_deref(), Some("s"));
}

#[test]
fn rebinding_survives_only_callback_changes() {
    let mut before = node("stop", "button", [0.0, 0.0, 80.0, 40.0]);
    before.text = "Stop".into();
    before.on_click = Some(11);
    let mut after = before.clone();
    after.on_click = Some(29);
    let presented = frame(vec![node("tick", "text", [0.0; 4]), after.clone()]);
    assert_eq!(presented.rebind(&before), Some(1));
    for change in [
        |n: &mut NodeSnapshot| n.text = "Go".into(),
        |n: &mut NodeSnapshot| n.bounds[0] = 1.0,
        |n: &mut NodeSnapshot| n.on_click = None,
        |n: &mut NodeSnapshot| n.button_content = Some("x".into()),
    ] {
        let mut changed = after.clone();
        change(&mut changed);
        assert_eq!(frame(vec![changed]).rebind(&before), None);
    }
}

fn slider_node() -> NodeSnapshot {
    let mut volume = node("volume", "slider", [10.0, 20.0, 200.0, 48.0]);
    volume.accessibility_label = Some("Volume".into());
    volume.range = Some(SliderSnapshot {
        min: 0.0,
        max: 10.0,
        step: 3.0,
        value: 3.0,
        inset: 24.0,
        enabled: true,
        on_change: Some(7),
    });
    volume
}

#[test]
fn slider_values_snap_and_reach_both_endpoints() {
    let volume = slider_node();
    let range = volume.range.clone().unwrap();
    assert_eq!(slider::value_at(&volume, 0.0), Some(0.0));
    assert_eq!(slider::value_at(&volume, 300.0), Some(10.0));
    assert_eq!(slider::normalize(&range, 4.0), 3.0);
    assert_eq!(slider::normalize(&range, 9.6), 10.0);
    assert_eq!(slider::shift(&range, 10.0, -1), 9.0);
    assert_eq!(slider::shift(&range, 9.0, -1), 6.0);
    assert_eq!(slider::shift(&range, 9.0, 1), 10.0);
    let mut bordered = volume.clone();
    bordered.control_bounds = Some([30.0, 25.0, 120.0, 38.0]);
    assert_eq!(slider::value_at(&bordered, 54.0), Some(0.0));
    assert_eq!(slider::value_at(&bordered, 126.0), Some(10.0));
}

#[test]
fn slider_proposals_coalesce_and_drags_yield_to_vertical_motion() {
    let volume = slider_node();
    let mut input = SliderInput::default();
    let page = PageTicket::new(4);
    assert!(input.propose(&volume, 6.0, page));
    let flight = input.begin_flight().unwrap();
    assert_eq!(input.value(&volume), Some(6.0));
    assert!(input.propose(&volume, 10.0, page));
    assert_eq!(input.value(&volume), Some(10.0));
    assert!(!input.propose(&volume, f64::NAN, page));
    input.cancel();
    assert!(!input.is_current(&flight));
    assert_eq!(input.value(&volume), Some(3.0));
    assert!(input.begin(Some(&volume), [50.0, 40.0]));
    assert!(!input.moved([52.0, 41.0], 8.0));
    assert!(input.moved([80.0, 43.0], 8.0));
    let mut resized = volume.clone();
    resized.bounds[2] = 300.0;
    input.presented(&[resized], false);
    assert!(!input.has_gesture(), "changed geometry cancels the drag");
    input.begin(Some(&volume), [50.0, 40.0]);
    assert!(!input.moved([52.0, 70.0], 8.0));
    assert!(!input.has_gesture());
}

fn radios() -> PresentedFrame {
    let option = |identity: &str, group: &str, checked, click: Option<u64>| {
        let mut option = node(identity, "radio-option", [0.0, 0.0, 40.0, 40.0]);
        option.radio_group = Some(group.into());
        option.checked = Some(checked);
        option.on_click = click;
        option
    };
    let mut pace = node("pace", "radio", [0.0, 0.0, 40.0, 120.0]);
    pace.accessibility_label = Some("Pace".into());
    frame(vec![
        pace,
        option("daily", "pace", false, Some(1)),
        option("weekly", "pace", true, Some(2)),
        option("later", "pace", false, None),
        node("other", "radio", [0.0, 0.0, 40.0, 40.0]),
        option("only", "other", true, Some(3)),
    ])
}

#[test]
fn radio_groups_take_one_tab_stop_and_arrows_wrap_inside_the_group() {
    let presented = radios();
    assert!(!keyboard::tab_stop(&presented, 1));
    assert!(keyboard::tab_stop(&presented, 2));
    assert!(!keyboard::tab_stop(&presented, 3));
    assert!(keyboard::tab_stop(&presented, 5));
    assert_eq!(keyboard::adjacent_option(&presented, 1, 1), Some(2));
    assert_eq!(keyboard::adjacent_option(&presented, 2, 1), Some(1));
    assert_eq!(keyboard::adjacent_option(&presented, 5, 1), Some(5));
    assert_eq!(keyboard::adjacent_option(&presented, 0, 1), None);
    let mut ids = SemanticIds::default();
    ids.update(presented.nodes.iter().map(|n| n.identity.as_str()));
    let semantics = Projection {
        frame: &presented,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
    };
    let group = semantics.node(ids.id(0).unwrap()).unwrap();
    assert_eq!(
        (group.role, group.name.as_str(), group.children.len()),
        (Role::RadioGroup, "Pace", 3)
    );
    let weekly = semantics.node(ids.id(2).unwrap()).unwrap();
    assert_eq!(
        (weekly.role, weekly.parent, weekly.position),
        (Role::RadioButton, ids.id(0), Some((1, 1)))
    );
    assert_eq!(weekly.checked, Some(true));
}

#[test]
fn projection_exposes_roles_ranges_masked_text_and_only_enabled_actions() {
    let volume = slider_node();
    let mut editor = node("editor", "textarea", [0.0, 50.0, 100.0, 40.0]);
    editor.accessibility_label = Some("Account password".into());
    editor.input = Some(InputSnapshot {
        value: String::new(),
        placeholder: "Secret".into(),
        secure: true,
        keyboard: "text".into(),
        auto_focus: false,
        submit_on_enter: false,
        enabled: true,
        on_input: None,
        on_submit: None,
        edit: Some(crate::text::input::Snapshot {
            text: "a😀b".into(),
            selection: [1, 3],
            composition: None,
            focused: true,
            can_undo: false,
            can_redo: false,
        }),
        edit_token: Some(7),
        geometry: None,
        scroll_offset: [0.0; 2],
    });
    let presented = frame(vec![volume.clone(), editor.clone()]);
    let mut ids = SemanticIds::default();
    ids.update(presented.nodes.iter().map(|n| n.identity.as_str()));
    let semantics = Projection {
        frame: &presented,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
    };
    let seek = semantics.node(ids.id(0).unwrap()).unwrap();
    assert_eq!(
        (seek.role, seek.name.as_str(), seek.clickable),
        (Role::Slider, "Volume", false)
    );
    assert!(
        seek.actions.contains(&Action::SetProgress)
            && seek.actions.contains(&Action::ScrollForward)
    );
    let field = semantics.node(ids.id(1).unwrap()).unwrap();
    let text = field.text_field.as_ref().unwrap();
    assert_eq!(
        (text.text.as_str(), text.selection, text.multiline),
        ("••••", Some([1, 3]), true)
    );
    assert_eq!(field.name, "Account password");
    assert!(field.focused && field.actions.contains(&Action::SetText));
    assert_eq!(semantics.input_focus(), ids.id(1));
    let mut disabled = volume;
    disabled.range.as_mut().unwrap().enabled = false;
    let presented = frame(vec![disabled]);
    let semantics = Projection {
        frame: &presented,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
    };
    let inert = semantics.node(ids.id(0).unwrap()).unwrap();
    assert!(!inert.enabled && !inert.actions.contains(&Action::SetProgress));
    let mut changed = editor.clone();
    changed.input.as_mut().unwrap().edit.as_mut().unwrap().text = "secret".into();
    changed
        .input
        .as_mut()
        .unwrap()
        .edit
        .as_mut()
        .unwrap()
        .selection = [6, 6];
    let before = frame(vec![node("x", "text", [0.0; 4]), editor]);
    let mut ids = SemanticIds::default();
    let after = frame(vec![node("x", "text", [0.0; 4]), changed]);
    ids.update(after.nodes.iter().map(|n| n.identity.as_str()));
    let events = projection::changes(Some(&before), &after, &ids);
    assert!(
        matches!(&events[0], Event::TextChanged { before, after, secure: true, .. } if before == "••••" && after == "••••••")
    );
    assert!(matches!(events[1], Event::SelectionChanged { end: 6, .. }));
    assert!(events.contains(&Event::ContentChanged));
    assert_eq!(projection::changes(Some(&after), &after, &ids), Vec::new());
}

fn press(phase: PointerPhase, x: f32, y: f32) -> Pointer {
    Pointer {
        phase,
        x,
        y,
        slop: 8.0,
        velocity: [0.0; 2],
        min_fling: 50.0,
    }
}

fn presented(
    interaction: &mut ViewInteraction,
    nodes: Vec<NodeSnapshot>,
    commit: u64,
) -> Presented {
    let page = PageTicket::new(1);
    let revision = interaction.scroll().revision();
    let offsets = interaction.scroll().snapshot();
    interaction.presented(
        Arc::new(PresentedFrame::new(CommitTicket::new(commit), 2.0, nodes)),
        revision,
        &offsets,
        page,
    )
}

#[test]
fn taps_dispatch_against_the_frame_they_started_on() {
    let mut button = node("go", "button", [0.0, 0.0, 50.0, 50.0]);
    button.on_click = Some(9);
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    presented(&mut interaction, vec![button.clone()], 1);
    let (consumed, effects) = interaction.pointer(press(PointerPhase::Down, 20.0, 20.0), page);
    assert!(
        consumed
            && effects.contains(&Effect::Press {
                node: 0,
                identity: "go".into(),
                styled: false
            })
    );
    let (_, effects) = interaction.pointer(press(PointerPhase::Up, 22.0, 22.0), page);
    assert!(effects.contains(&Effect::Dispatch));
    assert_eq!(
        interaction.take_dispatch(),
        [Dispatch::Click {
            commit: CommitTicket::new(1),
            handle: 9
        }]
    );
    // A press that outlives a frame with only a new handle still dispatches the new handle.
    interaction.pointer(press(PointerPhase::Down, 20.0, 20.0), page);
    button.on_click = Some(10);
    let rebound = presented(&mut interaction, vec![button.clone()], 2);
    assert!(rebound.effects.contains(&Effect::Press {
        node: 0,
        identity: "go".into(),
        styled: false
    }));
    interaction.pointer(press(PointerPhase::Up, 20.0, 20.0), page);
    assert_eq!(
        interaction.take_dispatch(),
        [Dispatch::Click {
            commit: CommitTicket::new(2),
            handle: 10
        }]
    );
    // A press whose label changed underneath it is cancelled.
    interaction.pointer(press(PointerPhase::Down, 20.0, 20.0), page);
    button.text = "Changed".into();
    presented(&mut interaction, vec![button], 3);
    interaction.pointer(press(PointerPhase::Up, 20.0, 20.0), page);
    assert!(interaction.take_dispatch().is_empty());
    let (consumed, _) = interaction.pointer(press(PointerPhase::Down, 500.0, 500.0), page);
    assert!(!consumed, "empty space is left to the host");
}

#[test]
fn drags_scroll_past_slop_and_release_into_a_fling() {
    let mut list = node("list", "scroll", [0.0, 0.0, 100.0, 100.0]);
    list.scroll = Some(region("list", None, 0.0, 300.0));
    let mut row = node("row", "button", [0.0, 0.0, 100.0, 40.0]);
    row.on_click = Some(3);
    row.scroll_parent = Some("list".into());
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    presented(&mut interaction, vec![list, row], 1);
    let (_, effects) = interaction.pointer(press(PointerPhase::Down, 50.0, 150.0), page);
    assert!(effects.contains(&Effect::Claim { exclusive: true }));
    let (_, effects) = interaction.pointer(press(PointerPhase::Move, 50.0, 140.0), page);
    assert!(effects.contains(&Effect::Redraw) && effects.contains(&Effect::ClearPress));
    assert_eq!(
        interaction.scroll().offset("list"),
        1.0,
        "only distance past slop scrolls, in logical pixels"
    );
    let mut up = press(PointerPhase::Up, 50.0, 100.0);
    up.velocity = [0.0, -900.0];
    let (_, effects) = interaction.pointer(up, page);
    assert!(effects.contains(&Effect::Fling {
        region: "list".into(),
        horizontal: false,
        velocity: 900.0
    }));
    assert!(interaction.take_dispatch().is_empty());
}

#[test]
fn keyboard_tab_reveals_and_focuses_then_activates_on_enter() {
    let mut list = node("list", "scroll", [0.0, 0.0, 100.0, 100.0]);
    list.scroll = Some(region("list", None, 0.0, 300.0));
    let mut first = node("first", "button", [0.0, 0.0, 100.0, 40.0]);
    first.on_click = Some(1);
    first.scroll_parent = Some("list".into());
    let mut far = node("far", "button", [0.0, 250.0, 100.0, 40.0]);
    far.visible_bounds = [0.0, 250.0, 100.0, 0.0];
    far.on_click = Some(2);
    far.scroll_parent = Some("list".into());
    let nodes = vec![list, first, far];
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    presented(&mut interaction, nodes.clone(), 1);
    let (consumed, effects) = interaction.key(Key::Tab, false, false, page);
    assert!(consumed && effects.contains(&Effect::FocusRing { node: 1 }));
    let (_, effects) = interaction.key(Key::Tab, false, false, page);
    assert!(
        effects.contains(&Effect::Redraw),
        "the offscreen target is revealed first"
    );
    let mut revealed = nodes.clone();
    revealed[0].scroll.as_mut().unwrap().offset = interaction.scroll().offset("list");
    revealed[2].visible_bounds = [0.0, 60.0, 100.0, 40.0];
    let after = presented(&mut interaction, revealed, 2);
    assert!(after.effects.contains(&Effect::FocusRing { node: 2 }));
    interaction.key(Key::Activate, false, false, page);
    assert_eq!(
        interaction.take_dispatch(),
        [Dispatch::Click {
            commit: CommitTicket::new(2),
            handle: 2
        }]
    );
}

#[test]
fn accessibility_actions_route_through_the_same_policy() {
    let mut chart = node("chart", "plot", [10.0, 20.0, 100.0, 100.0]);
    chart.accessibility_label = Some("Sales".into());
    chart.plot = Some(PlotSnapshot {
        kind: "pie".into(),
        values: vec![10.0, 0.0, 10.0],
        domain: [0.0, 10.0],
        pie_layout: PieLayout::default(),
        selected_index: 0,
        on_select: Some(7),
    });
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    presented(&mut interaction, vec![chart.clone()], 1);
    let id = interaction.semantics().unwrap().roots()[0];
    let node = interaction.semantics().unwrap().node(id).unwrap();
    assert!(node.actions.contains(&Action::ClearSelection));
    assert!(
        !node
            .actions
            .iter()
            .any(|a| matches!(a, Action::SelectItem { index: 1, .. })),
        "empty pie slices are not selectable"
    );
    assert!(
        interaction
            .accessibility(id, AccessibilityAction::ClearSelection, page)
            .0
    );
    assert!(matches!(
        interaction.take_dispatch()[..],
        [Dispatch::Plot {
            mode: 2,
            index: -1,
            ..
        }]
    ));
    assert!(
        !interaction
            .accessibility(id, AccessibilityAction::SelectItem(1), page)
            .0
    );
    let (focused, effects) = interaction.accessibility(id, AccessibilityAction::Focus, page);
    assert!(
        focused && effects.contains(&Effect::Accessibility(AccessibilityEffect::Focused { id }))
    );
    let hidden = presented(&mut interaction, Vec::new(), 2);
    assert!(
        hidden
            .effects
            .contains(&Effect::Accessibility(AccessibilityEffect::FocusCleared {
                id
            }))
    );
    interaction.set_inert(true);
    assert!(
        !interaction
            .accessibility(id, AccessibilityAction::Click, page)
            .0
    );
}

#[test]
fn outside_taps_dismiss_selected_plots_without_activating_content() {
    let mut chart = node("chart", "plot", [0.0, 0.0, 100.0, 100.0]);
    chart.plot = Some(PlotSnapshot {
        kind: "pie".into(),
        values: vec![1.0, 2.0],
        domain: [0.0, 2.0],
        pie_layout: PieLayout::default(),
        selected_index: 1,
        on_select: Some(5),
    });
    let mut interaction = ViewInteraction::default();
    let page = PageTicket::new(1);
    presented(&mut interaction, vec![chart], 1);
    let (consumed, _) = interaction.pointer(press(PointerPhase::Down, 400.0, 400.0), page);
    assert!(consumed, "a selected plot claims the dismissal tap");
    interaction.pointer(press(PointerPhase::Up, 400.0, 400.0), page);
    assert_eq!(
        interaction.take_dispatch(),
        [Dispatch::DismissPlots {
            commit: CommitTicket::new(1),
            identities: vec!["chart".into()]
        }]
    );
}
