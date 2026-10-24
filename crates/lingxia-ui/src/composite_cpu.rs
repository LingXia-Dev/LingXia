//! CPU reference backend for the shared compositor.
use crate::composite::{Backend, Layer};
use std::{ops::Range, sync::Arc};
use vello_cpu::{Image, ImageSource, Pixmap, RenderContext, Resources, kurbo::Rect};

pub struct Cpu<F> {
    draw: F,
    resources: Resources,
}

impl<F> Cpu<F> {
    pub fn new(draw: F) -> Self {
        Self {
            draw,
            resources: Resources::new(),
        }
    }

    fn finish(&mut self, mut context: RenderContext, width: u16, height: u16) -> Arc<Pixmap> {
        context.flush();
        let mut output = Pixmap::new(width, height);
        context.render(&mut output, &mut self.resources);
        Arc::new(output)
    }
}

fn image(context: &mut RenderContext, pixels: &Arc<Pixmap>) {
    context.set_paint(Image {
        image: ImageSource::Pixmap(pixels.clone()),
        sampler: Default::default(),
    });
    context.fill_rect(&Rect::new(
        0.0,
        0.0,
        pixels.width() as f64,
        pixels.height() as f64,
    ));
}

impl<F> Backend for Cpu<F>
where
    F: FnMut(Range<usize>, &mut RenderContext, &mut Resources) -> Result<(), String>,
{
    type Surface = Arc<Pixmap>;

    fn blank(&mut self, width: u16, height: u16) -> Result<Self::Surface, String> {
        Ok(Arc::new(Pixmap::new(width, height)))
    }

    fn draw(&mut self, target: &mut Self::Surface, span: Range<usize>) -> Result<(), String> {
        if span.is_empty() {
            return Ok(());
        }
        let mut context = RenderContext::new(target.width(), target.height());
        image(&mut context, target);
        (self.draw)(span, &mut context, &mut self.resources)?;
        *target = self.finish(context, target.width(), target.height());
        Ok(())
    }

    fn snapshot(&mut self, source: &Self::Surface) -> Result<Self::Surface, String> {
        Ok(source.clone())
    }

    fn composite(
        &mut self,
        target: &mut Self::Surface,
        source: &Self::Surface,
        layer: &Layer,
    ) -> Result<(), String> {
        let mut context = RenderContext::new(target.width(), target.height());
        image(&mut context, target);
        match layer {
            Layer::Clip(clip) => context.push_clip_layer(&clip.path()),
            Layer::Opacity(alpha) | Layer::Backdrop { opacity: alpha, .. } => {
                context.push_opacity_layer(*alpha)
            }
        }
        image(&mut context, source);
        context.pop_layer();
        *target = self.finish(context, target.width(), target.height());
        Ok(())
    }

    fn filter(&mut self, source: Self::Surface, layer: &Layer) -> Result<Self::Surface, String> {
        let Layer::Backdrop {
            clip,
            blur,
            saturation,
            ..
        } = layer
        else {
            return Err("UI_COMPOSITE: expected backdrop".into());
        };
        let (width, height) = (source.width(), source.height());
        let Some(region) = crate::backdrop_region::Region::new(*clip, width, height, *blur)? else {
            drop(source);
            return self.blank(width, height);
        };
        let mut cropped = Pixmap::new(region.width, region.height);
        for (y, row) in cropped
            .data_as_u8_slice_mut()
            .chunks_exact_mut(region.width as usize * 4)
            .enumerate()
        {
            let offset = ((region.y as usize + y) * width as usize + region.x as usize) * 4;
            row.copy_from_slice(
                &source.data_as_u8_slice()[offset..offset + region.width as usize * 4],
            );
        }
        drop(source);
        let mut context = RenderContext::new(region.width, region.height);
        if *blur > 0.0 {
            context.push_filter_layer(region.filter.clone());
        }
        // Populate the filter's entire padded input with reflected card pixels;
        // Vello's own edge mode would otherwise reflect the padded layer edges.
        context.set_paint(Image {
            image: ImageSource::Pixmap(Arc::new(cropped)),
            sampler: vello_cpu::peniko::ImageSampler::new()
                .with_extend(vello_cpu::peniko::Extend::Reflect),
        });
        context.fill_rect(&region.sampling_bounds);
        if *blur > 0.0 {
            context.pop_layer();
        }
        let mut filtered = self.finish(context, region.width, region.height);
        saturation.apply(Arc::get_mut(&mut filtered).expect("new private filter image"));
        let mut context = RenderContext::new(width, height);
        context.push_clip_layer(&clip.path());
        context.set_paint(Image {
            image: ImageSource::Pixmap(filtered),
            sampler: Default::default(),
        });
        context.set_paint_transform(vello_cpu::kurbo::Affine::translate((
            f64::from(region.x),
            f64::from(region.y),
        )));
        context.fill_rect(&region.bounds());
        context.pop_layer();
        Ok(self.finish(context, width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        color_filter::Saturation,
        composite::{Plan, Step},
        render::ClipSnapshot,
    };
    use vello_cpu::{
        color::{AlphaColor, Srgb, palette::css},
        kurbo::Shape,
    };

    fn clip(x: f32, width: f32) -> ClipSnapshot {
        ClipSnapshot {
            bounds: [x, 0.0, width, 96.0],
            radius: 0.0,
            radii: None,
        }
    }
    fn glass(blur: f32, saturation: f32) -> Layer {
        Layer::Backdrop {
            clip: clip(0.0, 96.0),
            blur,
            saturation: Saturation::new(saturation).unwrap(),
            opacity: 1.0,
        }
    }
    fn pixel(image: &Pixmap, x: usize, y: usize) -> &[u8] {
        let offset = (y * 96 + x) * 4;
        &image.data_as_u8_slice()[offset..offset + 4]
    }

    #[test]
    fn mirror_capture_excludes_colors_outside_owner_and_preserves_translucent_edges() {
        for blur in [4.0, 20.0] {
            let mut source = Pixmap::new(96, 96);
            for (index, pixel) in source
                .data_as_u8_slice_mut()
                .chunks_exact_mut(4)
                .enumerate()
            {
                let (x, y) = (index % 96, index / 96);
                pixel.copy_from_slice(if (16..80).contains(&x) && (16..80).contains(&y) {
                    &[0, 0, 128, 128]
                } else {
                    &[255, 0, 0, 255]
                });
            }
            let mut backend =
                Cpu::new(|_: Range<usize>, _: &mut RenderContext, _: &mut Resources| Ok(()));
            let output = backend
                .filter(
                    Arc::new(source),
                    &Layer::Backdrop {
                        clip: ClipSnapshot {
                            bounds: [16.0, 16.0, 64.0, 64.0],
                            radius: 0.0,
                            radii: None,
                        },
                        blur,
                        saturation: Saturation::new(1.0).unwrap(),
                        opacity: 1.0,
                    },
                )
                .unwrap();
            for (x, y) in [
                (16, 48),
                (79, 48),
                (48, 16),
                (48, 79),
                (16, 16),
                (79, 79),
                (48, 48),
            ] {
                assert_eq!(
                    pixel(&output, x, y),
                    [0, 0, 128, 128],
                    "blur={blur}, {x},{y}"
                );
            }
            assert_eq!(pixel(&output, 15, 48), [0, 0, 0, 0]);
        }
    }

    #[test]
    fn mirror_repeats_interior_pattern_instead_of_clamping_or_wrapping() {
        let mut source = Pixmap::new(96, 96);
        for (index, pixel) in source
            .data_as_u8_slice_mut()
            .chunks_exact_mut(4)
            .enumerate()
        {
            let (x, y) = (index % 96, index / 96);
            pixel.copy_from_slice(if !(16..80).contains(&x) || !(16..80).contains(&y) {
                &[0, 255, 0, 255]
            } else if x < 24 {
                &[255, 0, 0, 255]
            } else {
                &[0, 0, 255, 255]
            });
        }
        let mut backend =
            Cpu::new(|_: Range<usize>, _: &mut RenderContext, _: &mut Resources| Ok(()));
        let output = backend
            .filter(
                Arc::new(source),
                &Layer::Backdrop {
                    clip: ClipSnapshot {
                        bounds: [16.0, 16.0, 64.0, 64.0],
                        radius: 0.0,
                        radii: None,
                    },
                    blur: 20.0,
                    saturation: Saturation::new(1.0).unwrap(),
                    opacity: 1.0,
                },
            )
            .unwrap();
        let edge = pixel(&output, 16, 48);
        // Reflect contributes the eight red columns on both sides. Pad would
        // keep the negative half-plane red (~167); wrap gives only ~40 red.
        assert!(
            (50..120).contains(&edge[0]) && edge[1] == 0 && edge[2] > 120 && edge[3] == 255,
            "{edge:?}"
        );
    }

    #[test]
    fn automatic_root_selection_keeps_group_opacity_and_foreground_sharp() {
        let mut visits = [0; 5];
        let mut backend = Cpu::new(
            |range: Range<usize>, context: &mut RenderContext, _: &mut Resources| {
                for index in range {
                    visits[index] += 1;
                    match index {
                        0 => {
                            context.set_paint(css::BLUE);
                            context.fill_rect(&Rect::new(0.0, 0.0, 96.0, 96.0));
                        }
                        1 => {
                            context.set_paint(css::BLACK);
                            context.fill_rect(&Rect::new(0.0, 0.0, 96.0, 96.0));
                        }
                        2 => {
                            context.set_paint(css::WHITE);
                            context.fill_rect(&Rect::new(48.0, 0.0, 96.0, 96.0));
                        }
                        3 => {
                            context.set_paint(AlphaColor::<Srgb>::from_rgba8(255, 255, 255, 32));
                            context.fill_path(
                                &vello_cpu::kurbo::RoundedRect::new(16.0, 16.0, 80.0, 80.0, 12.0)
                                    .to_path(0.1),
                            );
                        }
                        4 => {
                            context.set_paint(css::RED);
                            context.fill_rect(&Rect::new(44.0, 40.0, 46.0, 56.0));
                        }
                        _ => unreachable!(),
                    }
                }
                Ok(())
            },
        );
        let plan = Plan::new(
            96,
            96,
            5,
            vec![
                Step::Draw(0..1),
                Step::Push(Layer::Opacity(0.5)),
                Step::Draw(1..3),
                Step::Push(Layer::Backdrop {
                    clip: ClipSnapshot {
                        bounds: [16.0, 16.0, 64.0, 64.0],
                        radius: 12.0,
                        radii: None,
                    },
                    blur: 4.0,
                    saturation: Saturation::new(1.0).unwrap(),
                    opacity: 1.0,
                }),
                Step::Draw(3..5),
                Step::Pop,
                Step::Pop,
            ],
        )
        .unwrap();
        let output = plan.render(&mut backend).unwrap();
        drop(backend);
        assert_eq!(
            visits, [1; 5],
            "background paints must not be replayed for capture"
        );
        let red = pixel(&output, 44, 48);
        assert!(
            (127..=128).contains(&red[0]) && red[1] == 0 && (127..=128).contains(&red[2]),
            "{red:?}"
        );
        let soft = pixel(&output, 47, 32);
        assert!(
            soft[0] > 30 && soft[0] < 90 && soft[0] == soft[1],
            "{soft:?}"
        );
        assert_eq!(pixel(&output, 43, 48)[0], pixel(&output, 43, 48)[1]);
        assert_eq!(pixel(&output, 16, 16)[0], 0);
    }

    #[test]
    fn unfinished_nested_overflow_clips_join_capture_from_inner_to_outer() {
        let mut backend = Cpu::new(
            |range: Range<usize>, context: &mut RenderContext, _: &mut Resources| {
                for index in range {
                    context.set_paint([css::BLUE, css::RED, css::WHITE][index]);
                    context.fill_rect(&Rect::new(
                        0.0,
                        0.0,
                        96.0,
                        if index == 2 { 48.0 } else { 96.0 },
                    ));
                }
                Ok(())
            },
        );
        let plan = Plan::new(
            96,
            96,
            3,
            vec![
                Step::Draw(0..1),
                Step::Push(Layer::Clip(clip(24.0, 48.0))),
                Step::Draw(1..2),
                Step::Push(Layer::Clip(clip(0.0, 96.0))),
                Step::Draw(2..3),
                Step::Push(glass(4.0, 0.0)),
                Step::Pop,
                Step::Pop,
                Step::Pop,
            ],
        )
        .unwrap();
        let output = plan.render(&mut backend).unwrap();
        assert_eq!(pixel(&output, 4, 24), [0, 0, 255, 255]);
        assert_eq!(pixel(&output, 80, 24), [0, 0, 255, 255]);
        let edge = pixel(&output, 24, 24);
        assert!(
            edge[0] > 90 && edge[0] < 200 && edge[0] == edge[1] && edge[1] == edge[2],
            "{edge:?}"
        );
        assert_eq!(pixel(&output, 48, 72), [54, 54, 54, 255]);
    }

    #[test]
    fn backdrop_inside_empty_opacity_root_does_not_filter_outer_page() {
        let mut backend = Cpu::new(
            |_: Range<usize>, context: &mut RenderContext, _: &mut Resources| {
                context.set_paint(css::BLUE);
                context.fill_rect(&Rect::new(0.0, 0.0, 96.0, 96.0));
                Ok(())
            },
        );
        let plan = Plan::new(
            96,
            96,
            1,
            vec![
                Step::Draw(0..1),
                Step::Push(Layer::Opacity(0.5)),
                Step::Push(glass(0.0, 0.0)),
                Step::Pop,
                Step::Pop,
            ],
        )
        .unwrap();
        let output = plan.render(&mut backend).unwrap();
        assert_eq!(pixel(&output, 48, 48), [0, 0, 255, 255]);
    }

    #[test]
    fn later_sibling_filters_prior_siblings_completed_content() {
        let mut backend = Cpu::new(
            |range: Range<usize>, context: &mut RenderContext, _: &mut Resources| {
                for index in range {
                    context.set_paint(if index == 0 { css::WHITE } else { css::RED });
                    context.fill_rect(&if index == 0 {
                        Rect::new(0.0, 0.0, 96.0, 96.0)
                    } else {
                        Rect::new(16.0, 16.0, 40.0, 80.0)
                    });
                }
                Ok(())
            },
        );
        let plan = Plan::new(
            96,
            96,
            2,
            vec![
                Step::Draw(0..1),
                Step::Push(glass(0.0, 1.0)),
                Step::Draw(1..2),
                Step::Pop,
                Step::Push(glass(4.0, 0.0)),
                Step::Pop,
            ],
        )
        .unwrap();
        let output = plan.render(&mut backend).unwrap();
        let red = pixel(&output, 28, 48);
        assert!(
            red[0] < 80 && red[0] == red[1] && red[1] == red[2],
            "{red:?}"
        );
    }
}
