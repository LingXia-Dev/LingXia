//! GPU scene encoding of a display list.
use crate::renderer::cpu_paint::image_geometry;
use crate::scene::display::*;
use crate::scene::node::*;
use crate::scene::paint::*;
use std::{collections::HashMap, ops::Range, sync::Arc};
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Affine, Rect as PaintRect, Shape};

impl DisplayList {
    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_scene(
        &self,
        cache: &mut glifo::GlyphPrepCache,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
    ) -> Result<(), String> {
        context.reset_and_resize(self.width, self.height);
        self.sparse_span(
            cache,
            context,
            image_ids,
            glyph_images,
            0..self.commands.len() + 1,
        )
    }

    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_span(
        &self,
        cache: &mut glifo::GlyphPrepCache,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        span: Range<usize>,
    ) -> Result<(), String> {
        let (background, paints) = self.composition_span(span)?;
        if background {
            context.set_paint(AlphaColor::<Srgb>::from_rgb8(246, 247, 251));
            context.fill_rect(&PaintRect::new(
                0.0,
                0.0,
                self.width as f64,
                self.height as f64,
            ));
        }
        self.sparse_paints(
            cache,
            context,
            image_ids,
            glyph_images,
            self.transform_before(paints.start),
            &self.commands[paints],
        )
    }

    /// One segment of a frame drawn on its own: the clips open at its start are pushed
    /// again under the transforms they were opened with, and every clip open at its end
    /// is closed.
    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_segment(
        &self,
        cache: &mut glifo::GlyphPrepCache,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        segment: &crate::renderer::gpu::scene::Segment,
    ) -> Result<(), String> {
        context.reset_and_resize(self.width, self.height);
        if segment.background {
            context.set_paint(AlphaColor::<Srgb>::from_rgb8(246, 247, 251));
            context.fill_rect(&PaintRect::new(
                0.0,
                0.0,
                self.width as f64,
                self.height as f64,
            ));
        }
        for (clip, transform) in &segment.clips {
            context.set_transform(
                Affine::scale(self.scale as f64) * checked_paint_transform(*transform)?,
            );
            push_clip(context, clip);
        }
        self.sparse_paints(
            cache,
            context,
            image_ids,
            glyph_images,
            segment.transform,
            &self.commands[segment.range.clone()],
        )?;
        for _ in 0..segment.open_at_end {
            context.pop_clip();
        }
        Ok(())
    }

    #[cfg(feature = "gpu")]
    #[allow(clippy::too_many_arguments)]
    fn sparse_paints(
        &self,
        cache: &mut glifo::GlyphPrepCache,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        initial_transform: Affine,
        commands: &[Paint],
    ) -> Result<(), String> {
        let mut transform =
            Affine::scale(self.scale as f64) * checked_paint_transform(initial_transform)?;
        context.set_transform(transform);
        for command in commands {
            match command {
                Paint::Transform(matrix) => {
                    transform =
                        Affine::scale(self.scale as f64) * checked_paint_transform(*matrix)?;
                    context.set_transform(transform);
                }
                Paint::Vector {
                    path,
                    color: [r, g, b, a],
                    gradient,
                    bounds,
                } => {
                    context.push_clip_layer(
                        &ClipSnapshot {
                            bounds: *bounds,
                            radius: 0.0,
                            radii: None,
                            transform: None,
                        }
                        .path(),
                    );
                    if let Some(brush) = gradient {
                        context.set_paint(brush.gradient.clone());
                        context.set_paint_transform(brush.transform);
                    } else {
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    }
                    context.fill_path(path);
                    context.reset_paint_transform();
                    context.pop_layer();
                }
                Paint::RasterShadow { raster } => {
                    let (rect, transform, clip) = image_geometry(
                        &raster.bitmap,
                        raster.bounds,
                        0.0,
                        crate::scene::image::Fit::Fill,
                        [0.5, 0.5],
                    )?;
                    let id = image_ids
                        .get(&(Arc::as_ptr(raster.bitmap.pixels()) as usize))
                        .ok_or("UI_SHADOW: texture not uploaded")?;
                    context.push_clip_path(&clip);
                    context.set_paint(vello_gpu::Image {
                        image: id.clone(),
                        sampler: Default::default(),
                    });
                    context.set_paint_transform(transform);
                    context.fill_rect(&rect);
                    context.reset_paint_transform();
                    context.pop_clip();
                }
                Paint::Shadow {
                    shadow,
                    color: [r, g, b, a],
                    bounds,
                    radius,
                } => {
                    if let Some(shape) = shadow.geometry(*bounds, *radius) {
                        context.push_clip_layer(&shape.knockout());
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                        if shape.sigma == 0.0 {
                            context.fill_path(&shape.shape().to_path(0.1));
                        } else {
                            context.fill_blurred_rounded_rect(
                                &shape.rect,
                                shape.radius,
                                shape.sigma,
                                false,
                            );
                        }
                        context.pop_layer();
                    }
                }
                Paint::Area {
                    points,
                    range,
                    color,
                } => {
                    context.set_paint(area_gradient(*range, *color));
                    context.fill_path(&area_path(points));
                }
                Paint::Stroke {
                    round,
                    color: [r, g, b, a],
                    points,
                    width,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.set_stroke(check_stroke(*width, *round));
                    context.stroke_path(&check_path(points));
                }
                Paint::Clip(shape) => push_clip(context, shape),
                Paint::PopClip => context.pop_clip(),
                Paint::PopOpacity => context.pop_layer(),
                Paint::Opacity { alpha, .. } => context.push_opacity_layer(*alpha),
                Paint::Backdrop { .. } | Paint::PopBackdrop => {
                    return Err("UI_COMPOSITE: backdrop requires composition".into());
                }
                Paint::Border {
                    corners,
                    color: [r, g, b, a],
                    bounds,
                    widths,
                    radius,
                    style,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.fill_path(
                        &crate::scene::border::Border::with_radii(
                            *bounds,
                            *widths,
                            corners.unwrap_or([[*radius; 2]; 4]),
                        )
                        .styled_path(*style),
                    );
                }
                Paint::Gradient {
                    corners,
                    gradient,
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    let brush = gradient.brush([*x, *y, *w, *h]);
                    context.set_paint(brush.gradient);
                    context.set_paint_transform(brush.transform);
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [*x, *y, *w, *h],
                            radius: *radius,
                            radii: *corners,
                            transform: None,
                        }
                        .path(),
                    );
                    context.reset_paint_transform();
                }
                Paint::Rect {
                    corners,
                    color: [r, g, b, a],
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [*x, *y, *w, *h],
                            radius: *radius,
                            radii: *corners,
                            transform: None,
                        }
                        .path(),
                    );
                }
                Paint::Image {
                    src,
                    bounds,
                    radius,
                    fit,
                    position,
                } => {
                    if let Some(bitmap) = self.images.get(src) {
                        let (rect, transform, clip) =
                            image_geometry(bitmap, *bounds, *radius, *fit, *position)?;
                        let id = image_ids
                            .get(&(Arc::as_ptr(bitmap.pixels()) as usize))
                            .ok_or("UI_IMAGE: texture not uploaded")?;
                        context.push_clip_path(&clip);
                        context.set_paint(vello_gpu::Image {
                            image: id.clone(),
                            sampler: Default::default(),
                        });
                        context.set_paint_transform(transform);
                        context.fill_rect(&rect);
                        context.reset_paint_transform();
                        context.pop_clip();
                    }
                }
                Paint::Text {
                    color: [r, g, b, a],
                    font,
                    size,
                    coordinates,
                    skew,
                    glyphs,
                    ink,
                    gradient,
                } => {
                    // `bg-clip-text`: the glyphs are a coverage mask the gradient is drawn through.
                    let clipped = gradient.as_ref().zip(*ink);
                    if clipped.is_some() {
                        context.push_layer(None, None, None, None, None);
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(0, 0, 0, 255));
                    } else {
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    }
                    if crate::text::outlines::external(font) {
                        if let Some(path) =
                            crate::text::outlines::run_path(font, *size, coordinates, *skew, glyphs)
                        {
                            context.fill_path(&path);
                        }
                    } else {
                        glifo::GlyphRunBuilder::new(
                            font.clone(),
                            transform,
                            Affine::IDENTITY,
                            crate::renderer::gpu::text::Backend {
                                sink: crate::renderer::gpu::text::GlyphSink {
                                    scene: context,
                                    images: glyph_images,
                                },
                                cache,
                            },
                        )
                        .font_size(*size)
                        .glyph_transform(Affine::new([1.0, 0.0, -*skew, 1.0, 0.0, 0.0]))
                        .normalized_coords(coordinates)
                        .fill_glyphs(glyphs.iter().copied())
                        .map_err(|e| e.to_string())?;
                    }
                    if let Some((brush, [x, y, w, h])) = clipped {
                        context.push_layer(
                            None,
                            Some(crate::renderer::cpu_paint::text_clip_blend()),
                            None,
                            None,
                            None,
                        );
                        context.set_paint(brush.gradient.clone());
                        context.set_paint_transform(brush.transform);
                        context.fill_rect(&PaintRect::new(
                            f64::from(x),
                            f64::from(y),
                            f64::from(x + w),
                            f64::from(y + h),
                        ));
                        context.reset_paint_transform();
                        context.pop_layer();
                        context.pop_layer();
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "gpu")]
fn push_clip(context: &mut vello_gpu::Scene, shape: &ClipSnapshot) {
    if shape.radius == 0.0 && shape.radii.is_none() {
        let [x, y, w, h] = shape.bounds;
        context.push_clip_rect(&PaintRect::new(
            x as f64,
            y as f64,
            (x + w) as f64,
            (y + h) as f64,
        ));
    } else {
        context.push_clip_path(&shape.path());
    }
}
