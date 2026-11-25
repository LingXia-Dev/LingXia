use crate::scene::test_prelude::*;

#[test]
fn strikethrough_tracks_wrapped_runs_and_cached_text_without_changing_layout() {
    let mut page = crate::ViewSession::new(&crate::session::tests::source(
        "function render(){const strike=useLocal('strike',false);return h('view',{style:{textDecorationLine:strike.value?'line-through':'none',color:'#123456'}},h('text',{style:{width:80,fontSize:20,textDecorationLine:'none',textAlign:'right'}},'one two three four'),h('button',{onClick:()=>strike.set(!strike.value)},'Toggle'))}",
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
        "function render(){const right=useLocal('right',true);return h('view',{style:{textAlign:right.value?'right':'left'}},h('text',{style:{width:160,padding:8,borderWidth:2,whiteSpace:'pre-line'}},'MMMM\\nM'),h('text',{style:{width:160,padding:8,borderWidth:2,textAlign:'center',whiteSpace:'pre-line'}},'MMMM\\nM'),h('button',{onClick:()=>right.set(!right.value)},'Toggle'))}",
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
                "function render(){{return h('view',{{style:{{textAlign:'{alignment}'}}}},h('textarea',{{dir:'auto',value:'{value}',style:{{width:160,height:100,padding:8,borderWidth:2}}}}))}}"
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
        "function render(){const wrap=useLocal('wrap',false);return h('view',{style:{width:320,padding:20,wordBreak:wrap.value?'break-all':'normal'}},h('view',{style:{flexDirection:'row',gap:12}},h('text',{style:{fontSize:14}},'IPv6'),h('view',{'data-testid':'cell',style:{flexGrow:1,minWidth:0,alignItems:'end'}},h('text',{'data-testid':'address',style:{fontSize:14,padding:12}},'2001:db8:1234:5678:90ab:cdef:1234:5678'))),h('button',{'data-testid':'toggle',onClick:()=>wrap.set(!wrap.value)},'Toggle'))}".replace("wordBreak", property).replace("break-all", value).as_str(),
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
        "function render(v){return h('view',{},h('text',{style:{width:80,fontFamily:'monospace',fontSize:16,wordBreak:'break-all',letterSpacing:v.data.spacing}},'ABCDEFGH'))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let mut heights = Vec::new();
    for spacing in [0, 8, 0] {
        page.write_state(crate::session::state::StateWrite::Snapshot {
            rev: heights.len() as u64,
            state: serde_json::json!({"spacing":spacing}),
        })
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
        "function render(){return h('view',{style:{fontFamily:'monospace',fontSize:18,lineHeight:'36px'}},h('input',{value:'iiii',style:{height:70}}),h('input',{value:'WWWW',style:{height:70}}),h('input',{value:'WWWW',style:{height:70,fontFamily:'sans-serif',lineHeight:'28px'}}))}",
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

/// A variable face measures ink from drawn outlines, where a space draws nothing.
#[test]
fn a_space_in_a_variable_face_does_not_void_the_runs_ink() {
    use skrifa::MetadataProvider;
    let Ok(data) = std::fs::read("/System/Library/Fonts/SFNS.ttf") else {
        eprintln!("skipped: host fonts unavailable");
        return;
    };
    let font = parley::FontData::new(parley::fontique::Blob::new(std::sync::Arc::new(data)), 0);
    let face = skrifa::FontRef::from_index(font.data.as_ref(), 0).unwrap();
    let map = face.charmap();
    let glyphs: Vec<vello_cpu::Glyph> = "A B"
        .chars()
        .enumerate()
        .map(|(at, c)| vello_cpu::Glyph {
            id: map.map(c).unwrap().to_u32(),
            x: 20.0 * at as f32,
            y: 40.0,
        })
        .collect();
    let [x, y, w, h] = glyph_ink(&font, 32.0, &[], &glyphs, 0.0, 1.0).expect("known ink");
    assert!(
        x < 5.0 && x + w > 45.0 && y < 30.0 && h > 15.0,
        "{x} {y} {w} {h}"
    );
    let blank = glyph_ink(&font, 32.0, &[], &glyphs[1..2], 0.0, 1.0);
    assert_eq!(blank, Some([0.0; 4]));
}

#[test]
fn italic_inherits_resets_and_does_not_reuse_upright_shaping() {
    let module = crate::session::tests::source(
        "function render(v){return h('view',{style:{backgroundColor:'#ffffff',color:'#000000',fontSize:32,fontStyle:v.data.parent}},h('text',{style:v.data.child?{fontStyle:v.data.child}:{}},'Italic text ffi'))}",
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
        page.write_state(crate::session::state::StateWrite::Snapshot {
            rev: pixels.len() as u64,
            state: serde_json::json!({"parent":parent,"child":child}),
        })
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
            "function render(){{return h('view',{{style:{{backgroundColor:'#ffffff',color:'#000000',fontSize:32,fontWeight:{parent_weight}}}}},h('text',{{style:{{{weight}lineHeight:'44px'}}}},'Hello world'))}}"
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
                "function render(){{return h('view',{{style:{{width:{width},gap:10}}}},h('text',{{'data-testid':'padded',style:{{fontSize:12,lineHeight:'20px',borderWidth:{border},paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8}}}},'{value}'),h('view',{{'data-testid':'wrapped',style:{{borderWidth:{border},paddingLeft:12,paddingRight:12,paddingTop:8,paddingBottom:8}}}},h('text',{{style:{{fontSize:12,lineHeight:'20px'}}}},'{value}')))}}"
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

    /// A centered `max-w-full` pill whose value is `min-w-0 grow truncate` stays inside the padded parent;
    /// without the max width it sizes to its min-content, the full nowrap text, as in CSS.
    #[test]
    fn truncated_value_keeps_a_centered_pill_inside_its_parent() {
        let label = |id, content: &str, fields: &[(usize, serde_json::Value)]| {
            let mut node = node(tag::TEXT, Some(1), id, fields);
            node.text = content.into();
            node
        };
        let records = vec![
            node(
                tag::COLUMN,
                None,
                "root",
                &[
                    (field::WIDTH, json!(200)),
                    (field::PADDING, json!(20)),
                    (field::ALIGN, json!("center")),
                ],
            ),
            node(
                tag::COLUMN,
                Some(0),
                "pill",
                &[
                    (field::FLEX_DIRECTION, json!("row")),
                    (field::PADDING, json!(16)),
                    (field::GAP, json!(8)),
                    (field::MAX_WIDTH_PERCENT, json!(100)),
                ],
            ),
            label("label", "My IP", &[(field::SHRINK, json!(0))]),
            label(
                "value",
                "240e:3a8:4ce5:6d70:6f26:bb66:3863:a7f1",
                &[
                    (field::MIN_WIDTH, json!(0)),
                    (field::GROW, json!(1)),
                    (field::OVERFLOW, json!("hidden")),
                    (field::WHITE_SPACE, json!("nowrap")),
                    (field::TEXT_OVERFLOW, json!("ellipsis")),
                ],
            ),
        ];
        let display = Renderer::default().layout(&records, 200, 200, 1.0).unwrap();
        let [x, _, w, _] = display.nodes[1].bounds;
        assert!(x >= 20.0 && x + w <= 180.0, "pill {x}+{w}");
        let [vx, _, vw, _] = display.nodes[3].bounds;
        assert!(vw > 0.0 && vx + vw <= x + w - 16.0 + 0.5, "value {vx}+{vw}");
        assert!(
            display.nodes[2].bounds[2] > 20.0,
            "the label keeps its width"
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

#[test]
fn white_space_collapses_like_css_before_shaping() {
    use crate::text::style::Collapse;
    assert_eq!(Collapse::All.apply("  a \n\t b  c "), "a b c");
    assert_eq!(Collapse::Spaces.apply("a  \n  b\t\tc\n"), "a\nb c\n");
    assert_eq!(Collapse::None.apply(" a \n b "), " a \n b ");
    let lines = |mode: &str| {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('view',{{style:{{whiteSpace:'{mode}'}}}},h('text',{{'data-testid':'t',style:{{width:400}}}},'one\\n  two  three'))}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(400, 200, 1.0).unwrap();
        let node = display.nodes.iter().find(|n| n.test_id == "t").unwrap();
        // Semantics keep the authored string whatever paint collapses.
        assert_eq!(node.text, "one\n  two  three");
        node.bounds[3]
    };
    let one = lines("normal");
    assert!((lines("pre-line") - 2.0 * one).abs() < 1.0);
    assert!((lines("pre-wrap") - 2.0 * one).abs() < 1.0);
    assert!((lines("nowrap") - one).abs() < 1.0);
}

#[test]
fn numeric_variants_inherit_as_opentype_features_and_reset_with_normal() {
    use crate::protocol::field;
    let records = crate::protocol::decode(
        serde_json::json!([
            0,
            -1,
            "",
            "",
            "",
            1,
            field::FONT_VARIANT_NUMERIC,
            "tabular-nums slashed-zero",
            2,
            0,
            "",
            "",
            "1010",
            0,
            2,
            0,
            "",
            "",
            "1010",
            1,
            field::FONT_VARIANT_NUMERIC,
            "normal"
        ])
        .as_array()
        .unwrap(),
    )
    .unwrap();
    let styles = crate::text::style::computed(&records, &[false; 3]);
    assert_eq!(styles[1].features, "\"tnum\" 1, \"zero\" 1");
    assert_eq!(styles[2].features, "");
    let source = crate::session::tests::source(
        "function render(){return h('text',{style:{fontVariantNumeric:'tabular-nums'}},'1010')}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    assert!(page.layout(200, 100, 1.0).is_ok());
}

#[path = "tests/runs.rs"]
mod runs;

#[path = "tests/bidi_isolation.rs"]
mod bidi_isolation;

#[test]
fn editors_take_the_box_direction_unless_dir_is_auto() {
    for (dir, rtl) in [("", false), ("dir:'auto',", true), ("dir:'rtl',", true)] {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('view',{{}},h('input',{{{dir}value:'\\u05d0\\u05d1\\u05d2'}}))}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(200, 100, 1.0).unwrap();
        assert_eq!(
            display.retained.inputs[1].as_ref().unwrap().layout.is_rtl(),
            rtl,
            "{dir}"
        );
    }
    let wire = serde_json::json!([
        crate::protocol::tag::COLUMN,
        -1,
        "",
        "",
        "",
        1,
        crate::protocol::field::DIRECTION,
        "auto"
    ]);
    assert!(
        crate::protocol::decode(wire.as_array().unwrap())
            .unwrap_err()
            .contains("editors")
    );
}

/// The lines a selectable paragraph of `width` breaks `text` into.
fn wrapped_lines(text: &str, width: f32) -> Vec<String> {
    let source = crate::session::tests::source(&format!(
        "function render(){{return h('view',{{}},h('text',{{style:{{userSelect:'text',fontSize:16,width:{width}}}}},'{text}'))}}"
    ));
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(400, 400, 1.0).unwrap();
    let paragraph = display
        .retained
        .selectables
        .iter()
        .flatten()
        .next()
        .unwrap();
    paragraph
        .layout
        .lines()
        .map(|line| paragraph.text[line.text_range()].to_owned())
        .collect()
}

/// Each break that falls inside a word, as the characters on either side of it.
fn mid_word_breaks(lines: &[String]) -> Vec<(char, char)> {
    lines
        .windows(2)
        .filter_map(|pair| {
            let end = pair[0].chars().last()?;
            let next = pair[1].chars().next()?;
            (!end.is_whitespace()).then_some((end, next))
        })
        .collect()
}

#[test]
fn a_long_address_breaks_before_the_at_and_before_a_domain_dot_never_mid_word() {
    let email = "ana.maria.lopez@example-mail.com";
    let text = format!("Write to {email} today");
    let mut counts = Vec::new();
    for width in [60.0, 90.0, 120.0, 160.0, 200.0, 400.0] {
        let lines = wrapped_lines(&text, width);
        assert_eq!(lines.concat(), text, "{width}");
        for (end, next) in mid_word_breaks(&lines) {
            assert!(next == '@' || next == '.', "{width}: {lines:?}");
            assert!(
                next == '@' || lines.iter().any(|line| line.starts_with('@')),
                "a dot breaks only once the @ has: {width}: {lines:?}"
            );
            let _ = end;
        }
        counts.push(lines.len());
    }
    assert!(counts[0] > 3 && counts[5] == 1, "{counts:?}");
}

#[test]
fn an_address_keeps_its_parts_whole_until_one_alone_does_not_fit() {
    // Narrowing the box walks the rungs in order: whole, split before the @, then also before
    // the domain's dot; never the dot before the @.
    let mut seen: Vec<Vec<String>> = Vec::new();
    let mut width = 200.0;
    while width >= 40.0 {
        let lines = wrapped_lines("ana@example.com", width);
        assert_eq!(lines.concat(), "ana@example.com", "{width}");
        if seen.last() != Some(&lines) {
            seen.push(lines);
        }
        width -= 1.0;
    }
    assert_eq!(
        seen[..3],
        [
            vec!["ana@example.com".to_owned()],
            vec!["ana".to_owned(), "@example.com".to_owned()],
            vec!["ana".to_owned(), "@example".to_owned(), ".com".to_owned()],
        ],
        "{seen:?}"
    );
}

#[test]
fn an_address_breaks_the_same_way_whenever_its_box_returns_to_a_width() {
    // One page whose box follows the viewport, as a box animating its width does.
    let source = crate::session::tests::source(
        "function render(){return h('view',{},h('text',{style:{userSelect:'text',fontSize:16,width:'100%'}},'Mail ana@example.com now'))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let mut lines_at = |width: u16| {
        let display = page.layout(width, 400, 1.0).unwrap();
        let paragraph = display
            .retained
            .selectables
            .iter()
            .flatten()
            .next()
            .unwrap();
        paragraph
            .layout
            .lines()
            .map(|line| paragraph.text[line.text_range()].to_owned())
            .collect::<Vec<_>>()
    };
    for width in [70, 95, 130] {
        let first = lines_at(width);
        for other in [200, 60, width, 110, width] {
            let lines = lines_at(other);
            if other == width {
                assert_eq!(lines, first, "{width}");
            }
        }
    }
}

#[test]
fn a_long_url_breaks_after_slashes_and_before_dots() {
    let url = "https://docs.example.com/guides/getting-started/install";
    for width in [70.0, 110.0, 150.0, 220.0] {
        let lines = wrapped_lines(url, width);
        assert_eq!(lines.concat(), url, "{width}");
        assert!(lines.len() > 1, "{width}: {lines:?}");
        for (end, next) in mid_word_breaks(&lines) {
            assert!(
                (end == '/' && next != '/') || end == '-' || next == '.',
                "{width}: {lines:?}"
            );
        }
    }
    // A two-letter domain ending is not left alone on its line: it moves with its name.
    let lines = wrapped_lines("ana@example.co", 100.0);
    assert_eq!(lines, ["ana", "@example.co"]);
}

fn lines_with(text: &str, width: f32, style: &str) -> Vec<String> {
    let source = crate::session::tests::source(&format!(
        "function render(){{return h('view',{{}},h('text',{{style:{{userSelect:'text',fontSize:16,width:{width}{style}}}}},'{text}'))}}"
    ));
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(400, 400, 1.0).unwrap();
    let paragraph = display
        .retained
        .selectables
        .iter()
        .flatten()
        .next()
        .unwrap();
    paragraph
        .layout
        .lines()
        .map(|line| paragraph.text[line.text_range()].to_owned())
        .collect()
}

#[test]
fn no_break_characters_hold_their_neighbours_on_one_line() {
    // The joiners reach the line breaker as written, never as a plain space.
    for (escape, joiner) in [
        ("\\u00a0", '\u{a0}'),
        ("\\u202f", '\u{202f}'),
        ("\\u2060", '\u{2060}'),
        ("\\u2011", '\u{2011}'),
    ] {
        let lines = wrapped_lines(&format!("Uptime 14{escape}days 6 hr and more"), 90.0);
        let held = format!("14{joiner}days");
        assert!(
            lines.iter().any(|line| line.contains(&held)),
            "{escape}: {lines:?}"
        );
        assert!(
            lines.len() > 1,
            "{escape}: the width must force a break: {lines:?}"
        );
    }
}

#[test]
fn a_soft_hyphen_is_a_break_only_where_the_line_needs_one() {
    let wide = wrapped_lines("extra\\u00adordinary", 300.0);
    assert_eq!(wide, ["extra\u{ad}ordinary"]);
    let narrow = wrapped_lines("extra\\u00adordinary", 60.0);
    assert_eq!(
        narrow.first().map(String::as_str),
        Some("extra\u{ad}"),
        "{narrow:?}"
    );
}

#[test]
fn a_newline_collapses_unless_white_space_keeps_it() {
    // CSS's default white-space: a newline is a space; `pre-line` keeps it.
    assert_eq!(
        lines_with("Firmware 3.4.2\\nUptime", 300.0, ""),
        ["Firmware 3.4.2 Uptime"]
    );
    let kept = lines_with("Firmware 3.4.2\\nUptime", 300.0, ",whiteSpace:'pre-line'");
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert!(
        kept[0].starts_with("Firmware 3.4.2") && kept[1] == "Uptime",
        "{kept:?}"
    );
}

#[test]
fn collapsing_white_space_leaves_no_break_spaces_alone() {
    use crate::text::style::Collapse;
    assert_eq!(
        Collapse::All.apply("a \u{a0} b\u{202f}c\u{3000}d"),
        "a \u{a0} b\u{202f}c\u{3000}d"
    );
    assert_eq!(Collapse::Spaces.apply("14\u{a0}\ndays"), "14\u{a0}\ndays");
}

#[test]
fn a_no_break_space_at_a_line_end_stays_in_the_text() {
    // Only a collapsible space hangs at a break; a no-break space is text and is never trimmed.
    let lines = wrapped_lines("aaaa\\u00a0 bbbb", 50.0);
    assert_eq!(lines.concat(), "aaaa\u{a0} bbbb", "{lines:?}");
    assert!(lines[0].starts_with("aaaa\u{a0}"), "{lines:?}");
}
