//! CSS Grid containers, tracks, placement and `order`.
fn layout(render: &str, width: u16) -> crate::DisplayList {
    // The root fills the viewport; wrap so grids size to their content.
    let source = crate::session::tests::source(&format!(
        "function render(){{return h('column',{{}},{render})}}"
    ));
    let mut page = crate::ViewSession::new(&source).unwrap();
    page.layout(width, 400, 1.0).unwrap()
}

fn bounds(display: &crate::DisplayList, id: &str) -> [f32; 4] {
    display
        .nodes
        .iter()
        .find(|node| node.test_id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .bounds
}

fn cell(id: &str, extra: &str) -> String {
    format!("h('column',{{testId:'{id}',style:{{height:20{extra}}}}})")
}

#[test]
fn any_container_lays_out_explicit_tracks_spans_and_rows() {
    let display = layout(
        &format!(
            "h('view',{{style:{{display:'grid',width:300,gridTemplateColumns:'100px 1fr 2fr',gridTemplateRows:'30px auto',columnGap:0,rowGap:10}}}},{},{},{},{})",
            cell("a", ""),
            cell("b", ",gridColumn:'span 2 / span 2'"),
            cell("c", ",gridColumn:'1 / -1'"),
            cell("d", ",gridColumn:'3',gridRow:'1'"),
        ),
        300,
    );
    assert_eq!(bounds(&display, "a"), [0.0, 0.0, 100.0, 20.0]);
    // `d` claims row 1, column 3 first, so `b` restarts at column 1 of the next row.
    let [d, b, c] = ["d", "b", "c"].map(|id| bounds(&display, id));
    assert!(
        (d[0] - (100.0 + 200.0 / 3.0)).abs() <= 1.0 && d[1] == 0.0,
        "{d:?}"
    );
    assert!(
        b[0] == 0.0 && b[1] == 40.0 && (b[2] - (100.0 + 200.0 / 3.0)).abs() <= 1.0,
        "{b:?}"
    );
    assert_eq!((c[0], c[2]), (0.0, 300.0));
}

#[test]
fn auto_fit_minmax_tracks_follow_the_container_width() {
    let render = format!(
        "h('view',{{style:{{display:'grid',gridTemplateColumns:'repeat(auto-fit,minmax(100px,1fr))'}}}},{},{},{},{})",
        cell("a", ""),
        cell("b", ""),
        cell("c", ""),
        cell("d", "")
    );
    let wide = layout(&render, 400);
    assert_eq!(bounds(&wide, "d")[0], 300.0);
    let narrow = layout(&render, 250);
    // Two 125px columns fit; the third item wraps to the second row.
    assert_eq!(bounds(&narrow, "b")[2], 125.0);
    assert_eq!(bounds(&narrow, "c")[1], 20.0);
}

#[test]
fn item_alignment_flow_and_order_follow_css() {
    let display = layout(
        "h('view',{style:{display:'grid',width:200,gridTemplateColumns:'repeat(2, minmax(0, 1fr))',gridAutoRows:'50px',justifyItems:'center',alignItems:'end'}},h('column',{testId:'a',style:{width:20,height:10,order:2}}),h('column',{testId:'b',style:{width:20,height:10,justifySelf:'start'}}),h('column',{testId:'c',style:{width:20,height:10}}))",
        200,
    );
    // `a` is ordered last: b, c fill the first row and a starts the second.
    assert_eq!(bounds(&display, "b"), [0.0, 40.0, 20.0, 10.0]);
    assert_eq!(bounds(&display, "c"), [140.0, 40.0, 20.0, 10.0]);
    assert_eq!(bounds(&display, "a"), [40.0, 90.0, 20.0, 10.0]);
    let column = layout(
        "h('view',{style:{display:'grid',gridTemplateRows:'repeat(2, 20px)',gridAutoFlow:'column',gridAutoColumns:'30px'}},h('column',{testId:'a'}),h('column',{testId:'b'}),h('column',{testId:'c'}))",
        200,
    );
    assert_eq!(bounds(&column, "c")[0..2], [30.0, 0.0]);
    // Reading order keeps the source even when `order` moves an item.
    let ordered = layout(
        "h('row',{},h('text',{testId:'x',style:{order:1}},'x'),h('text',{testId:'y'},'y'))",
        200,
    );
    assert!(bounds(&ordered, "y")[0] < bounds(&ordered, "x")[0]);
    assert_eq!(ordered.nodes[2].test_id, "x");
}

#[test]
fn invalid_tracks_name_the_offending_token() {
    let source = crate::session::tests::source(
        "function render(){return h('view',{style:{display:'grid',gridTemplateColumns:'repeat(auto-fill, 1fr)'}})}",
    );
    let error = crate::ViewSession::new(&source)
        .unwrap()
        .layout(100, 100, 1.0)
        .err()
        .unwrap();
    assert!(
        error.contains("UI_GRID") && error.contains("repeat(auto-fill, 1fr)"),
        "{error}"
    );
}
