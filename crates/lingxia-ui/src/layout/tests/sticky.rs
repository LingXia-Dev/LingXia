//! Sticky headers inside scrolls: composition offsets, containment, nesting.
use std::collections::HashMap;

fn display(render: &str) -> crate::DisplayList {
    let source = crate::session::tests::source(&format!("function render(){{return {render}}}"));
    crate::ViewSession::new(&source)
        .unwrap()
        .layout(200, 200, 1.0)
        .unwrap()
}

fn scrolled(display: &crate::DisplayList, offsets: &[(&str, f32)]) -> crate::DisplayList {
    let ids: HashMap<String, f32> = offsets
        .iter()
        .map(|(test_id, offset)| {
            let node = display
                .nodes
                .iter()
                .find(|n| n.test_id == *test_id)
                .unwrap();
            (node.scroll.as_ref().unwrap().id.clone(), *offset)
        })
        .collect();
    display.retained.compose(&ids).unwrap()
}

fn top(display: &crate::DisplayList, id: &str) -> f32 {
    display
        .nodes
        .iter()
        .find(|n| n.test_id == id)
        .unwrap()
        .bounds[1]
}

const SECTIONS: &str = "h('scroll',{testId:'s',style:{height:200}},h('column',{testId:'a'},h('text',{testId:'ha',style:{position:'sticky',top:0,height:30,backgroundColor:'#ffffff'}},'A'),h('column',{style:{height:270}})),h('column',{testId:'b'},h('text',{testId:'hb',style:{position:'sticky',top:0,height:30}},'B'),h('column',{style:{height:470}})))";

#[test]
fn headers_stick_then_get_pushed_by_the_end_of_their_section() {
    let base = display(SECTIONS);
    assert_eq!(top(&base, "ha"), 0.0);
    // Scrolled 100px: section A's header stays at the viewport top.
    assert_eq!(top(&scrolled(&base, &[("s", 100.0)]), "ha"), 0.0);
    // Scrolled 290px: A ends at 10, so its header is pushed up to -20.
    let pushed = scrolled(&base, &[("s", 290.0)]);
    assert_eq!(top(&pushed, "ha"), -20.0);
    assert_eq!(top(&pushed, "hb"), 10.0);
    // Scrolled 400px: B's header has taken over the top.
    assert_eq!(top(&scrolled(&base, &[("s", 400.0)]), "hb"), 0.0);
    // Stuck headers paint and hit above the content they pass over.
    let stuck = scrolled(&base, &[("s", 400.0)]);
    let order = |id: &str| stuck.nodes.iter().find(|n| n.test_id == id).unwrap().order;
    assert!(order("hb") > order("b"));
}

#[test]
fn an_inner_scroll_owns_its_sticky_and_the_outer_scroll_only_carries_it() {
    let base = display(
        "h('scroll',{testId:'outer',style:{height:200}},h('column',{style:{height:40}}),h('scroll',{testId:'inner',style:{height:100}},h('text',{testId:'h',style:{position:'sticky',top:0,height:20}},'H'),h('column',{style:{height:400}})),h('column',{style:{height:400}}))",
    );
    assert_eq!(top(&base, "h"), 40.0);
    // Scrolling the inner region keeps the header at the inner viewport top.
    assert_eq!(top(&scrolled(&base, &[("inner", 150.0)]), "h"), 40.0);
    // Scrolling only the outer region moves the inner scroll and its header together.
    assert_eq!(top(&scrolled(&base, &[("outer", 30.0)]), "h"), 10.0);
    assert_eq!(
        top(&scrolled(&base, &[("outer", 30.0), ("inner", 150.0)]), "h"),
        10.0
    );
}

#[test]
fn sticky_needs_a_scroll_and_stays_out_of_list_rows() {
    let fails = |render: &str| {
        let source =
            crate::session::tests::source(&format!("function render(){{return {render}}}"));
        crate::ViewSession::new(&source)
            .unwrap()
            .layout(100, 100, 1.0)
            .err()
            .unwrap()
    };
    assert!(
        fails("h('column',{},h('text',{style:{position:'sticky',top:0}},'x'))")
            .contains("ancestor scroll")
    );
    assert!(
        fails("h('list',{count:1,row:()=>h('text',{style:{position:'sticky',top:0}},'x')})")
            .contains("list rows")
    );
    assert!(
        fails("h('scroll',{},h('text',{style:{position:'sticky',top:'10%'}},'x'))")
            .contains("pixel insets")
    );
}
