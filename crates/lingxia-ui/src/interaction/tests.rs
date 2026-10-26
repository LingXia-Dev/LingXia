use crate::scene::test_prelude::*;

#[test]
fn zero_opacity_contains_input_decorations_without_removing_semantics() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{backgroundColor:'#ffffff',width:100,height:100}},h('input',{testId:'editor',value:'Hello',autoFocus:true,style:{opacity:0,width:100,height:60}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.layout(100, 100, 1.0).unwrap();
    let editor = display
        .nodes
        .iter()
        .find(|node| node.test_id == "editor")
        .unwrap();
    assert!(editor.input.as_ref().unwrap().geometry.is_some());
    assert!(editor.visible_bounds[2] > 0.0);
    let frame = display.rasterize().unwrap();
    assert!(
        frame
            .pixels
            .data_as_u8_slice()
            .chunks_exact(4)
            .all(|pixel| pixel == [255, 255, 255, 255])
    );
}

#[test]
fn borders_preserve_input_content_and_paint_after_editor_foreground() {
    let module = crate::session::tests::source(
        "function render(){return h('input',{autoFocus:true,value:'abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz',style:{width:160,height:56,padding:8,borderWidth:6,borderColor:'#ff0000',borderRadius:0,backgroundColor:'#ffffff'}})}",
    );
    for scale in [1.0, 2.0] {
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page
            .layout((200.0 * scale) as u16, (100.0 * scale) as u16, scale)
            .unwrap();
        let geometry = display.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap();
        assert_eq!(geometry.content_bounds, [14.0, 14.0, 132.0, 28.0]);
        assert_eq!(
            display.nodes[0].content_clip.unwrap().bounds,
            [6.0, 6.0, 148.0, 44.0]
        );
        let frame = display.rasterize().unwrap();
        for (x, y) in [(2.0, 28.0), (158.0, 28.0), (80.0, 2.0), (80.0, 54.0)] {
            let offset =
                ((y * scale) as usize * (200.0 * scale) as usize + (x * scale) as usize) * 4;
            assert_eq!(
                &frame.pixels.data_as_u8_slice()[offset..offset + 4],
                [255, 0, 0, 255]
            );
        }
    }
}

#[test]
fn aligned_single_line_editors_keep_caret_and_hit_geometry_together() {
    for tag in ["input", "search"] {
        for alignment in ["left", "center", "right"] {
            for value in ["MMM", "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM"] {
                let source = crate::session::tests::source(&format!(
                    "function render(){{return h('{tag}',{{autoFocus:true,value:'{value}',style:{{width:180,height:56,padding:8,borderWidth:2,textAlign:'{alignment}'}}}})}}"
                ));
                for scale in [1.0, 2.0] {
                    let mut page = crate::ViewSession::new(&source).unwrap();
                    let display = page
                        .prepare_native(
                            (200.0 * scale) as u16,
                            (100.0 * scale) as u16,
                            scale,
                            true,
                            None,
                            Ok,
                        )
                        .unwrap();
                    let node = &display.nodes[0];
                    let layout = &display.retained.inputs[0].as_ref().unwrap().layout;
                    assert_eq!(layout.len(), 1);
                    let input = node.input.as_ref().unwrap();
                    let geometry = input.geometry.as_ref().unwrap();
                    let caret = geometry.caret;
                    assert!(caret[0] >= geometry.content_bounds[0] - 0.1);
                    assert!(caret[0] <= geometry.content_bounds[0] + geometry.content_bounds[2]);
                    assert_eq!(
                        page.hit_test_input(
                            page.generation(),
                            &node.identity,
                            [caret[0], caret[1] + caret[3] / 2.0],
                            false
                        )
                        .unwrap(),
                        Some([value.len() as u32, value.len() as u32])
                    );
                    if value.len() == 3 {
                        let offset = layout.lines().next().unwrap().metrics().offset;
                        // Right-aligned text still reserves the visible caret width.
                        assert!(input.scroll_offset[0] <= 1.5);
                        if alignment == "left" {
                            assert_eq!(offset, 0.0);
                        } else {
                            assert!(offset > 20.0);
                        }
                    } else {
                        assert!(input.scroll_offset[0] > 0.0);
                    }
                }
            }
        }
    }
}

#[test]
fn search_keeps_icon_fixed_while_text_selection_and_caret_scroll() {
    use crate::text::input::Edit;
    let module = crate::session::tests::source(
        "function render(){return h('search',{value:'A long search query that must scroll',autoFocus:true,style:{fontSize:18,backgroundColor:'#ffffff',color:'#18243a'}})}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let first = page.layout(180, 80, 1.0).unwrap();
    let input = first.nodes[0].input.as_ref().unwrap();
    assert!(input.scroll_offset[0] > 0.0);
    assert!(input.geometry.as_ref().unwrap().caret[0] >= 44.0);
    let token = input.edit_token.unwrap();
    let id = first.nodes[0].identity.clone();
    let end = input.edit.as_ref().unwrap().selection[1];
    let baseline = first.rasterize().unwrap().pixels;
    page.edit_input(
        token,
        &id,
        &[Edit::Selection {
            anchor: end,
            focus: 0,
        }],
        false,
    )
    .unwrap()
    .unwrap();
    let selected = page.prepare_native(180, 80, 1.0, false, None, Ok).unwrap();
    let input = selected.nodes[0].input.as_ref().unwrap();
    assert_eq!(input.scroll_offset[0], 0.0);
    assert_eq!(input.geometry.as_ref().unwrap().caret[0], 44.0);
    let hits = InputHitMap::capture(&selected);
    let pixels = selected.rasterize().unwrap().pixels;
    let mut ink = 0;
    for y in 0..48 {
        for x in 0..44 {
            let offset = (y * 180 + x) * 4;
            let before = &baseline.data_as_u8_slice()[offset..offset + 4];
            assert_eq!(before, &pixels.data_as_u8_slice()[offset..offset + 4]);
            if (12..36).contains(&x) && (12..36).contains(&y) && before != [255, 255, 255, 255] {
                ink += 1;
            }
        }
    }
    assert!(
        ink > 30,
        "magnifier must be visible outside the editable content clip"
    );
    assert_eq!(hits.hit(&id, [15.0, 24.0], false), Some([0, 0]));
}

#[test]
fn letter_spacing_inherits_resets_and_changes_input_geometry() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{fontFamily:'monospace',fontSize:20,letterSpacing:4}},h('input',{value:'ABCD',style:{height:50}}),h('input',{value:'ABCD',style:{height:50,letterSpacing:0}}),h('input',{value:'ABCD',style:{height:50,letterSpacing:-1}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let frame = page.layout(320, 200, 1.0).unwrap();
    let widths: Vec<_> = [1, 2, 3]
        .map(|i| frame.retained.inputs[i].as_ref().unwrap().layout.width())
        .to_vec();
    assert!(widths[0] > widths[1] + 10.0);
    assert!(widths[2] < widths[1] - 2.0);
}
#[test]
fn styled_textarea_caret_and_hits_share_the_retained_line_spacing() {
    use crate::text::input::Edit;
    let module = crate::session::tests::source(
        "function render(){return h('textarea',{value:'one\\ntwo\\nthree',autoFocus:true,style:{height:160,padding:8,fontFamily:'monospace',fontWeight:700,fontSize:18,lineHeight:'36px'}})}",
    );
    for scale in [1.0, 2.0] {
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page
            .prepare_native(
                (300.0 * scale) as u16,
                (200.0 * scale) as u16,
                scale,
                true,
                None,
                Ok,
            )
            .unwrap();
        let id = first.nodes[0].identity.clone();
        let token = first.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
        page.edit_input(
            token,
            &id,
            &[Edit::Selection {
                anchor: 4,
                focus: 4,
            }],
            false,
        )
        .unwrap();
        let frame = page
            .prepare_native(
                (300.0 * scale) as u16,
                (200.0 * scale) as u16,
                scale,
                false,
                None,
                Ok,
            )
            .unwrap();
        let layout = &frame.retained.inputs[0].as_ref().unwrap().layout;
        assert_eq!(layout.height(), 108.0);
        let lines: Vec<_> = layout.lines().collect();
        assert!((lines[1].metrics().baseline - lines[0].metrics().baseline - 36.0).abs() < 0.01);
        let caret = frame.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap()
            .caret;
        assert_eq!(
            page.hit_test_input(
                page.generation(),
                &id,
                [caret[0], caret[1] + caret[3] / 2.0],
                false
            )
            .unwrap(),
            Some([4, 4])
        );
    }
}

#[test]
fn asymmetric_input_padding_keeps_caret_hits_and_search_reserve_consistent() {
    for (tag, reserve) in [("input", 0.0), ("textarea", 0.0), ("search", 32.0)] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('{tag}',{{value:'abc',autoFocus:true,style:{{width:200,height:80,paddingTop:10,paddingRight:20,paddingBottom:5,paddingLeft:30}}}})}}",
        ));
        for scale in [1.0, 2.0] {
            let mut page = crate::ViewSession::new(&module).unwrap();
            let frame = page
                .layout((240.0 * scale) as u16, (100.0 * scale) as u16, scale)
                .unwrap();
            let shaped = frame.retained.inputs[0].as_ref().unwrap();
            assert_eq!(shaped.padding, [30.0 + reserve, 10.0]);
            assert_eq!(shaped.content_size, [150.0 - reserve, 65.0]);
            let node = &frame.nodes[0];
            let caret = node
                .input
                .as_ref()
                .unwrap()
                .geometry
                .as_ref()
                .unwrap()
                .caret;
            assert!(caret[0] > 30.0 + reserve);
            assert!(caret[1] >= 10.0);
            if tag != "textarea" {
                assert!((caret[1] + caret[3] / 2.0 - 42.5).abs() < 0.01);
            } else {
                assert_eq!(shaped.text_offset_y, 0.0);
            }
            assert_eq!(
                page.hit_test_input(
                    page.generation(),
                    &node.identity,
                    [caret[0], caret[1] + caret[3] / 2.0],
                    false
                )
                .unwrap(),
                Some([3, 3])
            );
        }
    }
}

#[test]
fn selection_endpoints_preserve_logical_direction_across_display_mappings() {
    use crate::text::input::Edit;
    for (tag, value, secure) in [
        ("input", "a\u{1f600}z", false),
        ("input", "a\u{1f600}z", true),
        ("input", "ab\r\ncd", false),
        ("textarea", "first\nsecond", false),
        ("input", "abc \u{5d0}\u{5d1}\u{5d2} def", false),
    ] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('{tag}',{{value:{},secure:{secure},autoFocus:true,style:{{fontSize:20,padding:8}}}})}}",
            serde_json::to_string(value).unwrap()
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(400, 160, 1.0).unwrap();
        let node = &first.nodes[0];
        let input = node.input.as_ref().unwrap();
        let initial = input.geometry.as_ref().unwrap();
        assert_eq!(initial.anchor, initial.caret);
        let token = input.edit_token.unwrap();
        let end = value.encode_utf16().count() as u32;
        page.edit_input(
            token,
            &node.identity,
            &[Edit::Selection {
                anchor: 0,
                focus: end,
            }],
            false,
        )
        .unwrap();
        let forward = page.prepare_native(400, 160, 1.0, false, None, Ok).unwrap();
        let forward = forward.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .clone()
            .unwrap();
        page.edit_input(
            token,
            &node.identity,
            &[Edit::Selection {
                anchor: end,
                focus: 0,
            }],
            false,
        )
        .unwrap();
        let reverse = page.prepare_native(800, 320, 2.0, false, None, Ok).unwrap();
        let reverse = reverse.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap();
        assert_eq!(forward.anchor, reverse.caret, "{tag}, {secure}");
        assert_eq!(forward.caret, reverse.anchor, "{tag}, {secure}");
        assert_eq!(forward.content_bounds, reverse.content_bounds);
        assert_eq!(forward.selection, reverse.selection);
    }
}

#[test]
fn endpoint_geometry_scrolls_with_text_without_pinning_offscreen_handles() {
    use crate::text::input::Edit;
    for (tag, value, axis) in [
        (
            "input",
            "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
            0,
        ),
        ("textarea", "one\ntwo\nthree\nfour\nfive\nsix", 1),
    ] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('{tag}',{{value:{},autoFocus:true,style:{{height:60,fontSize:20,padding:8}}}})}}",
            serde_json::to_string(value).unwrap()
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(150, 80, 1.0).unwrap();
        let id = first.nodes[0].identity.clone();
        let token = first.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
        let end = value.encode_utf16().count() as u32;
        page.edit_input(
            token,
            &id,
            &[Edit::Selection {
                anchor: 0,
                focus: end,
            }],
            false,
        )
        .unwrap();
        let forward = page.prepare_native(150, 80, 1.0, false, None, Ok).unwrap();
        let a = forward.nodes[0].input.as_ref().unwrap();
        let ga = a.geometry.as_ref().unwrap();
        assert!(ga.anchor[axis] < ga.content_bounds[axis]);
        page.edit_input(
            token,
            &id,
            &[Edit::Selection {
                anchor: end,
                focus: 0,
            }],
            false,
        )
        .unwrap();
        let reverse = page.prepare_native(150, 80, 1.0, false, None, Ok).unwrap();
        let b = reverse.nodes[0].input.as_ref().unwrap();
        let gb = b.geometry.as_ref().unwrap();
        assert!(gb.anchor[axis] > gb.content_bounds[axis] + gb.content_bounds[axis + 2]);
        assert_eq!(ga.content_bounds, gb.content_bounds);
        assert!(
            (ga.anchor[axis] + a.scroll_offset[axis] - gb.caret[axis] - b.scroll_offset[axis])
                .abs()
                < 0.01
        );
        assert!(
            (ga.caret[axis] + a.scroll_offset[axis] - gb.anchor[axis] - b.scroll_offset[axis])
                .abs()
                < 0.01
        );
    }
}

#[test]
fn displayed_offsets_round_trip_without_splitting_unicode() {
    let value = "a\u{1f600}\r\n\u{4f60}e\u{301}";
    for secure in [false, true] {
        for single in [false, true] {
            for offset in 0..=value.encode_utf16().count() as u32 {
                if let Some(byte) = display_byte(value, offset, secure, single) {
                    assert_eq!(display_offset(value, byte, secure, single), Some(offset));
                } else {
                    assert_eq!(offset, 2);
                }
            }
            assert_eq!(display_offset(value, usize::MAX, secure, single), None);
        }
    }
}

#[test]
fn drag_edges_reach_both_ends_through_presented_shaped_cursors() {
    use crate::text::input::Edit;
    for (tag, value, secure, axis) in [
        (
            "input",
            "WiiiiWiiiiWiiiiWiiiiWiiiiWiiiiWiiiiWiiii",
            false,
            0,
        ),
        ("input", "abc\u{1f600}defghijklmnopqrstuvwxyz", true, 0),
        (
            "textarea",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight",
            false,
            1,
        ),
    ] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('{tag}',{{value:{},secure:{secure},autoFocus:true,style:{{height:60,fontSize:20,padding:8}}}})}}",
            serde_json::to_string(value).unwrap()
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let mut display = page.prepare_native(150, 80, 1.0, true, None, Ok).unwrap();
        let id = display.nodes[0].identity.clone();
        let token = display.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
        let end = value.encode_utf16().count() as u32;
        for direction in [-1, 1] {
            let target = if direction < 0 { 0 } else { end };
            let mut last = if direction < 0 { end } else { 0 };
            for _ in 0..100 {
                let input = display.nodes[0].input.as_ref().unwrap();
                let bounds = input.geometry.as_ref().unwrap().content_bounds;
                let mut point = [bounds[0] + 0.5, bounds[1] + 0.5];
                if direction > 0 {
                    point = [bounds[0] + bounds[2] - 0.5, bounds[1] + bounds[3] - 0.5];
                }
                let mut edge = [0, 0];
                edge[axis] = direction;
                let generation = page.generation();
                assert_eq!(
                    page.drag_hit_test_input(generation, &id, token + 1, point, edge)
                        .unwrap(),
                    None
                );
                assert_eq!(
                    page.drag_hit_test_input(generation, &id, token, point, [2, 0])
                        .unwrap(),
                    None
                );
                let next = page
                    .drag_hit_test_input(generation, &id, token, point, edge)
                    .unwrap()
                    .unwrap()[1];
                if last == target {
                    assert_eq!(next, target);
                    break;
                }
                assert!(
                    if direction < 0 {
                        next < last
                    } else {
                        next > last
                    },
                    "{tag} secure={secure}: {last} -> {next}"
                );
                page.edit_input(
                    token,
                    &id,
                    &[Edit::Selection {
                        anchor: 0,
                        focus: next,
                    }],
                    false,
                )
                .unwrap();
                assert_eq!(
                    page.drag_hit_test_input(generation, &id, token, point, edge)
                        .unwrap(),
                    None
                );
                display = page.prepare_native(150, 80, 1.0, false, None, Ok).unwrap();
                last = next;
            }
            assert_eq!(last, target, "{tag}, secure={secure}");
        }
    }
}

#[test]
fn input_hit_testing_uses_displayed_scroll_and_mask_mapping() {
    for (tag, value, secure) in [
        ("input", "abc\u{1f600}defghijklmnopqrstuvwxyz", false),
        ("input", "abc\u{1f600}defghijklmnopqrstuvwxyz", true),
        ("textarea", "one\ntwo\nthree\nfour\nfive\nsix", false),
    ] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('{tag}',{{value:{},secure:{secure},autoFocus:true,style:{{height:60,fontSize:20,padding:8}}}})}}",
            serde_json::to_string(value).unwrap()
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.prepare_native(150, 80, 1.0, true, None, Ok).unwrap();
        let node = &display.nodes[0];
        let input = node.input.as_ref().unwrap();
        let caret = input.geometry.as_ref().unwrap().caret;
        let point = [caret[0], caret[1] + caret[3] / 2.0];
        let hits = InputHitMap::capture(&display);
        let end = value.encode_utf16().count() as u32;
        assert_eq!(
            hits.hit(&node.identity, point, false),
            Some([end, end]),
            "{tag}, secure={secure}"
        );
        assert_eq!(hits.hit(&node.identity, [-1.0, point[1]], false), None);
        assert_eq!(hits.hit(&node.identity, [f32::NAN, point[1]], false), None);
        assert_eq!(hits.hit("missing", point, false), None);
        if secure {
            assert_eq!(hits.hit(&node.identity, point, true), Some([0, end]));
        }
    }
}

#[test]
fn placeholder_tint_is_local_and_does_not_recolor_value_or_caret() {
    for tag in ["input", "textarea", "search"] {
        for value in ["", "typed", " "] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('{tag}',{{value:'{value}',placeholder:'Hint',style:{{color:'#0000ff',placeholderColor:'#ff000080'}}}})}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let display = page.layout(200, 80, 1.0).unwrap();
            let expected = if value.is_empty() {
                [255, 0, 0, 128]
            } else {
                [0, 0, 255, 255]
            };
            let paints: Vec<_> = display
                .commands
                .iter()
                .filter_map(|p| match p {
                    Paint::Text { color, .. } => Some(*color),
                    _ => None,
                })
                .collect();
            assert!(!paints.is_empty());
            assert!(paints.iter().all(|c| *c == expected), "{tag}: {value:?}");
            assert_eq!(
                display.retained.inputs[0].as_ref().unwrap().color,
                [0, 0, 255, 255]
            );
        }
    }
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{color:'#0000ff'}},h('input',{placeholder:'Hint'}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(200, 80, 1.0).unwrap();
    assert!(
        display
            .commands
            .iter()
            .filter_map(|p| match p {
                Paint::Text { color, .. } => Some(color),
                _ => None,
            })
            .all(|c| *c == [0, 0, 255, 255])
    );
}

#[test]
fn word_hit_testing_and_empty_placeholder_do_not_use_character_width_estimates() {
    let module = crate::session::tests::source(
        "function render(){return h('column',{},h('input',{value:'hello world',style:{height:40,fontSize:20,padding:8}}),h('input',{placeholder:'not editable text',style:{height:40}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let display = page.prepare_native(300, 100, 1.0, true, None, Ok).unwrap();
    let hits = InputHitMap::capture(&display);
    let node = &display.nodes[1];
    let selected = hits
        .hit(
            &node.identity,
            [node.bounds[0] + 12.0, node.bounds[1] + 20.0],
            true,
        )
        .unwrap();
    assert_eq!(selected, [0, 5]);
    let empty = &display.nodes[2];
    assert_eq!(
        hits.hit(
            &empty.identity,
            [empty.bounds[0] + 100.0, empty.bounds[1] + 20.0],
            false
        ),
        Some([0, 0])
    );
}

#[test]
fn blink_hides_only_caret_without_changing_editor_or_rerunning_view() {
    let module = crate::session::tests::source(
        "let renders=0;function render(){renders++;return h('column',{},h('input',{value:'hello',autoFocus:true}),h('text',{},String(renders)))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let first = page.layout(200, 100, 1.0).unwrap();
    let id = first.nodes[1].identity.clone();
    let token = first.nodes[1].input.as_ref().unwrap().edit_token.unwrap();
    page.edit_input(
        token,
        &id,
        &[crate::text::input::Edit::Composition { start: 0, end: 5 }],
        false,
    )
    .unwrap()
    .unwrap();
    let visible = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
    let geometry = visible.nodes[1]
        .input
        .as_ref()
        .unwrap()
        .geometry
        .clone()
        .unwrap();
    let mut hidden = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
    assert_eq!(hidden.nodes[2].text, "1");
    assert_eq!(
        hidden.nodes[1]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap()
            .composition,
        geometry.composition
    );
    hidden.hide_caret();
    hidden.hide_caret();
    let on = visible.rasterize().unwrap().pixels;
    let off = hidden.rasterize().unwrap().pixels;
    let mut changed = 0;
    for (index, (a, b)) in on
        .data_as_u8_slice()
        .chunks_exact(4)
        .zip(off.data_as_u8_slice().chunks_exact(4))
        .enumerate()
    {
        if a == b {
            continue;
        }
        changed += 1;
        let x = (index % 200) as f32;
        let y = (index / 200) as f32;
        assert!(
            x >= geometry.caret[0].floor() && x < (geometry.caret[0] + geometry.caret[2]).ceil()
        );
        assert!(
            y >= geometry.caret[1].floor() && y < (geometry.caret[1] + geometry.caret[3]).ceil()
        );
    }
    assert!(changed > 5);
    let resumed = page.prepare_native(200, 100, 1.0, false, None, Ok).unwrap();
    assert_eq!(resumed.nodes[2].text, "1");
    assert!(resumed.caret_paint.is_some());
}

#[test]
fn long_inputs_reveal_selection_focus_without_resetting_the_viewport() {
    use crate::text::input::Edit;
    for (tag, value, axis) in [
        ("input", "abcdefghij".repeat(12), 0),
        ("textarea", "line\\n".repeat(12) + "last", 1),
    ] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('{tag}',{{value:'{value}',autoFocus:true,style:{{height:100,fontSize:20,backgroundColor:'#ffffff'}}}})}}"
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(180, 100, 1.0).unwrap();
        let input = first.nodes[0].input.as_ref().unwrap();
        let token = input.edit_token.unwrap();
        let id = first.nodes[0].identity.clone();
        let original_offset = input.scroll_offset;
        assert!(original_offset[axis] > 100.0, "{original_offset:?}");
        let caret = input.geometry.as_ref().unwrap().caret;
        assert!(
            caret[0] >= 12.0 && caret[0] + caret[2] <= 168.01,
            "{caret:?}"
        );
        assert!(
            caret[1] >= 12.0 && caret[1] + caret[3] <= 88.01,
            "{caret:?}"
        );
        let old_glyph = *first
            .commands
            .iter()
            .find_map(|p| match p {
                Paint::Text { glyphs, .. } => glyphs.first(),
                _ => None,
            })
            .unwrap();
        // Selecting toward the beginning must reveal the focus, not the anchor.
        let end = input.edit.as_ref().unwrap().selection[1];
        page.edit_input(
            token,
            &id,
            &[Edit::Selection {
                anchor: end,
                focus: 0,
            }],
            false,
        )
        .unwrap()
        .unwrap();
        let failed = page.prepare_native(180, 100, 1.0, false, None, |_| {
            Err::<(), _>("surface unavailable".into())
        });
        assert!(failed.is_err());
        let restored = page.prepare_native(180, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(
            restored.nodes[0].input.as_ref().unwrap().scroll_offset,
            original_offset
        );
        let token = restored.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .edit_token
            .unwrap();
        page.edit_input(
            token,
            &id,
            &[Edit::Selection {
                anchor: end,
                focus: 0,
            }],
            false,
        )
        .unwrap()
        .unwrap();
        let selected = page.prepare_native(180, 100, 1.0, false, None, Ok).unwrap();
        let selected_input = selected.nodes[0].input.as_ref().unwrap();
        assert_eq!(selected_input.scroll_offset, [0.0; 2]);
        let glyph = selected
            .commands
            .iter()
            .find_map(|p| match p {
                Paint::Text { glyphs, .. } => glyphs.first(),
                _ => None,
            })
            .unwrap();
        let delta = if axis == 0 {
            glyph.x - old_glyph.x
        } else {
            glyph.y - old_glyph.y
        };
        assert!((delta - original_offset[axis]).abs() < 0.01);
        let pixels = selected.rasterize().unwrap().pixels;
        // Selection and scrolled text must not cover the input padding.
        assert_eq!(
            &pixels.data_as_u8_slice()[(50 * 180 + 5) * 4..(50 * 180 + 5) * 4 + 3],
            &[255, 255, 255]
        );
        page.edit_input(
            token,
            &id,
            &[Edit::Replace {
                text: "x".into(),
                cursor: 1,
                composing: false,
            }],
            false,
        )
        .unwrap()
        .unwrap();
        let shortened = page.prepare_native(180, 100, 1.0, false, None, Ok).unwrap();
        assert_eq!(
            shortened.nodes[0].input.as_ref().unwrap().scroll_offset,
            [0.0; 2]
        );
    }
}

#[test]
fn displayed_offsets_preserve_unicode_and_password_boundaries() {
    let text = "a\u{1f600}\n\u{2028}z";
    assert_eq!(display_byte(text, 2, false, true), None);
    assert_eq!(display_byte(text, 3, false, true), Some(5));
    assert_eq!(display_byte(text, 5, false, true), Some(7));
    assert_eq!(display_byte(text, 5, false, false), Some(9));
    assert_eq!(display_byte(text, 3, true, true), Some(6));
    assert_eq!(display_byte(text, 6, true, true), Some(15));
    assert_eq!(display_byte(text, 7, true, true), None);
}

#[test]
fn input_geometry_uses_displayed_font_and_paints_selection_below_text() {
    use crate::text::input::Edit;
    let module = crate::session::tests::source(
        "function render(){return h('input',{value:'Hello world',autoFocus:true,style:{fontSize:32,color:'#000000',backgroundColor:'#ffffff'}})}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let first = page.layout(300, 80, 1.0).unwrap();
    let input = first.nodes[0].input.as_ref().unwrap();
    let caret = input.geometry.as_ref().unwrap().caret;
    assert!(caret[0] > 100.0 && caret[3] > 25.0, "{caret:?}");
    let token = input.edit_token.unwrap();
    let id = first.nodes[0].identity.clone();
    page.edit_input(
        token,
        &id,
        &[
            Edit::Selection {
                anchor: 0,
                focus: 5,
            },
            Edit::Composition { start: 0, end: 5 },
        ],
        false,
    )
    .unwrap()
    .unwrap();
    let selected = page.prepare_native(300, 80, 1.0, false, None, Ok).unwrap();
    let geometry = selected.nodes[0]
        .input
        .as_ref()
        .unwrap()
        .geometry
        .as_ref()
        .unwrap();
    assert_eq!(geometry.selection.len(), 1);
    assert_eq!(geometry.composition.len(), 1);
    assert!(geometry.selection[0][2] > 50.0);
    assert_eq!(geometry.composition[0][3], 1.0);
    let selection_paint = selected
        .commands
        .iter()
        .position(|p| {
            matches!(
                p,
                Paint::Rect { color, .. }
                    if *color == crate::theme::Palette::builtin(false)
                        .role(lingxia_theme::Role::AccentSoft)
            )
        })
        .unwrap();
    let glyph_paint = selected
        .commands
        .iter()
        .position(|p| matches!(p, Paint::Text { .. }))
        .unwrap();
    assert!(selection_paint < glyph_paint);
    selected.rasterize().unwrap();
    page.blur_input(token, &id).unwrap();
    let blurred = page.prepare_native(300, 80, 1.0, false, None, Ok).unwrap();
    assert!(blurred.nodes[0].input.as_ref().unwrap().geometry.is_none());
}

#[test]
fn empty_and_secure_inputs_keep_a_caret_and_multiline_selection_wraps() {
    use crate::text::input::Edit;
    for (tag, value, extra) in [
        ("input", "", "placeholder:'Type here',"),
        ("input", "a\\u{1f600}z", "secure:true,"),
        ("textarea", "first line\\nsecond line", ""),
    ] {
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('{tag}',{{value:'{value}',{extra}autoFocus:true,style:{{fontSize:24}}}})}}"
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let first = page.layout(300, 150, 1.0).unwrap();
        let input = first.nodes[0].input.as_ref().unwrap();
        let caret = input.geometry.as_ref().unwrap().caret;
        assert!(caret.iter().all(|v| v.is_finite()) && caret[3] > 20.0);
        let end = input.edit.as_ref().unwrap().selection[1];
        let token = input.edit_token.unwrap();
        let id = first.nodes[0].identity.clone();
        page.edit_input(
            token,
            &id,
            &[Edit::Selection {
                anchor: end,
                focus: 0,
            }],
            false,
        )
        .unwrap()
        .unwrap();
        let selected = page.prepare_native(300, 150, 1.0, false, None, Ok).unwrap();
        let geometry = selected.nodes[0]
            .input
            .as_ref()
            .unwrap()
            .geometry
            .as_ref()
            .unwrap();
        assert_eq!(
            geometry.selection.len(),
            if tag == "textarea" {
                2
            } else if value.is_empty() {
                0
            } else {
                1
            }
        );
        assert!(geometry.caret[0] <= caret[0]);
        selected.rasterize().unwrap();
    }
}

#[test]
fn caret_moves_with_retained_scroll_and_disappears_when_hidden() {
    let module = crate::session::tests::source(
        "function render(){return h('scroll',{style:{borderRadius:20}},h('input',{value:'hello',autoFocus:true}),h('spacer',{style:{height:300}}))}",
    );
    let mut page = crate::ViewSession::new(&module).unwrap();
    let first = page.layout(200, 100, 1.0).unwrap();
    let geometry = first.nodes[1]
        .input
        .as_ref()
        .unwrap()
        .geometry
        .as_ref()
        .unwrap();
    let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
    let moved = page
        .prepare_native(
            200,
            100,
            1.0,
            false,
            Some(&HashMap::from([(id.clone(), 10.0)])),
            Ok,
        )
        .unwrap();
    let next = moved.nodes[1]
        .input
        .as_ref()
        .unwrap()
        .geometry
        .as_ref()
        .unwrap();
    for (before, after) in [
        (geometry.anchor, next.anchor),
        (geometry.caret, next.caret),
        (geometry.content_bounds, next.content_bounds),
    ] {
        assert_eq!(after[0], before[0]);
        assert!((after[1] - before[1] + 10.0).abs() < 0.01);
    }
    assert_eq!(
        moved.nodes[1].clips[0].bounds,
        first.nodes[1].clips[0].bounds
    );
    let hidden = page
        .prepare_native(
            200,
            100,
            1.0,
            false,
            Some(&HashMap::from([(id, 100.0)])),
            Ok,
        )
        .unwrap();
    assert!(hidden.nodes[1].input.as_ref().unwrap().geometry.is_none());
}

#[test]
fn scroll_clips_paint_and_input_geometry_and_clamps_offsets() {
    let mut renderer = Renderer::default();
    let initial = renderer.layout(&page(), 100, 100, 1.0).unwrap();
    let scroll = initial.nodes[2].scroll.as_ref().unwrap();
    assert_eq!(scroll.max_offset, 40.0);
    let offsets = HashMap::from([(scroll.id.clone(), 999.0)]);
    let scrolled = renderer
        .layout_scrolled(&page(), 100, 100, 1.0, &offsets)
        .unwrap();
    assert_eq!(scrolled.nodes[2].scroll.as_ref().unwrap().offset, 40.0);
    assert_eq!(scrolled.nodes[3].bounds, [0.0, -20.0, 100.0, 50.0]);
    assert_eq!(scrolled.nodes[3].visible_bounds, [0.0, 20.0, 100.0, 10.0]);
    assert_eq!(scrolled.nodes[4].visible_bounds, [0.0, 30.0, 100.0, 50.0]);
    let frame = scrolled.rasterize().unwrap();
    let pixels = frame.pixels.data();
    assert_eq!(pixels[10 * 100 + 50], pixels[90 * 100 + 50]);
    assert_ne!(pixels[25 * 100 + 50], pixels[35 * 100 + 50]);
    assert_ne!(pixels[35 * 100 + 50], pixels[90 * 100 + 50]);
    assert!(
        renderer
            .layout_scrolled(
                &page(),
                100,
                100,
                1.0,
                &HashMap::from([("bad".into(), f32::NAN)])
            )
            .is_err()
    );
}

#[test]
fn rebound_controls_match_fresh_geometry_paint_and_event_handles() {
    let records = vec![
        node(0, None, "root", &[]),
        node(
            8,
            Some(0),
            "editor",
            &[(21, json!("text")), (26, json!(10)), (27, json!(11))],
        ),
        node(10, Some(0), "check", &[(39, json!(0)), (40, json!(12))]),
        node(
            15,
            Some(0),
            "slider",
            &[(41, json!("Volume")), (42, json!(3)), (46, json!(13))],
        ),
        node(
            5,
            Some(0),
            "scroll",
            &[(1, json!(80)), (17, json!(14)), (18, json!(15))],
        ),
        node(3, Some(4), "button", &[(8, json!(16))]),
        node(
            18,
            Some(0),
            "plot",
            &[
                (1, json!(100)),
                (41, json!("Series")),
                (51, json!("[10,20]")),
                (52, json!("line")),
                (53, json!("[\"#123456\"]")),
                (56, json!(17)),
                (57, json!(-1)),
            ],
        ),
    ];
    let mut renderer = Renderer::default();
    let first = renderer.layout(&records, 240, 400, 1.0).unwrap();
    let mut changed = records.clone();
    for record in &mut changed {
        for (id, value) in &mut record.fields {
            if matches!(*id, 8 | 17 | 18 | 26 | 27 | 40 | 46 | 56) {
                *value = json!(value.as_u64().unwrap() + 100);
            }
        }
    }
    let rebound = first
        .retained
        .rebind(&records, &changed)
        .unwrap()
        .compose(&HashMap::new())
        .unwrap();
    let fresh = renderer.layout(&changed, 240, 400, 1.0).unwrap();
    assert_eq!(
        serde_json::to_value(&rebound.nodes).unwrap(),
        serde_json::to_value(&fresh.nodes).unwrap()
    );
    assert_eq!(
        rebound.rasterize().unwrap().pixels.data_as_u8_slice(),
        fresh.rasterize().unwrap().pixels.data_as_u8_slice()
    );
    for (index, field, value) in [
        (1, 21, json!("edited")),
        (2, 39, json!(1)),
        (3, 42, json!(6)),
        (5, 9, json!(0)),
        (6, 51, json!("[20,10]")),
        (6, 57, json!(1)),
        (0, 58, json!(0.5)),
        (0, 59, json!(8)),
        (0, 60, json!(8)),
        (0, 61, json!(8)),
        (0, 62, json!(8)),
    ] {
        let mut different = changed.clone();
        if let Some((_, old)) = different[index]
            .fields
            .iter_mut()
            .find(|(id, _)| *id == field)
        {
            *old = value;
        } else {
            different[index].fields.push((field, value));
        }
        assert!(first.retained.rebind(&records, &different).is_none());
    }
}
