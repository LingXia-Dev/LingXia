//! An item larger than its container centres on it and spills past both edges, as CSS does
//! without `safe`; a scroll keeps its content's start reachable instead.

fn bounds(container: &str, child: &str) -> [f32; 4] {
    let module = crate::session::tests::source(&format!(
        "function render(){{return h('view',{{style:{{alignItems:'flex-start'}}}},h('view',{{style:{{marginLeft:100,marginTop:100,{container}}}}},h('view',{{'data-testid':'c',style:{{{child}}}}})))}}"
    ));
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    display
        .nodes
        .iter()
        .find(|node| node.test_id == "c")
        .unwrap()
        .bounds
}

#[test]
fn justify_center_spills_both_ways_in_a_column() {
    let b = bounds(
        "width:100,height:0,justifyContent:'center'",
        "width:100,height:40",
    );
    assert_eq!(b, [100.0, 80.0, 100.0, 40.0]);
}

#[test]
fn justify_center_spills_both_ways_in_a_row() {
    let b = bounds(
        "flexDirection:'row',width:40,height:20,justifyContent:'center'",
        "width:100,height:20,flexShrink:0",
    );
    assert_eq!(b, [70.0, 100.0, 100.0, 20.0]);
}

#[test]
fn items_center_spills_both_ways_in_a_column() {
    let b = bounds(
        "width:40,height:20,alignItems:'center'",
        "width:100,height:20",
    );
    assert_eq!(b, [70.0, 100.0, 100.0, 20.0]);
}

#[test]
fn items_center_spills_both_ways_in_a_row() {
    let b = bounds(
        "flexDirection:'row',width:100,height:0,alignItems:'center'",
        "width:100,height:40",
    );
    assert_eq!(b, [100.0, 80.0, 100.0, 40.0]);
}

#[test]
fn self_center_spills_both_ways() {
    let b = bounds(
        "width:40,height:20",
        "width:100,height:20,alignSelf:'center'",
    );
    assert_eq!(b, [70.0, 100.0, 100.0, 20.0]);
}

#[test]
fn a_centred_scroll_keeps_its_content_start_reachable() {
    let module = crate::session::tests::source(
        "function render(){return h('scroll',{style:{alignItems:'center',justifyContent:'center'}},h('view',{style:{width:200,height:180}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let frame = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(frame.nodes[1].bounds, [0.0, 0.0, 200.0, 180.0]);
    assert!(frame.nodes[0].scroll.as_ref().unwrap().max_offset >= 80.0);
}

#[test]
fn safe_alignment_keeps_an_overflowing_item_at_the_start() {
    let cases = [
        (
            "width:100,height:0,justifyContent:'safe center'",
            "width:100,height:40",
            [100.0, 100.0, 100.0, 40.0],
        ),
        (
            "width:100,height:0,justifyContent:'safe flex-end'",
            "width:100,height:40",
            [100.0, 100.0, 100.0, 40.0],
        ),
        (
            "width:40,height:20,alignItems:'safe center'",
            "width:100,height:20",
            [100.0, 100.0, 100.0, 20.0],
        ),
        (
            "width:40,height:20",
            "width:100,height:20,alignSelf:'safe center'",
            [100.0, 100.0, 100.0, 20.0],
        ),
        (
            "flexDirection:'row',flexWrap:'wrap',width:100,height:20,alignContent:'safe center'",
            "width:100,height:40",
            [100.0, 100.0, 100.0, 40.0],
        ),
        (
            "display:'grid',gridTemplateColumns:1,width:40,justifyItems:'safe center'",
            "width:100,height:20",
            [100.0, 100.0, 100.0, 20.0],
        ),
        (
            "display:'grid',gridTemplateColumns:1,width:40",
            "width:100,height:20,justifySelf:'safe center'",
            [100.0, 100.0, 100.0, 20.0],
        ),
    ];
    for (container, child, expected) in cases {
        assert_eq!(bounds(container, child), expected, "{container} / {child}");
    }
}

#[test]
fn without_safe_the_same_items_spill_both_ways() {
    let cases = [
        (
            "flexDirection:'row',flexWrap:'wrap',width:100,height:20,alignContent:'center'",
            "width:100,height:40",
            [100.0, 90.0, 100.0, 40.0],
        ),
        (
            "display:'grid',gridTemplateColumns:1,width:40,justifyItems:'center'",
            "width:100,height:20",
            [70.0, 100.0, 100.0, 20.0],
        ),
        (
            "display:'grid',gridTemplateColumns:1,width:40",
            "width:100,height:20,justifySelf:'center'",
            [70.0, 100.0, 100.0, 20.0],
        ),
    ];
    for (container, child, expected) in cases {
        assert_eq!(bounds(container, child), expected, "{container} / {child}");
    }
}
