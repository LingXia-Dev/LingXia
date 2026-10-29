use super::*;
#[test]
fn shared_selection_handles_categories_empty_slices_and_expanded_edges() {
    let mut plot = Plot::from_record(
        &record("bar", "[10,20,30,40]"),
        &crate::theme::Palette::builtin(false),
    )
    .unwrap()
    .snapshot();
    plot.on_select = Some(1);
    let bounds = [10.0, 20.0, 100.0, 100.0];
    for (x, expected) in [(10.0, 0), (36.9, 0), (37.0, 1), (60.0, 2), (109.9, 3)] {
        assert_eq!(plot.hit_test(bounds, [x, 70.0]), Some(expected));
    }
    for point in [[110.0, 70.0], [60.0, 120.0], [f32::NAN, 70.0]] {
        assert_eq!(plot.hit_test(bounds, point), None);
    }
    plot.kind = "pie".into();
    plot.values = vec![0.0, 1.0, 0.0, 1.0];
    plot.pie_layout = PieLayout {
        center: [0.5, 0.5],
        inner_radius: 0.4,
        outer_radius: 0.8,
        emphasis_scale: 5.0,
    };
    plot.selected_index = 1;
    assert_eq!(plot.next_selection(), Some(3));
    assert_eq!(plot.hit_test(bounds, [103.0, 70.0]), Some(1));
    assert_eq!(plot.hit_test(bounds, [17.0, 70.0]), None);
    assert_eq!(plot.hit_test(bounds, [60.0, 70.0]), None);
    assert_eq!(plot.hit_test(bounds, [80.0, 70.0]), Some(1));
    plot.on_select = None;
    assert_eq!(plot.next_selection(), None);
    assert_eq!(plot.hit_test(bounds, [103.0, 70.0]), None);
}

#[test]
fn shared_plot_hit_uses_rendered_content_and_ancestor_clips() {
    let source = crate::session::tests::source(
        "function render(){return h('scroll',{style:{width:80,height:80,borderRadius:30}},h('plot',{kind:'bar',values:[1,2],colors:['#ff0000'],onSelect:()=>{},'aria-label':'Series',style:{width:100,height:100}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(120, 120, 1.0).unwrap();
    let node = display
        .nodes
        .iter()
        .find(|node| node.plot.is_some())
        .unwrap();
    assert_eq!(node.hit_test_plot([40.0, 40.0]), Some(0));
    assert_eq!(node.hit_test_plot([1.0, 1.0]), None);
    assert_eq!(node.hit_test_plot([90.0, 40.0]), None);
}

#[test]
fn shared_selection_dispatch_rejects_stale_frames_and_wrong_nodes() {
    use crate::PlotSelection::{Index, Next, Point};
    let source = crate::session::tests::source(
        "function render(){const selected=useLocal('selected',-1);return h('plot',{kind:'pie',values:[1,0,1],colors:['#0000ff'],'aria-label':'Series',selectedIndex:selected.value,onSelect:index=>selected.set(index),style:{width:100,height:100}})}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let frame = page.layout(100, 100, 1.0).unwrap();
    let identity = frame.nodes[0].identity.clone();
    let generation = page.generation();
    assert!(!page.select_plot(generation, "missing", Next).unwrap());
    assert!(!page.select_plot(generation, &identity, Index(1)).unwrap());
    assert!(!page.select_plot(generation, &identity, Index(-2)).unwrap());
    assert!(
        !page
            .select_plot(generation, &identity, Point([f32::NAN, 50.0]))
            .unwrap()
    );
    assert!(
        page.select_plot(generation, &identity, Point([90.0, 50.0]))
            .unwrap()
    );
    let frame = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(frame.nodes[0].plot.as_ref().unwrap().selected_index, 0);
    assert!(!page.select_plot(generation, &identity, Index(-1)).unwrap());
    assert!(
        page.select_plot(page.generation(), &identity, Next)
            .unwrap()
    );
    let frame = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(frame.nodes[0].plot.as_ref().unwrap().selected_index, 2);
    assert!(
        page.select_plot(page.generation(), &identity, Point([50.0, 50.0]))
            .unwrap()
    );
    let frame = page.layout(100, 100, 1.0).unwrap();
    assert_eq!(frame.nodes[0].plot.as_ref().unwrap().selected_index, -1);
    assert!(
        !page
            .select_plot(page.generation(), &identity, Index(-1))
            .unwrap()
    );
}

#[test]
fn marker_emphasis_preserves_stroke_and_changes_only_selected_circle() {
    let mut line_record = record("line", "[10,20]");
    line_record
        .fields
        .extend([(94, serde_json::json!(1.1)), (57, serde_json::json!(0))]);
    let plot = Plot::from_record(&line_record, &crate::theme::Palette::builtin(false)).unwrap();
    let shapes = plot.shapes([0.0, 0.0, 100.0, 100.0]);
    let diameters: Vec<f32> = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Circle(bounds, _) => Some(bounds[2]),
            _ => None,
        })
        .collect();
    for (actual, expected) in diameters.iter().zip([8.6, 4.6, 8.0, 4.0]) {
        assert!((actual - expected).abs() < 0.001);
    }
    assert_eq!(diameters.len(), 4);
    for kind in ["bar", "pie"] {
        let mut record = record(kind, "[10,20]");
        record.fields.push((94, serde_json::json!(1.1)));
        assert!(Plot::from_record(&record, &crate::theme::Palette::builtin(false)).is_err());
    }
}
#[test]
fn marker_emphasis_raster_expands_white_center_and_blue_ring() {
    for selected in [-1, 0, 1] {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('plot',{{kind:'line',values:[10,20],colors:['#0000ff'],selectedIndex:{selected},markerEmphasisScale:2,'aria-label':'Series',style:{{width:100,height:100,backgroundColor:'#ffffff'}}}})}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let frame = page.layout(100, 100, 1.0).unwrap().rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        let pixel = |x: usize| &pixels[(50 * 100 + x) * 4..(50 * 100 + x) * 4 + 4];
        let white = [255, 255, 255, 255];
        let (ring, outside) = if selected == 0 {
            (pixel(33), pixel(30))
        } else {
            (pixel(30), pixel(33))
        };
        assert!(ring[0] < 64 && ring[1] < 64 && ring[2] == 255 && ring[3] == 255);
        assert_eq!(outside, &white);
        assert_eq!(pixel(35), &white);
    }
    for (tag, value) in [
        (18, serde_json::json!(0.9)),
        (18, serde_json::json!(4.1)),
        (1, serde_json::json!(1.1)),
    ] {
        let wire = serde_json::json!([tag, -1, "", "", "", 1, 94, value]);
        assert!(crate::protocol::decode(wire.as_array().unwrap()).is_err());
    }
}
#[test]
fn expanded_pie_is_clipped_to_its_unbordered_plot_viewport() {
    let source = crate::session::tests::source(
        "function render(){return h('column',{style:{backgroundColor:'#ffffff',padding:20}},h('plot',{kind:'pie',values:[1,1],colors:['#ff0000','#0000ff'],selectedIndex:0,'aria-label':'Series',pieLayout:{center:[0.5,0.5],innerRadius:0.1,outerRadius:0.8,emphasisScale:64},style:{width:100,height:100}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(160, 160, 1.0).unwrap();
    let frame = display.rasterize().unwrap();
    let pixels = frame.pixels.data_as_u8_slice();
    let pixel = |x: usize, y: usize| &pixels[(y * 160 + x) * 4..(y * 160 + x) * 4 + 4];
    assert_eq!(pixel(118, 70), &[255, 0, 0, 255]);
    for (x, y) in [(121, 70), (150, 70), (100, 19), (100, 121)] {
        assert_eq!(pixel(x, y), &[255, 255, 255, 255]);
    }
}
#[test]
fn pie_emphasis_expands_only_selected_outer_edge_in_logical_pixels() {
    for scale in [1.0, 2.0] {
        for selected in [-1, 0, 1] {
            let source = crate::session::tests::source(&format!(
                "function render(){{return h('plot',{{kind:'pie',values:[1,1],colors:['#ff0000','#0000ff'],selectedIndex:{selected},'aria-label':'Series',pieLayout:{{center:[0.5,0.44],innerRadius:0.4,outerRadius:0.68,emphasisScale:5}},style:{{width:300,height:210,backgroundColor:'#ffffff'}}}})}}"
            ));
            let mut page = crate::ViewSession::new(&source).unwrap();
            let display = page
                .layout((300.0 * scale) as u16, (210.0 * scale) as u16, scale)
                .unwrap();
            let frame = display.rasterize().unwrap();
            let pixels = frame.pixels.data_as_u8_slice();
            let pixel = |x: f32| {
                let index =
                    ((92.0 * scale) as usize * (300.0 * scale) as usize + (x * scale) as usize) * 4;
                &pixels[index..index + 4]
            };
            let white = [255, 255, 255, 255];
            assert_eq!(
                pixel(223.0),
                if selected == 0 {
                    &[255, 0, 0, 255]
                } else {
                    &white
                }
            );
            assert_eq!(
                pixel(76.0),
                if selected == 1 {
                    &[0, 0, 255, 255]
                } else {
                    &white
                }
            );
            for x in [110.0, 190.0, 72.0, 228.0] {
                assert_eq!(pixel(x), &white);
            }
        }
    }
    for value in [-1.0, 65.0] {
        let source = format!(
            r#"{{"center":[0.5,0.5],"innerRadius":0.4,"outerRadius":0.68,"emphasisScale":{value}}}"#
        );
        assert!(PieLayout::parse(&source).is_err());
    }
}
#[test]
fn configured_pie_radii_and_center_match_painted_pixels() {
    let source = crate::session::tests::source(
        "function render(){return h('plot',{kind:'pie',values:[1,1],colors:['#ff0000','#0000ff'],'aria-label':'Series',pieLayout:{center:[0.5,0.44],innerRadius:0.4,outerRadius:0.68},style:{width:300,height:210,backgroundColor:'#ffffff'}})}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let display = page.layout(300, 210, 1.0).unwrap();
    assert_eq!(
        display.nodes[0].plot.as_ref().unwrap().pie_layout.center,
        [0.5, 0.44]
    );
    let frame = display.rasterize().unwrap();
    let pixels = frame.pixels.data_as_u8_slice();
    let pixel = |x: usize, y: usize| &pixels[(y * 300 + x) * 4..(y * 300 + x) * 4 + 4];
    for x in [150, 190, 223] {
        assert_eq!(pixel(x, 92), &[255, 255, 255, 255]);
    }
    for x in [194, 219] {
        assert_eq!(pixel(x, 92), &[255, 0, 0, 255]);
    }
    assert_eq!(pixel(95, 92), &[0, 0, 255, 255]);
}
#[test]
fn pie_layout_wire_requires_bounded_json_on_plot() {
    for (tag, value) in [
        (1, serde_json::json!("{}")),
        (18, serde_json::json!(42)),
        (18, serde_json::json!(" ".repeat(1025))),
    ] {
        let wire = serde_json::json!([tag, -1, "", "", "", 1, 88, value]);
        assert!(crate::protocol::decode(wire.as_array().unwrap()).is_err());
    }
}
#[test]
fn pie_layout_rejects_invalid_geometry_and_non_pies() {
    for source in [
        "{}",
        "null",
        r#"{"center":[0.5,0.5],"innerRadius":0.8,"outerRadius":0.6}"#,
        r#"{"center":[-1,0.5],"innerRadius":0.4,"outerRadius":0.6}"#,
        r#"{"center":[0.5,0.5],"innerRadius":0.4,"outerRadius":0.6,"extra":1}"#,
    ] {
        assert!(PieLayout::parse(source).is_err());
    }
    let mut record = record("bar", "[1]");
    record.fields.push((
        88,
        serde_json::json!(r#"{"center":[0.5,0.44],"innerRadius":0.4,"outerRadius":0.68}"#),
    ));
    assert!(Plot::from_record(&record, &crate::theme::Palette::builtin(false)).is_err());
}
#[test]
fn signed_bars_round_only_top_corners() {
    let source = crate::session::tests::source(
        "function render(){return h('plot',{kind:'bar',values:[10,-10],domainMin:-20,domainMax:20,colors:['#3b82f6'],'aria-label':'Series',style:{width:100,height:128,backgroundColor:'#101010'}})}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let frame = page.layout(400, 512, 4.0).unwrap().rasterize().unwrap();
    let pixels = frame.pixels.data_as_u8_slice();
    let pixel = |x: usize, y: usize| &pixels[(y * 400 + x) * 4..(y * 400 + x) * 4 + 4];
    for (x, top, bottom) in [
        (49, 137, 254),
        (166, 137, 254),
        (233, 257, 374),
        (350, 257, 374),
    ] {
        assert_eq!(pixel(x, top), &[16, 16, 16, 255], "top corner");
        assert_eq!(
            pixel(x, bottom),
            &[59, 130, 246, 255],
            "square bottom corner"
        );
    }
}
#[test]
fn line_markers_have_white_centers_blue_rings_and_round_corners() {
    let source = crate::session::tests::source(
        "function render(){return h('plot',{kind:'line',values:[10,10],domainMin:0,domainMax:20,colors:['#3b82f6'],'aria-label':'Series',style:{width:100,height:128,backgroundColor:'#101010'}})}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let frame = page.layout(400, 512, 4.0).unwrap().rasterize().unwrap();
    let pixels = frame.pixels.data_as_u8_slice();
    let pixel = |x: usize, y: usize| &pixels[(y * 400 + x) * 4..(y * 400 + x) * 4 + 4];
    assert_eq!(pixel(108, 256), &[255, 255, 255, 255]);
    assert_eq!(pixel(120, 256), &[59, 130, 246, 255]);
    assert_eq!(pixel(121, 269), &[16, 16, 16, 255]);
}
#[test]
fn selection_snapshot_and_events_follow_the_committed_generation() {
    let source = crate::session::tests::source(
        "function render(){const selected=useLocal('selected',-1);return h('plot',{kind:'line',values:[10,20],colors:['#0000ff'],'aria-label':'Series',selectedIndex:selected.value,onSelect:index=>selected.set(index)})}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let frame = page.layout(100, 100, 1.0).unwrap();
    let plot = frame.nodes[0].plot.as_ref().unwrap();
    assert_eq!(plot.selected_index, -1);
    assert_eq!(plot.values, [10.0, 20.0]);
    let generation = page.generation();
    let handle = plot.on_select.unwrap();
    page.number_event(generation, handle, 2.0).unwrap();
    assert_eq!(
        page.layout(100, 100, 1.0).unwrap().nodes[0]
            .plot
            .as_ref()
            .unwrap()
            .selected_index,
        -1
    );
    let frame = page.layout(100, 100, 1.0).unwrap();
    let current = frame.nodes[0].plot.as_ref().unwrap().on_select.unwrap();
    page.number_event(page.generation(), current, 1.0).unwrap();
    assert_eq!(
        page.layout(100, 100, 1.0).unwrap().nodes[0]
            .plot
            .as_ref()
            .unwrap()
            .selected_index,
        1
    );
    assert!(!page.number_event(generation, handle, 0.0).unwrap());
    let frame = page.layout(100, 100, 1.0).unwrap();
    let clear = frame.nodes[0].plot.as_ref().unwrap().on_select.unwrap();
    page.number_event(page.generation(), clear, -1.0).unwrap();
    assert_eq!(
        page.layout(100, 100, 1.0).unwrap().nodes[0]
            .plot
            .as_ref()
            .unwrap()
            .selected_index,
        -1
    );
    for value in [-2, 2, 128] {
        let mut record = record("line", "[10,20]");
        record.fields.push((57, value.into()));
        assert!(Plot::from_record(&record, &crate::theme::Palette::builtin(false)).is_err());
    }
}
#[test]
fn selection_wire_rejects_invalid_handles_indices_and_other_tags() {
    let wire = serde_json::json!([
        18,
        -1,
        "",
        "",
        "",
        6,
        41,
        "Chart",
        51,
        "[10,20]",
        52,
        "line",
        53,
        "[\"#123456\"]",
        56,
        1,
        57,
        -1
    ]);
    let base = wire.as_array().unwrap();
    assert!(crate::protocol::decode(base).is_ok());
    for bad in [
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(true),
        serde_json::json!(1.5),
        serde_json::json!(9_007_199_254_740_992_u64),
    ] {
        let mut buffer = base.clone();
        buffer[15] = bad;
        assert!(crate::protocol::decode(&buffer).is_err());
    }
    for bad in [
        serde_json::json!(-2),
        serde_json::json!(2),
        serde_json::json!("0"),
        serde_json::json!(0.5),
        serde_json::json!(true),
    ] {
        let mut buffer = base.clone();
        buffer[17] = bad;
        assert!(crate::protocol::decode(&buffer).is_err());
    }
    for field in [56, 57] {
        let wire = serde_json::json!([3, -1, "", "", "", 1, field, 1]);
        assert!(crate::protocol::decode(wire.as_array().unwrap()).is_err());
    }
}
fn record(kind: &str, values: &str) -> Record {
    Record {
        tag: 18,
        parent: None,
        key: String::new(),
        test_id: String::new(),
        text: String::new(),
        fields: vec![
            (41, "Chart".into()),
            (51, values.into()),
            (52, kind.into()),
            (53, "[\"#435DD9\",\"#10B981\"]".into()),
        ],
    }
}
#[test]
fn explicit_domain_scales_geometry_and_preserves_exact_data() {
    for kind in ["bar", "line"] {
        let mut data = record(kind, "[-10,20]");
        data.fields
            .extend([(85, (-20.0).into()), (86, 40.0.into())]);
        let plot = Plot::from_record(&data, &crate::theme::Palette::builtin(false)).unwrap();
        assert_eq!(plot.snapshot().domain, [-20.0, 40.0]);
        assert_eq!(plot.snapshot().values, [-10.0, 20.0]);
        let shapes = plot.shapes([0.0, 0.0, 100.0, 128.0]);
        if kind == "bar" {
            let Shape::Bar(first, _) = shapes[0] else {
                panic!("bar")
            };
            let Shape::Bar(second, _) = shapes[1] else {
                panic!("bar")
            };
            assert_eq!([first[1], first[3]], [84.0, 20.0]);
            assert_eq!([second[1], second[3]], [44.0, 40.0]);
        } else {
            let Shape::Stroke(ref points, _, _) = shapes[0] else {
                panic!("line")
            };
            assert_eq!([points[0][1], points[1][1]], [104.0, 44.0]);
        }
    }
    for (kind, min, max) in [
        ("bar", 0.0, 10.0),
        ("bar", 10.0, 30.0),
        ("bar", 20.0, 20.0),
        ("pie", 0.0, 30.0),
    ] {
        let mut data = record(kind, "[10,20]");
        data.fields.extend([(85, min.into()), (86, max.into())]);
        assert!(Plot::from_record(&data, &crate::theme::Palette::builtin(false)).is_err());
    }
    let mut data = record("line", "[0.1,0.3]");
    data.fields.extend([(85, 0.0.into()), (86, 0.3.into())]);
    assert!(
        Plot::from_record(&data, &crate::theme::Palette::builtin(false)).is_ok(),
        "bounds retain f64 precision"
    );
    data.fields.pop();
    assert!(Plot::from_record(&data, &crate::theme::Palette::builtin(false)).is_err());
}
#[test]
fn wire_domain_fields_require_numeric_bounds_on_plots() {
    for (tag, min, max) in [
        (18, serde_json::json!(0), serde_json::json!(30)),
        (18, serde_json::json!("0"), serde_json::json!(30)),
        (18, serde_json::json!(0), serde_json::json!(1e10)),
        (18, serde_json::json!(0), serde_json::json!(10)),
        (0, serde_json::json!(0), serde_json::json!(30)),
    ] {
        let mut fields = record("bar", "[10,20]").fields;
        fields.extend([(85, min.clone()), (86, max.clone())]);
        let mut wire = vec![
            serde_json::json!(tag),
            serde_json::json!(-1),
            "".into(),
            "".into(),
            "".into(),
            fields.len().into(),
        ];
        for (id, value) in fields {
            wire.extend([id.into(), value]);
        }
        assert_eq!(
            crate::protocol::decode(&wire).is_ok(),
            tag == crate::protocol::tag::PLOT && min == 0 && max == 30
        );
    }
}
#[test]
fn smoothing_preserves_samples_without_overshooting_each_interval() {
    let points = [[0.0, 0.0], [10.0, 30.0], [20.0, -10.0], [30.0, 12.0]];
    let curve = smooth_points(&points);
    assert_eq!(curve.len(), 25);
    for (i, pair) in points.windows(2).enumerate() {
        assert_eq!(curve[i * 8], pair[0]);
        for point in &curve[i * 8..=i * 8 + 8] {
            assert!((pair[0][0]..=pair[1][0]).contains(&point[0]));
            assert!((pair[0][1].min(pair[1][1])..=pair[0][1].max(pair[1][1])).contains(&point[1]));
        }
    }
    assert_eq!(curve[24], points[3]);
}

#[test]
fn area_fades_toward_zero_and_translates_with_scroll() {
    let source = crate::session::tests::source(
        "function render(){return h('scroll',{style:{height:80}},h('plot',{kind:'line',values:[10,10],colors:['#ff0000'],smooth:true,area:true,'aria-label':'Area',style:{width:100,height:200,backgroundColor:'#ffffff'}}))}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let first = page.layout(100, 80, 1.0).unwrap();
    let id = first.nodes[0].scroll.as_ref().unwrap().id.clone();
    let pixels = first.rasterize().unwrap();
    let at = |frame: &crate::Frame, x: usize, y: usize| {
        frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4 + 1]
    };
    assert!(at(&pixels, 50, 15) < at(&pixels, 50, 60));
    assert!(at(&pixels, 50, 15) > 150);
    assert_eq!(at(&pixels, 10, 60), 255);
    let shifted = page
        .prepare_native(
            100,
            80,
            1.0,
            false,
            Some(&std::collections::HashMap::from([(id, 40.0)])),
            Ok,
        )
        .unwrap()
        .rasterize()
        .unwrap();
    assert_eq!(at(&pixels, 50, 60), at(&shifted, 50, 20));
}

#[test]
fn area_flags_and_expanded_geometry_are_bounded_on_the_wire() {
    let wire = serde_json::json!([
        18,
        -1,
        "",
        "",
        "",
        6,
        41,
        "Chart",
        51,
        "[1,2]",
        52,
        "line",
        53,
        "[\"#ff0000\"]",
        54,
        1,
        55,
        1
    ]);
    let mut buffer = wire.as_array().unwrap().clone();
    assert!(crate::protocol::decode(&buffer).is_ok());
    for value in [
        serde_json::json!(2),
        serde_json::json!(true),
        serde_json::json!(-1),
    ] {
        buffer[17] = value;
        assert!(crate::protocol::decode(&buffer).is_err());
    }
    for kind in ["bar", "pie"] {
        let mut item = record(kind, "[1,2]");
        item.fields.push((54, 1.into()));
        assert!(Plot::from_record(&item, &crate::theme::Palette::builtin(false)).is_err());
    }
    let mut item = record("line", &serde_json::to_string(&vec![1; 128]).unwrap());
    item.fields.extend([(54, 1.into()), (55, 1.into())]);
    let plot = Plot::from_record(&item, &crate::theme::Palette::builtin(false)).unwrap();
    assert_eq!(plot.work(), 2292);
    let geometry = plot.shapes([0.0, 0.0, 100.0, 100.0]);
    assert!(matches!(&geometry[0], Shape::Area(points, _, _) if points.len() == 1019));
    let mut many = serde_json::json!([0, -1, "", "", "", 0])
        .as_array()
        .unwrap()
        .clone();
    for i in 0..8 {
        let mut node = wire.as_array().unwrap().clone();
        node[1] = 0.into();
        node[2] = i.to_string().into();
        node[9] = serde_json::to_string(&vec![1; 128]).unwrap().into();
        many.extend(node);
    }
    assert!(
        crate::protocol::decode(&many)
            .unwrap_err()
            .contains("plot geometry budget")
    );
}

#[test]
fn negative_area_fades_to_its_top_baseline_on_a_dark_background() {
    let source = crate::session::tests::source(
        "function render(){return h('plot',{kind:'line',values:[-10,-10],colors:['#0000ff'],smooth:true,area:true,'aria-label':'Negative area',style:{width:100,height:100,backgroundColor:'#101010'}})}",
    );
    let mut page = crate::ViewSession::new(&source).unwrap();
    let frame = page.layout(100, 100, 1.0).unwrap().rasterize().unwrap();
    let blue = |x: usize, y: usize| frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4 + 2];
    assert!(blue(50, 15) > 16);
    assert!(blue(50, 85) > blue(50, 15));
    assert_eq!(blue(10, 50), 16);
}
#[test]
fn rejects_unbounded_or_invalid_series() {
    for (kind, values) in [
        ("pie", "[-1]"),
        ("bar", "[]"),
        ("line", "[1e10]"),
        ("other", "[1]"),
        ("bar", "[null]"),
    ] {
        assert!(
            Plot::from_record(
                &record(kind, values),
                &crate::theme::Palette::builtin(false)
            )
            .is_err()
        );
    }
    assert!(
        Plot::from_record(
            &record("bar", &serde_json::to_string(&vec![1; 129]).unwrap()),
            &crate::theme::Palette::builtin(false)
        )
        .is_err()
    );
}
#[test]
fn negative_bars_share_zero_baseline_and_donut_work_is_bounded() {
    let plot = Plot::from_record(
        &record("bar", "[-10,10]"),
        &crate::theme::Palette::builtin(false),
    )
    .unwrap();
    let shapes = plot.shapes([0.0, 0.0, 100.0, 100.0]);
    let Shape::Bar(first, _) = shapes[0] else {
        panic!()
    };
    let Shape::Bar(second, _) = shapes[1] else {
        panic!()
    };
    assert_eq!(first[1], 50.0);
    assert_eq!(second[1] + second[3], 50.0);
    let pie = Plot::from_record(
        &record("pie", &serde_json::to_string(&vec![1; 128]).unwrap()),
        &crate::theme::Palette::builtin(false),
    )
    .unwrap();
    let slices = pie.shapes([0.0, 0.0, 100.0, 100.0]);
    assert_eq!(slices.len(), 128);
    let segments: usize = slices
        .iter()
        .map(|shape| match shape {
            Shape::Stroke(points, _, _) => points.len() - 1,
            Shape::Bar(..) | Shape::Circle(..) | Shape::Area(..) => {
                panic!("pie must contain continuous arcs")
            }
        })
        .sum();
    assert!(segments <= 384);
    assert!(
        Plot::from_record(
            &record("pie", "[0,0]"),
            &crate::theme::Palette::builtin(false)
        )
        .unwrap()
        .shapes([0.0, 0.0, 100.0, 100.0])
        .is_empty()
    );
}

#[test]
fn native_decode_and_reference_pixels_cover_each_plot_kind() {
    for kind in ["bar", "line", "pie"] {
        let source = crate::session::tests::source(&format!(
            "function render(){{return h('plot',{{'aria-label':'Chart',kind:'{kind}',values:[10,20],colors:['#ff0000','#0000ff'],style:{{width:100,height:100,backgroundColor:'#ffffff'}}}})}}"
        ));
        let mut page = crate::ViewSession::new(&source).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        assert_eq!(display.nodes[0].tag, "plot");
        let frame = display.rasterize().unwrap();
        let pixels = frame.pixels.data_as_u8_slice();
        assert!(
            pixels.chunks_exact(4).filter(|p| p[1] < 100).count() > 100,
            "{kind}"
        );
        if kind == "pie" {
            let center = (50 * 100 + 50) * 4;
            assert_eq!(&pixels[center..center + 4], &[255, 255, 255, 255]);
            // Interior samples must remain opaque across tessellation joins.
            for degree in 0..360 {
                if degree % 120 < 4 || degree % 120 > 116 {
                    continue;
                }
                let angle = (degree as f64).to_radians() - std::f64::consts::FRAC_PI_2;
                let x = (50.0 + angle.cos() * 36.0) as usize;
                let y = (50.0 + angle.sin() * 36.0) as usize;
                assert!(pixels[(y * 100 + x) * 4 + 1] < 10, "seam at {degree}");
            }
        }
    }
    let mut invalid = vec![
        serde_json::json!(18),
        serde_json::json!(-1),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(3),
    ];
    invalid.extend([
        serde_json::json!(51),
        serde_json::json!("[-1]"),
        serde_json::json!(52),
        serde_json::json!("pie"),
        serde_json::json!(53),
        serde_json::json!("[\"#ffffff\"]"),
    ]);
    assert!(crate::protocol::decode(&invalid).is_err());
    let mut many = vec![
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(0),
    ];
    for i in 0..64 {
        many.extend([
            serde_json::json!(18),
            serde_json::json!(0),
            serde_json::json!(i.to_string()),
            serde_json::json!(""),
            serde_json::json!(""),
            serde_json::json!(4),
            serde_json::json!(41),
            serde_json::json!("Chart"),
            serde_json::json!(51),
            serde_json::json!("[1,2]"),
            serde_json::json!(52),
            serde_json::json!("pie"),
            serde_json::json!(53),
            serde_json::json!("[\"#ffffff\"]"),
        ]);
    }
    assert!(
        crate::protocol::decode(&many)
            .unwrap_err()
            .contains("plot geometry budget")
    );
}
