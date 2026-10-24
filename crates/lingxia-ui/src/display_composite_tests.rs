use crate::{DisplayList, ViewSession, image::Bitmap};
use std::collections::HashMap;
use vello_cpu::Pixmap;

fn display(scale: f32, offset: f32) -> DisplayList {
    let module = crate::session::tests::source(
        "function render(){return h('column',{style:{padding:4,background:'#ffffff'}},h('scroll',{testId:'scroll',style:{height:64,radius:10,borderWidth:1,borderColor:'#555555'}},h('column',{style:{opacity:0.55,gap:2}},h('text',{style:{fontSize:12,fontFamily:'mono'}},'UI'),h('image',{src:'photo',style:{width:24,height:16,radius:3}}),h('vector',{paths:[{d:'M0 0L24 0L12 24Z'}],style:{width:20,height:12,color:'#0000ff'}}),h('column',{style:{height:80,background:'#00aaff'}})),h('column',{style:{opacity:0.55,height:24,background:'#ff0000'}})))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let layout = page.layout(96, 96, scale).unwrap();
    let id = layout
        .nodes
        .iter()
        .find_map(|node| node.scroll.as_ref().map(|scroll| scroll.id.clone()))
        .unwrap();
    let mut display = layout
        .retained
        .compose(&HashMap::from([(id, offset)]))
        .unwrap();
    let mut photo = Pixmap::new(4, 4);
    for (index, pixel) in photo.data_as_u8_slice_mut().chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if index % 4 < 2 {
            &[200, 0, 0, 200]
        } else {
            &[0, 150, 0, 200]
        });
    }
    display
        .bind_images(&HashMap::from([(
            "photo".to_owned(),
            Bitmap::from_pixels(photo),
        )]))
        .unwrap();
    display
}

fn transformed_display(scale: f32, offset: f32, stretch: bool) -> DisplayList {
    use crate::animation::{Visual, interpolate::Transform};
    display(scale, offset).animate_for_test(
        2,
        Visual {
            opacity: Some(0.4),
            transforms: vec![
                Transform::Translate([7.0, 4.0]),
                Transform::Scale([0.8, if stretch { 0.6 } else { 0.8 }]),
            ],
        },
    )
}

#[test]
fn transformed_paint_spans_preserve_cpu_text_images_and_clip_geometry() {
    for scale in [1.0, 1.5, 2.0] {
        for offset in [0.0, 12.0, 30.0] {
            for stretch in [false, true] {
                let expected = transformed_display(scale, offset, stretch)
                    .rasterize()
                    .unwrap();
                let actual = transformed_display(scale, offset, stretch)
                    .rasterize_composited()
                    .unwrap();
                let original = display(scale, offset).rasterize().unwrap();
                assert_ne!(
                    expected.pixels.data_as_u8_slice(),
                    original.pixels.data_as_u8_slice()
                );
                for (i, (a, b)) in actual
                    .pixels
                    .data_as_u8_slice()
                    .iter()
                    .zip(expected.pixels.data_as_u8_slice())
                    .enumerate()
                {
                    assert!(
                        a.abs_diff(*b) <= 2,
                        "scale={scale} offset={offset} stretch={stretch} byte={i}: {a} vs {b}"
                    );
                }
            }
        }
    }
}

#[test]
fn display_plan_preserves_cpu_text_images_clipping_opacity_and_retained_scroll() {
    for scale in [1.0, 1.5, 2.0] {
        for offset in [0.0, 12.0, 30.0] {
            let expected = display(scale, offset).rasterize().unwrap();
            let actual = display(scale, offset).rasterize_composited().unwrap();
            assert_eq!(
                serde_json::to_value(&actual.nodes).unwrap(),
                serde_json::to_value(&expected.nodes).unwrap()
            );
            for (index, (actual, expected)) in actual
                .pixels
                .data_as_u8_slice()
                .iter()
                .zip(expected.pixels.data_as_u8_slice())
                .enumerate()
            {
                assert!(
                    actual.abs_diff(*expected) <= 2,
                    "scale={scale} offset={offset} byte={index}: {actual} vs {expected}"
                );
            }
        }
    }
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
fn display_plan_preserves_gpu_text_images_clipping_opacity_and_retained_scroll() {
    use crate::{
        backdrop_backend_tests::readback,
        composite::{Plan, Step},
    };
    let mut context = vello::util::RenderContext::new();
    let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let mut compositor = crate::composite_gpu::Compositor::new(device);
    for scale in [1.0, 1.5, 2.0] {
        for offset in [0.0, 12.0, 30.0] {
            for transformed in [None, Some(false), Some(true)] {
                let display = match transformed {
                    Some(stretch) => transformed_display(scale, offset, stretch),
                    None => display(scale, offset),
                };
                let plan = Plan::new(96, 96, 1, vec![Step::Draw(0..1)]).unwrap();
                let mut glyph_cache = glifo::GlyphPrepCache::default();
                let mut glyph_images = HashMap::new();
                let mut textures = crate::gpu_images::ImageTextures::default();
                let image_ids = display
                    .bitmaps()
                    .map(|bitmap| {
                        (
                            std::sync::Arc::as_ptr(bitmap.pixels()) as usize,
                            crate::gpu_text::source(bitmap.pixels()),
                        )
                    })
                    .collect();
                let flat = compositor
                    .render(&plan, device, queue, |_, scene, bindings| {
                        display.sparse_scene(
                            &mut glyph_cache,
                            scene,
                            &image_ids,
                            &mut glyph_images,
                        )?;
                        *bindings = textures.bindings(
                            device,
                            queue,
                            display
                                .bitmaps()
                                .map(|bitmap| bitmap.pixels().clone())
                                .chain(glyph_images.values().cloned()),
                        );
                        Ok(())
                    })
                    .unwrap();
                let expected = readback(device, queue, flat.texture());
                drop(flat);
                let composed = compositor.render_display(&display, device, queue).unwrap();
                let actual = readback(device, queue, composed.texture());
                for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                    assert!(
                        actual.abs_diff(*expected) <= 3,
                        "scale={scale} offset={offset} transformed={transformed:?} byte={index}: {actual} vs {expected}"
                    );
                }
            }
        }
    }
}

fn backdrop_display(scale: f32, offset: f32) -> DisplayList {
    let module = crate::session::tests::source(
        "function render(){return h('scroll',{style:{height:72,background:'#ffffff',radius:6}},h('stack',{style:{height:160}},h('column',{style:{width:80,height:160,background:'#0000ff'}}),h('column',{style:{width:32,height:160,background:'#ff0000'}}),h('column',{testId:'glass',style:{offset:{x:8,y:12},width:48,height:40,radius:4,backdropBlur:4,backdropSaturation:0}},h('column',{style:{width:8,height:8,background:'#00ff00'}}))))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(96, 96, scale).unwrap();
    let id = display.nodes[0].scroll.as_ref().unwrap().id.clone();
    display
        .retained
        .compose(&HashMap::from([(id, offset)]))
        .unwrap()
}

#[test]
fn authored_backdrop_preserves_foreground_and_scroll_scope() {
    use crate::composite::{Layer, Step};
    for scale in [1.0, 1.5, 2.0] {
        for offset in [0.0, 12.0, 30.0] {
            let display = backdrop_display(scale, offset);
            assert!(display.requires_composition());
            let plan = display.composition_plan().unwrap();
            let mut clip_seen = false;
            let mut backdrop_seen = false;
            for step in plan.steps() {
                match step {
                    Step::Push(Layer::Clip(_)) => clip_seen = true,
                    Step::Push(Layer::Backdrop { clip, blur, .. }) => {
                        assert!(clip_seen);
                        assert_eq!(*blur, 4.0 * scale);
                        assert_eq!(clip.bounds[1], (12.0 - offset) * scale);
                        backdrop_seen = true;
                    }
                    _ => {}
                }
            }
            assert!(backdrop_seen);
            let frame = display.rasterize().unwrap();
            if scale == 1.0 && offset == 0.0 {
                let pixel = |x: usize, y: usize| {
                    &frame.pixels.data_as_u8_slice()[(y * 96 + x) * 4..(y * 96 + x) * 4 + 4]
                };
                assert_eq!(
                    pixel(12, 16),
                    &[0, 255, 0, 255],
                    "foreground stays sharp and saturated"
                );
                let gray = pixel(24, 32);
                assert!(gray[0].abs_diff(gray[1]) <= 1 && gray[1].abs_diff(gray[2]) <= 1);
                assert_eq!(
                    pixel(4, 32),
                    &[255, 0, 0, 255],
                    "outside owner remains unfiltered"
                );
            }
        }
    }
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter"]
fn authored_backdrop_gpu_matches_cpu_across_retained_scroll() {
    let mut context = vello::util::RenderContext::new();
    let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let mut compositor = crate::composite_gpu::Compositor::new(device);
    for scale in [1.0, 1.5, 2.0] {
        for offset in [0.0, 12.0, 30.0] {
            let display = backdrop_display(scale, offset);
            let output = compositor.render_display(&display, device, queue).unwrap();
            let actual = crate::backdrop_backend_tests::readback(device, queue, output.texture());
            let expected = display.rasterize().unwrap();
            for (index, (a, b)) in actual
                .chunks_exact(512)
                .flat_map(|row| row[..96 * 4].iter())
                .zip(expected.pixels.data_as_u8_slice())
                .enumerate()
            {
                assert!(
                    a.abs_diff(*b) <= 3,
                    "scale={scale} offset={offset} byte={index}: {a} vs {b}"
                );
            }
        }
    }
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
fn shared_renderer_preserves_interleaved_page_pixels_and_retained_outputs() {
    use crate::{backdrop_backend_tests::readback, composite_gpu::Compositor};
    let domain = crate::gpu::Context::default();
    let id = pollster::block_on(domain.lock().unwrap().device(None)).expect("GPU adapter required");
    let engine = domain.renderer(id).unwrap();
    let second_engine = domain.renderer(id).unwrap();
    assert_eq!(domain.renderer_count().unwrap(), 1);
    let context = domain.lock().unwrap();
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let mut changed_image = display(1.5, 12.0);
    let mut photo = Pixmap::new(4, 4);
    for pixel in photo.data_as_u8_slice_mut().chunks_exact_mut(4) {
        pixel.copy_from_slice(&[0, 0, 220, 220]);
    }
    changed_image
        .bind_images(&HashMap::from([(
            "photo".into(),
            Bitmap::from_pixels(photo),
        )]))
        .unwrap();
    let pages = [
        display(1.0, 0.0),
        changed_image,
        backdrop_display(1.0, 0.0),
        backdrop_display(2.0, 30.0),
    ];
    let expected: Vec<_> = pages
        .iter()
        .map(|page| {
            let mut isolated = Compositor::new(device);
            let output = isolated.render_display(page, device, queue).unwrap();
            readback(device, queue, output.texture())
        })
        .collect();
    let mut left = Compositor::with_renderer(engine.clone());
    let mut right = Compositor::with_renderer(second_engine);
    let mut retained = Vec::new();
    for round in 0..3 {
        for (index, page) in pages.iter().enumerate() {
            let compositor = if index % 2 == 0 {
                &mut left
            } else {
                &mut right
            };
            let output = compositor.render_display(page, device, queue).unwrap();
            retained.push((index, output));
        }
        left.clear();
        if round == 1 {
            // Destroy a page while another page and earlier submitted outputs survive.
            left = Compositor::with_renderer(engine.clone());
        }
    }
    drop(left);
    drop(right);
    for (index, output) in retained {
        assert_eq!(
            readback(device, queue, output.texture()),
            expected[index],
            "shared renderer must not substitute another page's images, text, clips or filters"
        );
    }
}
