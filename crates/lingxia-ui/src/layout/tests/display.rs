//! Display keywords, fixed positioning and their paint and semantics consequences.

fn layout(render: &str) -> crate::DisplayList {
    let source = crate::session::tests::source(&format!("function render(){{return {render}}}"));
    let mut page = crate::ViewSession::new(&source).unwrap();
    page.layout(200, 200, 1.0).unwrap()
}

fn bounds(display: &crate::DisplayList, id: &str) -> [f32; 4] {
    display
        .nodes
        .iter()
        .find(|node| node.test_id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .bounds
}

#[test]
fn hidden_boxes_take_no_space_paint_nothing_and_leave_semantics() {
    let shown = layout(
        "h('row',{},h('button',{testId:'a',style:{width:40,display:'none',backgroundColor:'#ff0000'},onClick(){}},'Gone'),h('column',{testId:'b',style:{width:30,height:10}}))",
    );
    assert_eq!(bounds(&shown, "b")[0], 0.0);
    let gone = shown.nodes.iter().find(|n| n.test_id == "a").unwrap();
    assert_eq!(gone.on_click, None);
    assert_eq!(gone.accessibility_hidden, Some(true));
    let frame = shown.rasterize().unwrap();
    assert!(
        frame
            .pixels
            .data_as_u8_slice()
            .chunks(4)
            .all(|p| p != [255, 0, 0, 255])
    );
}

#[test]
fn block_stretches_a_column_and_inline_boxes_shrink_to_content() {
    let display = layout(
        "h('row',{style:{width:200,alignItems:'center'}},h('column',{testId:'block',style:{display:'block',width:100}},h('column',{testId:'child',style:{height:10}}),h('column',{testId:'next',style:{height:10}})),h('row',{testId:'inline',style:{display:'inline-flex',height:30}},h('column',{style:{width:20,height:5}})))",
    );
    // A block lays children in a column at full width, even under a row tag.
    assert_eq!(bounds(&display, "child")[2], 100.0);
    assert_eq!(
        bounds(&display, "next")[1],
        bounds(&display, "child")[1] + 10.0
    );
    // Inline boxes align to the start of the cross axis instead of the parent's center.
    assert_eq!(bounds(&display, "inline")[1], 0.0);
    assert_eq!(bounds(&display, "inline")[2], 20.0);
}

#[test]
fn fixed_boxes_position_against_the_page_and_escape_scroll_and_clips() {
    let source = crate::session::tests::source(
        "function render(){return h('page',{},h('scroll',{testId:'s',style:{height:100,marginTop:50}},h('column',{style:{height:400,overflow:'hidden',marginLeft:30}},h('column',{testId:'fab',style:{position:'fixed',bottom:10,right:10,width:20,height:20,backgroundColor:'#00ff00'}}))))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(200, 200, 1.0).unwrap();
    assert_eq!(bounds(&display, "fab"), [170.0, 170.0, 20.0, 20.0]);
    let scroll = display.nodes.iter().find(|n| n.test_id == "s").unwrap();
    let id = scroll.scroll.as_ref().unwrap().id.clone();
    let scrolled = display
        .retained
        .compose(&std::collections::HashMap::from([(id, 120.0)]))
        .unwrap();
    assert_eq!(bounds(&scrolled, "fab"), [170.0, 170.0, 20.0, 20.0]);
    let fab = scrolled.nodes.iter().find(|n| n.test_id == "fab").unwrap();
    assert!(fab.clips.is_empty() && fab.visible_bounds[2] == 20.0);
    let pixels = scrolled.rasterize().unwrap().pixels;
    let at = (180 * 200 + 180) * 4;
    assert_eq!(&pixels.data_as_u8_slice()[at..at + 4], &[0, 255, 0, 255]);
}

#[test]
fn aspect_ratio_derives_the_free_axis_and_ch_tracks_the_font() {
    let display = layout(
        "h('column',{style:{width:200}},h('column',{testId:'video',style:{width:160,aspectRatio:16/9}}),h('column',{testId:'square',style:{height:30,aspectRatio:1,alignSelf:'start'}}),h('text',{testId:'small',style:{maxWidth:'10ch',fontSize:10}},'0000000000000000000000'),h('text',{testId:'large',style:{maxWidth:'10ch',fontSize:20}},'0000000000000000000000'))",
    );
    assert_eq!(bounds(&display, "video")[3], 90.0);
    assert_eq!(bounds(&display, "square")[2], 30.0);
    let [small, large] = ["small", "large"].map(|id| bounds(&display, id)[2]);
    assert!(
        small > 20.0 && small <= 100.0 && (large / small - 2.0).abs() < 0.1,
        "{small} {large}"
    );
}

#[test]
fn current_color_backgrounds_follow_the_text_color() {
    let display = layout(
        "h('column',{style:{color:'#00ff00'}},h('column',{style:{width:10,height:10,backgroundColor:'currentColor'}}))",
    );
    let pixels = display.rasterize().unwrap().pixels;
    assert_eq!(&pixels.data_as_u8_slice()[..4], &[0, 255, 0, 255]);
}

#[test]
fn unstyled_defaults_follow_the_page_theme() {
    use crate::scene::paint::Paint;
    let source = crate::session::tests::source(
        "function render(){return h('column',{},h('text',{testId:'t'},'Hi'),h('button',{testId:'b',onClick(){}},'Go'))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let paints = |page: &mut crate::ViewSession, theme: &str| {
        page.set_environment(serde_json::json!({"theme":theme,"insets":{"top":0,"right":0,"bottom":0,"left":0},"keyboard":{"height":0},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"runner":false}))
            .unwrap();
        let display = page.layout(200, 200, 1.0).unwrap();
        let of = |id: &str| {
            let index = display.nodes.iter().position(|n| n.test_id == id).unwrap();
            display.retained.commands[display.retained.ranges[index].clone()].to_vec()
        };
        let rect = |paints: &[Paint]| {
            paints.iter().find_map(|p| match p {
                Paint::Rect { color, .. } => Some(*color),
                _ => None,
            })
        };
        let text = |paints: &[Paint]| {
            paints.iter().find_map(|p| match p {
                Paint::Text { color, .. } => Some(*color),
                _ => None,
            })
        };
        let root = display.retained.commands[display.retained.ranges[0].clone()].to_vec();
        (rect(&root), text(&of("t")), rect(&of("b")))
    };
    let themed =
        |token: &str, dark: bool| Some(crate::protocol::themed_color(token, dark).unwrap());
    for dark in [false, true] {
        let (root, text, button) = paints(&mut page, if dark { "dark" } else { "light" });
        assert_eq!(root, themed("background", dark), "root, dark={dark}");
        assert_eq!(text, themed("text", dark), "text, dark={dark}");
        assert_eq!(button, themed("primary", dark), "button, dark={dark}");
    }
    assert_ne!(themed("background", false), themed("background", true));
}

#[test]
fn a_transform_ends_with_its_node_and_never_reaches_the_next_sibling() {
    use crate::scene::paint::Paint;
    let source = crate::session::tests::source(
        "function render(){return h('column',{},h('column',{},h('row',{},h('column',{testId:'card',style:{width:48,height:48,backgroundColor:'#ff0000',rotate:12,scale:[0.9,0.9],transformOrigin:[0,1]}}))),h('column',{},h('text',{testId:'after'},'Text')))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    let mut current = vello_cpu::kurbo::Affine::IDENTITY;
    let mut transforms_at_text = Vec::new();
    for paint in &display.commands {
        match paint {
            Paint::Transform(matrix) => current = *matrix,
            Paint::Text { .. } => transforms_at_text.push(current),
            _ => {}
        }
    }
    assert!(!transforms_at_text.is_empty());
    assert!(
        transforms_at_text
            .iter()
            .all(|m| *m == vello_cpu::kurbo::Affine::IDENTITY),
        "{transforms_at_text:?}"
    );
}

#[test]
fn a_growing_aspect_box_in_a_start_aligned_row_derives_its_height() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{padding:16}},h('row',{style:{gap:12,alignItems:'flex-start'}},h('column',{testId:'video',style:{flexGrow:1,flexBasis:0,aspectRatio:16/9}}),h('column',{testId:'square',style:{width:64,aspectRatio:1}})))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(360, 600, 1.0).unwrap();
    let bounds = |id: &str| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .bounds
    };
    let video = bounds("video");
    assert_eq!(video[2], 360.0 - 32.0 - 12.0 - 64.0);
    assert!((video[3] - video[2] * 9.0 / 16.0).abs() < 0.5, "{video:?}");
    assert_eq!(bounds("square")[2..], [64.0, 64.0]);
}
