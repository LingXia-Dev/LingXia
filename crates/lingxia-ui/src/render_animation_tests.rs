use super::*;
use crate::animation::{Visual, interpolate::Transform};
use std::collections::BTreeMap;

#[test]
fn subtree_translation_recovers_offscreen_paint_without_moving_siblings() {
    let mut page = crate::ViewSession::new(&crate::session::tests::source(
        "function render(){return h('stack',{style:{width:100,height:100}},h('column',{testId:'card',style:{offset:{x:120,y:10},width:30,height:30,background:'#ff0000'}}),h('column',{testId:'sibling',style:{offset:{x:0,y:50},width:20,height:20,background:'#00ff00'}}));}"
    )).unwrap();
    let original = page.layout(100, 100, 1.0).unwrap();
    let card = original.nodes.iter().find(|n| n.test_id == "card").unwrap();
    assert_eq!(card.visible_bounds[2], 0.0);
    let visuals = BTreeMap::from([(
        card.identity.clone(),
        Visual {
            opacity: None,
            transforms: vec![Transform::Translate([-100.0, 0.0])],
        },
    )]);
    let display = original
        .retained
        .compose_animated(&HashMap::new(), &visuals)
        .unwrap();
    assert_eq!(display.nodes[1].bounds, [20.0, 10.0, 30.0, 30.0]);
    assert_eq!(display.nodes[1].visible_bounds, display.nodes[1].bounds);
    assert_eq!(display.nodes[2].bounds, original.nodes[2].bounds);
    let frame = display.rasterize().unwrap();
    let pixel = |x: usize, y: usize| {
        &frame.pixels.data_as_u8_slice()[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4]
    };
    assert_eq!(pixel(25, 15), [255, 0, 0, 255]);
    assert_eq!(pixel(5, 55), [0, 255, 0, 255]);
}

#[test]
fn transformed_plot_selection_uses_original_geometry_inside_static_scroll_clips() {
    for kind in ["bar", "pie"] {
        let source = format!(
            "function render(){{return h('scroll',{{style:{{width:100,height:80,radius:8}}}},h('column',{{testId:'card',style:{{width:80,height:140}}}},h('plot',{{kind:'{kind}',values:[2,4,6],colors:['#123456'],accessibilityLabel:'Series',onSelect:()=>{{}},style:{{width:80,height:60}}}})));}}"
        );
        let mut page = crate::ViewSession::new(&crate::session::tests::source(&source)).unwrap();
        let original = page.layout(100, 100, 1.0).unwrap();
        let scroll = original.nodes[0].scroll.as_ref().unwrap().id.clone();
        let offsets = HashMap::from([(scroll, 10.0)]);
        let original = original.retained.compose(&offsets).unwrap();
        let owner = &original.nodes[1];
        let visual = Visual {
            opacity: Some(0.5),
            transforms: vec![
                Transform::Translate([4.0, -10.0]),
                Transform::Scale([0.7, 0.6]),
            ],
        };
        let [x, y, w, h] = owner.bounds.map(f64::from);
        let matrix = visual.matrix([x + w / 2.0, y + h / 2.0]).unwrap();
        let display = original
            .retained
            .compose_animated(
                &offsets,
                &BTreeMap::from([(owner.identity.clone(), visual)]),
            )
            .unwrap();
        let plot = &display.nodes[2];
        assert_eq!(plot.clips, original.nodes[2].clips);
        let original_plot = original.nodes[2].plot.as_ref().unwrap();
        for x in 5..75 {
            for y in 5..45 {
                // Stay away from exact bin edges: the host supplies f32 points.
                let point = [x as f32 + 0.37, y as f32 + 0.29];
                let mapped =
                    matrix * vello_cpu::kurbo::Point::new(point[0] as f64, point[1] as f64);
                let mapped = [mapped.x as f32, mapped.y as f32];
                if plot.clips.iter().all(|clip| clip.contains(mapped)) {
                    assert_eq!(
                        plot.hit_test_plot(mapped),
                        original_plot.hit_test(original.nodes[2].bounds, point),
                        "kind={kind} point={point:?}"
                    );
                }
            }
        }
        assert!(plot.hit_test_plot([-1.0, 20.0]).is_none());
    }
}

#[test]
fn animated_opacity_replaces_base_alpha_and_isolates_overlapping_children() {
    let mut page = crate::ViewSession::new(&crate::session::tests::source(
        "function render(){return h('stack',{style:{width:40,height:40,opacity:0.8}},h('column',{style:{width:40,height:40,background:'#ff0000'}}),h('column',{style:{width:40,height:40,background:'#0000ff'}}));}"
    )).unwrap();
    let display = page.layout(50, 50, 1.0).unwrap().animate_for_test(
        0,
        Visual {
            opacity: Some(0.5),
            transforms: vec![],
        },
    );
    let frame = display.rasterize().unwrap();
    let pixel = &frame.pixels.data_as_u8_slice()[(10 * 50 + 10) * 4..(10 * 50 + 10) * 4 + 4];
    for (actual, expected) in pixel.iter().zip([123u8, 124, 253, 255]) {
        assert!(actual.abs_diff(expected) <= 1);
    }
}

#[test]
fn animated_interactive_geometry_is_rejected_before_build_when_unsupported() {
    for tag in ["input", "scroll", "slider"] {
        let label = if tag == "slider" {
            "accessibilityLabel:'Control',"
        } else {
            ""
        };
        let source = format!(
            "function render(){{return h('column',{{}},h('{tag}',{{{label}style:{{width:50,height:30}}}}));}}"
        );
        let mut page = crate::ViewSession::new(&crate::session::tests::source(&source)).unwrap();
        let display = page.layout(100, 100, 1.0).unwrap();
        let visuals = BTreeMap::from([(
            display.nodes[0].identity.clone(),
            Visual {
                opacity: None,
                transforms: vec![Transform::Scale([0.9, 0.9])],
            },
        )]);
        assert!(
            display
                .retained
                .compose_animated(&HashMap::new(), &visuals)
                .is_err(),
            "{tag}"
        );
    }
}

fn primitives(scale: f32) -> DisplayList {
    let mut page = crate::ViewSession::new(&crate::session::tests::source(
        "function render(){return h('column',{});}",
    ))
    .unwrap();
    let mut display = page.layout(80, 80, scale).unwrap();
    display.commands = vec![
        Paint::Transform(Affine::new([2.0, 0.0, 0.0, 1.5, 8.0, 6.0])),
        Paint::Clip(ClipSnapshot {
            bounds: [0.0, 0.0, 16.0, 16.0],
            radius: 0.0,
            radii: None,
        }),
        Paint::Opacity {
            node: 0,
            alpha: 0.5,
        },
        Paint::Rect {
            color: [255, 0, 0, 255],
            bounds: [0.0, 0.0, 8.0, 8.0],
            radius: 0.0,
            corners: None,
        },
        Paint::PopOpacity,
        Paint::PopClip,
        Paint::Transform(Affine::IDENTITY),
        Paint::Rect {
            color: [0, 255, 0, 255],
            bounds: [0.0, 0.0, 4.0, 4.0],
            radius: 0.0,
            corners: None,
        },
    ];
    display
}

#[test]
fn transformed_primitives_have_expected_pixels_and_reset_for_siblings() {
    for scale in [1.0, 2.0] {
        let frame = primitives(scale).rasterize().unwrap();
        let pixel = |x: usize, y: usize| {
            let i = ((y as f32 * scale) as usize * 80 + (x as f32 * scale) as usize) * 4;
            &frame.pixels.data_as_u8_slice()[i..i + 4]
        };
        assert_eq!(pixel(1, 1), [0, 255, 0, 255]);
        assert_eq!(pixel(6, 6), [246, 247, 251, 255]);
        let red = pixel(22, 16);
        assert!(red[0] > 245 && (120..130).contains(&red[1]) && (120..130).contains(&red[2]));
        assert_eq!(pixel(25, 19), [246, 247, 251, 255]);
        let composed = primitives(scale).rasterize_composited().unwrap();
        for (a, b) in frame
            .pixels
            .data_as_u8_slice()
            .iter()
            .zip(composed.pixels.data_as_u8_slice())
        {
            assert!(a.abs_diff(*b) <= 2);
        }
    }
}

#[test]
fn unsupported_transforms_fail_in_both_paint_paths() {
    for matrix in [
        Affine::scale(0.0),
        Affine::scale(-1.0),
        Affine::rotate(0.1),
        Affine::translate((f64::NAN, 0.0)),
        Affine::translate((1e13, 0.0)),
    ] {
        let mut display = primitives(1.0);
        display.commands[0] = Paint::Transform(matrix);
        assert!(display.rasterize().is_err());
        let mut display = primitives(1.0);
        display.commands[0] = Paint::Transform(matrix);
        assert!(display.rasterize_composited().is_err());
    }
}

#[test]
fn backdrop_clip_and_blur_follow_uniform_scale_and_reject_nonuniform_scale() {
    let mut display = primitives(2.0);
    display.commands = vec![
        Paint::Transform(Affine::new([0.5, 0.0, 0.0, 0.5, 8.0, 6.0])),
        Paint::Backdrop {
            node: 0,
            clip: ClipSnapshot {
                bounds: [0.0, 0.0, 16.0, 16.0],
                radius: 2.0,
                radii: None,
            },
            blur: 4.0,
            saturation: 1.0,
            opacity: 1.0,
        },
        Paint::PopBackdrop,
    ];
    let plan = display.composition_plan().unwrap();
    let backdrop = plan
        .steps()
        .iter()
        .find_map(|step| match step {
            crate::composite::Step::Push(crate::composite::Layer::Backdrop {
                clip, blur, ..
            }) => Some((clip, blur)),
            _ => None,
        })
        .unwrap();
    assert_eq!(backdrop.0.bounds, [16.0, 12.0, 16.0, 16.0]);
    assert_eq!(backdrop.0.radius, 2.0);
    assert_eq!(*backdrop.1, 4.0);
    display.commands[0] = Paint::Transform(Affine::scale_non_uniform(0.5, 0.6));
    assert!(display.composition_plan().is_err());
}
