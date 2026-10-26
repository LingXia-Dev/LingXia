//! CPU reference rasterization of a display list.
use crate::scene::display::*;
use crate::scene::node::*;
use crate::scene::paint::*;
use std::sync::Arc;
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Affine, Rect as PaintRect, RoundedRect, Shape};
use vello_cpu::{Pixmap, RenderContext, Resources};

impl DisplayList {
    /// CPU reference execution of the multipass path; ordinary pages retain
    /// the single-context fast path until backdrop authoring is integrated.
    pub fn rasterize_composited(self) -> Result<Frame, String> {
        let plan = self.composition_plan()?;
        let mut backend = crate::renderer::cpu::Cpu::new(
            |span, context: &mut RenderContext, resources: &mut Resources| {
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
                self.paint_cpu(
                    context,
                    resources,
                    self.transform_before(paints.start),
                    self.commands[paints].iter().cloned(),
                )
            },
        );
        let pixels = plan.render(&mut backend)?;
        drop(backend);
        let pixels = Arc::try_unwrap(pixels)
            .map_err(|_| "UI_COMPOSITE: output still borrowed".to_owned())?;
        Ok(Frame {
            pixels,
            nodes: self.nodes,
        })
    }

    pub fn requires_composition(&self) -> bool {
        self.commands
            .iter()
            .any(|p| matches!(p, Paint::Backdrop { .. }))
    }

    pub fn rasterize(mut self) -> Result<Frame, String> {
        if self.requires_composition() {
            return self.rasterize_composited();
        }
        let mut context = RenderContext::new(self.width, self.height);
        let mut resources = Resources::new();
        context.set_paint(AlphaColor::<Srgb>::from_rgb8(246, 247, 251));
        context.fill_rect(&PaintRect::new(
            0.0,
            0.0,
            self.width as f64,
            self.height as f64,
        ));
        let commands = std::mem::take(&mut self.commands);
        self.paint_cpu(&mut context, &mut resources, Affine::IDENTITY, commands)?;
        context.flush();
        let mut pixels = Pixmap::new(self.width, self.height);
        context.render(&mut pixels, &mut resources);
        Ok(Frame {
            pixels,
            nodes: self.nodes,
        })
    }

    fn paint_cpu(
        &self,
        context: &mut RenderContext,
        resources: &mut Resources,
        initial_transform: Affine,
        commands: impl IntoIterator<Item = Paint>,
    ) -> Result<(), String> {
        context.set_transform(
            Affine::scale(self.scale as f64) * checked_paint_transform(initial_transform)?,
        );
        for command in commands {
            match command {
                Paint::Transform(matrix) => context.set_transform(
                    Affine::scale(self.scale as f64) * checked_paint_transform(matrix)?,
                ),
                Paint::Vector {
                    path,
                    color: [r, g, b, a],
                    gradient,
                    bounds,
                } => {
                    context.push_clip_layer(
                        &ClipSnapshot {
                            bounds,
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
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    }
                    context.fill_path(&path);
                    context.reset_paint_transform();
                    context.pop_layer();
                }
                Paint::RasterShadow { raster } => {
                    let (rect, transform, clip) = image_geometry(
                        &raster.bitmap,
                        raster.bounds,
                        0.0,
                        crate::scene::image::Fit::Fill,
                    )?;
                    context.push_clip_layer(&clip);
                    context.set_paint(vello_cpu::Image {
                        image: vello_cpu::ImageSource::Pixmap(raster.bitmap.pixels().clone()),
                        sampler: Default::default(),
                    });
                    context.set_paint_transform(transform);
                    context.fill_rect(&rect);
                    context.reset_paint_transform();
                    context.pop_layer();
                }
                Paint::Shadow {
                    shadow,
                    color: [r, g, b, a],
                    bounds,
                    radius,
                } => {
                    if let Some(shape) = shadow.geometry(bounds, radius) {
                        context.push_clip_layer(&shape.knockout());
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
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
                    context.set_paint(area_gradient(range, color));
                    context.fill_path(&area_path(&points));
                }
                Paint::Stroke {
                    round,
                    color: [r, g, b, a],
                    points,
                    width,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context.set_stroke(check_stroke(width, round));
                    context.stroke_path(&check_path(&points));
                }
                Paint::Clip(shape) => context.push_clip_layer(&shape.path()),
                Paint::PopClip | Paint::PopOpacity => context.pop_layer(),
                Paint::Opacity { alpha, .. } => context.push_opacity_layer(alpha),
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
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context.fill_path(
                        &crate::scene::border::Border::with_radii(
                            bounds,
                            widths,
                            corners.unwrap_or([[radius; 2]; 4]),
                        )
                        .styled_path(style),
                    );
                }
                Paint::Gradient {
                    corners,
                    gradient,
                    bounds: [x, y, w, h],
                    radius,
                } => {
                    let brush = gradient.brush([x, y, w, h]);
                    context.set_paint(brush.gradient);
                    context.set_paint_transform(brush.transform);
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [x, y, w, h],
                            radius,
                            radii: corners,
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
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: [x, y, w, h],
                            radius,
                            radii: corners,
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
                } => {
                    if let Some(bitmap) = self.images.get(&src) {
                        let (rect, transform, clip) = image_geometry(bitmap, bounds, radius, fit)?;
                        context.push_clip_layer(&clip);
                        context.set_paint(vello_cpu::Image {
                            image: vello_cpu::ImageSource::Pixmap(bitmap.pixels().clone()),
                            sampler: Default::default(),
                        });
                        context.set_paint_transform(transform);
                        context.fill_rect(&rect);
                        context.reset_paint_transform();
                        context.pop_layer();
                    }
                }
                Paint::Text {
                    color: [r, g, b, a],
                    font,
                    size,
                    coordinates,
                    skew,
                    glyphs,
                    ..
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
                    context
                        .glyph_run(resources, &font)
                        .font_size(size)
                        .glyph_transform(Affine::new([1.0, 0.0, -skew, 1.0, 0.0, 0.0]))
                        .normalized_coords(&coordinates)
                        .fill_glyphs(glyphs.into_iter())
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn image_geometry(
    bitmap: &crate::scene::image::Bitmap,
    bounds: [f32; 4],
    radius: f32,
    fit: crate::scene::image::Fit,
) -> Result<(PaintRect, Affine, vello_cpu::kurbo::BezPath), String> {
    let pixels = bitmap.pixels();
    let [x, y, w, h] =
        crate::scene::image::placement([pixels.width(), pixels.height()], bounds, fit)?;
    let rect = PaintRect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64);
    let transform = Affine::translate((x as f64, y as f64))
        * Affine::scale_non_uniform(
            w as f64 / f64::from(pixels.width()),
            h as f64 / f64::from(pixels.height()),
        );
    let [x, y, w, h] = bounds;
    let clip = RoundedRect::new(
        x as f64,
        y as f64,
        (x + w) as f64,
        (y + h) as f64,
        radius as f64,
    )
    .to_path(0.1);
    Ok((rect, transform, clip))
}
