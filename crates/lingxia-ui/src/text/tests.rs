use crate::scene::test_prelude::*;

#[test]
fn strikethrough_tracks_wrapped_runs_and_cached_text_without_changing_layout() {
    let mut page = crate::ViewSession::new(&crate::session::tests::source(
        "function render(){const strike=useLocal('strike',false);return h('column',{style:{textDecorationLine:strike.value?'line-through':'none',color:'#123456'}},h('text',{style:{width:80,fontSize:20,textDecorationLine:'none',textAlign:'right'}},'one two three four'),h('button',{onClick:()=>strike.set(!strike.value)},'Toggle'))}",
    )).unwrap();
    let mut plain = None;
    for strike in [false, true, false] {
        let frame = page.layout(200, 200, 1.0).unwrap();
        let commands = &frame.retained.commands[frame.retained.ranges[1].clone()];
        let text_runs = commands
            .iter()
            .filter(|p| matches!(p, Paint::Text { .. }))
            .count();
        let strikes: Vec<_> = commands
            .iter()
            .filter_map(|p| match p {
                Paint::Rect {
                    corners: _,
                    bounds,
                    color: [18, 52, 86, 255],
                    radius: 0.0,
                } => Some(bounds),
                _ => None,
            })
            .collect();
        assert!(text_runs > 1, "fixture must wrap");
        assert_eq!(strikes.len(), if strike { text_runs } else { 0 });
        for bounds in strikes {
            assert!(bounds[2] > 0.0 && bounds[2] <= 80.0 && bounds[3] >= 1.0);
        }
        let callback = frame.nodes[2].on_click.unwrap();
        let pixels = frame
            .rasterize()
            .unwrap()
            .pixels
            .data_as_u8_slice()
            .to_vec();
        if strike {
            assert_ne!(plain.as_ref().unwrap(), &pixels);
        } else if let Some(expected) = &plain {
            assert_eq!(expected, &pixels);
        } else {
            plain = Some(pixels);
        }
        page.click(page.generation(), callback).unwrap();
    }
}

#[test]
fn text_alignment_updates_cached_glyph_positions_and_preserves_overrides() {
    let source = crate::session::tests::source(
        "function render(){const right=useLocal('right',true);return h('column',{style:{textAlign:right.value?'right':'left'}},h('text',{style:{width:160,padding:8,borderWidth:2}},'MMMM\\nM'),h('text',{style:{width:160,padding:8,borderWidth:2,textAlign:'center'}},'MMMM\\nM'),h('button',{onClick:()=>right.set(!right.value)},'Toggle'))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut right_pixels = None;
    let mut centered = None;
    for right in [true, false, true] {
        let display = page.layout(200, 240, 1.0).unwrap();
        let origins = |index: usize| {
            display.retained.commands[display.retained.ranges[index].clone()]
                .iter()
                .filter_map(|paint| {
                    if let Paint::Text { glyphs, .. } = paint {
                        glyphs.first().map(|glyph| glyph.x)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        let positions = origins(1usize);
        assert_eq!(positions.len(), 2);
        if right {
            assert!(positions[1] > positions[0] + 20.0);
        } else {
            assert!((positions[0] - positions[1]).abs() < 0.01);
        }
        if let Some(previous) = &centered {
            assert_eq!(&origins(2), previous);
        } else {
            centered = Some(origins(2));
        }
        let callback = display.nodes[3].on_click.unwrap();
        let pixels = display
            .rasterize()
            .unwrap()
            .pixels
            .data_as_u8_slice()
            .to_vec();
        if right {
            if let Some(previous) = &right_pixels {
                assert_eq!(&pixels, previous);
            } else {
                right_pixels = Some(pixels);
            }
        }
        page.click(page.generation(), callback).unwrap();
    }
}

#[test]
fn inherited_text_alignment_respects_each_line_and_direction() {
    for rtl in [false, true] {
        for alignment in ["left", "center", "right", "start", "end", "justify"] {
            let value = if rtl {
                "\\u05d0\\u05d1\\u05d2\\n\\u05d0"
            } else {
                "MMMM\\nM"
            };
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('column',{{style:{{textAlign:'{alignment}'}}}},h('textarea',{{value:'{value}',style:{{width:160,height:100,padding:8,borderWidth:2}}}}))}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let display = page.layout(200, 160, 1.0).unwrap();
            let layout = &display.retained.inputs[1].as_ref().unwrap().layout;
            assert_eq!(layout.len(), 2);
            assert_eq!(layout.is_rtl(), rtl);
            let fraction = match alignment {
                "center" => 0.5,
                "right" => 1.0,
                "left" => 0.0,
                "end" => {
                    if rtl {
                        0.0
                    } else {
                        1.0
                    }
                }
                _ => {
                    if rtl {
                        1.0
                    } else {
                        0.0
                    }
                }
            };
            for line in layout.lines() {
                let metrics = line.metrics();
                assert!(
                    (metrics.offset - (140.0 - metrics.advance) * fraction).abs() < 0.02,
                    "{alignment}, rtl={rtl}: {metrics:?}"
                );
            }
        }
    }
}

#[test]
fn word_break_wraps_inherited_text_and_keeps_cached_modes_distinct() {
    for (property, value) in [
        ("wordBreak", "break-all"),
        ("overflowWrap", "anywhere"),
        ("overflowWrap", "break-word"),
    ] {
        let source = crate::session::tests::source(
        "function render(){const wrap=useLocal('wrap',false);return h('column',{style:{width:320,padding:20,wordBreak:wrap.value?'break-all':'normal'}},h('row',{style:{gap:12}},h('text',{style:{fontSize:14}},'IPv6'),h('column',{testId:'cell',style:{flexGrow:1,minWidth:0,alignItems:'end'}},h('text',{testId:'address',style:{fontSize:14,padding:12}},'2001:db8:1234:5678:90ab:cdef:1234:5678'))),h('button',{testId:'toggle',onClick:()=>wrap.set(!wrap.value)},'Toggle'))}".replace("wordBreak", property).replace("break-all", value).as_str(),
    );
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&source).unwrap();
            let mut normal_height = 0.0;
            for wrapped in [false, true, false, true] {
                let display = page.layout(320, 600, scale).unwrap();
                let address = display
                    .nodes
                    .iter()
                    .find(|node| node.test_id == "address")
                    .unwrap();
                let cell = display
                    .nodes
                    .iter()
                    .find(|node| node.test_id == "cell")
                    .unwrap();
                assert_eq!(address.text, "2001:db8:1234:5678:90ab:cdef:1234:5678");
                if wrapped {
                    assert!(
                        address.bounds[2] <= cell.bounds[2] + 0.1,
                        "address {:?}, cell {:?}",
                        address.bounds,
                        cell.bounds
                    );
                    assert!(address.bounds[3] > normal_height);
                } else {
                    normal_height = address.bounds[3];
                }
                let callback = display
                    .nodes
                    .iter()
                    .find(|node| node.test_id == "toggle")
                    .unwrap()
                    .on_click
                    .unwrap();
                display.rasterize().unwrap();
                page.click(page.generation(), callback).unwrap();
            }
        }
    }
}

#[test]
fn letter_spacing_changes_wrapping_without_stale_cached_layouts() {
    let module = crate::session::tests::source(
        "function render(v){return h('column',{},h('text',{style:{width:80,fontFamily:'monospace',fontSize:16,wordBreak:'break-all',letterSpacing:v.data.spacing}},'ABCDEFGH'))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let mut heights = Vec::new();
    for spacing in [0, 8, 0] {
        page.receive_state(&serde_json::json!({"v":2,"kind":"state.snapshot","rev":heights.len(),"state":{"spacing":spacing}}))
            .unwrap();
        let frame = page.layout(200, 200, 1.0).unwrap();
        heights.push(frame.nodes[1].bounds[3]);
    }
    assert!(heights[1] > heights[0]);
    assert_eq!(heights[2], heights[0]);
}
#[test]
fn typography_inherits_family_and_absolute_line_height_with_local_overrides() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{fontFamily:'monospace',fontSize:18,lineHeight:'36px'}},h('input',{value:'iiii',style:{height:70}}),h('input',{value:'WWWW',style:{height:70}}),h('input',{value:'WWWW',style:{height:70,fontFamily:'sans-serif',lineHeight:'28px'}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let frame = page.layout(320, 240, 1.0).unwrap();
    let a = &frame.retained.inputs[1].as_ref().unwrap().layout;
    let b = &frame.retained.inputs[2].as_ref().unwrap().layout;
    let c = &frame.retained.inputs[3].as_ref().unwrap().layout;
    assert!(
        (a.width() - b.width()).abs() < 0.01,
        "monospaced advances must match"
    );
    assert_eq!(a.height(), 36.0);
    assert_eq!(b.height(), 36.0);
    assert_eq!(c.height(), 28.0);
    assert!(
        (c.width() - b.width()).abs() > 1.0,
        "local sans override must select a different face"
    );
}

#[test]
fn synthetic_italic_leans_right_and_ink_bounds_contain_pixels() {
    let module = crate::session::tests::source(
        "function render(){return h('text',{style:{fontSize:48,padding:16,color:'#000000',backgroundColor:'#ffffff'}},'IIII')}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let mut centers = Vec::new();
    for slope in [0.0, 12f64.to_radians().tan()] {
        let mut display = page.layout(240, 100, 1.0).unwrap();
        let mut bounds = None;
        for paint in &mut display.commands {
            if let Paint::Text {
                font,
                size,
                coordinates,
                glyphs,
                skew,
                ink,
                ..
            } = paint
            {
                *skew = slope;
                *ink = glyph_ink(font, *size, coordinates, glyphs, slope as f32, 1.0);
                bounds = *ink;
            }
        }
        let bounds = bounds.unwrap();
        let raster = display.rasterize().unwrap();
        let mut sum = 0.0;
        let mut count = 0.0;
        for (index, pixel) in raster.pixels.data_as_u8_slice().chunks_exact(4).enumerate() {
            if pixel[0] < 128 {
                let x = (index % 240) as f32;
                let y = (index / 240) as f32;
                assert!(
                    x >= bounds[0]
                        && x <= bounds[0] + bounds[2]
                        && y >= bounds[1]
                        && y <= bounds[1] + bounds[3]
                );
                sum += x;
                count += 1.0;
            }
        }
        assert!(count > 0.0);
        centers.push(sum / count);
    }
    assert!(
        centers[1] > centers[0] + 1.0,
        "synthetic italic must lean right: {centers:?}"
    );
}

#[test]
fn italic_inherits_resets_and_does_not_reuse_upright_shaping() {
    let module = crate::session::tests::source(
        "function render(v){return h('column',{style:{backgroundColor:'#ffffff',color:'#000000',fontSize:32,fontStyle:v.data.parent}},h('text',{style:v.data.child?{fontStyle:v.data.child}:{}},'Italic text ffi'))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let mut pixels = Vec::new();
    for (parent, child) in [
        ("normal", ""),
        ("italic", ""),
        ("italic", "normal"),
        ("normal", "italic"),
        ("normal", ""),
    ] {
        page.receive_state(&serde_json::json!({"v":2,"kind":"state.snapshot","rev":pixels.len(),"state":{"parent":parent,"child":child}}))
            .unwrap();
        pixels.push(
            page.layout(320, 100, 1.0)
                .unwrap()
                .rasterize()
                .unwrap()
                .pixels
                .data_as_u8_slice()
                .to_vec(),
        );
    }
    assert_ne!(pixels[0], pixels[1]);
    assert_eq!(pixels[0], pixels[2]);
    assert_eq!(pixels[1], pixels[3]);
    assert_eq!(pixels[0], pixels[4]);
}

#[test]
fn font_weight_inherits_and_explicit_children_override_it() {
    let ink = |parent_weight, weight| {
        let weight = if weight == 0 {
            String::new()
        } else {
            format!("fontWeight:{weight},")
        };
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('column',{{style:{{backgroundColor:'#ffffff',color:'#000000',fontSize:32,fontWeight:{parent_weight}}}}},h('text',{{style:{{{weight}lineHeight:'44px'}}}},'Hello world'))}}"
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(300, 80, 1.0).unwrap().rasterize().unwrap();
        frame
            .pixels
            .data_as_u8_slice()
            .chunks_exact(4)
            .map(|pixel| (255 - pixel[0]) as u64)
            .sum::<u64>()
    };
    let normal = ink(400, 400);
    assert_eq!(normal, ink(700, 400));
    assert_eq!(ink(400, 700), ink(700, 0));
    assert!(ink(400, 700) > normal, "bold glyphs must paint more ink");
}

#[test]
fn padded_multiline_text_matches_equivalent_container_height() {
    let value = "A standalone window with no sidebar or shell. Desktop only.";
    for (scale, border) in [(1.0, 0), (2.0, 0), (1.0, 2), (2.0, 2)] {
        for width in 300..341 {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('column',{{style:{{width:{width},gap:10}}}},h('text',{{testId:'padded',style:{{fontSize:12,lineHeight:'20px',borderWidth:{border},paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8}}}},'{value}'),h('column',{{testId:'wrapped',style:{{borderWidth:{border},paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8}}}},h('text',{{style:{{fontSize:12,lineHeight:'20px'}}}},'{value}')))}}"
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let frame = page
                .layout((420.0 * scale) as u16, (800.0 * scale) as u16, scale)
                .unwrap();
            let padded = frame.nodes.iter().find(|n| n.test_id == "padded").unwrap();
            let wrapped = frame.nodes.iter().find(|n| n.test_id == "wrapped").unwrap();
            assert!(
                (padded.bounds[3] - wrapped.bounds[3]).abs() <= 1.0,
                "width {width}, scale {scale}, border {border}: padded {:?}, wrapped {:?}",
                padded.bounds,
                wrapped.bounds
            );
        }
    }
}

#[test]
fn cached_shaping_preserves_pixels_when_width_style_and_text_change() {
    let mut cached = Renderer::default();
    let mut fresh = Renderer::default();
    for (width, size, weight, family, height, text) in [
        (
            240,
            18,
            400,
            "sans",
            24,
            "A long title wraps across narrow cards",
        ),
        (
            90,
            18,
            400,
            "sans",
            24,
            "A long title wraps across narrow cards",
        ),
        (
            240,
            18,
            400,
            "sans",
            24,
            "A long title wraps across narrow cards",
        ),
        (150, 22, 700, "mono", 32, "Wi il 12345"),
        (150, 22, 400, "sans", 28, "Wi il 12345"),
        (
            160,
            18,
            400,
            "sans",
            24,
            "Latin \u{4e2d}\u{6587} \u{1f600} \u{5e9}\u{5dc}\u{5d5}\u{5dd}",
        ),
        (
            80,
            18,
            400,
            "sans",
            24,
            "Latin \u{4e2d}\u{6587} \u{1f600} \u{5e9}\u{5dc}\u{5d5}\u{5dd}",
        ),
    ] {
        let mut label = node(
            2,
            Some(0),
            "label",
            &[
                (4, json!(size)),
                (36, json!(weight)),
                (37, json!(height)),
                (38, json!(family)),
            ],
        );
        label.text = text.into();
        let records = vec![node(0, None, "root", &[]), label];
        fresh.clear_text_cache();
        let actual = cached.layout(&records, width, 300, 1.0).unwrap();
        let expected = fresh.layout(&records, width, 300, 1.0).unwrap();
        assert_eq!(
            serde_json::to_value(&actual.nodes).unwrap(),
            serde_json::to_value(&expected.nodes).unwrap()
        );
        assert_eq!(
            actual.rasterize().unwrap().pixels.data_as_u8_slice(),
            expected.rasterize().unwrap().pixels.data_as_u8_slice()
        );
    }
}

#[test]
fn text_ink_outside_a_zero_height_layout_box_is_not_culled() {
    let mut text = node(2, Some(0), "text", &[(1, json!(0)), (4, json!(24))]);
    text.text = "Visible overflow".into();
    let records = vec![node(0, None, "root", &[]), text];
    let display = Renderer::default().layout(&records, 240, 100, 1.0).unwrap();
    assert_eq!(display.nodes[1].visible_bounds[3], 0.0);
    assert!(
        display
            .commands
            .iter()
            .any(|command| matches!(command, Paint::Text { .. }))
    );
}

mod text_overflow {
    use crate::protocol::{field, tag};
    use crate::scene::test_prelude::*;

    fn painted_glyphs(display: &crate::DisplayList) -> usize {
        display
            .commands
            .iter()
            .map(|paint| match paint {
                Paint::Text { glyphs, .. } => glyphs.len(),
                _ => 0,
            })
            .sum()
    }

    fn text(content: &str, fields: &[(usize, serde_json::Value)]) -> Vec<Record> {
        let mut label = node(tag::TEXT, Some(0), "label", fields);
        label.text = content.into();
        vec![
            node(tag::COLUMN, None, "root", &[(field::WIDTH, json!(80))]),
            label,
        ]
    }

    #[test]
    fn nowrap_keeps_one_line_and_ellipsis_truncates_without_changing_semantics() {
        let long = "A sentence much longer than the box";
        let wrapped = Renderer::default()
            .layout(&text(long, &[]), 200, 200, 1.0)
            .unwrap();
        let single = Renderer::default()
            .layout(
                &text(long, &[(field::WHITE_SPACE, json!("nowrap"))]),
                200,
                200,
                1.0,
            )
            .unwrap();
        assert!(single.nodes[1].bounds[3] < wrapped.nodes[1].bounds[3]);
        let truncated = Renderer::default()
            .layout(
                &text(
                    long,
                    &[
                        (field::WHITE_SPACE, json!("nowrap")),
                        (field::TEXT_OVERFLOW, json!("ellipsis")),
                    ],
                ),
                200,
                200,
                1.0,
            )
            .unwrap();
        assert!(painted_glyphs(&truncated) < painted_glyphs(&single));
        assert_eq!(
            truncated.nodes[1].text, long,
            "semantics keep the authored text"
        );
        let clamped = Renderer::default()
            .layout(
                &text(
                    long,
                    &[
                        (field::TEXT_LINES, json!(2)),
                        (field::TEXT_OVERFLOW, json!("ellipsis")),
                    ],
                ),
                200,
                200,
                1.0,
            )
            .unwrap();
        assert!(painted_glyphs(&clamped) < painted_glyphs(&wrapped));
        let short = Renderer::default()
            .layout(
                &text(
                    "Hi",
                    &[
                        (field::WHITE_SPACE, json!("nowrap")),
                        (field::TEXT_OVERFLOW, json!("ellipsis")),
                    ],
                ),
                200,
                200,
                1.0,
            )
            .unwrap();
        assert_eq!(
            painted_glyphs(&short),
            2,
            "text that fits is painted unchanged"
        );
    }

    #[test]
    fn transforms_paint_while_semantics_keep_the_source_and_underlines_draw() {
        let display = Renderer::default()
            .layout(
                &text(
                    "hello world",
                    &[
                        (field::TEXT_TRANSFORM, json!("capitalize")),
                        (field::TEXT_DECORATION_LINE, json!("underline")),
                    ],
                ),
                200,
                200,
                1.0,
            )
            .unwrap();
        assert_eq!(display.nodes[1].text, "hello world");
        assert_eq!(
            crate::text::style::Transform::Capitalize.apply("hello world"),
            "Hello World"
        );
        assert_eq!(
            crate::text::style::Transform::Uppercase.apply("straße"),
            "STRASSE"
        );
        assert!(display.commands.iter().any(|paint| matches!(paint, Paint::Rect { bounds, .. } if bounds[3] <= 3.0 && bounds[2] > 10.0)), "the underline is a thin rule");
    }
}
