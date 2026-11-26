//! GPU scene encoding of a display list.
use crate::renderer::cpu_paint::{image_geometry, pixel_snapped, snapped_clip};
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
        cache: &mut crate::renderer::gpu::text::Glyphs<'_>,
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
            (0, 0),
        )
    }

    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_span(
        &self,
        cache: &mut crate::renderer::gpu::text::Glyphs<'_>,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        span: Range<usize>,
        origin: (u16, u16),
    ) -> Result<(), String> {
        let (background, paints) = self.composition_span(span)?;
        if background && !self.see_through {
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
            Affine::translate((-f64::from(origin.0), -f64::from(origin.1)))
                * Affine::scale(self.scale as f64),
            self.transform_before(paints.start),
            &self.commands[paints],
            None,
        )
    }

    /// One segment of a frame drawn on its own: the clips open at its start are pushed
    /// again under the transforms they were opened with, and every clip open at its end
    /// is closed. With `underlay` the frame first fills from the kept base bound at
    /// [`crate::renderer::gpu::scene::UNDERLAY`].
    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_segment(
        &self,
        cache: &mut crate::renderer::gpu::text::Glyphs<'_>,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        segment: &crate::renderer::gpu::scene::Segment,
        underlay: bool,
    ) -> Result<(), String> {
        context.reset_and_resize(self.width, self.height);
        if underlay {
            self.underlay(
                context,
                &PaintRect::new(0.0, 0.0, self.width as f64, self.height as f64),
            );
        }
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
            Affine::scale(self.scale as f64),
            segment.transform,
            &self.commands[segment.range.clone()],
            None,
        )?;
        for _ in 0..segment.open_at_end {
            context.pop_clip();
        }
        Ok(())
    }

    /// Fills `area` with the kept base's pixels.
    #[cfg(feature = "gpu")]
    fn underlay(&self, context: &mut vello_gpu::Scene, area: &PaintRect) {
        context.set_transform(Affine::IDENTITY);
        context.set_paint(vello_gpu::Image {
            image: vello_gpu::ImageSource::external_texture(
                crate::renderer::gpu::scene::UNDERLAY,
                vello_gpu::RectU16::new(0, 0, self.width, self.height),
                // The base starts with the opaque page background.
                false,
            ),
            // Pixel-aligned and unscaled: nearest keeps it exact.
            sampler: vello_cpu::peniko::ImageSampler::new()
                .with_quality(vello_cpu::peniko::ImageQuality::Low),
        });
        context.fill_rect(area);
    }

    /// `segment` and the kept base under it, limited to the device-pixel box `area`
    /// `[x0, y0, x1, y1]`: drawn over the frame the target holds, it replaces only that box.
    #[cfg(feature = "gpu")]
    pub(crate) fn sparse_damage(
        &self,
        cache: &mut crate::renderer::gpu::text::Glyphs<'_>,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        segment: &crate::renderer::gpu::scene::Segment,
        area: [u16; 4],
    ) -> Result<(), String> {
        context.reset_and_resize(self.width, self.height);
        let [x0, y0, x1, y1] = area.map(f64::from);
        let bounds = PaintRect::new(x0, y0, x1, y1);
        context.set_transform(Affine::IDENTITY);
        context.push_clip_rect(&bounds);
        // Opaque, so it replaces what the target held in the box.
        self.underlay(context, &bounds);
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
            Affine::scale(self.scale as f64),
            segment.transform,
            &self.commands[segment.range.clone()],
            Some(bounds),
        )?;
        for _ in 0..segment.open_at_end {
            context.pop_clip();
        }
        context.pop_clip();
        Ok(())
    }

    /// One segment of the frame limited to the device-pixel rectangle `area` `[x, y, w, h]`,
    /// drawn into rows `row..row + h` of `context`, which holds several such rectangles.
    #[cfg(feature = "gpu")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sparse_area(
        &self,
        cache: &mut crate::renderer::gpu::text::Glyphs<'_>,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        segment: &crate::renderer::gpu::scene::Segment,
        area: [u16; 4],
        row: u16,
    ) -> Result<(), String> {
        let [x, y, width, height] = area.map(f64::from);
        let row = f64::from(row);
        let bounds = PaintRect::new(0.0, row, width, row + height);
        context.set_transform(Affine::IDENTITY);
        context.push_clip_rect(&bounds);
        if segment.background {
            context.set_paint(AlphaColor::<Srgb>::from_rgb8(246, 247, 251));
            context.fill_rect(&bounds);
        }
        let device = Affine::translate((-x, row - y)) * Affine::scale(self.scale as f64);
        for (clip, transform) in &segment.clips {
            context.set_transform(device * checked_paint_transform(*transform)?);
            push_clip(context, clip);
        }
        self.sparse_paints(
            cache,
            context,
            image_ids,
            glyph_images,
            device,
            segment.transform,
            &self.commands[segment.range.clone()],
            None,
        )?;
        for _ in 0..segment.open_at_end {
            context.pop_clip();
        }
        context.pop_clip();
        Ok(())
    }

    #[cfg(feature = "gpu")]
    #[allow(clippy::too_many_arguments)]
    fn sparse_paints(
        &self,
        cache: &mut crate::renderer::gpu::text::Glyphs<'_>,
        context: &mut vello_gpu::Scene,
        image_ids: &HashMap<usize, vello_gpu::ImageSource>,
        glyph_images: &mut HashMap<usize, Arc<vello_gpu::Pixmap>>,
        device: Affine,
        initial_transform: Affine,
        commands: &[Paint],
        cull: Option<PaintRect>,
    ) -> Result<(), String> {
        let mut logical = checked_paint_transform(initial_transform)?;
        let mut transform = device * logical;
        context.set_transform(transform);
        let early = self.early_holes(initial_transform, commands);
        for (index, command) in commands.iter().enumerate() {
            for (_, at, shape) in early.iter().filter(|(outer, ..)| *outer == index) {
                context.set_transform(device * checked_paint_transform(*at)?);
                cut_hole(context, &shape.path());
                context.set_transform(transform);
            }
            // Outside the device box `cull` it would draw nothing: skip encoding it.
            if let Some(area) = cull
                && let Ok(Some(Some([x, y, w, h]))) = self.paint_box(command, logical)
            {
                let painted = device.transform_rect_bbox(PaintRect::new(
                    f64::from(x),
                    f64::from(y),
                    f64::from(x + w),
                    f64::from(y + h),
                ));
                if painted.intersect(area).is_zero_area() {
                    continue;
                }
            }
            match command {
                Paint::Transform(matrix) => {
                    logical = checked_paint_transform(*matrix)?;
                    transform = device * logical;
                    context.set_transform(transform);
                }
                Paint::Vector {
                    path,
                    color: [r, g, b, a],
                    gradient,
                    bounds,
                } => {
                    let [x, y, w, h] = bounds.map(f64::from);
                    context.push_clip_rect(&PaintRect::new(x, y, x + w, y + h));
                    if let Some(brush) = gradient {
                        context.set_paint(brush.gradient.clone());
                        context.set_paint_transform(brush.transform);
                    } else {
                        context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    }
                    context.fill_path(path);
                    context.reset_paint_transform();
                    context.pop_clip();
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
                        context.push_clip_path(&shape.knockout());
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
                        context.pop_clip();
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
                Paint::Clip(shape) => push_clip(context, &snapped_clip(shape, transform)),
                Paint::PopClip => context.pop_clip(),
                Paint::PopOpacity => context.pop_layer(),
                Paint::Mask { .. } => context.push_layer(None, None, None, None, None),
                Paint::Blur { radius, .. } => context.push_layer(
                    None,
                    None,
                    None,
                    None,
                    Some(crate::renderer::cpu_paint::blur_filter(*radius)),
                ),
                Paint::PopBlur => context.pop_layer(),
                Paint::PopMask(mask) => {
                    transform = device * checked_paint_transform(mask.transform)?;
                    context.set_transform(transform);
                    let [x, y, w, h] = mask.bounds.map(f64::from);
                    for layer in &mask.layers {
                        context.push_layer(
                            None,
                            Some(crate::scene::mask::blend()),
                            None,
                            None,
                            None,
                        );
                        context.set_paint(layer.gradient.clone());
                        context.set_paint_transform(layer.transform);
                        context.fill_rect(&PaintRect::new(x, y, x + w, y + h));
                        context.reset_paint_transform();
                        context.pop_layer();
                    }
                    context.pop_layer();
                }
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
                            pixel_snapped(*bounds, transform),
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
                            bounds: pixel_snapped([*x, *y, *w, *h], transform),
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
                    bounds,
                    radius,
                } => {
                    context.set_paint(AlphaColor::<Srgb>::from_rgba8(*r, *g, *b, *a));
                    context.fill_path(
                        &ClipSnapshot {
                            bounds: pixel_snapped(*bounds, transform),
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
                        let (rect, transform, clip) = image_geometry(
                            bitmap,
                            pixel_snapped(*bounds, transform),
                            *radius,
                            *fit,
                            *position,
                        )?;
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
                Paint::Video {
                    player,
                    bounds,
                    radius,
                    fit,
                    position,
                } => {
                    trace_texture(player, bounds, *fit);
                    // Nothing shows until the player's first frame gives its size.
                    if let Some(size) = crate::renderer::gpu::video::frame_size(player) {
                        let (rect, transform, clip) =
                            crate::renderer::cpu_paint::texel_image_geometry(
                                size, *bounds, *radius, *fit, *position, self.scale,
                            )?;
                        context.push_clip_path(&clip);
                        context.set_paint(vello_gpu::Image {
                            image: vello_gpu::ImageSource::external_texture(
                                crate::renderer::gpu::video::texture_id(player),
                                vello_gpu::RectU16::new(0, 0, size[0], size[1]),
                                false,
                            ),
                            sampler: Default::default(),
                        });
                        context.set_paint_transform(transform);
                        context.fill_rect(&rect);
                        context.reset_paint_transform();
                        context.pop_clip();
                    }
                }
                Paint::WebHole {
                    key,
                    picture,
                    bounds,
                    radius,
                    corners,
                    degraded,
                } => {
                    // A capture shows a view that has not drawn as its box's background.
                    let bitmap = (*degraded
                        || (self.web_pictures && crate::scene::webview::is_ready_key(key)))
                    .then(|| self.images.get(picture))
                    .flatten();
                    if let Some(bitmap) = bitmap {
                        let (rect, transform, clip) = image_geometry(
                            bitmap,
                            *bounds,
                            0.0,
                            crate::scene::image::Fit::Fill,
                            [0.5, 0.5],
                        )?;
                        let id = image_ids
                            .get(&(Arc::as_ptr(bitmap.pixels()) as usize))
                            .ok_or("UI_IMAGE: texture not uploaded")?;
                        context.push_clip_path(
                            &ClipSnapshot {
                                bounds: *bounds,
                                radius: *radius,
                                radii: *corners,
                                transform: None,
                            }
                            .path(),
                        );
                        context.push_clip_path(&clip);
                        context.set_paint(vello_gpu::Image {
                            image: id.clone(),
                            sampler: Default::default(),
                        });
                        context.set_paint_transform(transform);
                        context.fill_rect(&rect);
                        context.reset_paint_transform();
                        context.pop_clip();
                        context.pop_clip();
                    } else if !self.web_pictures && crate::scene::webview::is_ready_key(key) {
                        // Live, or held live: the native view shows through.
                        cut_hole(
                            context,
                            &ClipSnapshot {
                                bounds: *bounds,
                                radius: *radius,
                                radii: *corners,
                                transform: None,
                            }
                            .path(),
                        );
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
                    let clipped = gradient.as_ref().zip(crate::scene::paint::clip_text_box(
                        *ink, glyphs, *size, *skew,
                    ));
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
                    } else if let Some(resources) = cache.atlas.as_deref_mut() {
                        context
                            .glyph_run(resources, font)
                            // The atlas key has no transform: a slanted glyph would share the
                            // upright one's entry.
                            .atlas_cache(*skew == 0.0)
                            .font_size(*size)
                            .glyph_transform(Affine::new([1.0, 0.0, -*skew, 1.0, 0.0, 0.0]))
                            .normalized_coords(coordinates)
                            .fill_glyphs(glyphs.iter().copied())
                            .map_err(|e| e.to_string())?;
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
                                cache: cache.prep,
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
    if shape.radius == 0.0 && shape.radii.is_none() && shape.transform.is_none() {
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

/// Logs each change of a natural-fit (web view) box against the texture drawn in it, so a
/// device run reads a sheet's open and close frame by frame.
fn trace_texture(player: &str, bounds: &[f32; 4], fit: crate::scene::image::Fit) {
    if fit != crate::scene::image::Fit::Natural {
        return;
    }
    static SEEN: std::sync::Mutex<Option<std::collections::HashMap<String, String>>> =
        std::sync::Mutex::new(None);
    let texture = crate::renderer::gpu::video::frame_size(player)
        .map_or_else(|| "none".to_owned(), |[w, h]| format!("{w}x{h}"));
    let state = format!("box {:.0}x{:.0} texture {texture}", bounds[2], bounds[3]);
    let mut seen = SEEN.lock().unwrap_or_else(|p| p.into_inner());
    let seen = seen.get_or_insert_with(Default::default);
    if seen.get(player) != Some(&state) {
        log::info!(target: "lingxia_view", "webview texture {player}: {state}");
        // Players come and go for the process's life; at worst a closed one logs once more.
        if seen.len() >= 64 {
            seen.clear();
        }
        seen.insert(player.to_owned(), state);
    }
}

/// Clears `shape` from what is drawn beneath it, so the native view under the surface shows.
fn cut_hole(context: &mut vello_gpu::Scene, shape: &vello_cpu::kurbo::BezPath) {
    context.push_layer(
        None,
        Some(vello_cpu::peniko::BlendMode::new(
            vello_cpu::peniko::Mix::Normal,
            vello_cpu::peniko::Compose::DestOut,
        )),
        None,
        None,
        None,
    );
    context.set_paint(AlphaColor::<Srgb>::from_rgba8(0, 0, 0, 255));
    context.fill_path(shape);
    context.pop_layer();
}
