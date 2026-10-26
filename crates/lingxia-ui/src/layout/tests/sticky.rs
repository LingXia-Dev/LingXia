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

#[test]
fn a_pinned_header_pushes_the_previous_one_fully_out() {
    // The view-lab shape: a bordered, rounded scroll; headers with padding over bordered rows.
    let group = |id: &str, rows: usize| {
        format!(
            "h('column',{{}},h('text',{{testId:'{id}',style:{{position:'sticky',top:0,paddingTop:6,paddingBottom:6,fontSize:12,lineHeight:'16px',backgroundColor:'#eeeeee'}}}},'{id}'){})",
            (0..rows)
                .map(|row| format!(",h('text',{{style:{{paddingTop:12,paddingBottom:12,borderBottomWidth:1,borderColor:'#cccccc'}}}},'row {row}')"))
                .collect::<String>()
        )
    };
    let base = display(&format!(
        "h('column',{{style:{{padding:16}}}},h('scroll',{{testId:'s',style:{{height:192,borderWidth:1,borderColor:'#cccccc',borderRadius:12}}}},{},{},{}))",
        group("today", 2),
        group("yesterday", 3),
        group("week", 2)
    ));
    let rect = |d: &crate::DisplayList, id: &str| {
        let n = d.nodes.iter().find(|n| n.test_id == id).unwrap();
        (n.bounds, n.visible_bounds)
    };
    for offset in 0..300 {
        let d = scrolled(&base, &[("s", offset as f32)]);
        for (earlier, later) in [("today", "yesterday"), ("yesterday", "week")] {
            let (_, shown) = rect(&d, earlier);
            let (next, _) = rect(&d, later);
            if shown[3] > 0.0 {
                let (scroll, _) = rect(&d, "s");
                assert!(
                    shown[1] >= scroll[1] + 1.0,
                    "offset {offset}: {earlier} above the scroll {shown:?}"
                );
                assert!(
                    shown[1] + shown[3] <= next[1] + 0.01,
                    "offset {offset}: {earlier} shows {shown:?} below {later} at {next:?}"
                );
            }
        }
    }
}

#[test]
fn a_pinned_header_and_its_scroll_edge_share_a_device_pixel_row() {
    let scale = 2.75;
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{paddingTop:10.3}},h('scroll',{testId:'s',style:{height:100,borderWidth:1,borderColor:'#cccccc',borderRadius:11}},h('column',{},h('text',{testId:'a',style:{position:'sticky',top:0,height:28,backgroundColor:'#eeeeee'}},'Today'),h('column',{style:{height:90}})),h('column',{},h('text',{testId:'b',style:{position:'sticky',top:0,height:28,backgroundColor:'#eeeeee'}},'Yesterday'),h('column',{style:{height:300}}))))}",
    );
    let base = crate::ViewSession::new(&source)
        .unwrap()
        .layout(330, 600, scale)
        .unwrap();
    for offset in [118.0f32, 153.6, 209.0] {
        let d = scrolled(&base, &[("s", offset)]);
        let pinned = d.nodes.iter().find(|n| n.test_id == "b").unwrap();
        let edge = pinned.clips[0].bounds[1];
        let device = pinned.bounds[1] * scale;
        assert!(
            (device - device.round()).abs() < 1e-3,
            "offset {offset}: {device}"
        );
        assert!(
            (pinned.bounds[1] - edge).abs() < 1e-4,
            "offset {offset}: {} vs {edge}",
            pinned.bounds[1]
        );
    }
}
