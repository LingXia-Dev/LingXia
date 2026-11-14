//! `truncate` on a single-line input: the placeholder and an unfocused value end in an ellipsis;
//! a focused editor shows the whole value; taps and the screen reader always see the whole value.
use crate::ViewSession;
use crate::scene::paint::Paint;

const LONG: &str = "A value much longer than the narrow field";

fn page(value: &str, truncate: bool) -> ViewSession {
    let overflow = if truncate {
        "textOverflow:'ellipsis',"
    } else {
        ""
    };
    ViewSession::new(&super::source(&format!(
        "function render(){{return h('input',{{'data-testid':'f',value:'{value}',placeholder:'{LONG}',style:{{width:120,padding:8,overflow:'hidden',whiteSpace:'nowrap',{overflow}}}}})}}"
    )))
    .unwrap()
}

fn glyphs(display: &crate::DisplayList) -> usize {
    display
        .commands
        .iter()
        .map(|paint| match paint {
            Paint::Text { glyphs, .. } => glyphs.len(),
            _ => 0,
        })
        .sum()
}

#[test]
fn a_truncated_input_ellipsizes_its_placeholder_and_its_value_only_while_unfocused() {
    let chars = LONG.chars().count();
    // Placeholder.
    let mut empty = page("", true);
    let display = empty.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
    assert!(
        glyphs(&display) < chars,
        "the placeholder ends in an ellipsis"
    );

    // Unfocused value: painted short, but hits, retained layout and semantics keep it whole.
    let mut field = page(LONG, true);
    let display = field.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
    let short = glyphs(&display);
    assert!(short < chars, "an unfocused value ends in an ellipsis");
    let node = display.nodes[0].clone();
    assert_eq!(node.input.as_ref().unwrap().value, LONG);
    let mut clipped = page(LONG, false);
    let plain = clipped
        .prepare_native(300, 200, 1.0, true, None, Ok)
        .unwrap();
    assert!(short < glyphs(&plain), "cut, not just clipped");
    let width = |display: &crate::DisplayList| {
        display.retained.inputs[0]
            .as_ref()
            .unwrap()
            .layout
            .full_width()
    };
    assert_eq!(
        width(&display),
        width(&plain),
        "taps map against the whole value"
    );
    let right = [
        node.bounds[0] + node.bounds[2] - 10.0,
        node.bounds[1] + node.bounds[3] / 2.0,
    ];
    let hit = field
        .hit_test_input(field.generation(), &node.identity, right, false)
        .unwrap();
    assert!(hit.is_some());
    assert_eq!(
        hit,
        clipped
            .hit_test_input(clipped.generation(), &plain.nodes[0].identity, right, false)
            .unwrap(),
        "a tap on the ellipsis lands where it would on the unclipped value"
    );
    let mut interaction = crate::interaction::view::ViewInteraction::default();
    interaction.presented(
        std::sync::Arc::new(crate::interaction::frame::PresentedFrame::new(
            crate::session::lifecycle::CommitTicket::new(1),
            1.0,
            display.nodes.clone(),
        )),
        0,
        &Default::default(),
        crate::session::lifecycle::PageTicket::new(1),
    );
    let semantics = interaction.semantics().unwrap();
    let id = semantics.ids.id(0).unwrap();
    assert_eq!(
        semantics.node(id).unwrap().text,
        LONG,
        "the screen reader reads the whole value"
    );

    // Focused: the editor shows the whole value and scrolls; blurred, the ellipsis returns.
    let token = field
        .focus_input(field.generation(), &node.identity)
        .unwrap()
        .unwrap()
        .token;
    let focused = field.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
    assert!(glyphs(&focused) > short, "a focused editor is not cut");
    assert!(field.blur_input(token, &node.identity).unwrap());
    let blurred = field.prepare_native(300, 200, 1.0, true, None, Ok).unwrap();
    assert_eq!(glyphs(&blurred), short);
}
