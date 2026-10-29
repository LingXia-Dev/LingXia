//! Right-to-left layout: environment direction, `dir`, flow-relative properties.
use serde_json::json;

fn environment(language: &str) -> serde_json::Value {
    json!({"theme":"light","insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":language,"canDock":false})
}

fn page(render: &str, language: &str) -> crate::ViewSession {
    let source =
        crate::session::tests::source(&format!("function render({{env}}){{return {render}}}"));
    let mut page = crate::ViewSession::new(&source).unwrap();
    page.set_environment(environment(language)).unwrap();
    page
}

fn bounds(display: &crate::DisplayList, id: &str) -> [f32; 4] {
    display
        .nodes
        .iter()
        .find(|node| node.test_id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .bounds
}

const ROW: &str = "h('column',{},h('row',{'data-testid':'row',style:{width:200,paddingInlineStart:10}},h('column',{'data-testid':'a',style:{width:40,height:10,marginInlineEnd:5}}),h('column',{'data-testid':'b',style:{width:40,height:10}})),h('column',{'data-testid':'abs',style:{position:'relative',height:20}},h('column',{'data-testid':'pin',style:{position:'absolute',insetInlineStart:4,width:10,height:10}})),h('column',{'data-testid':'push',style:{width:30,height:5,marginInlineStart:'auto'}}))";

#[test]
fn rows_and_flow_relative_edges_mirror_under_rtl() {
    let ltr = page(ROW, "en").layout(200, 200, 1.0).unwrap();
    assert_eq!([bounds(&ltr, "a")[0], bounds(&ltr, "b")[0]], [10.0, 55.0]);
    assert_eq!(bounds(&ltr, "pin")[0], 4.0);
    assert_eq!(bounds(&ltr, "push")[0], 170.0);
    let rtl = page(ROW, "ar-EG").layout(200, 200, 1.0).unwrap();
    // The first child starts at the right edge, inside the start padding.
    assert_eq!([bounds(&rtl, "a")[0], bounds(&rtl, "b")[0]], [150.0, 105.0]);
    assert_eq!(bounds(&rtl, "pin")[0], 186.0);
    assert_eq!(bounds(&rtl, "push")[0], 0.0);
}

#[test]
fn switching_the_display_language_re_resolves_without_a_new_render() {
    let counted = ROW.replacen(
        "h('column',{},",
        "h('column',{},h('text',{'data-testid':'runs'},String(globalThis.runs=(globalThis.runs||0)+1)),",
        1,
    );
    let mut session = page(&counted, "en");
    let before = session.layout(200, 200, 1.0).unwrap();
    session.set_environment(environment("he")).unwrap();
    // A native frame lays the committed, flow-relative records out again without running the View.
    let after = session
        .prepare_native(200, 200, 1.0, false, None, Ok)
        .unwrap();
    assert_eq!(bounds(&before, "a")[0], 10.0);
    assert_eq!(bounds(&after, "a")[0], 150.0);
    let runs = |display: &crate::DisplayList| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "runs")
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(runs(&before), runs(&after));
}

#[test]
fn dir_overrides_the_environment_and_text_start_follows_direction() {
    let display = page(
        "h('column',{style:{width:200}},h('row',{'data-testid':'ltr',dir:'ltr',style:{width:200}},h('column',{'data-testid':'c',style:{width:20,height:5}})),h('text',{'data-testid':'t',style:{width:200}},'abc'),h('text',{'data-testid':'l',style:{width:200,textAlign:'left'}},'abc'))",
        "fa",
    )
    .layout(200, 200, 1.0)
    .unwrap();
    assert_eq!(bounds(&display, "c")[0], 0.0);
    let glyph_x = |id: &str| {
        let index = display.nodes.iter().position(|n| n.test_id == id).unwrap();
        display.retained.commands[display.retained.ranges[index].clone()]
            .iter()
            .find_map(|paint| match paint {
                crate::scene::paint::Paint::Text { glyphs, .. } => Some(glyphs[0].x),
                _ => None,
            })
            .unwrap()
    };
    assert!(glyph_x("t") > 150.0, "{}", glyph_x("t"));
    assert!(glyph_x("l") < 10.0);
}

#[test]
fn logical_radii_and_borders_resolve_against_the_node_direction() {
    let source = "h('column',{dir:'rtl'},h('column',{'data-testid':'box',style:{width:40,height:40,borderInlineStartWidth:4,borderStartStartRadius:12,backgroundColor:'#ff0000',borderColor:'#000000'}}))";
    let display = page(source, "en").layout(100, 100, 1.0).unwrap();
    let node = display.nodes.iter().find(|n| n.test_id == "box").unwrap();
    // Start-start is top-right in RTL; the start border is on the right.
    assert_eq!(node.radii.unwrap()[1], [12.0, 12.0]);
    assert_eq!(node.radii.unwrap()[0], [0.0, 0.0]);
    // A column's cross start is the right edge in RTL.
    let [x, y, w, _] = node.bounds.map(|v| v as usize);
    assert_eq!(x + w, 100);
    let pixels = display.rasterize().unwrap().pixels;
    let at = |x: usize, y: usize| pixels.data_as_u8_slice()[(y * 100 + x) * 4..][..3].to_vec();
    assert_eq!(at(x + w - 2, y + 20), [0, 0, 0]);
    assert_eq!(at(x + 2, y + 20), [255, 0, 0]);
}

#[test]
fn environment_reports_direction_to_the_view() {
    let display = page("h('text',{'data-testid':'d'},env.direction)", "ur-PK")
        .layout(100, 50, 1.0)
        .unwrap();
    assert_eq!(display.nodes[0].text, "rtl");
    assert!(crate::text::fonts::right_to_left("az-Arab"));
    assert!(!crate::text::fonts::right_to_left("az-Latn"));
    assert!(!crate::text::fonts::right_to_left("zh-Hans-CN"));
}

#[test]
fn rtl_horizontal_scrolls_start_at_the_inline_start_and_move_toward_the_left() {
    let render = "h('scroll',{'data-testid':'s',style:{overflowX:'auto',width:100,height:20}},h('row',{},h('column',{'data-testid':'first',style:{width:80,height:10,flexShrink:0}}),h('column',{'data-testid':'second',style:{width:80,height:10,flexShrink:0}})))";
    for (language, start, scrolled) in [("ar", 20.0, 80.0), ("en", 0.0, -60.0)] {
        let mut session = page(render, language);
        let display = session.layout(100, 100, 1.0).unwrap();
        let scroll = display.nodes.iter().find(|n| n.test_id == "s").unwrap();
        let (id, max) = {
            let s = scroll.scroll.as_ref().unwrap();
            (s.id.clone(), s.max_offset)
        };
        assert_eq!(max, 60.0);
        // Offset 0 shows the inline start; the full offset shows the inline end.
        assert_eq!(bounds(&display, "first")[0], start, "{language}");
        let end = display
            .retained
            .compose(&std::collections::HashMap::from([(id, 60.0)]))
            .unwrap();
        assert_eq!(bounds(&end, "first")[0], scrolled, "{language}");
    }
}
