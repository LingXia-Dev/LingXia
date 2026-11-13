//! A finger reaches a small control through its box grown to the platform minimum.
use super::frame::PresentedFrame;
use super::policy_tests::node;
use crate::scene::node::{ClipSnapshot, InputSnapshot, NodeSnapshot};
use crate::semantics::ids::SemanticIds;
use crate::semantics::projection::Projection;
use crate::session::lifecycle::CommitTicket;

const IOS: f32 = 44.0;

fn frame(nodes: Vec<NodeSnapshot>) -> PresentedFrame {
    PresentedFrame::new(CommitTicket::new(1), 1.0, nodes)
}

fn page() -> NodeSnapshot {
    node("p", "page", [0.0, 0.0, 400.0, 800.0])
}

fn button(identity: &str, bounds: [f32; 4]) -> NodeSnapshot {
    let mut button = node(identity, "button", bounds);
    button.on_click = Some(1);
    button
}

fn field(identity: &str, bounds: [f32; 4]) -> NodeSnapshot {
    let mut field = node(identity, "input", bounds);
    field.input = Some(InputSnapshot {
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
    });
    field
}

fn clip(bounds: [f32; 4]) -> ClipSnapshot {
    ClipSnapshot {
        bounds,
        radius: 0.0,
        radii: None,
        transform: None,
    }
}

#[test]
fn a_short_field_takes_a_press_in_its_band_and_a_fine_pointer_does_not() {
    let shot = frame(vec![page(), field("p/f", [20.0, 100.0, 200.0, 32.0])]);
    assert_eq!(
        shot.press_hit([60.0, 95.0], IOS),
        Some(1),
        "6 pt band above"
    );
    assert_eq!(
        shot.press_hit([60.0, 137.0], IOS),
        Some(1),
        "6 pt band below"
    );
    assert_eq!(shot.press_hit([60.0, 93.0], IOS), None, "past the band");
    assert_eq!(
        shot.press_hit([60.0, 95.0], 0.0),
        None,
        "a mouse keeps to the box"
    );
    assert_eq!(
        shot.press_hit([60.0, 95.0], IOS),
        shot.press_hit([60.0, 95.0], 48.0)
    );
    assert_eq!(shot.nodes[1].bounds, [20.0, 100.0, 200.0, 32.0]);
}

#[test]
fn a_small_checkbox_grows_on_both_axes_and_an_opt_out_keeps_its_box() {
    let mut check = node("p/c", "checkbox", [100.0, 100.0, 20.0, 20.0]);
    check.on_click = Some(1);
    let shot = frame(vec![page(), check.clone()]);
    assert_eq!(shot.press_hit([89.0, 110.0], IOS), Some(1));
    assert_eq!(shot.press_hit([110.0, 131.0], IOS), Some(1));
    assert_eq!(shot.press_hit([87.0, 110.0], IOS), None);
    check.touch_target_none = true;
    let shot = frame(vec![page(), check]);
    assert_eq!(shot.press_hit([89.0, 110.0], IOS), None);
    assert_eq!(shot.press_hit([110.0, 110.0], IOS), Some(1));
}

#[test]
fn stacked_fields_split_their_shared_band_at_the_midline() {
    let shot = frame(vec![
        page(),
        field("p/a", [0.0, 0.0, 200.0, 32.0]),
        field("p/b", [0.0, 40.0, 200.0, 32.0]),
    ]);
    assert_eq!(shot.press_hit([50.0, 35.0], IOS), Some(1));
    assert_eq!(shot.press_hit([50.0, 37.0], IOS), Some(2));
    assert_eq!(
        shot.press_hit([50.0, 36.0], IOS),
        Some(2),
        "a tie goes to the one painted higher"
    );
}

#[test]
fn two_small_buttons_split_at_the_midline_and_neither_takes_the_other_box() {
    let shot = frame(vec![
        page(),
        button("p/a", [100.0, 100.0, 24.0, 24.0]),
        button("p/b", [128.0, 100.0, 24.0, 24.0]),
    ]);
    assert_eq!(shot.press_hit([125.0, 112.0], IOS), Some(1));
    assert_eq!(shot.press_hit([127.0, 112.0], IOS), Some(2));
    assert_eq!(shot.press_hit([129.0, 112.0], IOS), Some(2), "b's own box");
    assert_eq!(shot.press_hit([122.0, 112.0], IOS), Some(1), "a's own box");
}

#[test]
fn a_button_beside_an_icon_button_never_takes_the_icon() {
    let shot = frame(vec![
        page(),
        button("p/b", [100.0, 100.0, 60.0, 24.0]),
        button("p/i", [160.0, 100.0, 24.0, 24.0]),
    ]);
    // On the icon's own box, even nearer the button's edge: the icon.
    assert_eq!(shot.press_hit([161.0, 112.0], IOS), Some(2));
    assert_eq!(shot.press_hit([159.0, 112.0], IOS), Some(1));
    // Above the icon, inside both bands: the icon is nearer.
    assert_eq!(shot.press_hit([170.0, 96.0], IOS), Some(2));
}

#[test]
fn a_scroll_clip_cuts_the_band() {
    let mut inside = field("p/s/f", [0.0, 50.0, 200.0, 32.0]);
    inside.clips = vec![clip([0.0, 50.0, 400.0, 300.0])];
    let shot = frame(vec![
        page(),
        node("p/s", "scroll", [0.0, 50.0, 400.0, 300.0]),
        inside,
    ]);
    assert_eq!(
        shot.press_hit([50.0, 46.0], IOS),
        None,
        "above the viewport"
    );
    assert_eq!(
        shot.press_hit([50.0, 85.0], IOS),
        Some(2),
        "below, inside it"
    );
}

#[test]
fn a_dialog_close_button_grows_inside_the_panel_and_the_panel_keeps_its_presses() {
    let backdrop = button("p/d/backdrop", [0.0, 0.0, 400.0, 800.0]);
    let mut panel = node("p/d/panel", "column", [50.0, 200.0, 300.0, 300.0]);
    panel.panel = true;
    let close = button("p/d/panel/close", [320.0, 210.0, 20.0, 20.0]);
    let shot = frame(vec![page(), backdrop, panel, close]);
    assert_eq!(
        shot.press_hit([316.0, 220.0], IOS),
        Some(3),
        "band inside the panel"
    );
    assert_eq!(
        shot.press_hit([330.0, 206.0], IOS),
        Some(3),
        "band inside the panel"
    );
    assert_eq!(
        shot.press_hit([100.0, 400.0], IOS),
        None,
        "the panel itself takes nothing"
    );
    assert_eq!(
        shot.press_hit([100.0, 400.0], 0.0),
        None,
        "nor with a mouse"
    );
    assert_eq!(
        shot.press_hit([20.0, 100.0], IOS),
        Some(1),
        "outside it: the backdrop"
    );
}

#[test]
fn a_page_button_under_a_modal_never_reaches_through_its_backdrop() {
    let under = button("p/under", [30.0, 190.0, 20.0, 20.0]);
    let backdrop = button("p/d/backdrop", [0.0, 0.0, 400.0, 800.0]);
    let panel = node("p/d/panel", "column", [50.0, 200.0, 300.0, 300.0]);
    let shot = frame(vec![page(), under, backdrop, panel]);
    assert_eq!(shot.press_hit([45.0, 186.0], IOS), Some(2));
    assert_eq!(shot.press_hit([40.0, 200.0], IOS), Some(2));
}

#[test]
fn a_link_grows_only_vertically_line_by_line() {
    let mut link = node("p/t/l", "link", [10.0, 100.0, 300.0, 36.0]);
    link.on_click = Some(1);
    link.hit_rects = vec![[200.0, 100.0, 110.0, 16.0], [10.0, 120.0, 60.0, 16.0]];
    let shot = frame(vec![
        page(),
        node("p/t", "text", [0.0, 100.0, 400.0, 36.0]),
        link,
    ]);
    assert_eq!(
        shot.press_hit([250.0, 90.0], IOS),
        Some(2),
        "above the first line"
    );
    assert_eq!(
        shot.press_hit([40.0, 145.0], IOS),
        Some(2),
        "below the second line"
    );
    assert_eq!(
        shot.press_hit([75.0, 128.0], IOS),
        None,
        "beside a line: no sideways growth"
    );
}

#[test]
fn a_small_button_in_a_pressable_row_takes_presses_near_it() {
    let row = button("p/r", [0.0, 100.0, 300.0, 56.0]);
    let inner = button("p/r/b", [250.0, 116.0, 24.0, 24.0]);
    let title = node("p/r/t", "text", [16.0, 116.0, 200.0, 24.0]);
    let shot = frame(vec![page(), row, title, inner]);
    assert_eq!(
        shot.press_hit([244.0, 128.0], IOS),
        Some(3),
        "6 pt left of the button"
    );
    assert_eq!(
        shot.press_hit([100.0, 128.0], IOS),
        Some(1),
        "far from it: the row"
    );
    assert_eq!(
        shot.press_hit([244.0, 128.0], 0.0),
        Some(1),
        "a mouse: the row"
    );
}

#[test]
fn a_tap_outside_a_popover_on_a_small_target_still_lands_on_its_backdrop() {
    let small = button("p/s", [100.0, 100.0, 20.0, 20.0]);
    let backdrop = button("p/o/backdrop", [0.0, 0.0, 400.0, 800.0]);
    let panel = node("p/o/panel", "column", [200.0, 300.0, 150.0, 100.0]);
    let shot = frame(vec![page(), small, backdrop, panel]);
    assert_eq!(shot.press_hit([95.0, 110.0], IOS), Some(2));
    assert_eq!(shot.press_hit([110.0, 110.0], IOS), Some(2));
}

#[test]
fn a_box_painted_over_the_band_keeps_it() {
    let small = button("p/s", [100.0, 100.0, 20.0, 20.0]);
    let header = node("p/h", "column", [0.0, 0.0, 400.0, 96.0]);
    let label = node("p/l", "text", [124.0, 100.0, 80.0, 20.0]);
    let shot = frame(vec![page(), small, header, label]);
    assert_eq!(
        shot.press_hit([110.0, 92.0], IOS),
        None,
        "under a header painted above"
    );
    assert_eq!(
        shot.press_hit([126.0, 110.0], IOS),
        Some(1),
        "text never blocks"
    );
}

#[test]
fn touch_exploration_reaches_a_small_control_by_the_same_rule() {
    let mut title = node("p/t", "text", [16.0, 300.0, 100.0, 20.0]);
    title.text = "Title".into();
    let mut small = button("p/b", [100.0, 100.0, 20.0, 20.0]);
    small.text = "Close".into();
    let shot = frame(vec![page(), small, title]);
    let mut ids = SemanticIds::default();
    ids.update(shot.nodes.iter().map(|n| n.identity.as_ref()));
    let semantics = Projection {
        frame: &shot,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
        drags: None,
        obscured: [0.0; 4],
    };
    assert_eq!(semantics.hit([95.0, 110.0], IOS), ids.id(1), "the band");
    assert_ne!(
        semantics.hit([95.0, 110.0], 0.0),
        ids.id(1),
        "no band without a minimum"
    );
    assert_eq!(
        semantics.hit([20.0, 310.0], IOS),
        ids.id(2),
        "text under the finger"
    );
}
