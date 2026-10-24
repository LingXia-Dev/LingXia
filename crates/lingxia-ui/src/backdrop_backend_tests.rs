//! Backend feasibility checks, not coverage of authored backdrop-filter styles.
use std::sync::Arc;
use vello_cpu::{
    Image, ImageSource, Pixmap, RenderContext, Resources,
    color::{AlphaColor, Srgb, palette::css},
    filter_effects::{Filter, FilterFunction},
    kurbo::{Rect, RoundedRect, Shape},
};

const SIZE: u16 = 96;

fn frame() -> Rect {
    Rect::new(0.0, 0.0, SIZE as f64, SIZE as f64)
}

fn card() -> vello_cpu::kurbo::BezPath {
    RoundedRect::new(16.0, 16.0, 80.0, 80.0, 12.0).to_path(0.1)
}

fn blur() -> Filter {
    Filter::from_function(FilterFunction::Blur { radius: 4.0 })
}

fn verify(pixels: &[u8], stride: usize) {
    let pixel =
        |x: usize, y: usize| -> &[u8] { &pixels[y * stride + x * 4..y * stride + x * 4 + 4] };
    // The transition is softened only inside the card; the captured parent survives.
    assert_eq!(pixel(47, 8), [0, 0, 0, 255]);
    assert_eq!(pixel(48, 8), [255, 255, 255, 255]);
    assert_eq!(pixel(16, 16), [0, 0, 0, 255]);
    assert_eq!(pixel(24, 24), [32, 32, 32, 255]);
    let softened = pixel(47, 32);
    assert!(softened[0] > 60 && softened[0] < 180, "{softened:?}");
    assert_eq!(softened[0], softened[1]);
    assert_eq!(softened[1], softened[2]);
    assert_eq!(softened[3], 255);
    // A thin foreground stem and its neighbors must not enter the blur input.
    assert_eq!(pixel(44, 48), [255, 0, 0, 255]);
    assert_eq!(pixel(45, 48), [255, 0, 0, 255]);
    assert_eq!(pixel(43, 48)[0], pixel(43, 48)[1]);
    assert_eq!(pixel(46, 48)[0], pixel(46, 48)[1]);
}

#[test]
fn cpu_captured_background_blurs_beneath_sharp_foreground() {
    let mut parent = RenderContext::new(SIZE, SIZE);
    parent.set_paint(css::BLACK);
    parent.fill_rect(&frame());
    parent.set_paint(css::WHITE);
    parent.fill_rect(&Rect::new(48.0, 0.0, 96.0, 96.0));
    parent.flush();
    let mut resources = Resources::new();
    let mut snapshot = Pixmap::new(SIZE, SIZE);
    parent.render(&mut snapshot, &mut resources);

    let image = Image {
        image: ImageSource::Pixmap(Arc::new(snapshot)),
        sampler: Default::default(),
    };
    let mut output = RenderContext::new(SIZE, SIZE);
    output.set_paint(image.clone());
    output.fill_rect(&frame());
    output.push_clip_layer(&card());
    output.push_filter_layer(blur());
    output.set_paint(image);
    output.fill_rect(&frame());
    output.pop_layer();
    output.set_paint(AlphaColor::<Srgb>::from_rgba8(255, 255, 255, 32));
    output.fill_rect(&frame());
    output.set_paint(css::RED);
    output.fill_rect(&Rect::new(44.0, 40.0, 46.0, 56.0));
    output.pop_layer();
    output.flush();
    let mut pixels = Pixmap::new(SIZE, SIZE);
    output.render(&mut pixels, &mut resources);
    verify(pixels.data_as_u8_slice(), SIZE as usize * 4);

    let mut outer = RenderContext::new(SIZE, SIZE);
    outer.set_paint(css::BLUE);
    outer.fill_rect(&frame());
    outer.push_clip_layer(&Rect::new(8.0, 8.0, 88.0, 88.0).to_path(0.1));
    outer.push_opacity_layer(0.5);
    outer.set_paint(Image {
        image: ImageSource::Pixmap(Arc::new(pixels)),
        sampler: Default::default(),
    });
    outer.fill_rect(&frame());
    outer.pop_layer();
    outer.pop_layer();
    outer.flush();
    let mut pixels = Pixmap::new(SIZE, SIZE);
    outer.render(&mut pixels, &mut resources);
    verify_isolated(pixels.data_as_u8_slice(), SIZE as usize * 4);
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
fn gpu_texture_capture_blurs_and_continues_existing_target() {
    use crate::gpu_pass::{SceneRenderer, Target};
    use vello_gpu::{RenderSize, Scene, TargetInit, TextureBindings, TextureId};
    let mut context = vello::util::RenderContext::new();
    let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
    eprintln!(
        "Backdrop backend adapter: {:?}",
        context.devices[id].adapter().get_info()
    );
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let size = RenderSize {
        width: SIZE,
        height: SIZE,
    };
    let extent = wgpu::Extent3d {
        width: SIZE as u32,
        height: SIZE as u32,
        depth_or_array_layers: 1,
    };
    let texture = |label, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        })
    };
    let target = texture(
        "backdrop test target",
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let root = texture(
        "backdrop outer root",
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let root_view = root.create_view(&Default::default());
    let snapshot = texture(
        "backdrop test capture",
        wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let target_view = target.create_view(&Default::default());
    let depth = vello_gpu::Renderer::create_depth_texture_view(device, &size);
    let mut renderer = SceneRenderer::default();
    let mut outer = Scene::new(SIZE, SIZE);
    outer.set_paint(css::BLUE);
    outer.fill_rect(&frame());
    renderer
        .draw(
            device,
            queue,
            &outer,
            &TextureBindings::new(),
            Target {
                size: RenderSize {
                    width: SIZE,
                    height: SIZE,
                },
                view: &root_view,
                depth: Some(&depth),
                init: TargetInit::Clear(Default::default()),
            },
        )
        .unwrap();
    let mut parent = Scene::new(SIZE, SIZE);
    parent.set_paint(css::BLACK);
    parent.fill_rect(&frame());
    parent.set_paint(css::WHITE);
    parent.fill_rect(&Rect::new(48.0, 0.0, 96.0, 96.0));
    renderer
        .draw(
            device,
            queue,
            &parent,
            &TextureBindings::new(),
            Target {
                size,
                view: &target_view,
                depth: Some(&depth),
                init: TargetInit::Clear(Default::default()),
            },
        )
        .unwrap();

    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_texture(target.as_image_copy(), snapshot.as_image_copy(), extent);
    queue.submit([encoder.finish()]);

    let mut bindings = TextureBindings::new();
    bindings.insert(TextureId(1), snapshot.create_view(&Default::default()));
    let mut glass = Scene::new(SIZE, SIZE);
    glass.push_clip_layer(&card());
    glass.push_filter_layer(blur());
    glass.set_paint(vello_gpu::Image {
        image: vello_gpu::ImageSource::external_texture(
            TextureId(1),
            vello_gpu::RectU16::new(0, 0, SIZE, SIZE),
            true,
        ),
        sampler: Default::default(),
    });
    glass.fill_rect(&frame());
    glass.pop_layer();
    glass.set_paint(AlphaColor::<Srgb>::from_rgba8(255, 255, 255, 32));
    glass.fill_rect(&frame());
    glass.set_paint(css::RED);
    glass.fill_rect(&Rect::new(44.0, 40.0, 46.0, 56.0));
    glass.pop_layer();
    renderer
        .draw(
            device,
            queue,
            &glass,
            &bindings,
            Target {
                size: RenderSize {
                    width: SIZE,
                    height: SIZE,
                },
                view: &target_view,
                depth: Some(&depth),
                init: TargetInit::SrcOver,
            },
        )
        .unwrap();

    let pixels = readback(device, queue, &target);
    verify(&pixels, READBACK_STRIDE as usize);

    // The opacity root samples its own black/white backdrop, not the blue page.
    // Apply group opacity only after its backdrop and sharp foreground are done.
    bindings.insert(TextureId(2), target_view.clone());
    let mut composite = Scene::new(SIZE, SIZE);
    composite.push_clip_layer(&Rect::new(8.0, 8.0, 88.0, 88.0).to_path(0.1));
    composite.push_opacity_layer(0.5);
    composite.set_paint(vello_gpu::Image {
        image: vello_gpu::ImageSource::external_texture(
            TextureId(2),
            vello_gpu::RectU16::new(0, 0, SIZE, SIZE),
            true,
        ),
        sampler: Default::default(),
    });
    composite.fill_rect(&frame());
    composite.pop_layer();
    composite.pop_layer();
    renderer
        .draw(
            device,
            queue,
            &composite,
            &bindings,
            Target {
                size: RenderSize {
                    width: SIZE,
                    height: SIZE,
                },
                view: &root_view,
                depth: Some(&depth),
                init: TargetInit::SrcOver,
            },
        )
        .unwrap();
    let pixels = readback(device, queue, &root);
    verify_isolated(&pixels, READBACK_STRIDE as usize);
}

fn verify_isolated(pixels: &[u8], stride: usize) {
    let pixel = |x: usize, y: usize| &pixels[y * stride + x * 4..y * stride + x * 4 + 4];
    let close = |actual: &[u8], expected: [u8; 4]| {
        assert!(
            actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1),
            "{actual:?} != {expected:?}"
        );
    };
    assert_eq!(pixel(4, 48), [0, 0, 255, 255]);
    close(pixel(44, 48), [128, 0, 128, 255]);
    close(pixel(16, 16), [0, 0, 128, 255]);
    close(pixel(24, 24), [16, 16, 144, 255]);
    close(pixel(64, 8), [128, 128, 255, 255]);
    let soft = pixel(47, 32);
    assert!(soft[0] > 30 && soft[0] < 90, "{soft:?}");
    assert_eq!(soft[0], soft[1]);
    assert!(
        soft[2].abs_diff(soft[0].saturating_add(128)) <= 1,
        "{soft:?}"
    );
}

#[cfg(feature = "gpu")]
const READBACK_STRIDE: u32 = 512;

#[cfg(feature = "gpu")]
pub(super) fn readback(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::Texture,
) -> Vec<u8> {
    let extent = target.size();
    // Readback is only for assertions; the rendering path keeps the capture on GPU.
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("backdrop test readback"),
        size: READBACK_STRIDE as u64 * SIZE as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(READBACK_STRIDE),
                rows_per_image: Some(SIZE as u32),
            },
        },
        extent,
    );
    queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            send.send(result).unwrap();
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .unwrap();
    receive
        .recv_timeout(std::time::Duration::from_secs(30))
        .unwrap()
        .unwrap();
    let pixels = readback.slice(..).get_mapped_range().unwrap();
    let result = pixels.to_vec();
    drop(pixels);
    readback.unmap();
    result
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
fn gpu_saturation_matches_cpu_and_keeps_batched_parameters_independent() {
    use crate::{color_filter::Saturation, gpu_color_filter::SaturationPass};
    let mut context = vello::util::RenderContext::new();
    let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let mut input = Pixmap::new(SIZE, SIZE);
    for (index, pixel) in input.data_as_u8_slice_mut().chunks_exact_mut(4).enumerate() {
        let x = index % SIZE as usize;
        let y = index / SIZE as usize;
        let alpha = [0, 1, 17, 128, 255][y % 5];
        pixel.copy_from_slice(&[
            (x * alpha / (SIZE as usize - 1)) as u8,
            (y * alpha / (SIZE as usize - 1)) as u8,
            ((x + y) % SIZE as usize * alpha / (SIZE as usize - 1)) as u8,
            alpha as u8,
        ]);
    }
    let make_texture = |width| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("saturation parity"),
            size: wgpu::Extent3d {
                width,
                height: SIZE as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    };
    let source = make_texture(SIZE as u32);
    queue.write_texture(
        source.as_image_copy(),
        input.data_as_u8_slice(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(SIZE as u32 * 4),
            rows_per_image: Some(SIZE as u32),
        },
        source.size(),
    );
    let filter = SaturationPass::new(device);
    let outputs: Vec<_> = [0.0, 0.5, 1.0, 1.8, 4.0, 16.0]
        .into_iter()
        .map(|amount| (Saturation::new(amount).unwrap(), make_texture(SIZE as u32)))
        .collect();
    let mut encoder = device.create_command_encoder(&Default::default());
    assert!(
        filter
            .apply(device, &mut encoder, &source, &source.clone(), outputs[0].0)
            .is_err()
    );
    assert!(
        filter
            .apply(
                device,
                &mut encoder,
                &source,
                &make_texture(48),
                outputs[0].0
            )
            .is_err()
    );
    for (amount, target) in &outputs {
        filter
            .apply(device, &mut encoder, &source, target, *amount)
            .unwrap();
    }
    queue.submit([encoder.finish()]);
    for (amount, target) in outputs {
        let actual = readback(device, queue, &target);
        let mut expected = input.clone();
        amount.apply(&mut expected);
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                let source_offset = (y * SIZE as usize + x) * 4;
                let offset = y * READBACK_STRIDE as usize + x * 4;
                let reference = &expected.data_as_u8_slice()[source_offset..source_offset + 4];
                let pixel = &actual[offset..offset + 4];
                assert_eq!(pixel[3], reference[3], "alpha changed at {x},{y}");
                for channel in 0..3 {
                    assert!(
                        pixel[channel].abs_diff(reference[channel]) <= 1,
                        "saturation={} at {x},{y}: {pixel:?} != {reference:?}",
                        amount.amount()
                    );
                    assert!(pixel[channel] <= pixel[3], "non-premultiplied output");
                }
            }
        }
    }

    // Exercise the actual GPU chain: Gaussian output -> saturation, with no
    // readback or CPU completion wait between the two rendering stages.
    let mut scene = vello_gpu::Scene::new(SIZE, SIZE);
    scene.push_filter_layer(blur());
    scene.set_paint(css::RED);
    scene.fill_rect(&Rect::new(0.0, 0.0, 48.0, 96.0));
    scene.set_paint(css::LIME);
    scene.fill_rect(&Rect::new(48.0, 0.0, 96.0, 96.0));
    scene.pop_layer();
    let size = vello_gpu::RenderSize {
        width: SIZE,
        height: SIZE,
    };
    let depth = vello_gpu::Renderer::create_depth_texture_view(device, &size);
    let mut renderer = crate::gpu_pass::SceneRenderer::default();
    renderer
        .draw(
            device,
            queue,
            &scene,
            &vello_gpu::TextureBindings::new(),
            crate::gpu_pass::Target {
                size,
                view: &source.create_view(&Default::default()),
                depth: Some(&depth),
                init: vello_gpu::TargetInit::Clear(Default::default()),
            },
        )
        .unwrap();
    let output = make_texture(SIZE as u32);
    let amount = Saturation::new(1.8).unwrap();
    let mut encoder = device.create_command_encoder(&Default::default());
    filter
        .apply(device, &mut encoder, &source, &output, amount)
        .unwrap();
    queue.submit([encoder.finish()]);
    let before = readback(device, queue, &source);
    let after = readback(device, queue, &output);
    let mut reference = Pixmap::new(SIZE, SIZE);
    for (destination, source_row) in reference
        .data_as_u8_slice_mut()
        .chunks_exact_mut(SIZE as usize * 4)
        .zip(before.chunks_exact(READBACK_STRIDE as usize))
    {
        destination.copy_from_slice(&source_row[..SIZE as usize * 4]);
    }
    amount.apply(&mut reference);
    for (actual, expected) in after
        .chunks_exact(READBACK_STRIDE as usize)
        .zip(reference.data_as_u8_slice().chunks_exact(SIZE as usize * 4))
    {
        for (actual, expected) in actual[..SIZE as usize * 4].iter().zip(expected) {
            assert!(actual.abs_diff(*expected) <= 1);
        }
    }
    let offset = 48 * READBACK_STRIDE as usize + 44 * 4;
    assert!(
        after[offset] > before[offset] + 10,
        "saturation must run after blur"
    );
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
fn shared_composite_plan_matches_cpu_for_nested_roots_clips_and_siblings() {
    use crate::{
        color_filter::Saturation,
        composite::{Layer, Plan, Step},
        render::ClipSnapshot,
    };
    let mut context = vello::util::RenderContext::new();
    let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let mut compositor = crate::composite_gpu::Compositor::new(device);
    let clip = |x, width| ClipSnapshot {
        bounds: [x, 0.0, width, 96.0],
        radius: 0.0,
        radii: None,
    };
    let glass = |saturation| Layer::Backdrop {
        clip: clip(0.0, 96.0),
        blur: 4.0,
        saturation: Saturation::new(saturation).unwrap(),
        opacity: 1.0,
    };
    let programs = [
        vec![
            Step::Draw(0..1),
            Step::Push(Layer::Clip(clip(24.0, 48.0))),
            Step::Draw(1..2),
            Step::Push(Layer::Clip(clip(0.0, 96.0))),
            Step::Draw(2..3),
            Step::Push(glass(0.0)),
            Step::Pop,
            Step::Pop,
            Step::Pop,
        ],
        vec![
            Step::Draw(0..1),
            Step::Push(Layer::Opacity(0.5)),
            Step::Push(glass(0.0)),
            Step::Pop,
            Step::Pop,
        ],
        vec![
            Step::Draw(0..1),
            Step::Push(Layer::Opacity(0.5)),
            Step::Draw(1..3),
            Step::Push(glass(1.8)),
            Step::Draw(3..4),
            Step::Pop,
            Step::Pop,
        ],
        vec![
            Step::Draw(0..1),
            Step::Push(glass(1.0)),
            Step::Draw(1..2),
            Step::Pop,
            Step::Push(glass(0.0)),
            Step::Pop,
        ],
    ];
    let paints = [
        (css::BLUE, Rect::new(0.0, 0.0, 96.0, 96.0)),
        (css::RED, Rect::new(0.0, 0.0, 96.0, 96.0)),
        (css::WHITE, Rect::new(0.0, 0.0, 96.0, 48.0)),
        (css::LIME, Rect::new(44.0, 40.0, 46.0, 56.0)),
    ];
    for (case, steps) in programs.into_iter().enumerate() {
        let plan = Plan::new(SIZE, SIZE, paints.len(), steps).unwrap();
        let mut cpu = crate::composite_cpu::Cpu::new(
            |range: std::ops::Range<usize>, scene: &mut RenderContext, _: &mut Resources| {
                for index in range {
                    scene.set_paint(paints[index].0);
                    scene.fill_rect(&paints[index].1);
                }
                Ok(())
            },
        );
        let expected = plan.render(&mut cpu).unwrap();
        let mut visits = [0; 4];
        let output = compositor
            .render(
                &plan,
                device,
                queue,
                |range: std::ops::Range<usize>,
                 scene: &mut vello_gpu::Scene,
                 _: &mut vello_gpu::TextureBindings| {
                    for index in range {
                        visits[index] += 1;
                        scene.set_paint(paints[index].0);
                        scene.fill_rect(&paints[index].1);
                    }
                    Ok(())
                },
            )
            .unwrap();
        assert!(
            visits.iter().all(|count| *count <= 1),
            "draws replayed in case {case}"
        );
        let actual = readback(device, queue, output.texture());
        for y in 0..SIZE as usize {
            for x in 0..SIZE as usize {
                let offset = y * READBACK_STRIDE as usize + x * 4;
                let reference = (y * SIZE as usize + x) * 4;
                for channel in 0..4 {
                    let actual = actual[offset + channel];
                    let expected = expected.data_as_u8_slice()[reference + channel];
                    assert!(
                        actual.abs_diff(expected) <= 3,
                        "case {case}, {x},{y}, channel {channel}: {actual} vs {expected}"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
fn compositor_pool_reuses_submitted_work_without_overwriting_live_outputs() {
    use crate::{
        color_filter::Saturation,
        composite::{Layer, Plan, Step},
        render::ClipSnapshot,
    };
    let shared = crate::gpu::Context::default();
    let other_page = shared.clone();
    let mut context = shared.lock().unwrap();
    let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let plan = Plan::new(
        SIZE,
        SIZE,
        1,
        vec![
            Step::Draw(0..1),
            Step::Push(Layer::Backdrop {
                clip: ClipSnapshot {
                    bounds: [16.0, 16.0, 64.0, 64.0],
                    radius: 0.0,
                    radii: None,
                },
                blur: 4.0,
                saturation: Saturation::new(1.0).unwrap(),
                opacity: 1.0,
            }),
            Step::Pop,
        ],
    )
    .unwrap();
    let mut compositor = crate::composite_gpu::Compositor::new(device);
    let render = |compositor: &mut crate::composite_gpu::Compositor, color| {
        compositor
            .render(&plan, device, queue, |_, scene, _| {
                scene.set_paint(color);
                scene.fill_rect(&frame());
                Ok(())
            })
            .unwrap()
    };
    let first = render(&mut compositor, css::RED);
    let held = first.clone();
    drop(first);
    drop(render(&mut compositor, css::BLUE));
    let warm = compositor.texture_stats();
    for _ in 0..8 {
        drop(render(&mut compositor, css::BLUE));
    }
    assert_eq!(
        compositor.texture_stats().allocations,
        warm.allocations,
        "steady-state frames must reuse scratch textures"
    );
    assert!(compositor.texture_stats().reuses > warm.reuses);
    assert_eq!(
        compositor.texture_stats().depth_allocations,
        warm.depth_allocations
    );
    assert_eq!(compositor.texture_stats().depth_textures, 2);
    let last = render(&mut compositor, css::LIME);
    // A second page shares the queue, but its scratch and output stay independent.
    let mut other = crate::composite_gpu::Compositor::new(device);
    let blue = render(&mut other, css::BLUE);
    other.clear();
    drop(other);
    // Read only after all frames are submitted; no CPU waits serialize reuse.
    let red = readback(device, queue, held.texture());
    let green = readback(device, queue, last.texture());
    let blue_pixels = readback(device, queue, blue.texture());
    let offset = 48 * READBACK_STRIDE as usize + 48 * 4;
    assert_eq!(&red[offset..offset + 4], [255, 0, 0, 255]);
    assert_eq!(&green[offset..offset + 4], [0, 255, 0, 255]);
    assert_eq!(&blue_pixels[offset..offset + 4], [0, 0, 255, 255]);
    drop(blue);
    compositor.clear();
    assert_eq!(compositor.texture_stats().idle_bytes, 0);
    assert_eq!(compositor.texture_stats().depth_textures, 0);
    assert_eq!(
        compositor.texture_stats().bytes,
        2 * u64::from(SIZE).pow(2) * 4
    );
    drop(held);
    drop(last);
    assert_eq!(
        compositor.texture_stats().bytes,
        0,
        "pre-clear leases must not refill the pool"
    );
    drop(render(&mut compositor, css::BLUE));
    assert!(compositor.texture_stats().idle_bytes > 0);
    compositor.clear();
    assert_eq!(compositor.texture_stats().bytes, 0);
    drop(context);
    drop(shared);
    assert_eq!(other_page.device_count().unwrap(), 1);
}

#[cfg(feature = "gpu")]
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
fn mirrored_backdrop_crop_preserves_gpu_card_edges() {
    use crate::{
        color_filter::Saturation,
        composite::{Layer, Plan, Step},
        render::ClipSnapshot,
    };
    let mut context = vello::util::RenderContext::new();
    let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
    let device = &context.devices[id].device;
    let queue = &context.devices[id].queue;
    let mut compositor = crate::composite_gpu::Compositor::new(device);
    for blur in [4.0, 20.0] {
        let plan = Plan::new(
            SIZE,
            SIZE,
            1,
            vec![
                Step::Draw(0..1),
                Step::Push(Layer::Backdrop {
                    clip: ClipSnapshot {
                        bounds: [16.0, 16.0, 64.0, 64.0],
                        radius: 0.0,
                        radii: None,
                    },
                    blur,
                    saturation: Saturation::new(1.0).unwrap(),
                    opacity: 1.0,
                }),
                Step::Pop,
            ],
        )
        .unwrap();
        let output = compositor
            .render(&plan, device, queue, |_, scene, _| {
                scene.set_paint(css::RED);
                for rect in [
                    Rect::new(0.0, 0.0, 96.0, 16.0),
                    Rect::new(0.0, 80.0, 96.0, 96.0),
                    Rect::new(0.0, 16.0, 16.0, 80.0),
                    Rect::new(80.0, 16.0, 96.0, 80.0),
                ] {
                    scene.fill_rect(&rect);
                }
                scene.set_paint(AlphaColor::<Srgb>::from_rgba8(0, 0, 255, 128));
                scene.fill_rect(&Rect::new(16.0, 16.0, 80.0, 80.0));
                Ok(())
            })
            .unwrap();
        let pixels = readback(device, queue, output.texture());
        for (x, y) in [
            (16, 48),
            (79, 48),
            (48, 16),
            (48, 79),
            (16, 16),
            (79, 79),
            (48, 48),
        ] {
            let offset = y * READBACK_STRIDE as usize + x * 4;
            let pixel = &pixels[offset..offset + 4];
            assert_eq!(
                pixel[0], 0,
                "outside red leaked: blur={blur}, {x},{y}: {pixel:?}"
            );
            assert_eq!(pixel[1], 0);
            assert!(
                pixel[2].abs_diff(192) <= 1 && pixel[3].abs_diff(192) <= 1,
                "blur={blur}, {x},{y}: {pixel:?}"
            );
        }
        let offset = 48 * READBACK_STRIDE as usize + 15 * 4;
        assert_eq!(&pixels[offset..offset + 4], [255, 0, 0, 255]);
    }
    let plan = Plan::new(
        SIZE,
        SIZE,
        1,
        vec![
            Step::Draw(0..1),
            Step::Push(Layer::Backdrop {
                clip: ClipSnapshot {
                    bounds: [16.0, 16.0, 64.0, 64.0],
                    radius: 0.0,
                    radii: None,
                },
                blur: 20.0,
                saturation: Saturation::new(1.0).unwrap(),
                opacity: 1.0,
            }),
            Step::Pop,
        ],
    )
    .unwrap();
    let output = compositor
        .render(&plan, device, queue, |_, scene, _| {
            scene.set_paint(css::LIME);
            scene.fill_rect(&frame());
            scene.set_paint(css::RED);
            scene.fill_rect(&Rect::new(16.0, 16.0, 24.0, 80.0));
            scene.set_paint(css::BLUE);
            scene.fill_rect(&Rect::new(24.0, 16.0, 80.0, 80.0));
            Ok(())
        })
        .unwrap();
    let pixels = readback(device, queue, output.texture());
    let offset = 48 * READBACK_STRIDE as usize + 16 * 4;
    let edge = &pixels[offset..offset + 4];
    assert!(
        (50..120).contains(&edge[0]) && edge[1] == 0 && edge[2] > 120 && edge[3] == 255,
        "mirror must differ from pad/wrap: {edge:?}"
    );
}
