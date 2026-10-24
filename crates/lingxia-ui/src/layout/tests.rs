use crate::scene::test_prelude::*;

#[test]
fn flex_basis_is_independent_of_growth_in_both_axes() {
    for (axis, size, index) in [("row", "width", 2), ("column", "height", 3)] {
        for (basis, expected) in [
            ("", [125.0, 175.0]),
            (",flexBasis:'auto'", [125.0, 175.0]),
            (",flexBasis:0", [150.0, 150.0]),
            (",flexBasis:'25%'", [150.0, 150.0]),
        ] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('{axis}',{{style:{{width:300,height:300}}}},h('column',{{testId:'a',style:{{{size}:50,grow:1{basis}}}}}),h('column',{{testId:'b',style:{{{size}:100,grow:1{basis}}}}}))}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            for scale in [1.0, 2.0] {
                let display = page.layout(600, 600, scale).unwrap();
                for (id, wanted) in ["a", "b"].into_iter().zip(expected) {
                    let actual = display
                        .nodes
                        .iter()
                        .find(|node| node.test_id == id)
                        .unwrap()
                        .bounds[index];
                    assert!(
                        (actual - wanted).abs() < 0.01,
                        "{axis} {basis} scale={scale}: {actual} vs {wanted}"
                    );
                }
            }
        }
    }
}

#[test]
fn percentage_basis_recomputes_after_parent_resize() {
    let source = crate::session::tests::source(
        "function render(){const wide=useLocal('wide',false);return h('column',{},h('row',{style:{width:wide.value?240:160}},h('column',{testId:'half',style:{flexBasis:'50%',height:20,shrink:0}})),h('button',{testId:'resize',onClick:()=>wide.set(!wide.value)},'Resize'))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    assert_eq!(
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "half")
            .unwrap()
            .bounds[2],
        80.0
    );
    let button = display
        .nodes
        .iter()
        .find(|n| n.test_id == "resize")
        .unwrap();
    page.click(page.generation(), button.on_click.unwrap())
        .unwrap();
    let display = page.layout(300, 300, 1.0).unwrap();
    assert_eq!(
        display
            .nodes
            .iter()
            .find(|n| n.test_id == "half")
            .unwrap()
            .bounds[2],
        120.0
    );
}
#[test]
fn flex_wrap_reflows_items_with_gaps_and_reverse_order() {
    for mode in ["nowrap", "wrap", "wrap-reverse"] {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('column',{{}},h('row',{{testId:'wrap',style:{{flexWrap:'{mode}',gap:10}}}},[0,1,2].map(i=>h('column',{{key:String(i),testId:'item'+i,style:{{width:70,height:20}}}}))))}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(150, 200, 1.0).unwrap();
        let bounds = |id: &str| {
            display
                .nodes
                .iter()
                .find(|node| node.test_id == id)
                .unwrap()
                .bounds
        };
        assert_eq!(bounds("item1")[0], 80.0);
        if mode == "nowrap" {
            assert_eq!(bounds("item2")[0], 160.0);
            assert_eq!(bounds("wrap")[3], 20.0);
        } else {
            assert_eq!(bounds("wrap")[3], 50.0);
            assert_eq!(bounds("item2")[0], 0.0);
            assert_eq!(
                (bounds("item0")[1], bounds("item2")[1]),
                if mode == "wrap" {
                    (0.0, 30.0)
                } else {
                    (30.0, 0.0)
                }
            );
            let narrow = page.layout(70, 200, 1.0).unwrap();
            assert_eq!(
                narrow
                    .nodes
                    .iter()
                    .find(|node| node.test_id == "wrap")
                    .unwrap()
                    .bounds[3],
                80.0
            );
        }
    }
}
#[test]
fn composed_button_lays_out_content_and_groups_semantics_at_each_scale() {
    for scale in [1.0, 2.0] {
        let module = crate::session::tests::source(
            "function render(){const n=useLocal('n',0);return h('column',{},h('button',{testId:'action',accessibilityLabel:'Open inbox',style:{padding:16,width:240,background:'#ffffff'},onClick:()=>n.set(n.value+1)},h('column',{style:{gap:4}},h('text',{testId:'title',style:{fontSize:18}},'Inbox'),h('text',{testId:'description',style:{fontSize:12}},'Unread '+n.value))))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(300, 200, scale).unwrap();
        let button = first
            .nodes
            .iter()
            .find(|node| node.test_id == "action")
            .unwrap();
        let title = first
            .nodes
            .iter()
            .find(|node| node.test_id == "title")
            .unwrap();
        let description = first
            .nodes
            .iter()
            .find(|node| node.test_id == "description")
            .unwrap();
        assert_eq!(button.text, "Open inbox");
        assert_eq!(button.accessibility_hidden, None);
        assert_eq!(title.accessibility_hidden, Some(true));
        assert_eq!(description.accessibility_hidden, Some(true));
        assert_eq!(title.bounds[0], button.bounds[0] + 16.0);
        assert!(description.bounds[1] >= title.bounds[1] + title.bounds[3]);
        assert!(
            description.bounds[1] + description.bounds[3] <= button.bounds[1] + button.bounds[3]
        );
        let old_content = button.button_content.clone();
        let old_bounds = button.bounds;
        let generation = page.generation();
        let callback = button.on_click.unwrap();
        first.rasterize().unwrap();
        assert!(page.click(generation, callback).unwrap());
        let second = page.layout(300, 200, scale).unwrap();
        let changed = second
            .nodes
            .iter()
            .find(|node| node.test_id == "action")
            .unwrap();
        assert_ne!(
            old_content, changed.button_content,
            "child changes invalidate gesture rebinding"
        );
        assert_eq!(changed.bounds, old_bounds);
        assert!(!page.click(generation, callback).unwrap());
    }
}

#[test]
fn composed_checkbox_lays_out_content_and_groups_semantics_at_each_scale() {
    for scale in [1.0, 2.0] {
        let module = crate::session::tests::source(
            "function render(){const n=useLocal('n',0);return h('column',{},h('checkbox',{value:n.value%2===1,testId:'action',accessibilityLabel:'Open inbox',style:{padding:16,width:240,background:'#ffffff'},onChange:()=>n.set(n.value+1)},h('column',{style:{gap:4}},h('text',{testId:'title',style:{fontSize:18}},'Inbox'),h('text',{testId:'description',style:{fontSize:12}},'Unread '+n.value))))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(300, 200, scale).unwrap();
        let button = first
            .nodes
            .iter()
            .find(|node| node.test_id == "action")
            .unwrap();
        let title = first
            .nodes
            .iter()
            .find(|node| node.test_id == "title")
            .unwrap();
        let description = first
            .nodes
            .iter()
            .find(|node| node.test_id == "description")
            .unwrap();
        assert_eq!(button.text, "Open inbox");
        assert_eq!(button.checked, Some(false));
        assert_eq!(button.accessibility_hidden, None);
        assert_eq!(title.accessibility_hidden, Some(true));
        assert_eq!(description.accessibility_hidden, Some(true));
        assert_eq!(title.bounds[0], button.bounds[0] + 16.0);
        assert!(description.bounds[1] >= title.bounds[1] + title.bounds[3]);
        assert!(
            description.bounds[1] + description.bounds[3] <= button.bounds[1] + button.bounds[3]
        );
        let old_content = button.button_content.clone();
        let old_bounds = button.bounds;
        let generation = page.generation();
        let callback = button.on_click.unwrap();
        first.rasterize().unwrap();
        assert!(page.click(generation, callback).unwrap());
        let second = page.layout(300, 200, scale).unwrap();
        let changed = second
            .nodes
            .iter()
            .find(|node| node.test_id == "action")
            .unwrap();
        assert_ne!(
            old_content, changed.button_content,
            "child changes invalidate gesture rebinding"
        );
        assert_eq!(changed.checked, Some(true));
        assert_eq!(changed.bounds, old_bounds);
        assert!(!page.click(generation, callback).unwrap());
    }
}

#[test]
fn percentage_growth_cannot_publish_unbounded_geometry() {
    let source = crate::session::tests::source(
        "function render(){let child=h('text',{},'value');for(let i=0;i<12;i++)child=h('column',{style:{width:'16384%'}},child);return h('column',{style:{width:300}},child)}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    assert!(
        page.layout(300, 300, 1.0)
            .err()
            .unwrap()
            .contains("coordinate budget")
    );
}

#[test]
fn percentage_dimensions_follow_parent_content_boxes_and_scale() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{width:'100%',height:'100%',padding:20,borderWidth:5}},h('column',{testId:'half',style:{width:'50%',height:'50%',background:'#ff0000'}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    for (width, height, scale) in [(300, 200, 1.0), (600, 400, 2.0), (500, 300, 1.0)] {
        let display = page.layout(width, height, scale).unwrap();
        let half = display.nodes.iter().find(|n| n.test_id == "half").unwrap();
        let expected = [
            25.0,
            25.0,
            (width as f32 / scale - 50.0) / 2.0,
            (height as f32 / scale - 50.0) / 2.0,
        ];
        for (actual, expected) in half.bounds.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 0.01, "{actual} vs {expected}");
        }
    }
}

#[test]
fn percentage_max_width_wraps_text_and_min_width_overrides_a_smaller_width() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{width:'100%',padding:20,align:'end'}},h('text',{testId:'address',style:{maxWidth:'72%',wordBreak:'break-all',fontSize:16}},'2001:db8:1234:5678:90ab:cdef:1234:5678'),h('column',{testId:'limited',style:{width:'10%',minWidth:'50%',maxWidth:'80%',height:10}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut heights = Vec::new();
    for width in [200, 400] {
        let display = page.layout(width, 400, 1.0).unwrap();
        let content = width as f32 - 40.0;
        let address = display
            .nodes
            .iter()
            .find(|n| n.test_id == "address")
            .unwrap();
        assert!(
            address.bounds[2] <= content * 0.72 + 0.01,
            "{:?}",
            address.bounds
        );
        assert!((address.bounds[0] + address.bounds[2] - (width as f32 - 20.0)).abs() < 0.01);
        heights.push(address.bounds[3]);
        let limited = display
            .nodes
            .iter()
            .find(|n| n.test_id == "limited")
            .unwrap();
        assert!((limited.bounds[2] - content * 0.5).abs() < 0.01);
    }
    assert!(heights[0] > heights[1]);
}

#[test]
fn automatic_textarea_height_tracks_wrapping_and_respects_limits() {
    let mut heights = Vec::new();
    for value in [
        "",
        "Hello",
        "A long sentence with several words that wraps over the available width.",
        &"More words to wrap. ".repeat(30),
    ] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('column',{{style:{{width:180}}}},h('textarea',{{testId:'editor',value:{},style:{{height:'auto',minHeight:42,maxHeight:120,fontSize:14,lineHeight:21,padding:10}}}}))}}",
            serde_json::to_string(value).unwrap()
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(240, 400, 1.0).unwrap();
        heights.push(
            frame
                .nodes
                .iter()
                .find(|n| n.test_id == "editor")
                .unwrap()
                .bounds[3],
        );
    }
    assert_eq!(heights[0], 42.0);
    assert_eq!(heights[1], 42.0);
    assert!(
        heights[2] > heights[1] && heights[2] <= 120.0,
        "{heights:?}"
    );
    assert_eq!(heights[3], 120.0);
}

#[test]
fn asymmetric_padding_controls_child_geometry_and_painted_insets() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{padding:10,paddingLeft:40,paddingRight:20,paddingBottom:30,background:'#ffffff'}},h('spacer',{style:{grow:1,background:'#ff0000'}}))}",
    );
    for scale in [1.0, 2.0] {
        let mut page = crate::ViewSession::new(&module).unwrap();
        let width = (300.0 * scale) as u16;
        let frame = page.layout(width, (100.0 * scale) as u16, scale).unwrap();
        assert_eq!(frame.nodes[1].bounds, [40.0, 10.0, 240.0, 60.0]);
        let raster = frame.rasterize().unwrap();
        let pixel = |x: f32, y: f32| {
            let offset = (((y * scale) as usize * width as usize) + (x * scale) as usize) * 4;
            &raster.pixels.data_as_u8_slice()[offset..offset + 4]
        };
        assert_eq!(pixel(39.0, 15.0), &[255, 255, 255, 255]);
        assert_eq!(pixel(41.0, 15.0), &[255, 0, 0, 255]);
        assert_eq!(pixel(281.0, 15.0), &[255, 255, 255, 255]);
        assert_eq!(pixel(50.0, 9.0), &[255, 255, 255, 255]);
        assert_eq!(pixel(50.0, 71.0), &[255, 255, 255, 255]);
    }
}

#[test]
fn mixed_text_and_control_baselines_match_painted_glyphs() {
    for align in ["align:'baseline'", "align:'start'"] {
        let own = if align.contains("start") {
            "alignSelf:'baseline',"
        } else {
            ""
        };
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('row',{{style:{{{align},gap:12}}}},
                h('text',{{style:{{{own}fontSize:36,lineHeight:40}}}},'100'),
                h('column',{{style:{{{own}padding:7,borderWidth:2}}}},
                    h('text',{{style:{{width:50,fontSize:18,lineHeight:28}}}},'11 22 33')),
                h('button',{{style:{{{own}width:75,height:64,fontSize:13,padding:4}}}},'Go'))}}"
        ));
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&module).unwrap();
            let frame = page
                .layout((360.0 * scale) as u16, (180.0 * scale) as u16, scale)
                .unwrap();
            let mut baselines = Vec::new();
            for size in [36.0, 18.0, 13.0] {
                let baseline = frame
                    .commands
                    .iter()
                    .find_map(|paint| match paint {
                        Paint::Text {
                            size: actual,
                            glyphs,
                            ..
                        } if (*actual - size).abs() < 0.01 => glyphs.first().map(|glyph| glyph.y),
                        _ => None,
                    })
                    .expect("painted text run");
                baselines.push(baseline);
            }
            for baseline in &baselines[1..] {
                assert!(
                    (baseline - baselines[0]).abs() <= scale,
                    "{align}, scale {scale}: {baselines:?}"
                );
            }
        }
    }
}

#[test]
fn button_labels_center_vertically_without_artificial_padding() {
    let baseline = |height| {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('button',{{style:{{width:160,height:{height},padding:0,fontSize:17}}}},'Go')}}",
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(200, 160, 1.0).unwrap();
        frame
            .commands
            .iter()
            .find_map(|paint| match paint {
                Paint::Text { glyphs, .. } => glyphs.first().map(|glyph| glyph.y),
                _ => None,
            })
            .unwrap()
    };
    assert!((baseline(120) - baseline(80) - 20.0).abs() < 0.01);
}

#[test]
fn layout_alignment_and_self_override_use_logical_dimensions_at_each_scale() {
    let module = crate::session::tests::source(
        "function render(){return h('row',{style:{padding:10,align:'center',justify:'between'}},h('spacer',{style:{width:40,height:20}}),h('spacer',{style:{width:60,height:30,alignSelf:'end'}}))}",
    );
    for scale in [1.0, 2.0] {
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page
            .layout((300.0 * scale) as u16, (100.0 * scale) as u16, scale)
            .unwrap();
        assert_eq!(frame.nodes[1].bounds, [10.0, 40.0, 40.0, 20.0]);
        assert_eq!(frame.nodes[2].bounds, [230.0, 60.0, 60.0, 30.0]);
    }
}

#[test]
fn layout_size_limits_and_shrink_preserve_fixed_siblings() {
    let module = crate::session::tests::source(
        "function render(){return h('row',{},h('spacer',{style:{width:200,height:30,minWidth:80,maxWidth:120,minHeight:40,maxHeight:50,shrink:1}}),h('spacer',{style:{width:160,height:20,shrink:0}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let narrow = page.layout(240, 100, 1.0).unwrap();
    assert_eq!(narrow.nodes[1].bounds, [0.0, 0.0, 80.0, 40.0]);
    assert_eq!(narrow.nodes[2].bounds, [80.0, 0.0, 160.0, 20.0]);
    let wide = page.layout(400, 100, 1.0).unwrap();
    assert_eq!(wide.nodes[1].bounds, [0.0, 0.0, 120.0, 40.0]);
    assert_eq!(wide.nodes[2].bounds, [120.0, 0.0, 160.0, 20.0]);
}

#[test]
fn column_alignment_respects_min_width_and_max_height() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{align:'center',justify:'center'}},h('spacer',{style:{width:20,height:200,minWidth:60,maxWidth:80,minHeight:40,maxHeight:50}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let frame = page.layout(100, 200, 1.0).unwrap();
    assert_eq!(frame.nodes[1].bounds, [20.0, 75.0, 60.0, 50.0]);
}

#[test]
fn centered_overflow_keeps_scroll_content_start_reachable() {
    let module = crate::session::tests::source(
        "function render(){return h('scroll',{style:{align:'center',justify:'end'}},h('spacer',{style:{width:200,height:180}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let frame = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(frame.nodes[1].bounds, [0.0, 0.0, 200.0, 180.0]);
    assert!(frame.nodes[0].scroll.as_ref().unwrap().max_offset >= 80.0);
}

#[test]
fn late_images_share_layout_and_obey_fit_rounding_and_scroll_clipping() {
    use ::image::ImageEncoder;
    let mut bytes = Vec::new();
    ::image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &[255, 0, 0, 255, 0, 0, 255, 255],
            2,
            1,
            ::image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    let images = HashMap::from([(
        "photo".to_string(),
        crate::scene::image::Bitmap::decode(&bytes).unwrap(),
    )]);
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{background:'#ffffff'}},h('image',{src:'photo',fit:'contain',style:{width:100,height:100,radius:20}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let pending = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(pending.image_sources().collect::<Vec<_>>(), vec!["photo"]);
    let retained = pending.retained.clone();
    let placeholder = pending.rasterize().unwrap();
    assert_eq!(
        &placeholder.pixels.data_as_u8_slice()[0..4],
        &[255, 255, 255, 255]
    );
    let mut display = retained.compose(&HashMap::new()).unwrap();
    display.bind_images(&images).unwrap();
    let loaded = display.rasterize().unwrap();
    let pixel = |x: usize, y: usize| {
        &loaded.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
    };
    assert_eq!(pixel(50, 10), &[255, 255, 255, 255]);
    assert!(pixel(10, 50)[0] > 240 && pixel(10, 50)[2] < 15);
    assert!(pixel(90, 50)[2] > 240 && pixel(90, 50)[0] < 15);
    let source = crate::session::tests::source(
        "function render(){return h('scroll',{},h('spacer',{style:{height:150}}),h('image',{src:'photo',style:{width:100,height:100,radius:30}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let first = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(first.image_sources().count(), 0);
    let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
    let mut visible = first
        .retained
        .compose(&HashMap::from([(id, 150.)]))
        .unwrap();
    assert_eq!(visible.image_sources().count(), 1);
    visible.bind_images(&images).unwrap();
    let frame = visible.rasterize().unwrap();
    assert_eq!(&frame.pixels.data_as_u8_slice()[..4], &[246, 247, 251, 255]);
    assert_ne!(
        &frame.pixels.data_as_u8_slice()[(50 * 100 + 50) * 4..(50 * 100 + 50) * 4 + 3],
        &[246, 247, 251]
    );
}
#[test]
fn node_identity_tracks_keyed_ancestry_and_tag_not_callback_or_label() {
    let mut renderer = Renderer::default();
    let mut records = vec![
        node(0, None, "root", &[]),
        node(3, Some(0), "save", &[(8, json!(1))]),
    ];
    let initial = renderer.layout(&records, 200, 200, 1.0).unwrap();
    let id = initial.nodes[1].identity.clone();
    records[1].fields = vec![(8, json!(99))];
    records[1].text = "Saved".into();
    records.insert(1, node(4, Some(0), "new", &[(1, json!(10))]));
    let reordered = renderer.layout(&records, 200, 200, 1.0).unwrap();
    assert_eq!(reordered.nodes[2].identity, id);
    records[2].tag = 2;
    assert_ne!(
        renderer.layout(&records, 200, 200, 1.0).unwrap().nodes[2].identity,
        id
    );
    records[2].tag = 3;
    records[0].key = "other-page".into();
    assert_ne!(
        renderer.layout(&records, 200, 200, 1.0).unwrap().nodes[2].identity,
        id
    );
}

#[test]
fn keyed_scroll_identity_survives_sibling_insertion_and_content_shrink() {
    let mut renderer = Renderer::default();
    let mut records = page();
    let initial = renderer.layout(&records, 100, 100, 1.0).unwrap();
    let id = initial.nodes[2].scroll.as_ref().unwrap().id.clone();
    records.insert(1, node(4, Some(0), "inserted", &[(1, json!(5))]));
    records[4].parent = Some(3);
    records[5].parent = Some(3);
    let frame = renderer
        .layout_scrolled(
            &records,
            100,
            100,
            1.0,
            &HashMap::from([(id.clone(), 40.0)]),
        )
        .unwrap();
    assert_eq!(frame.nodes[3].scroll.as_ref().unwrap().id, id);
    assert_eq!(frame.nodes[3].scroll.as_ref().unwrap().offset, 40.0);
    records.remove(5);
    let frame = renderer
        .layout_scrolled(&records, 100, 100, 1.0, &HashMap::from([(id, 40.0)]))
        .unwrap();
    assert_eq!(frame.nodes[3].scroll.as_ref().unwrap().offset, 0.0);
}

#[test]
fn growing_ancestors_reserve_a_bounded_scroll_viewport() {
    let records = vec![
        node(0, None, "root", &[]),
        node(4, Some(0), "header", &[(1, json!(20))]),
        node(1, Some(0), "row", &[(7, json!(1))]),
        node(0, Some(2), "content", &[(7, json!(1))]),
        node(4, Some(3), "tabs", &[(1, json!(20))]),
        node(5, Some(3), "feed", &[(7, json!(1))]),
        node(4, Some(5), "items", &[(1, json!(1000))]),
    ];
    let frame = Renderer::default().layout(&records, 100, 100, 1.0).unwrap();
    assert_eq!(frame.nodes[5].bounds, [0.0, 40.0, 100.0, 60.0]);
    assert_eq!(frame.nodes[5].scroll.as_ref().unwrap().max_offset, 940.0);
}
