//! Rich text paragraphs: run styles, link targets and semantics.
use crate::interaction::frame::PresentedFrame;
use crate::scene::paint::Paint;
use crate::semantics::{
    ids::SemanticIds,
    projection::{Projection, Role},
};

fn layout(render: &str, width: u16) -> crate::DisplayList {
    let source = crate::session::tests::source(&format!("function render(){{return {render}}}"));
    crate::ViewSession::new(&source)
        .unwrap()
        .layout(width, 300, 1.0)
        .unwrap()
}

fn node<'a>(display: &'a crate::DisplayList, id: &str) -> &'a crate::scene::node::NodeSnapshot {
    display
        .nodes
        .iter()
        .find(|n| n.test_id == id)
        .unwrap_or_else(|| panic!("{id}"))
}

const RICH: &str = "h('text',{testId:'p',style:{fontSize:16,color:'#000000'}},'Plain ',h('span',{style:{color:'#ff0000',fontWeight:700,backgroundColor:'#ffff00'}},'red bold'),' and ',h('link',{testId:'l',onPress(){}},'a link'))";

#[test]
fn runs_share_one_paragraph_with_their_own_paint() {
    let display = layout(RICH, 400);
    let index = display.nodes.iter().position(|n| n.test_id == "p").unwrap();
    let paints = &display.retained.commands[display.retained.ranges[index].clone()];
    let colors: Vec<[u8; 4]> = paints
        .iter()
        .filter_map(|paint| match paint {
            Paint::Text { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert!(colors.contains(&[0, 0, 0, 255]) && colors.contains(&[255, 0, 0, 255]));
    // Links default to the accent colour and an underline in that colour.
    let accent = crate::theme::Palette::builtin(false)
        .resolve("accent")
        .unwrap();
    assert!(colors.contains(&accent));
    assert!(paints.iter().any(
        |p| matches!(p, Paint::Rect { color, bounds, .. } if *color == accent && bounds[3] <= 2.0)
    ));
    // The highlight paints before every glyph of the paragraph.
    let highlight = paints
        .iter()
        .position(|p| {
            matches!(
                p,
                Paint::Rect {
                    color: [255, 255, 0, 255],
                    ..
                }
            )
        })
        .unwrap();
    let first_text = paints
        .iter()
        .position(|p| matches!(p, Paint::Text { .. }))
        .unwrap();
    assert!(highlight < first_text);
    assert_eq!(node(&display, "p").text, "Plain red bold and a link");
    // One line: the link starts after the earlier runs, on the first line.
    let link = node(&display, "l");
    assert_eq!(link.hit_rects.len(), 1);
    assert!(
        link.bounds[0] > 60.0 && link.bounds[1] < 5.0,
        "{:?}",
        link.bounds
    );
}

#[test]
fn wrapped_links_hit_only_their_line_pieces_and_project_as_links() {
    let display = layout(
        "h('text',{style:{width:120,fontSize:16}},'Read the ',h('link',{testId:'l',onPress(){}},'terms of service and privacy policy'),' now')",
        120,
    );
    let link = node(&display, "l");
    assert!(link.hit_rects.len() >= 2, "{:?}", link.hit_rects);
    let first = link.hit_rects[0];
    let inside = [first[0] + first[2] / 2.0, first[1] + first[3] / 2.0];
    assert!(crate::interaction::frame::contains(link, inside, false));
    // The union box's top-left lies in the first line's plain text, not the link.
    assert!(!crate::interaction::frame::contains(
        link,
        [link.bounds[0] + 1.0, link.bounds[1] + 1.0],
        false
    ));
    let frame = PresentedFrame::new(
        crate::session::lifecycle::CommitTicket::new(1),
        1.0,
        display.nodes.clone(),
    );
    let mut ids = SemanticIds::default();
    ids.update(frame.nodes.iter().map(|n| n.identity.as_str()));
    let projection = Projection {
        frame: &frame,
        ids: &ids,
        keyboard_focus: None,
        inert: false,
    };
    let index = frame.nodes.iter().position(|n| n.test_id == "l").unwrap();
    let semantic = projection.node(ids.id(index).unwrap()).unwrap();
    assert_eq!(
        (semantic.role, semantic.name.as_str()),
        (Role::Link, "terms of service and privacy policy")
    );
    assert!(semantic.clickable && semantic.focusable);
    assert!(
        frame
            .nodes
            .iter()
            .filter(|n| n.tag == "span")
            .all(|n| n.accessibility_hidden == Some(true))
    );
}

#[test]
fn run_admission_and_budgets_are_native() {
    use crate::protocol::{decode, limits, tag};
    use serde_json::json;
    let text = |children: Vec<serde_json::Value>, own: &str| {
        let mut wire = vec![
            json!(tag::TEXT),
            json!(-1),
            json!(""),
            json!(""),
            json!(own),
            json!(0),
        ];
        for child in children {
            wire.extend(child.as_array().unwrap().iter().cloned());
        }
        decode(&wire)
    };
    let run = |kind: usize| json!([kind, 0, "", "", "x", 0]);
    assert!(text(vec![run(tag::SPAN), run(tag::LINK)], "").is_ok());
    assert!(
        text(vec![run(tag::SPAN)], "inline")
            .unwrap_err()
            .contains("UI_TEXT")
    );
    assert!(
        text(vec![json!([tag::COLUMN, 0, "", "", "", 0])], "")
            .unwrap_err()
            .contains("UI_TEXT")
    );
    assert!(
        text((0..=limits::RUNS).map(|_| run(tag::SPAN)).collect(), "")
            .unwrap_err()
            .contains("runs")
    );
    // A link is interactive, so a button's text may not hold one.
    let wire = json!([
        tag::BUTTON,
        -1,
        "",
        "",
        "",
        0,
        tag::TEXT,
        0,
        "",
        "",
        "",
        0,
        tag::LINK,
        6,
        "",
        "",
        "go",
        0
    ]);
    assert!(
        decode(wire.as_array().unwrap())
            .unwrap_err()
            .contains("UI_BUTTON")
    );
}

#[test]
fn pressed_links_repaint_their_run_through_the_pressed_delta() {
    let source = crate::session::tests::source(
        "function render(){return h('text',{},'See ',h('link',{testId:'l',onPress(){},className:'active:text-red-500',style:{color:'#0000ff'}},'more'))}",
    )
    .replace(
        "__lingxiaPublish(render);",
        "configureClasses([{name:'active:text-red-500',when:{state:'active'},style:{color:'#ff0000ff'}}]);__lingxiaPublish(render);",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(200, 50, 1.0).unwrap();
    let link = node(&display, "l");
    assert!(link.pressed_style);
    let identity = link.identity.clone();
    let text_colors = |display: &crate::DisplayList| -> Vec<[u8; 4]> {
        display
            .retained
            .commands
            .iter()
            .filter_map(|paint| match paint {
                Paint::Text { color, .. } => Some(*color),
                _ => None,
            })
            .collect()
    };
    assert!(text_colors(&display).contains(&[0, 0, 255, 255]));
    page.set_pointer_state(Some(&identity), None);
    let pressed = page.layout(200, 50, 1.0).unwrap();
    assert!(text_colors(&pressed).contains(&[255, 0, 0, 255]));
}

#[test]
fn styled_links_are_one_target_and_one_semantic_link() {
    let display = layout(
        "h('text',{style:{fontSize:16}},'Read ',h('link',{testId:'l',onPress(){}},'the ',h('span',{style:{fontWeight:700}},'terms'),' now'))",
        400,
    );
    let links: Vec<_> = display.nodes.iter().filter(|n| n.tag == "link").collect();
    assert_eq!(links.len(), 3);
    let leader = links[0];
    assert!(leader.on_click.is_some() && leader.accessibility_hidden.is_none());
    assert_eq!(leader.text, "the terms now");
    assert_eq!(leader.hit_rects.len(), 3);
    for piece in &links[1..] {
        assert_eq!(
            (piece.on_click, piece.accessibility_hidden),
            (None, Some(true))
        );
    }
    // A point on the bold piece hits the link.
    let bold = leader.hit_rects[1];
    assert!(crate::interaction::frame::contains(
        leader,
        [bold[0] + bold[2] / 2.0, bold[1] + bold[3] / 2.0],
        false
    ));
}
