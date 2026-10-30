//! The projection's container hierarchy and composed geometry.
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::ViewInteraction;
use crate::semantics::projection::Role;
use crate::session::lifecycle::{CommitTicket, PageTicket};
use std::collections::HashMap;
use std::sync::Arc;

fn present(display: crate::DisplayList) -> ViewInteraction {
    let mut interaction = ViewInteraction::default();
    interaction.presented(
        Arc::new(PresentedFrame::new(
            CommitTicket::new(1),
            1.0,
            display.nodes,
        )),
        0,
        &Default::default(),
        PageTicket::new(1),
    );
    interaction
}

fn page(render: &str) -> crate::ViewSession {
    let source = crate::session::tests::source(&format!("function render(){{return {render}}}"));
    crate::ViewSession::new(&source).unwrap()
}

const TREE: &str = "h('column',{},h('text',{'data-testid':'title',role:'heading'},'Bills'),h('column',{'data-testid':'list',role:'list'},h('column',{'data-testid':'a',role:'listitem'},h('row',{},h('text',{'data-testid':'a-text'},'Water'))),h('column',{'data-testid':'b',role:'listitem'},h('text',{},'Power')),h('column',{'data-testid':'c',role:'listitem'},h('text',{},'Gas'))),h('column',{'data-testid':'dialog',role:'menu','aria-label':'Confirm'},h('row',{},h('text',{'data-testid':'body'},'Pay all?'),h('button',{'data-testid':'ok',onClick(){}},'OK'))),h('scroll',{'data-testid':'feed',style:{height:80}},h('text',{'data-testid':'f1'},'one'),h('text',{},'two')))";

#[test]
fn containers_with_a_role_hold_their_descendants_and_wrappers_collapse() {
    let i = present(page(TREE).layout(300, 600, 1.0).unwrap());
    let p = i.semantics().unwrap();
    let by_test_id = |id: &str| {
        let index = p.frame.nodes.iter().position(|n| n.test_id == id).unwrap();
        p.node(p.ids.id(index).unwrap()).unwrap()
    };
    let test_ids = |ids: &[u32]| -> Vec<String> {
        ids.iter().map(|id| p.node(*id).unwrap().test_id).collect()
    };
    let list = by_test_id("list");
    assert_eq!(list.role, Role::List);
    assert_eq!(test_ids(&list.children), ["a", "b", "c"]);
    assert_eq!(list.position, Some((0, 3)));
    let second = by_test_id("b");
    assert_eq!(
        (second.parent, second.position),
        (Some(list.id), Some((1, 1)))
    );
    // The row wrapper collapses: the item holds its text directly.
    assert_eq!(test_ids(&by_test_id("a").children), ["a-text"]);
    assert_eq!(by_test_id("a-text").parent, Some(by_test_id("a").id));
    assert_eq!(test_ids(&by_test_id("dialog").children), ["body", "ok"]);
    let feed = by_test_id("feed");
    assert_eq!(test_ids(&feed.children), ["f1", ""]);
    // Roots stay in reading order and leave contained nodes to their container.
    assert_eq!(test_ids(&p.roots()), ["title", "list", "dialog", "feed"]);
    assert!(by_test_id("title").children.is_empty() && by_test_id("title").parent.is_none());
}

// Fixed, sticky and transformed nodes inside clipped, scrolled cards keep painted bounds.
const GEOMETRY: &str = "h('page',{},h('scroll',{'data-testid':'page-scroll',style:{flexGrow:1,flexBasis:0}},h('column',{style:{padding:16,gap:16}},h('column',{style:{borderWidth:1,borderColor:'#000000',borderRadius:16,overflow:'hidden'}},h('column',{style:{position:'fixed',bottom:16,right:16,padding:4}},h('text',{},'fixed')),h('text',{style:{rotate:12}},'tilted'),h('text',{style:{translate:[0,'50%']}},'moved')),h('scroll',{style:{height:120}},h('column',{},h('text',{style:{position:'sticky',top:0}},'Today'),h('spacer',{style:{height:400}}),h('text',{},'end'))),h('spacer',{style:{height:2000}}),h('button',{onClick(){}},'Bottom'))))";

#[test]
fn every_visible_node_has_painted_bounds_at_any_scroll_offset() {
    let mut page = page(GEOMETRY);
    let first = page.layout(360, 640, 1.0).unwrap();
    let scroll = first.nodes[1].scroll.as_ref().unwrap().id.clone();
    for offset in [0.0f32, 300.0, 1800.0] {
        let offsets = HashMap::from([(scroll.clone(), offset)]);
        let display = page
            .prepare_native(360, 640, 1.0, false, Some(&offsets), Ok)
            .unwrap();
        let i = present(display);
        let p = i.semantics().unwrap();
        let mut seen = 0;
        for index in 0..p.frame.nodes.len() {
            let Some(node) = p.ids.id(index).and_then(|id| p.node(id)) else {
                continue;
            };
            if node.visible {
                seen += 1;
                assert!(
                    node.bounds[2] > 0.0 && node.bounds[3] > 0.0,
                    "{:?} at offset {offset}: {:?}",
                    node.name,
                    node.bounds
                );
            }
        }
        let fixed = (0..p.frame.nodes.len())
            .filter_map(|index| p.ids.id(index).and_then(|id| p.node(id)))
            .find(|n| n.name == "fixed")
            .unwrap();
        assert!(fixed.visible, "fixed stays on screen at offset {offset}");
        assert!(seen > 2);
    }
}

#[test]
fn authored_states_and_scroll_position_reach_the_projection() {
    let mut session = page(
        "h('column',{},h('button',{'data-testid':'b',onClick(){},'aria-pressed':true,'aria-expanded':false},'Bold'),h('column',{'data-testid':'l',role:'list','aria-busy':true},h('text',{'data-testid':'i',role:'listitem','aria-selected':true},'One')),h('scroll',{'data-testid':'s',style:{height:50}},h('column',{style:{height:200}})))",
    );
    let first = session.layout(300, 400, 1.0).unwrap();
    let scroll = first.nodes.iter().find(|n| n.test_id == "s").unwrap();
    let offsets = HashMap::from([(scroll.scroll.as_ref().unwrap().id.clone(), 30.0)]);
    let i = present(
        session
            .prepare_native(300, 400, 1.0, false, Some(&offsets), Ok)
            .unwrap(),
    );
    let p = i.semantics().unwrap();
    let by = |id: &str| {
        let index = p.frame.nodes.iter().position(|n| n.test_id == id).unwrap();
        p.node(p.ids.id(index).unwrap()).unwrap()
    };
    let button = by("b");
    assert_eq!(
        (
            button.pressed,
            button.selected,
            button.expanded,
            button.busy
        ),
        (Some(true), None, Some(false), None)
    );
    assert_eq!((by("l").busy, by("i").selected), (Some(true), Some(true)));
    let scroll = by("s").scroll.unwrap();
    assert_eq!(
        (scroll.offset, scroll.max_offset, scroll.horizontal),
        (30.0, 150.0, false)
    );
    let json = serde_json::to_value(&button).unwrap();
    assert_eq!(
        (json["pressed"].as_bool(), json.get("busy")),
        (Some(true), None)
    );
}
