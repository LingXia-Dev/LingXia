use crate::scene::test_prelude::*;

#[test]
fn bar_paths_stay_inside_short_narrow_and_zero_bounds() {
    use vello_cpu::kurbo::Shape;
    for (width, height) in [
        (36.0, 30.0),
        (1.0, 30.0),
        (36.0, 0.5),
        (0.25, 0.125),
        (36.0, 0.0),
    ] {
        let path = plot_bar_path([10.0, 20.0, width, height]);
        assert_eq!(path.elements().len(), 8);
        let bounds = path.bounding_box();
        for (actual, expected) in [
            (bounds.x0, 10.0),
            (bounds.y0, 20.0),
            (bounds.x1, 10.0 + f64::from(width)),
            (bounds.y1, 20.0 + f64::from(height)),
        ] {
            assert!((actual - expected).abs() < 1e-9);
        }
        assert!(path.area().is_finite());
    }
}
#[test]
fn retained_scroll_combines_axes_and_reveals_through_nested_regions() {
    let module = crate::session::tests::source(
        "function render(){return h('scroll',{'data-testid':'outer',style:{width:180,height:140,backgroundColor:'#ffffff'}},h('spacer',{style:{height:30}}),h('scroll',{'data-testid':'inner',style:{overflowX:'auto',width:180,height:80,borderWidth:2,borderColor:'#000000',borderRadius:12}},h('row',{style:{width:400,height:60}},h('spacer',{style:{width:220}}),h('button',{'data-testid':'target',onClick:()=>{},style:{width:80,height:50,backgroundColor:'#0000ff',color:'#ffffff'}},'Target'))),h('spacer',{style:{height:300}}))}",
    );
    for scale in [1.0, 2.0] {
        let mut page = crate::ViewSession::new(&module).unwrap();
        let original = page
            .layout((180.0 * scale) as u16, (140.0 * scale) as u16, scale)
            .unwrap();
        let retained = original.retained.clone();
        let outer = retained
            .nodes
            .iter()
            .find(|n| n.test_id == "outer")
            .unwrap()
            .scroll
            .as_ref()
            .unwrap()
            .id
            .clone();
        let inner = retained
            .nodes
            .iter()
            .find(|n| n.test_id == "inner")
            .unwrap()
            .scroll
            .as_ref()
            .unwrap();
        assert!(matches!(inner.axis, ScrollAxis::Horizontal));
        assert!(inner.max_offset >= 220.0);
        let inner = inner.id.clone();
        let before = retained.compose(&HashMap::new()).unwrap();
        let target = before.nodes.iter().find(|n| n.test_id == "target").unwrap();
        let identity = target.identity.clone();
        let original_bounds = target.bounds;
        assert_eq!(target.visible_bounds[2], 0.0);
        let offsets = HashMap::from([(outer.clone(), 20.0), (inner.clone(), 180.0)]);
        let shifted = retained.compose(&offsets).unwrap();
        let target = shifted
            .nodes
            .iter()
            .find(|n| n.test_id == "target")
            .unwrap();
        assert_eq!(
            target.bounds,
            [
                original_bounds[0] - 180.0,
                original_bounds[1] - 20.0,
                original_bounds[2],
                original_bounds[3]
            ]
        );
        assert!(target.visible_bounds[2] > 0.0);
        let inner_node = shifted.nodes.iter().find(|n| n.test_id == "inner").unwrap();
        assert_eq!(inner_node.bounds[0], 0.0);
        assert_eq!(inner_node.bounds[1], 10.0);
        assert_eq!(
            target.clips.last().unwrap().bounds,
            inner_node.content_clip.unwrap().bounds
        );
        let pixels = shifted.rasterize().unwrap().pixels;
        let pixel = |x: usize, y: usize| {
            let index = ((y as f32 * scale) as usize * (180.0 * scale) as usize
                + (x as f32 * scale) as usize)
                * 4;
            pixels.data_as_u8_slice()[index..index + 4].to_vec()
        };
        assert_eq!(pixel(50, 20), [0, 0, 255, 255]);
        assert_eq!(pixel(20, 20), [255, 255, 255, 255]);
        let mut revealed_offsets = HashMap::new();
        let revealed = before.reveal(&identity, &mut revealed_offsets).unwrap();
        assert!(revealed_offsets[&inner] > 0.0);
        assert!(!revealed_offsets.contains_key(&outer));
        let target = revealed
            .nodes
            .iter()
            .find(|n| n.test_id == "target")
            .unwrap();
        assert!(target.visible_bounds[2] > 0.0);
        let restored = retained.compose(&HashMap::new()).unwrap();
        assert_eq!(
            restored
                .nodes
                .iter()
                .find(|n| n.test_id == "target")
                .unwrap()
                .bounds,
            original_bounds
        );
    }
}

#[test]
fn bordered_scroll_keeps_children_inside_the_inner_ellipse() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{backgroundColor:'#ffffff'}},h('scroll',{'data-testid':'scroll',style:{width:100,height:80,borderTopWidth:2,borderRightWidth:4,borderBottomWidth:6,borderLeftWidth:8,borderColor:'#ff0000',borderRadius:20,backgroundColor:'#ffffff'}},h('spacer',{'data-testid':'child',style:{height:200,backgroundColor:'#0000ff'}})))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let first = page.layout(120, 160, 1.0).unwrap();
    let scroll = first
        .nodes
        .iter()
        .find(|node| node.test_id == "scroll")
        .unwrap();
    let id = scroll.scroll.as_ref().unwrap().id.clone();
    assert_eq!(scroll.content_clip.unwrap().bounds, [8.0, 2.0, 88.0, 72.0]);
    let shifted = first
        .retained
        .compose(&HashMap::from([(id, 50.0)]))
        .unwrap();
    let child = shifted
        .nodes
        .iter()
        .find(|node| node.test_id == "child")
        .unwrap();
    assert_eq!(child.bounds[0], 8.0);
    assert_eq!(child.clips.last().unwrap().radii.unwrap()[0], [12.0, 18.0]);
    assert!(!child.clips.last().unwrap().contains([9.0, 3.0]));
    let frame = shifted.rasterize().unwrap();
    for (x, y, color) in [
        (5usize, 40usize, [255, 0, 0, 255]),
        (50, 77, [255, 0, 0, 255]),
        (50, 40, [0, 0, 255, 255]),
        (10, 5, [255, 0, 0, 255]),
    ] {
        let offset = (y * 120 + x) * 4;
        assert_eq!(
            &frame.pixels.data_as_u8_slice()[offset..offset + 4],
            color,
            "pixel {x},{y}"
        );
    }
}

#[test]
fn subtree_opacity_composites_overlap_once_and_keeps_sibling_groups_separate() {
    let module = crate::session::tests::source(
        "function render(){return h('stack',{style:{width:100,height:100,backgroundColor:'#ffffff'}},h('stack',{style:{width:80,height:80,opacity:0.5,backgroundColor:'#ff0000'}},h('spacer',{style:{width:60,height:60,backgroundColor:'#0000ff'}})),h('spacer',{style:{left:40,top:40,width:40,height:40,opacity:0.5,backgroundColor:'#00ff00'}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    let retained = display.retained.clone();
    let frame = display.rasterize().unwrap();
    let pixel = |x: usize, y: usize| {
        &frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
    };
    assert_eq!(
        pixel(10, 10),
        [127, 127, 255, 255],
        "blue covers red before group blending"
    );
    assert_eq!(pixel(70, 10), [255, 127, 127, 255]);
    let overlap = pixel(50, 50);
    assert!(
        overlap[0] >= 63
            && overlap[0] <= 65
            && overlap[1] >= 191
            && overlap[1] <= 193
            && overlap[2] >= 127
            && overlap[2] <= 129
    );
    let recomposed = retained
        .compose(&HashMap::new())
        .unwrap()
        .rasterize()
        .unwrap();
    assert_eq!(
        frame.pixels.data_as_u8_slice(),
        recomposed.pixels.data_as_u8_slice()
    );
}

#[test]
fn vector_paths_inherit_color_fit_clip_and_move_with_retained_scroll() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{backgroundColor:'#ffffff',color:'#ff0000'}},h('scroll',{style:{height:40}},h('vector',{'data-testid':'vector',viewBox:[0,0,24,24],paths:[{d:'M-12 -12H36V36H-12z'}],style:{width:80,height:80,padding:10,flexShrink:0}}),h('spacer',{style:{height:80,flexShrink:0}})))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    let scroll = display
        .nodes
        .iter()
        .find_map(|n| n.scroll.as_ref())
        .unwrap()
        .id
        .clone();
    let shifted = display
        .retained
        .compose(&HashMap::from([(scroll, 30.0)]))
        .unwrap();
    let frame = shifted.rasterize().unwrap();
    let pixel = |x: usize, y: usize| {
        &frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
    };
    assert_eq!(pixel(5, 10), [255, 255, 255, 255]);
    assert_eq!(pixel(15, 10), [255, 0, 0, 255]);
    assert_eq!(pixel(75, 10), [255, 255, 255, 255]);
    assert_eq!(pixel(15, 45), [255, 255, 255, 255]);
}

#[test]
fn vector_fill_and_butt_strokes_preserve_holes_and_device_scale() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{backgroundColor:'#ffffff'}},h('vector',{viewBox:[0,0,24,24],paths:[{d:'M2 2h20v20H2z',fill:'none',stroke:'#0000ff',strokeWidth:2}],style:{width:48,height:48}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    for scale in [1.0, 2.0] {
        let frame = page.layout(100, 100, scale).unwrap().rasterize().unwrap();
        let pixel = |x: usize, y: usize| {
            let x = (x as f32 * scale) as usize;
            let y = (y as f32 * scale) as usize;
            &frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
        };
        assert_eq!(pixel(4, 24), [0, 0, 255, 255]);
        assert_eq!(pixel(24, 24), [255, 255, 255, 255]);
        assert_eq!(pixel(0, 24), [255, 255, 255, 255]);
    }
}

#[test]
fn vector_transforms_cannot_expand_bounded_paths_past_coordinate_budget() {
    let source = crate::session::tests::source(
        "function render(){let child=h('vector',{viewBox:[0,0,.015625,.015625],paths:[{d:'M0 0H16000V16000H0z'}],style:{width:'100%',height:'100%'}});for(let i=0;i<3;i++)child=h('column',{style:{width:'16384%',height:'16384%'}},child);return h('column',{style:{width:300,height:300}},child)}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    assert!(
        page.layout(300, 300, 1.0)
            .err()
            .unwrap()
            .contains("vector geometry exceeds coordinate budget")
    );
}

#[test]
fn outer_shadows_knock_out_transparent_owners_and_preserve_layer_order() {
    let source = crate::session::tests::source(
        "function render(){return h('stack',{style:{width:150,height:100,backgroundColor:'#ffffff'}},h('column',{'data-testid':'owner',style:{width:80,height:40,left:20,top:20,boxShadow:[{offsetX:10,color:'#ff000080'},{offsetX:15,color:'#0000ff'}]}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(150, 100, 1.0).unwrap();
    let owner = display.nodes.iter().find(|n| n.test_id == "owner").unwrap();
    assert_eq!(owner.bounds, [20.0, 20.0, 80.0, 40.0]);
    let frame = display.rasterize().unwrap();
    let sample = |x: usize, y: usize| {
        &frame.pixels.data_as_u8_slice()[(y * 150 + x) * 4..(y * 150 + x) * 4 + 4]
    };
    assert_eq!(sample(50, 40), [255, 255, 255, 255]);
    assert_eq!(sample(105, 40), [128, 0, 127, 255]);
    assert_eq!(sample(113, 40), [0, 0, 255, 255]);
    assert_eq!(sample(116, 40), [255, 255, 255, 255]);
}

#[test]
fn equal_corner_overrides_keep_the_image_clip_and_uniform_fast_path() {
    let source = crate::session::tests::source(
        "function render(){return h('image',{src:'photo',style:{width:100,height:80,borderTopLeftRadius:8,borderTopRightRadius:8,borderBottomRightRadius:8,borderBottomLeftRadius:8}})}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(100, 80, 1.0).unwrap();
    assert_eq!(display.nodes[0].radius, 8.0);
    assert!(display.nodes[0].radii.is_none());
    assert!(
        display
            .commands
            .iter()
            .any(|paint| matches!(paint,Paint::Image {radius,..} if *radius==8.0))
    );
}
#[test]
fn authored_corner_styles_render_shadow_and_reuse_mask_without_external_images() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{backgroundColor:'#ffffff'}},h('column',{'data-testid':'bubble',style:{width:100,height:60,borderRadius:24,borderBottomRightRadius:6,backgroundColor:'#0000ff',boxShadow:[{blur:8,color:'#00000080'}]}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut first = page.layout(140, 100, 1.0).unwrap();
    assert_eq!(
        first.nodes[1].radii,
        Some([[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]])
    );
    assert_eq!(first.image_sources().count(), 0);
    first.bind_images(&HashMap::new()).unwrap();
    let masks: Vec<_> = first.bitmaps().map(|b| b.pixels().clone()).collect();
    assert_eq!(masks.len(), 1);
    let next = page.layout(140, 100, 1.0).unwrap();
    assert!(Arc::ptr_eq(
        &masks[0],
        next.bitmaps().next().unwrap().pixels()
    ));
    let frame = first.rasterize().unwrap();
    let pixels = frame.pixels.data_as_u8_slice();
    let pixel = |x: usize, y: usize| &pixels[(y * 140 + x) * 4..(y * 140 + x) * 4 + 4];
    assert_eq!(pixel(97, 57), &[0, 0, 255, 255]);
    assert!(
        pixel(101, 59)[0] < pixel(101, 1)[0],
        "small corner casts a larger nearby shadow"
    );
}
#[test]
fn independent_corner_snapshot_controls_editor_hits_and_scroll_clips() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{},h('scroll',{style:{width:100,height:60,borderRadius:24}},h('input',{value:'ABCD',style:{width:100,height:60,borderRadius:0,flexShrink:0}}),h('spacer',{style:{height:100,flexShrink:0}})))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(120, 100, 1.0).unwrap();
    let corners = Some([[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]]);
    let mut retained = display.retained.as_ref().clone();
    retained.nodes[1].radii = corners;
    let mut display = Arc::new(retained).compose(&HashMap::new()).unwrap();
    assert_eq!(display.nodes[2].clips[0].radii, corners);
    let identity = display.nodes[2].identity.clone();
    let hits = InputHitMap::capture(&display);
    assert!(hits.hit(&identity, [97.0, 2.0], false).is_none());
    assert!(hits.hit(&identity, [97.0, 57.0], false).is_some());
    display.nodes[2].clips.clear();
    display.nodes[2].radii = corners;
    let hits = InputHitMap::capture(&display);
    assert!(hits.hit(&identity, [97.0, 2.0], false).is_none());
    assert!(hits.hit(&identity, [97.0, 57.0], false).is_some());
}
#[test]
fn independent_corner_paints_survive_retained_scroll_and_keep_contours() {
    for paint_style in [
        "backgroundColor:'#0000ff'",
        "backgroundImage:{direction:'bottom',stops:[{offset:0,color:'#0000ff'},{offset:1,color:'#0000ff'}]}",
        "borderWidth:4,borderColor:'#0000ff'",
    ] {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('column',{{style:{{backgroundColor:'#ffffff'}}}},h('scroll',{{style:{{width:100,height:80}}}},h('column',{{style:{{width:100,height:60,flexShrink:0,borderRadius:24,{paint_style}}}}}),h('spacer',{{style:{{height:100,flexShrink:0}}}})))}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(120, 100, 1.0).unwrap();
        let mut retained = display.retained.as_ref().clone();
        let mut changed = 0;
        for paint in &mut retained.commands[retained.ranges[2].clone()] {
            match paint {
                Paint::Rect { corners, .. }
                | Paint::Gradient { corners, .. }
                | Paint::Border { corners, .. } => {
                    *corners = Some([[24.0; 2], [24.0; 2], [6.0; 2], [24.0; 2]]);
                    changed += 1;
                }
                _ => {}
            }
        }
        assert!(changed > 0);
        let id = retained.nodes[1].scroll.as_ref().unwrap().id.clone();
        let retained = Arc::new(retained);
        for offset in [0, 10] {
            let frame = retained
                .compose(&HashMap::from([(id.clone(), offset as f32)]))
                .unwrap()
                .rasterize()
                .unwrap();
            let pixels = frame.pixels.data_as_u8_slice();
            let pixel = |x: usize, y: usize| &pixels[(y * 120 + x) * 4..(y * 120 + x) * 4 + 4];
            assert_eq!(
                pixel(97, 57 - offset),
                &[0, 0, 255, 255],
                "small bottom-right corner: {paint_style}"
            );
            if offset == 0 {
                assert_eq!(
                    pixel(97, 2),
                    &[255, 255, 255, 255],
                    "large top-right corner"
                );
            }
        }
    }
}
#[test]
fn blurred_shadows_fade_outside_input_clips() {
    for tag in ["column", "input"] {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('stack',{{style:{{width:150,height:100,backgroundColor:'#ffffff'}}}},h('{tag}',{{style:{{width:80,height:40,borderRadius:0,left:20,top:20,boxShadow:[{{blur:10,color:'#000000'}}]}}}}))}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(150, 100, 1.0).unwrap().rasterize().unwrap();
        let sample = |x: usize| frame.pixels.data_as_u8_slice()[(40 * 150 + x) * 4];
        assert!(
            sample(101) > 120 && sample(101) < 180,
            "{tag}: {}",
            sample(101)
        );
        assert!(
            sample(101) < sample(105) && sample(105) < sample(110),
            "{tag}"
        );
        assert_eq!(sample(114), 255);
    }
}

#[test]
fn offscreen_owner_shadow_remains_visible_without_increasing_scroll_extent() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{width:100,height:100,backgroundColor:'#ffffff'}},h('scroll',{'data-testid':'scroll',style:{height:60}},h('column',{'data-testid':'owner',style:{width:40,height:20,flexShrink:0,boxShadow:[{offsetY:40,color:'#ff0000'}]}}),h('spacer',{style:{height:100,flexShrink:0}})))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    let scroll = display
        .nodes
        .iter()
        .find_map(|n| n.scroll.as_ref())
        .unwrap();
    assert_eq!(scroll.max_offset, 60.0);
    let shifted = display
        .retained
        .compose(&HashMap::from([(scroll.id.clone(), 30.0)]))
        .unwrap();
    assert_eq!(
        shifted
            .nodes
            .iter()
            .find(|n| n.test_id == "owner")
            .unwrap()
            .visible_bounds[3],
        0.0
    );
    let frame = shifted.rasterize().unwrap();
    assert_eq!(
        &frame.pixels.data_as_u8_slice()[(15 * 100 + 10) * 4..(15 * 100 + 10) * 4 + 4],
        [255, 0, 0, 255]
    );
    assert_eq!(
        &frame.pixels.data_as_u8_slice()[(65 * 100 + 10) * 4..(65 * 100 + 10) * 4 + 4],
        [255, 255, 255, 255]
    );
}

#[test]
fn linear_gradient_preserves_alpha_radius_borders_and_scroll_coordinates() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{width:100,height:100,backgroundColor:'#ffffff'}},h('scroll',{'data-testid':'scroll',style:{width:80,height:40}},h('column',{style:{width:80,height:80,flexShrink:0,borderRadius:12,borderWidth:4,borderColor:'#00ff00',backgroundImage:{direction:'bottom',stops:[{offset:0,color:'#ff000080'},{offset:1,color:'#0000ff80'}]}}})),h('spacer',{style:{height:20}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    let scroll = display
        .nodes
        .iter()
        .find_map(|n| n.scroll.as_ref())
        .unwrap()
        .id
        .clone();
    let shifted = display
        .retained
        .compose(&HashMap::from([(scroll, 20.0)]))
        .unwrap();
    let before = display.rasterize().unwrap();
    let after = shifted.rasterize().unwrap();
    let sample = |frame: &Frame, x: usize, y: usize| {
        frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4].to_vec()
    };
    assert_eq!(sample(&before, 0, 0), [255, 255, 255, 255]);
    assert_eq!(sample(&before, 40, 1), [0, 255, 0, 255]);
    assert_eq!(sample(&before, 40, 45), [255, 255, 255, 255]);
    assert_eq!(sample(&after, 40, 45), [255, 255, 255, 255]);
    let original = sample(&before, 40, 30);
    assert_eq!(original, sample(&after, 40, 10));
    assert!(original[0] > original[2] && original[1] >= 126 && original[1] <= 128);
}

#[test]
fn gradient_interpolation_space_and_premultiplied_alpha_affect_pixels() {
    let sample = |space: &str, first: &str, last: &str| {
        let render = format!(
            "function render(){{return h('column',{{style:{{width:101,height:20,backgroundColor:'#000000',backgroundImage:{{direction:'right',colorSpace:'{space}',stops:[{{offset:0,color:'{first}'}},{{offset:1,color:'{last}'}}]}}}}}})}}"
        );
        let mut page = crate::ViewSession::new(&crate::session::tests::source(&render)).unwrap();
        let frame = page.layout(101, 20, 1.0).unwrap().rasterize().unwrap();
        frame.pixels.data_as_u8_slice()[(10 * 101 + 50) * 4..(10 * 101 + 50) * 4 + 4].to_vec()
    };
    let srgb = sample("srgb", "#ff0000", "#0000ff");
    let oklab = sample("oklab", "#ff0000", "#0000ff");
    assert!((srgb[0] as i32 - 128).abs() <= 2 && (srgb[2] as i32 - 128).abs() <= 2);
    assert!(oklab[1] > 20, "{oklab:?}");
    let alpha = sample("srgb", "#ff000000", "#0000ffff");
    assert_eq!(alpha[0], 0, "{alpha:?}");
    assert!((alpha[2] as i32 - 128).abs() <= 2, "{alpha:?}");
}

#[test]
fn nested_opacity_and_scroll_clipping_preserve_group_boundaries() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{width:100,height:100,backgroundColor:'#ffffff'}},h('scroll',{'data-testid':'scroll',style:{width:80,height:40,opacity:0.5}},h('column',{style:{opacity:0.5}},h('spacer',{style:{width:80,height:80,backgroundColor:'#000000'}}))),h('spacer',{style:{width:100,height:20,backgroundColor:'#ff0000'}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    let scroll = display
        .nodes
        .iter()
        .find_map(|node| node.scroll.as_ref())
        .unwrap()
        .id
        .clone();
    let shifted = display
        .retained
        .compose(&HashMap::from([(scroll, 20.0)]))
        .unwrap();
    for frame in [display.rasterize().unwrap(), shifted.rasterize().unwrap()] {
        let bytes = frame.pixels.data_as_u8_slice();
        assert_eq!(
            &bytes[(10 * 100 + 10) * 4..(10 * 100 + 10) * 4 + 4],
            [191, 191, 191, 255]
        );
        assert_eq!(
            &bytes[(50 * 100 + 10) * 4..(50 * 100 + 10) * 4 + 4],
            [255, 0, 0, 255]
        );
        assert_eq!(
            &bytes[(70 * 100 + 10) * 4..(70 * 100 + 10) * 4 + 4],
            [255, 255, 255, 255]
        );
    }
}

#[cfg(feature = "gpu")]
#[test]
fn scene_key_compares_paint_content_and_not_semantic_metadata() {
    let module = crate::session::tests::source(
        "function render(){return h('scroll',{style:{height:60,borderRadius:12,backgroundColor:'#ffffff'}},h('text',{},'Hello world'),h('spacer',{style:{height:200}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let mut display = page.layout(240, 100, 1.0).unwrap();
    let key = SceneKey::capture(&display).unwrap();
    display.nodes[0].test_id = "new-semantics".into();
    assert!(key.matches(&display));
    for index in 0..display.commands.len() {
        let original = display.commands[index].clone();
        let changed = match &mut display.commands[index] {
            Paint::Rect { bounds, .. } => {
                bounds[0] += 1.0;
                true
            }
            Paint::Clip(clip) => {
                clip.radius += 1.0;
                true
            }
            Paint::Text { glyphs, .. } if !glyphs.is_empty() => {
                glyphs[0].x += 1.0;
                true
            }
            _ => false,
        };
        if changed {
            assert!(!key.matches(&display));
        }
        display.commands[index] = original;
    }
    let text = display
        .commands
        .iter()
        .position(|p| matches!(p, Paint::Text { .. }))
        .unwrap();
    let original = display.commands[text].clone();
    for change in 0..4 {
        if let Paint::Text {
            color,
            font,
            size,
            coordinates,
            ..
        } = &mut display.commands[text]
        {
            match change {
                0 => color[0] ^= 1,
                1 => font.index += 1,
                2 => *size += 1.0,
                _ => coordinates.push(1),
            }
        }
        assert!(!key.matches(&display));
        display.commands[text] = original.clone();
    }
    display.scale = 2.0;
    assert!(!key.matches(&display));
    display.scale = 1.0;
    display.width += 1;
    assert!(!key.matches(&display));
}

#[cfg(feature = "gpu")]
#[test]
fn scene_key_tracks_immutable_image_identity_and_input_bounds() {
    use ::image::ImageEncoder;
    let mut png = Vec::new();
    ::image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&[255, 0, 0, 255], 1, 1, ::image::ExtendedColorType::Rgba8)
        .unwrap();
    let bitmap = crate::scene::image::Bitmap::decode(&png).unwrap();
    let module = crate::session::tests::source(
        "function render(){return h('image',{src:'public/a.png',style:{width:20,height:20}})}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let mut display = page.layout(100, 100, 1.0).unwrap();
    display.images.insert("public/a.png".into(), bitmap.clone());
    let key = SceneKey::capture(&display).unwrap();
    display.images.insert("public/a.png".into(), bitmap);
    assert!(key.matches(&display));
    display.images.insert(
        "public/a.png".into(),
        crate::scene::image::Bitmap::decode(&png).unwrap(),
    );
    assert!(
        !key.matches(&display),
        "new pixels under the same source invalidate the scene"
    );
    display.images.clear();
    assert!(!key.matches(&display));
    let command = display.commands[0].clone();
    display.commands.resize(257, command);
    assert!(SceneKey::capture(&display).is_none());
}

#[test]
fn rounded_scroll_masks_are_shared_and_stay_fixed_during_scroll() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{padding:10,backgroundColor:'#ffffff'}},h('scroll',{style:{height:80,borderRadius:24}},h('column',{style:{height:40,backgroundColor:'#ff0000'}}),h('column',{style:{height:120,backgroundColor:'#0000ff'}})))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(
        display
            .commands
            .iter()
            .filter(|p| matches!(p, Paint::Clip(_)))
            .count(),
        1
    );
    assert_eq!(display.nodes[2].clips, display.nodes[3].clips);
    assert_eq!(display.nodes[2].clips[0].bounds, [10., 10., 80., 80.]);
    let id = display.nodes[1].scroll.as_ref().unwrap().id.clone();
    let retained = display.retained.clone();
    for offset in [0., 20., 80.] {
        let frame = retained
            .compose(&HashMap::from([(id.clone(), offset)]))
            .unwrap();
        assert_eq!(frame.nodes[3].clips[0].bounds, [10., 10., 80., 80.]);
        let image = frame.rasterize().unwrap().pixels;
        for (x, y) in [(10, 10), (89, 10), (10, 89), (89, 89)] {
            assert_eq!(
                &image.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 3],
                &[255, 255, 255]
            );
        }
        assert_eq!(
            &image.data_as_u8_slice()[(80 * 100 + 50) * 4..(80 * 100 + 50) * 4 + 3],
            &[0, 0, 255]
        );
    }
}
#[test]
fn nested_rounding_uses_original_bounds_and_hides_fully_masked_nodes() {
    let source = crate::session::tests::source(
        "function render(){return h('scroll',{style:{backgroundColor:'#ffffff'}},h('scroll',{style:{height:80,borderRadius:30}},h('column',{style:{height:200,backgroundColor:'#0000ff'}})),h('spacer',{style:{height:100}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    let id = display.nodes[0].scroll.as_ref().unwrap().id.clone();
    let frame = display
        .retained
        .compose(&HashMap::from([(id, 20.)]))
        .unwrap();
    assert_eq!(frame.nodes[2].clips[0].bounds, [0., -20., 100., 80.]);
    let pixels = frame.rasterize().unwrap().pixels;
    assert_eq!(
        &pixels.data_as_u8_slice()[(100 + 10) * 4..(100 + 10) * 4 + 3],
        &[0, 0, 255]
    );
    let mut page = crate::ViewSession::new(&crate::session::tests::source("function render(){return h('scroll',{style:{borderRadius:40}},h('image',{src:'hidden',style:{width:1,height:1}}))}")).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(display.image_sources().count(), 0);
    assert_eq!(display.nodes[1].visible_bounds[2], 0.0);
}

#[test]
fn nested_scroll_clips_intersect_without_expanding_outer_content() {
    let records = vec![
        node(5, None, "outer", &[]),
        node(4, Some(0), "space", &[(1, json!(80))]),
        node(5, Some(0), "inner", &[(1, json!(50))]),
        node(3, Some(2), "item", &[(1, json!(200))]),
    ];
    let frame = Renderer::default().layout(&records, 100, 100, 1.0).unwrap();
    let inner = &frame.nodes[2];
    assert_eq!(inner.scroll.as_ref().unwrap().max_offset, 150.0);
    let visible = frame.nodes[3].visible_bounds;
    assert!(visible[1] >= inner.visible_bounds[1]);
    assert!(visible[1] + visible[3] <= 100.0);
    assert_eq!(frame.nodes[0].scroll.as_ref().unwrap().max_offset, 30.0);
}

#[test]
fn retained_scroll_reuses_shaping_and_culls_only_invisible_paint() {
    let mut records = vec![node(5, None, "feed", &[])];
    for index in 0..20 {
        let mut item = node(
            3,
            Some(0),
            &format!("item-{index}"),
            &[(1, json!(50)), (8, json!(index + 1))],
        );
        item.text = format!("Item {index}");
        records.push(item);
    }
    let mut renderer = Renderer::default();
    let first = renderer.layout(&records, 200, 100, 1.0).unwrap();
    let retained = Arc::clone(&first.retained);
    let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
    assert_eq!(first.nodes[1].scroll_parent.as_deref(), Some(id.as_str()));
    let offsets = HashMap::from([(id, 425.0)]);
    let scrolled = retained.compose(&offsets).unwrap();
    assert!(Arc::ptr_eq(&first.retained, &scrolled.retained));
    assert_eq!(first.nodes[1].bounds[1], 0.0);
    assert_eq!(scrolled.nodes[1].bounds[1], -425.0);
    assert_eq!(
        scrolled.nodes[1].scroll_parent,
        first.nodes[1].scroll_parent
    );
    assert!(
        scrolled
            .commands
            .iter()
            .filter(|command| matches!(command, Paint::Text { .. }))
            .count()
            <= 3
    );
    assert!(scrolled.commands.len() < retained.commands.len());
    let expected = renderer
        .layout_scrolled(&records, 200, 100, 1.0, &offsets)
        .unwrap();
    assert_eq!(
        serde_json::to_value(&scrolled.nodes).unwrap(),
        serde_json::to_value(&expected.nodes).unwrap()
    );
    assert_eq!(
        scrolled.rasterize().unwrap().pixels.data(),
        expected.rasterize().unwrap().pixels.data()
    );
    assert!(
        retained
            .compose(&HashMap::from([("bad".into(), f32::INFINITY)]))
            .is_err()
    );
}

#[test]
fn boolean_controls_present_checked_state_and_distinct_pixels_at_each_scale() {
    for scale in [1.0, 2.75] {
        for tag in [10, 11] {
            let mut control = node(
                tag,
                Some(0),
                "preference",
                &[(39, json!(0)), (40, json!(42))],
            );
            control.text = "Preference".into();
            let mut records = vec![node(0, None, "root", &[]), control];
            let mut renderer = Renderer::default();
            let width = (240.0 * scale) as u16;
            let height = (100.0 * scale) as u16;
            let off = renderer.layout(&records, width, height, scale).unwrap();
            assert_eq!(off.nodes[1].checked, Some(false));
            assert_eq!(off.nodes[1].on_click, Some(42));
            assert!(off.nodes[1].bounds[3] >= 44.0);
            records[1].fields[0].1 = json!(1);
            let on = renderer.layout(&records, width, height, scale).unwrap();
            assert_eq!(on.nodes[1].checked, Some(true));
            assert_eq!(off.nodes[1].bounds, on.nodes[1].bounds);
            let on_pixels = on.rasterize().unwrap().pixels;
            assert_ne!(off.rasterize().unwrap().pixels.data(), on_pixels.data());
            records[1].fields.push((9, json!(0)));
            let disabled = renderer.layout(&records, width, height, scale).unwrap();
            assert_eq!(disabled.nodes[1].checked, Some(true));
            assert_eq!(disabled.nodes[1].on_click, None);
            assert_ne!(
                on_pixels.data(),
                disabled.rasterize().unwrap().pixels.data()
            );
        }
    }
}

#[test]
fn clipped_text_shows_its_gradient_only_through_its_glyphs() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{width:200,height:80,backgroundColor:'#ffffff'}},h('text',{style:{fontSize:64,fontWeight:700,color:'#00000000',backgroundClip:'text',backgroundImage:{direction:'right',colorSpace:'srgb',stops:[{offset:0,color:'#ff0000'},{offset:1,color:'#0000ff'}]}}},'WWW'))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(200, 80, 1.0).unwrap();
    let text = display
        .nodes
        .iter()
        .find(|node| node.tag == "text")
        .unwrap()
        .bounds;
    assert!(display.cost().layers >= 2, "a mask and a blend layer");
    let retained = display.retained.clone();
    let frame = display.rasterize().unwrap();
    let data = frame.pixels.data_as_u8_slice();
    let (mut red, mut blue, mut white, mut inside) = (0, 0, 0, 0);
    for y in text[1] as usize..(text[1] + text[3]) as usize {
        for x in text[0] as usize..(text[0] + text[2]).min(200.0) as usize {
            let [r, g, b, _] = data[(y * 200 + x) * 4..(y * 200 + x) * 4 + 4] else {
                unreachable!()
            };
            inside += 1;
            if g > 240 && r > 240 && b > 240 {
                white += 1;
            } else if g < 40 && r > 200 && b < 80 && x < (text[0] + text[2] / 3.0) as usize {
                red += 1;
            } else if g < 40 && b > 200 && r < 80 && x > (text[0] + text[2] * 2.0 / 3.0) as usize {
                blue += 1;
            }
        }
    }
    assert!(red > 50 && blue > 50, "red {red} blue {blue}");
    assert!(
        white > inside / 4,
        "the box around the glyphs stays clear: {white}/{inside}"
    );
    let recomposed = retained
        .compose(&HashMap::new())
        .unwrap()
        .rasterize()
        .unwrap();
    assert_eq!(data, recomposed.pixels.data_as_u8_slice());
}

/// A 100px red box faded by `fade`, with a 20px blue child inside it, over white.
fn masked(fade: &str, backdrop: bool) -> crate::ViewSession {
    let module = crate::session::tests::source(&format!(
        "function render(){{return h('stack',{{style:{{width:100,height:100,backgroundColor:'#ffffff'}}}},h('stack',{{className:'fade',style:{{width:100,height:100,backgroundColor:'#ff0000'}}}},h('spacer',{{style:{{width:20,height:100,backgroundColor:'#0000ff'}}}})){})}}",
        if backdrop {
            ",h('spacer',{style:{left:80,top:0,width:20,height:20,backdropFilter:'blur(2px)'}})"
        } else {
            ""
        }
    ))
    .replace(
        "__lingxiaPublish(render);",
        &format!("configureClasses([{{name:'fade',when:{{}},style:{{maskImage:{fade}}}}}]);__lingxiaPublish(render);"),
    );
    crate::ViewSession::new(&module).unwrap()
}

#[test]
fn a_mask_fades_its_subtree_and_composites_the_same() {
    let fade = "[{to:'bottom',stops:[{offset:0,alpha:1},{px:100,alpha:0}]}]";
    let mut page = masked(fade, false);
    let display = page.layout(100, 100, 1.0).unwrap();
    assert!(display.cost().layers >= 2, "the group and one fade");
    let frame = display.rasterize().unwrap();
    let pixel = |frame: &crate::Frame, x: usize, y: usize| {
        let at = (y * 100 + x) * 4;
        frame.pixels.data_as_u8_slice()[at..at + 4].to_vec()
    };
    let top = pixel(&frame, 50, 0);
    assert!(top[0] == 255 && top[1] <= 4, "opaque at the start: {top:?}");
    assert!(
        pixel(&frame, 50, 99)[1] > 240,
        "faded to the white page at the end"
    );
    let mid = pixel(&frame, 50, 50);
    assert!(
        mid[0] > 240 && (100..160).contains(&mid[1]),
        "half way: {mid:?}"
    );
    // The child fades with it.
    let child = pixel(&frame, 10, 50);
    assert!(
        (100..160).contains(&child[0]) && child[2] > 240,
        "{child:?}"
    );
    // A backdrop in the frame takes the composited path; the mask draws the same.
    let mut composited = masked(fade, true);
    let display = composited.layout(100, 100, 1.0).unwrap();
    assert!(display.requires_composition());
    let other = display.rasterize().unwrap();
    for (x, y) in [(50, 0), (50, 50), (50, 99), (10, 50)] {
        let (a, b) = (pixel(&frame, x, y), pixel(&other, x, y));
        assert!(
            a.iter().zip(&b).all(|(a, b)| a.abs_diff(*b) <= 2),
            "({x},{y}): {a:?} vs {b:?}"
        );
    }
}

#[test]
fn a_blurred_subtree_spreads_past_its_box_and_composites_the_same() {
    let page = |backdrop: bool| {
        crate::session::tests::source(&format!(
            "function render(){{return h('stack',{{style:{{width:60,height:60,backgroundColor:'#ffffff'}}}},h('spacer',{{style:{{left:20,top:20,width:20,height:20,backgroundColor:'#ff0000',filter:'blur(4px)'}}}}){})}}",
            if backdrop {
                ",h('spacer',{style:{left:0,top:0,width:10,height:10,backdropFilter:'blur(2px)'}})"
            } else {
                ""
            }
        ))
    };
    let raster = |backdrop: bool| {
        let mut session = crate::ViewSession::new(&page(backdrop)).unwrap();
        let display = session.layout(60, 60, 1.0).unwrap();
        assert_eq!(display.requires_composition(), backdrop);
        display.rasterize().unwrap()
    };
    let frame = raster(false);
    let pixel = |frame: &crate::Frame, x: usize, y: usize| {
        let at = (y * 60 + x) * 4;
        frame.pixels.data_as_u8_slice()[at..at + 4].to_vec()
    };
    let halo = pixel(&frame, 17, 30);
    assert!(
        halo[0] > 240 && halo[1] < 240,
        "a halo outside the box: {halo:?}"
    );
    let edge = pixel(&frame, 20, 30);
    assert!(edge[1] > 60, "the edge is soft: {edge:?}");
    assert!(pixel(&frame, 30, 30)[1] < 40, "the middle stays red");
    assert!(pixel(&frame, 2, 30)[1] > 250, "far away is untouched");
    let other = raster(true);
    for (x, y) in [(17, 30), (20, 30), (30, 30)] {
        let (a, b) = (pixel(&frame, x, y), pixel(&other, x, y));
        assert!(
            a.iter().zip(&b).all(|(a, b)| a.abs_diff(*b) <= 3),
            "({x},{y}): {a:?} vs {b:?}"
        );
    }
}
