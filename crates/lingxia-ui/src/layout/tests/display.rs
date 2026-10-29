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
        "h('row',{},h('button',{'data-testid':'a',style:{width:40,display:'none',backgroundColor:'#ff0000'},onClick(){}},'Gone'),h('column',{'data-testid':'b',style:{width:30,height:10}}))",
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
        "h('row',{style:{width:200,alignItems:'center'}},h('column',{'data-testid':'block',style:{display:'block',width:100}},h('column',{'data-testid':'child',style:{height:10}}),h('column',{'data-testid':'next',style:{height:10}})),h('row',{'data-testid':'inline',style:{display:'inline-flex',height:30}},h('column',{style:{width:20,height:5}})))",
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
        "function render(){return h('page',{},h('scroll',{'data-testid':'s',style:{height:100,marginTop:50}},h('column',{style:{height:400,overflow:'hidden',marginLeft:30}},h('column',{'data-testid':'fab',style:{position:'fixed',bottom:10,right:10,width:20,height:20,backgroundColor:'#00ff00'}}))))}",
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
        "h('column',{style:{width:200}},h('column',{'data-testid':'video',style:{width:160,aspectRatio:16/9}}),h('column',{'data-testid':'square',style:{height:30,aspectRatio:1,alignSelf:'start'}}),h('text',{'data-testid':'small',style:{maxWidth:'10ch',fontSize:10}},'0000000000000000000000'),h('text',{'data-testid':'large',style:{maxWidth:'10ch',fontSize:20}},'0000000000000000000000'))",
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
        "function render(){return h('column',{},h('text',{'data-testid':'t'},'Hi'),h('button',{'data-testid':'b',onClick(){}},'Go'))}",
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
        |role: &str, dark: bool| Some(crate::theme::Palette::builtin(dark).resolve(role).unwrap());
    for dark in [false, true] {
        let (root, text, button) = paints(&mut page, if dark { "dark" } else { "light" });
        assert_eq!(root, themed("canvas", dark), "root, dark={dark}");
        assert_eq!(text, themed("text", dark), "text, dark={dark}");
        assert_eq!(button, themed("accent", dark), "button, dark={dark}");
    }
    assert_ne!(themed("canvas", false), themed("canvas", true));
}

#[test]
fn a_transform_ends_with_its_node_and_never_reaches_the_next_sibling() {
    use crate::scene::paint::Paint;
    let source = crate::session::tests::source(
        "function render(){return h('column',{},h('column',{},h('row',{},h('column',{'data-testid':'card',style:{width:48,height:48,backgroundColor:'#ff0000',rotate:12,scale:[0.9,0.9],transformOrigin:[0,1]}}))),h('column',{},h('text',{'data-testid':'after'},'Text')))}",
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
        "function render(){return h('column',{style:{padding:16}},h('row',{style:{gap:12,alignItems:'flex-start'}},h('column',{'data-testid':'video',style:{flexGrow:1,flexBasis:0,aspectRatio:16/9}}),h('column',{'data-testid':'square',style:{width:64,aspectRatio:1}})))}",
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

#[test]
fn fixed_boxes_measure_insets_from_the_safe_content_box() {
    let source = crate::session::tests::source(
        "function render(){return h('page',{},h('scroll',{},h('column',{'data-testid':'fab',style:{position:'fixed',bottom:16,right:16,width:40,height:20}}),h('column',{'data-testid':'pin',style:{position:'fixed',top:8,left:8,width:10,height:10}})))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut place = |size: [u16; 2], insets: [f32; 4], keyboard: f32| {
        let [top, right, bottom, left] = insets;
        page.set_environment(serde_json::json!({"theme":"light","insets":{"top":top,"right":right,"bottom":bottom,"left":left},"keyboard":{"height":keyboard},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"runner":false}))
            .unwrap();
        let display = page.layout(size[0], size[1], 1.0).unwrap();
        let at = |id: &str| {
            display
                .nodes
                .iter()
                .find(|n| n.test_id == id)
                .unwrap()
                .bounds
        };
        (at("fab"), at("pin"))
    };
    // Portrait: status bar on top, tab and navigation bars below.
    let (fab, pin) = place([360, 640], [24.0, 0.0, 80.0, 0.0], 0.0);
    assert_eq!(
        fab,
        [360.0 - 16.0 - 40.0, 640.0 - 80.0 - 16.0 - 20.0, 40.0, 20.0]
    );
    assert_eq!(pin[..2], [8.0, 24.0 + 8.0]);
    // Landscape: the navigation bar moves to a side.
    let (fab, pin) = place([640, 360], [24.0, 48.0, 56.0, 0.0], 0.0);
    assert_eq!(
        fab[..2],
        [640.0 - 48.0 - 16.0 - 40.0, 360.0 - 56.0 - 16.0 - 20.0]
    );
    assert_eq!(pin[..2], [8.0, 32.0]);
    // An open keyboard taller than the bottom inset lifts the box above it.
    let (fab, _) = place([360, 640], [24.0, 0.0, 80.0, 0.0], 300.0);
    assert_eq!(fab[1], 640.0 - 300.0 - 16.0 - 20.0);
}

#[test]
fn viewport_units_measure_the_safe_content_box() {
    let source = crate::session::tests::source(
        "function render(){return h('page',{},h('column',{'data-testid':'half',style:{height:'50vh',width:'50vw'}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    for (keyboard, height) in [
        (0.0, (640.0 - 24.0 - 80.0) / 2.0),
        (300.0, (640.0 - 24.0 - 300.0) / 2.0),
    ] {
        page.set_environment(serde_json::json!({"theme":"light","insets":{"top":24,"right":20,"bottom":80,"left":20},"keyboard":{"height":keyboard},"os":"android","formFactor":"phone","displayLanguage":"en","canDock":false,"runner":false}))
            .unwrap();
        let display = page.layout(360, 640, 1.0).unwrap();
        let half = display
            .nodes
            .iter()
            .find(|n| n.test_id == "half")
            .unwrap()
            .bounds;
        assert_eq!(
            half[2..],
            [(360.0 - 40.0) / 2.0, height],
            "keyboard {keyboard}"
        );
    }
}

#[test]
fn a_textarea_with_rows_grows_with_its_lines_up_to_max_rows() {
    let height = |lines: usize| {
        let value = vec!["line"; lines].join("\\n");
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('column',{{}},h('textarea',{{'data-testid':'t','aria-label':'Note',value:'{value}',rows:2,maxRows:4,style:{{padding:0,lineHeight:'20px'}}}}))}}"
        ));
        let display = crate::ViewSession::new(&source)
            .unwrap()
            .layout(300, 600, 1.0)
            .unwrap();
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "t")
            .unwrap()
            .bounds[3]
    };
    assert_eq!(height(1), 40.0);
    assert_eq!(height(3), 60.0);
    assert_eq!(height(9), 80.0);
}

#[test]
fn an_image_placeholder_paints_first_and_loads_alongside() {
    use crate::scene::paint::Paint;
    let source = crate::session::tests::source(
        "function render(){return h('column',{},h('image',{src:'photo.jpg',placeholder:'photo-tiny.jpg',alt:'Lake',style:{width:100,height:60}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(200, 200, 1.0).unwrap();
    let images: Vec<&str> = display
        .commands
        .iter()
        .filter_map(|paint| match paint {
            Paint::Image { src, .. } => Some(src.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(images, ["photo-tiny.jpg", "photo.jpg"]);
    let requested: Vec<String> = page
        .image_requests()
        .into_iter()
        .map(|r| r.source)
        .collect();
    assert_eq!(requested, ["photo-tiny.jpg", "photo.jpg"]);
}

#[test]
fn an_unstyled_buttons_text_pairs_with_its_fill() {
    use crate::scene::paint::Paint;
    let source = crate::session::tests::source(
        "function render(){return h('column',{},h('button',{'data-testid':'plain',onClick(){}},'A'),h('button',{'data-testid':'card',onClick(){},style:{backgroundColor:'surface'}},'B'),h('button',{'data-testid':'risk',onClick(){},style:{backgroundColor:'danger'}},'C'))}",
    );
    let display = crate::ViewSession::new(&source)
        .unwrap()
        .layout(200, 200, 1.0)
        .unwrap();
    let ink = |id: &str| {
        let index = display.nodes.iter().position(|n| n.test_id == id).unwrap();
        display.retained.commands[display.retained.ranges[index].clone()]
            .iter()
            .find_map(|p| match p {
                Paint::Text { color, .. } => Some(*color),
                _ => None,
            })
            .unwrap()
    };
    let palette = crate::theme::Palette::builtin(false);
    use lingxia_theme::Role;
    assert_eq!(ink("plain"), palette.role(Role::OnAccent));
    assert_eq!(ink("card"), palette.role(Role::Text));
    assert_eq!(ink("risk"), palette.role(Role::OnDanger));
}
