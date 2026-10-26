//! Paint commands and node snapshots built from resolved layout.
use crate::layout::Renderer;
use crate::layout::style::*;
use crate::protocol::{Record, field, tag};
use crate::scene::node::*;
use crate::scene::paint::*;
use crate::scene::retained::*;
use crate::semantics::groups::ControlGroups;
use crate::text::shape::glyph_ink;
use crate::text::style::{Decoration, TextStyle};
use parley::{AlignmentOptions, Layout, PositionedLayoutItem};
use std::hash::Hasher;
use std::sync::Arc;
use taffy::prelude::*;
use vello_cpu::Glyph;

/// The first of the consecutive link pieces that share `index`'s handle.
pub(crate) fn link_leader(records: &[Record], index: usize) -> usize {
    let handle = |i: usize| records[i].number(field::ON_CLICK);
    let mut leader = index;
    while leader > 0
        && records[leader].tag == tag::LINK
        && records[leader - 1].tag == tag::LINK
        && records[leader - 1].parent == records[leader].parent
        && handle(leader).is_some()
        && handle(leader - 1) == handle(leader)
    {
        leader -= 1;
    }
    leader
}

fn field_value(record: &Record, id: usize) -> Option<serde_json::Value> {
    record
        .fields
        .iter()
        .find(|(field, _)| *field == id)
        .map(|(_, value)| value.clone())
}

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_scene(
        &mut self,
        records: &[Record],
        tree: &TaffyTree<usize>,
        ids: &[NodeId],
        (styles, texts, paragraphs): (
            &[TextStyle],
            &[String],
            &[Option<crate::text::runs::Paragraph>],
        ),
        layouts: &mut [Option<Layout<crate::text::RunBrush>>],
        groups: &ControlGroups,
        (width, height, scale): (u16, u16, f32),
    ) -> Result<RetainedLayout, String> {
        let colors: Vec<&str> = styles.iter().map(|style| style.color.as_str()).collect();
        let dark = self.dark;
        let color = |value: &str| crate::protocol::themed_color(value, dark);
        let button_owners = &groups.owners;
        let button_hashes = &groups.content;
        let mut commands = Vec::new();
        let mut vector_work = 0;
        let mut shadow_masks = std::collections::HashSet::new();
        let mut shadow_bytes = 0;
        let mut origins: Vec<(f32, f32)> = Vec::new();
        let mut nodes = Vec::new();
        let viewport = [0.0, 0.0, width as f32 / scale, height as f32 / scale];
        let mut ranges = Vec::new();
        let mut input_layouts = Vec::new();
        let identities = crate::protocol::identities(records);
        let mut scroll_parents: Vec<Option<String>> = Vec::new();
        let parents = crate::layout::style::layout_parents(records);
        let hidden = crate::layout::style::hidden(records);
        let directions = crate::layout::style::directions(records, self.rtl);
        // `accessibilityHidden` removes a whole subtree from semantics.
        let mut unspoken: Vec<bool> = Vec::with_capacity(records.len());
        for record in records {
            unspoken.push(
                record.number(field::ACCESSIBILITY_HIDDEN) == Some(1.0)
                    || record.parent.is_some_and(|parent| unspoken[parent]),
            );
        }
        let mut content_boxes: Vec<[f32; 4]> = Vec::with_capacity(records.len());
        let mut selectables = Vec::with_capacity(records.len());
        let mut selectable_count = 0;
        let mut stickies = Vec::with_capacity(records.len());
        let mut run_rects: std::collections::HashMap<usize, Vec<[f32; 4]>> =
            std::collections::HashMap::new();
        let transforms = records
            .iter()
            .map(|record| {
                record
                    .string(field::TRANSFORM)
                    .map(crate::scene::transform::StaticTransform::parse)
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        // A zero scale hides paint and hits for its subtree but keeps layout and semantics.
        let mut collapsed: Vec<bool> = Vec::with_capacity(records.len());
        for (index, record) in records.iter().enumerate() {
            collapsed.push(
                hidden[index]
                    || transforms[index].is_some_and(|t| t.collapsed())
                    || record.parent.is_some_and(|parent| collapsed[parent]),
            );
        }
        for (index, record) in records.iter().enumerate() {
            let mut layout = *tree.layout(ids[index]).map_err(|e| e.to_string())?;
            layout.padding.left += layout.border.left;
            layout.padding.right += layout.border.right;
            layout.padding.top += layout.border.top;
            layout.padding.bottom += layout.border.bottom;
            let (px, py) = parents[index].map(|p| origins[p]).unwrap_or((0.0, 0.0));
            let [tx, ty] = transforms[index]
                .map(|t| t.translation([layout.size.width, layout.size.height]))
                .unwrap_or_default();
            let x = px + layout.location.x + tx;
            let y = py + layout.location.y + ty;
            let identity = identities[index].clone();
            let bounds = [x, y, layout.size.width, layout.size.height];
            // Nested percentages can multiply otherwise bounded authored lengths.
            if [
                x,
                y,
                x + layout.size.width,
                y + layout.size.height,
                layout.size.width,
                layout.size.height,
            ]
            .iter()
            .any(|value| {
                !value.is_finite()
                    || value.abs() > 1e12
                    || !(value * scale).is_finite()
                    || (value * scale).abs() > 1e12
            }) {
                return Err("UI_LIMIT: layout geometry exceeds coordinate budget".into());
            }

            let border_widths = [
                layout.border.top,
                layout.border.right,
                layout.border.bottom,
                layout.border.left,
            ];
            let default_radius = record.number(field::RADIUS).unwrap_or(
                if matches!(
                    record.tag,
                    tag::BUTTON | tag::INPUT | tag::TEXTAREA | tag::SEARCH
                ) {
                    12.0
                } else {
                    0.0
                },
            );
            let authored = crate::layout::style::radius_corners(record, directions[index])
                .map(|id| record.number(id).unwrap_or(default_radius));
            let geometry = if authored.iter().all(|r| *r == authored[0]) {
                crate::scene::border::Border::new(bounds, border_widths, authored[0])
            } else {
                crate::scene::border::Border::with_radii(
                    bounds,
                    border_widths,
                    authored.map(|r| [r; 2]),
                )
            };
            let corners = (!authored.iter().all(|r| *r == authored[0]))
                .then(|| geometry.outer.radii.map(|pair| pair.map(|v| v as f32)));
            let radius = authored[0];
            let border = border_widths
                .iter()
                .any(|w| *w > 0.0)
                .then_some(geometry.clone());
            let content_clip = (border.is_some() || corners.is_some() || record.tag == tag::PLOT)
                .then(|| ClipSnapshot {
                    bounds: geometry.inner.bounds.map(|v| v as f32),
                    radius: 0.0,
                    radii: Some(geometry.inner.radii.map(|pair| pair.map(|v| v as f32))),
                    transform: None,
                });
            let control_bounds = content_clip.map(|clip| clip.bounds).unwrap_or(bounds);
            let visible_bounds = intersect(bounds, viewport);
            let scroll_parent = parents[index].and_then(|p| scroll_parents[p].clone());
            let scroll = (matches!(record.tag, tag::SCROLL | tag::LIST)).then(|| {
                let axis = if record.string(field::SCROLL_AXIS) == Some("horizontal") {
                    ScrollAxis::Horizontal
                } else {
                    ScrollAxis::Vertical
                };
                let max_offset = match axis {
                    ScrollAxis::Horizontal => layout.scroll_width(),
                    ScrollAxis::Vertical => layout.scroll_height(),
                };
                ScrollSnapshot {
                    rtl: axis == ScrollAxis::Horizontal && directions[index],
                    contain: matches!(record.string(field::OVERSCROLL), Some("contain" | "none")),
                    content_insets: [
                        layout.padding.top,
                        layout.padding.right,
                        layout.padding.bottom,
                        layout.padding.left,
                    ],
                    content_size: [
                        (layout.size.width - layout.padding.left - layout.padding.right).max(0.0),
                        (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0),
                    ],
                    axis,
                    on_reach_start: record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == field::ON_REACH_START)
                        .and_then(|(_, v)| v.as_u64()),
                    on_reach_end: record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == field::ON_REACH_END)
                        .and_then(|(_, v)| v.as_u64()),
                    parent: scroll_parent.clone(),
                    offset: 0.0,
                    max_offset,
                    id: record
                        .string(field::LIST_ID)
                        .map(str::to_owned)
                        .unwrap_or_else(|| identity.clone()),
                }
            });
            scroll_parents.push(
                scroll
                    .as_ref()
                    .map(|s| s.id.clone())
                    .or(scroll_parent.clone()),
            );
            origins.push((x, y));
            content_boxes.push([
                x + layout.padding.left,
                y + layout.padding.top,
                (layout.size.width - layout.padding.left - layout.padding.right).max(0.0),
                (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0),
            ]);
            stickies.push((record.string(field::POSITION) == Some("sticky")).then(|| {
                let rtl = directions[index];
                let side = |physical, left| {
                    crate::layout::style::side(
                        record,
                        physical,
                        field::INSET_INLINE_START,
                        field::INSET_INLINE_END,
                        left,
                        rtl,
                    )
                };
                let mut scroll = parents[index];
                while let Some(ancestor) = scroll.filter(|a| records[*a].tag != tag::SCROLL) {
                    scroll = parents[ancestor];
                }
                let parent = parents[index].unwrap_or(0);
                crate::scene::sticky::Sticky {
                    scroll: scroll.unwrap_or(0),
                    insets: [
                        record.number(field::INSET_TOP),
                        record.number(side(field::INSET_RIGHT, false)),
                        record.number(field::INSET_BOTTOM),
                        record.number(side(field::INSET_LEFT, true)),
                    ],
                    container: (Some(parent) != scroll).then(|| content_boxes[parent]),
                }
            }));
            let paint_start = commands.len();
            if let Some(source) = record.string(field::BOX_SHADOW) {
                for shadow in crate::scene::shadow::BoxShadow::parse(source)?
                    .into_iter()
                    .rev()
                {
                    let tint = if shadow.color == "currentColor" {
                        colors[index]
                    } else {
                        &shadow.color
                    };
                    let color = color(tint)?;
                    if color[3] != 0 {
                        if let Some(corners) = corners {
                            if let Some(mut raster) = self.shadow_cache.get(
                                [bounds[2], bounds[3]],
                                corners.map(|r| r[0]),
                                &shadow,
                                color,
                                scale,
                            )? {
                                let identity = Arc::as_ptr(raster.bitmap.pixels()) as usize;
                                if shadow_masks.insert(identity) {
                                    shadow_bytes += raster.bitmap.byte_len();
                                }
                                if shadow_bytes > 16 * 1024 * 1024 {
                                    return Err("UI_SHADOW: frame mask budget exceeded".into());
                                }
                                raster.bounds[0] += bounds[0];
                                raster.bounds[1] += bounds[1];
                                commands.push(Paint::RasterShadow { raster });
                            }
                        } else {
                            commands.push(Paint::Shadow {
                                shadow,
                                color,
                                bounds,
                                radius,
                            });
                        }
                    }
                }
            }

            if let Some(background) =
                record
                    .string(field::BACKGROUND)
                    .or(if record.tag == tag::BUTTON {
                        Some("primary")
                    } else if matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) {
                        Some("surface")
                    } else if index == 0 {
                        Some("background")
                    } else {
                        None
                    })
            {
                let [r, g, b, a] = color(if background == "currentColor" {
                    colors[index]
                } else {
                    background
                })?;
                commands.push(Paint::Rect {
                    corners,
                    color: [r, g, b, a],
                    bounds: [x, y, layout.size.width, layout.size.height],
                    radius,
                });
            }
            if let Some(source) = record.string(field::BACKGROUND_GRADIENT)
                && let Some(gradient) = crate::scene::gradient::BackgroundGradient::parse(source)?
            {
                commands.push(Paint::Gradient {
                    corners,
                    gradient: Arc::new(gradient),
                    bounds: [x, y, layout.size.width, layout.size.height],
                    radius,
                });
            }
            if border.is_some() {
                let tint = record
                    .string(field::BORDER_COLOR)
                    .filter(|value| *value != "currentColor")
                    .unwrap_or(colors[index]);
                commands.push(Paint::Border {
                    corners,
                    color: color(tint)?,
                    bounds,
                    widths: border_widths,
                    radius,
                    style: crate::scene::border::LineStyle::from_wire(
                        record.string(field::BORDER_STYLE),
                    ),
                });
            }
            if record.tag == tag::VECTOR {
                let vector = crate::scene::vector::Vector::parse(
                    record.string(field::VECTOR_SOURCE).unwrap(),
                )?;
                let vector_bounds = [
                    control_bounds[0] + layout.padding.left,
                    control_bounds[1] + layout.padding.top,
                    (control_bounds[2] - layout.padding.left - layout.padding.right).max(0.0),
                    (control_bounds[3] - layout.padding.top - layout.padding.bottom).max(0.0),
                ];
                let (shapes, work) = vector.resolved_shapes(vector_bounds)?;
                vector_work += work;
                if vector_work > 32768 {
                    return Err("UI_LIMIT: vector geometry budget exceeded".into());
                }
                for (path, tint) in shapes {
                    let box_ = path.control_box();
                    if !path.is_finite()
                        || [box_.x0, box_.y0, box_.x1, box_.y1]
                            .iter()
                            .any(|value| value.abs() * f64::from(scale.max(1.0)) > 1e12)
                    {
                        return Err("UI_LIMIT: vector geometry exceeds coordinate budget".into());
                    }
                    let (tint, gradient) = match tint {
                        crate::scene::vector::Tint::Color(tint) => (
                            color(if tint == "currentColor" {
                                colors[index]
                            } else {
                                &tint
                            })?,
                            None,
                        ),
                        crate::scene::vector::Tint::Gradient(brush) => ([0; 4], Some(brush)),
                    };
                    commands.push(Paint::Vector {
                        path: Arc::new(path),
                        color: tint,
                        gradient,
                        bounds: vector_bounds,
                    });
                }
            }
            let mut plot_snapshot = None;
            if record.tag == tag::PLOT {
                let plot = crate::scene::plot::Plot::from_record(record)?;
                plot_snapshot = Some(plot.snapshot());
                for shape in plot.shapes(control_bounds) {
                    commands.push(match shape {
                        crate::scene::plot::Shape::Area(points, range, color) => Paint::Area {
                            points,
                            range,
                            color,
                        },
                        crate::scene::plot::Shape::Circle(bounds, color) => Paint::Rect {
                            corners: None,
                            color,
                            bounds,
                            radius: bounds[2] / 2.0,
                        },
                        crate::scene::plot::Shape::Bar(bounds, color) => Paint::Vector {
                            color,
                            gradient: None,
                            bounds,
                            path: Arc::new(plot_bar_path(bounds)),
                        },
                        crate::scene::plot::Shape::Stroke(points, width, color) => Paint::Stroke {
                            color,
                            points,
                            width,
                            round: plot.kind != "pie",
                        },
                    });
                }
            }
            let range = if record.tag == tag::SLIDER {
                Some(crate::SliderSnapshot::from_record(record)?)
            } else {
                None
            };
            if let Some(range) = &range {
                let left = control_bounds[0] + range.inset;
                let track_width = (control_bounds[2] - 2.0 * range.inset).max(0.0);
                let center = control_bounds[1] + control_bounds[3] / 2.0;
                let fraction = ((range.value - range.min) / (range.max - range.min)) as f32;
                let accent = if range.enabled {
                    color(record.string(field::COLOR).unwrap_or("primary"))?
                } else {
                    [160, 168, 190, 255]
                };
                commands.push(Paint::Rect {
                    corners: None,
                    color: [193, 199, 210, 255],
                    bounds: [left, center - 2.0, track_width, 4.0],
                    radius: 2.0,
                });
                commands.push(Paint::Rect {
                    corners: None,
                    color: accent,
                    bounds: [left, center - 2.0, track_width * fraction, 4.0],
                    radius: 2.0,
                });
                commands.push(Paint::Rect {
                    corners: None,
                    color: accent,
                    bounds: [
                        left + track_width * fraction - 12.0,
                        center - 12.0,
                        24.0,
                        24.0,
                    ],
                    radius: 12.0,
                });
            }
            if record.tag == tag::SEARCH {
                let cx = x + layout.padding.left - 30.0;
                let cy = y
                    + layout.padding.top
                    + (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0)
                        / 2.0;
                let tint = if record.number(field::ENABLED) == Some(0.0) {
                    [193, 199, 210, 255]
                } else {
                    color(colors[index])?
                };
                commands.push(Paint::Rect {
                    corners: None,
                    color: tint,
                    bounds: [cx, cy - 10.0, 18.0, 18.0],
                    radius: 9.0,
                });
                commands.push(Paint::Rect {
                    corners: None,
                    color: color(record.string(field::BACKGROUND).unwrap_or("surface"))?,
                    bounds: [cx + 2.0, cy - 8.0, 14.0, 14.0],
                    radius: 7.0,
                });
                commands.push(Paint::Stroke {
                    round: true,
                    color: tint,
                    points: vec![
                        [cx + 15.0, cy + 5.0],
                        [cx + 18.0, cy + 8.0],
                        [cx + 21.0, cy + 11.0],
                    ],
                    width: 2.0,
                });
            }
            let mut text_offset_y = 0.0;
            let mut highlights = Vec::new();
            let text_start = commands.len();
            if let Some(text) = &mut layouts[index] {
                let started = std::time::Instant::now();
                let content_width = (layout.size.width
                    - layout.padding.left
                    - layout.padding.right
                    - check_reserve(record.tag))
                .max(0.0);
                // Overflowing single-line editors retain a nonnegative text origin;
                // caret scrolling, not alignment, reveals the hidden text.
                let alignment_width = if matches!(record.tag, tag::INPUT | tag::SEARCH) {
                    content_width.max(text.full_width())
                } else {
                    content_width
                };
                text.break_all_lines(Some(alignment_width));
                if record.string(field::TEXT_OVERFLOW) == Some("ellipsis")
                    && let Some(short) = self.ellipsize(
                        record,
                        &styles[index],
                        &texts[index],
                        &paragraphs[index]
                            .iter()
                            .flat_map(|p| p.runs.iter())
                            .map(|(run, range)| (range.clone(), &styles[*run]))
                            .collect::<Vec<_>>(),
                        content_width,
                        record.number(field::TEXT_LINES).map(|n| n as usize),
                    )
                {
                    *text = short;
                    text.break_all_lines(Some(alignment_width));
                }
                text.align(styles[index].align, AlignmentOptions::default());
                let placeholder = matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH)
                    && record.string(field::INPUT_VALUE).unwrap_or("").is_empty();
                let tint = if placeholder {
                    record
                        .string(field::PLACEHOLDER_COLOR)
                        .unwrap_or(colors[index])
                } else {
                    colors[index]
                };
                let [r, g, b, a] = color(tint)?;
                self.text_time += started.elapsed();

                let label_x = if matches!(record.tag, tag::CHECKBOX | tag::RADIO_OPTION) {
                    check_reserve(record.tag)
                } else {
                    0.0
                };
                let label_y = if matches!(
                    record.tag,
                    tag::BUTTON
                        | tag::INPUT
                        | tag::CHECKBOX
                        | tag::SWITCH
                        | tag::RADIO_OPTION
                        | tag::SEARCH
                ) {
                    ((layout.size.height
                        - layout.padding.top
                        - layout.padding.bottom
                        - text.height())
                        / 2.0)
                        .max(0.0)
                } else {
                    0.0
                };
                text_offset_y = label_y;
                for line in text.lines().take(
                    record
                        .number(field::TEXT_LINES)
                        .map(|n| n as usize)
                        .unwrap_or(usize::MAX),
                ) {
                    let metrics = line.metrics();
                    for item in line.items() {
                        if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                            // In rich text the brush names the run record that owns these glyphs.
                            let owner = paragraphs[index]
                                .as_ref()
                                .and_then(|p| p.run_at(glyph_run.style().brush))
                                .unwrap_or(index);
                            let extent = [
                                x + layout.padding.left + label_x + glyph_run.offset(),
                                y + layout.padding.top + label_y + metrics.block_min_coord,
                                glyph_run.advance(),
                                metrics.block_max_coord - metrics.block_min_coord,
                            ];
                            if owner != index {
                                if let Some(background) = records[owner].string(field::BACKGROUND) {
                                    highlights.push(Paint::Rect {
                                        corners: None,
                                        color: color(if background == "currentColor" {
                                            colors[owner]
                                        } else {
                                            background
                                        })?,
                                        bounds: extent,
                                        radius: 0.0,
                                    });
                                }
                                run_rects.entry(owner).or_default().push(extent);
                            }
                            let [r, g, b, a] = if owner == index {
                                [r, g, b, a]
                            } else {
                                color(colors[owner])?
                            };
                            let run = glyph_run.run();
                            let coordinates: Vec<i16> = run
                                .normalized_coords()
                                .iter()
                                .map(|v| v.to_bits())
                                .collect();
                            let mut advance = glyph_run.offset();
                            let baseline = glyph_run.baseline();
                            let glyphs = glyph_run.glyphs().map(move |g| {
                                let glyph = Glyph {
                                    id: g.id,
                                    x: x + layout.padding.left + label_x + advance + g.x,
                                    y: y + layout.padding.top + label_y + baseline - g.y,
                                };
                                advance += g.advance;
                                glyph
                            });
                            let glyphs: Vec<Glyph> = glyphs.collect();
                            let skew =
                                f64::from(run.synthesis().skew().unwrap_or(0.0).to_radians().tan());
                            let ink = glyph_ink(
                                run.font(),
                                run.font_size(),
                                &coordinates,
                                &glyphs,
                                skew as f32,
                                (1.0 / scale).max(1.0),
                            );
                            commands.push(Paint::Text {
                                ink,
                                color: [r, g, b, a],
                                font: run.font().clone(),
                                size: run.font_size(),
                                coordinates,
                                skew,
                                glyphs,
                            });
                            if let Some((owner, decoration)) = styles[owner].decoration {
                                let metrics = run.font_metrics();
                                let (offset, size) = match decoration {
                                    Decoration::LineThrough => {
                                        (metrics.strikethrough_offset, metrics.strikethrough_size)
                                    }
                                    Decoration::Underline => {
                                        (metrics.underline_offset, metrics.underline_size)
                                    }
                                };
                                commands.push(Paint::Rect {
                                    corners: None,
                                    color: color(colors[owner])?,
                                    bounds: [
                                        x + layout.padding.left + label_x + glyph_run.offset(),
                                        y + layout.padding.top + label_y + baseline - offset,
                                        glyph_run.advance(),
                                        size.max(1.0 / scale),
                                    ],
                                    radius: 0.0,
                                });
                            }
                        }
                    }
                }
            }
            selectables.push(
                layouts[index]
                    .as_ref()
                    .filter(|_| {
                        // Truncated text paints a different string than it holds.
                        record.tag == tag::TEXT
                            && styles[index].select != 0
                            && record.string(field::TEXT_OVERFLOW) != Some("ellipsis")
                            && !collapsed[index]
                            && selectable_count < crate::scene::retained::MAX_SELECTABLE
                    })
                    .map(|text| {
                        selectable_count += 1;
                        Arc::new(crate::scene::retained::SelectableText {
                            layout: text.clone(),
                            origin: [
                                x + layout.padding.left,
                                y + layout.padding.top + text_offset_y,
                            ],
                            content: [
                                x + layout.padding.left,
                                y + layout.padding.top,
                                (layout.size.width - layout.padding.left - layout.padding.right)
                                    .max(0.0),
                                (layout.size.height - layout.padding.top - layout.padding.bottom)
                                    .max(0.0),
                            ],
                            text: texts[index].clone(),
                        })
                    }),
            );
            // Run backgrounds sit under every glyph of the paragraph.
            commands.splice(text_start..text_start, highlights);
            if matches!(record.tag, tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION)
                && !(record.tag == tag::CHECKBOX && button_hashes.contains_key(&index))
            {
                let checked = record.number(field::CHECKED) == Some(1.0);
                let enabled = record.number(field::ENABLED) != Some(0.0)
                    && record.parent.is_none_or(|p| {
                        records[p].tag != tag::RADIO
                            || records[p].number(field::ENABLED) != Some(0.0)
                    })
                    && record.number(field::ON_CHECKED_CHANGE).is_some();
                let accent = if enabled {
                    [67, 93, 217, 255]
                } else {
                    [160, 168, 190, 255]
                };
                let neutral = if enabled {
                    [131, 143, 163, 255]
                } else {
                    [193, 199, 210, 255]
                };
                let cy = y
                    + layout.padding.top
                    + (layout.size.height - layout.padding.top - layout.padding.bottom).max(0.0)
                        / 2.0;
                if record.tag == tag::RADIO_OPTION {
                    let cx = x + layout.padding.left;
                    commands.push(Paint::Rect {
                        corners: None,
                        color: if checked { accent } else { neutral },
                        bounds: [cx, cy - 12.0, 24.0, 24.0],
                        radius: 12.0,
                    });
                    commands.push(Paint::Rect {
                        corners: None,
                        color: color(record.string(field::BACKGROUND).unwrap_or("surface"))?,
                        bounds: [cx + 2.0, cy - 10.0, 20.0, 20.0],
                        radius: 10.0,
                    });
                    if checked {
                        commands.push(Paint::Rect {
                            corners: None,
                            color: accent,
                            bounds: [cx + 6.0, cy - 6.0, 12.0, 12.0],
                            radius: 6.0,
                        });
                    }
                } else if record.tag == tag::CHECKBOX {
                    let cx = x + layout.padding.left;
                    commands.push(Paint::Rect {
                        corners: None,
                        color: if checked { accent } else { neutral },
                        bounds: [cx, cy - 12.0, 24.0, 24.0],
                        radius: 5.0,
                    });
                    if checked {
                        commands.push(Paint::Stroke {
                            round: true,
                            color: [255; 4],
                            points: vec![
                                [cx + 6.0, cy],
                                [cx + 10.0, cy + 4.0],
                                [cx + 18.0, cy - 5.0],
                            ],
                            width: 2.5,
                        });
                    } else {
                        commands.push(Paint::Rect {
                            corners: None,
                            color: color(record.string(field::BACKGROUND).unwrap_or("surface"))?,
                            bounds: [cx + 2.0, cy - 10.0, 20.0, 20.0],
                            radius: 3.0,
                        });
                    }
                } else {
                    let cx = x + layout.size.width - layout.padding.right - 52.0;
                    commands.push(Paint::Rect {
                        corners: None,
                        color: if checked { accent } else { neutral },
                        bounds: [cx, cy - 16.0, 52.0, 32.0],
                        radius: 16.0,
                    });
                    commands.push(Paint::Rect {
                        corners: None,
                        color: [255; 4],
                        bounds: [cx + if checked { 24.0 } else { 4.0 }, cy - 12.0, 24.0, 24.0],
                        radius: 12.0,
                    });
                }
            }
            if record.tag == tag::IMAGE {
                commands.push(Paint::Image {
                    src: record
                        .string(field::IMAGE_SRC)
                        .ok_or("UI_IMAGE: missing source")?
                        .to_owned(),
                    bounds: control_bounds,
                    radius: if content_clip.is_some() { 0.0 } else { radius },
                    fit: match record.string(field::IMAGE_FIT) {
                        Some("contain") => crate::scene::image::Fit::Contain,
                        Some("fill") => crate::scene::image::Fit::Fill,
                        _ => crate::scene::image::Fit::Cover,
                    },
                });
            }
            input_layouts.push(
                if matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) {
                    Some(InputLayout {
                        layout: layouts[index].take().unwrap(),
                        padding: [layout.padding.left, layout.padding.top],
                        text_offset_y,
                        content_size: [
                            (layout.size.width - layout.padding.left - layout.padding.right)
                                .max(0.0),
                            (layout.size.height - layout.padding.top - layout.padding.bottom)
                                .max(0.0),
                        ],
                        color: color(colors[index])?,
                    })
                } else {
                    None
                },
            );
            let run = matches!(record.tag, tag::SPAN | tag::LINK);
            // A run paints through its paragraph; its own box is the union of its line pieces.
            if collapsed[index] || run {
                commands.truncate(paint_start);
            }
            // A styled link arrives as consecutive pieces sharing one handle; the first
            // piece carries the whole link for hits and semantics.
            let link_group: Vec<usize> =
                if record.tag == tag::LINK && link_leader(records, index) == index {
                    (index..records.len())
                        .take_while(|member| link_leader(records, *member) == index)
                        .collect()
                } else {
                    Vec::new()
                };
            let piece = record.tag == tag::LINK && link_leader(records, index) != index;
            let hit_rects = if !link_group.is_empty() {
                link_group
                    .iter()
                    .flat_map(|member| run_rects.remove(member).unwrap_or_default())
                    .collect()
            } else if run {
                run_rects.remove(&index).unwrap_or_default()
            } else {
                Vec::new()
            };
            let (bounds, visible_bounds) = match hit_rects.split_first() {
                Some((first, rest)) => {
                    let union = rest.iter().fold(*first, |[x, y, w, h], r| {
                        let (left, top) = (x.min(r[0]), y.min(r[1]));
                        let right = (x + w).max(r[0] + r[2]);
                        let bottom = (y + h).max(r[1] + r[3]);
                        [left, top, right - left, bottom - top]
                    });
                    (union, intersect(union, viewport))
                }
                None => (bounds, visible_bounds),
            };
            ranges.push(paint_start..commands.len());
            nodes.push(NodeSnapshot {
                plot_local: None,
                clips_children: record.string(field::OVERFLOW) == Some("hidden"),
                pressed_style: record
                    .fields
                    .iter()
                    .any(|(id, _)| *id == field::PRESSED_STYLE),
                order: 0,
                radii: corners,
                plot: plot_snapshot,
                content_clip,
                control_bounds: (border.is_some()
                    && matches!(record.tag, tag::IMAGE | tag::SLIDER | tag::PLOT))
                .then_some(control_bounds),
                scroll_parent,
                radio_group: (record.tag == tag::RADIO_OPTION)
                    .then(|| identities[record.parent.unwrap()].clone()),
                accessibility_label: record
                    .string(field::ACCESSIBILITY_LABEL)
                    .or_else(|| record.string(field::IMAGE_ALT))
                    .filter(|label| !label.is_empty())
                    .map(str::to_owned),
                hit_rects,
                selectable: selectables.last().and_then(Option::as_ref).map(|_| {
                    if styles[index].select == 2 {
                        "all"
                    } else {
                        "text"
                    }
                }),
                text_selection: None,
                selection_range: None,
                role: record.string(field::ROLE).map(str::to_owned),
                heading_level: record.number(field::HEADING_LEVEL).map(|level| level as u8),
                live_region: match record.string(field::ACCESSIBILITY_LIVE_REGION) {
                    Some("off") => None,
                    Some(kind) => Some(kind.to_owned()),
                    None => match record.string(field::ROLE) {
                        Some("alert") => Some("assertive".into()),
                        Some("status") => Some("polite".into()),
                        _ => None,
                    },
                },
                accessibility_hidden: (hidden[index]
                    || unspoken[index]
                    || record.tag == tag::SPAN
                    || piece
                    || button_owners[index].is_some()
                    || record.string(field::IMAGE_ALT) == Some(""))
                .then_some(true),
                button_content: button_hashes
                    .get(&index)
                    .map(|hash| format!("{:016x}", hash.finish())),
                identity,
                checked: matches!(record.tag, tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION)
                    .then(|| record.number(field::CHECKED) == Some(1.0)),
                range,
                input: matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH).then(|| {
                    InputSnapshot {
                        edit: None,
                        edit_token: None,
                        geometry: None,
                        scroll_offset: [0.0; 2],
                        value: record.string(field::INPUT_VALUE).unwrap_or("").into(),
                        placeholder: record.string(field::PLACEHOLDER).unwrap_or("").into(),
                        secure: record.number(field::SECURE) == Some(1.0),
                        keyboard: record
                            .string(field::KEYBOARD)
                            .unwrap_or(if record.tag == tag::SEARCH {
                                "search"
                            } else {
                                "text"
                            })
                            .into(),
                        auto_focus: record.number(field::AUTO_FOCUS) == Some(1.0),
                        submit_on_enter: record.number(field::SUBMIT_ON_ENTER) == Some(1.0),
                        enabled: record.number(field::ENABLED) != Some(0.0),
                        on_input: record
                            .fields
                            .iter()
                            .find(|(id, _)| *id == field::ON_INPUT)
                            .and_then(|(_, v)| v.as_u64()),
                        on_submit: record
                            .fields
                            .iter()
                            .find(|(id, _)| *id == field::ON_SUBMIT)
                            .and_then(|(_, v)| v.as_u64()),
                        attributes: crate::scene::node::EditorAttributes {
                            max_length: record.number(field::MAX_LENGTH).map(|n| n as u32),
                            auto_complete: record.string(field::AUTO_COMPLETE).map(str::to_owned),
                            enter_key_hint: record.string(field::ENTER_KEY_HINT).map(str::to_owned),
                            auto_capitalize: record
                                .string(field::AUTO_CAPITALIZE)
                                .map(str::to_owned),
                            spell_check: record.number(field::SPELL_CHECK).map(|flag| flag == 1.0),
                            min: field_value(record, field::INPUT_MIN),
                            max: field_value(record, field::INPUT_MAX),
                            step: record.number(field::INPUT_STEP).map(f64::from),
                            picker: record
                                .string(field::KEYBOARD)
                                .filter(|k| matches!(*k, "date" | "time"))
                                .map(str::to_owned),
                        },
                    }
                }),
                tag: crate::protocol::TAGS[record.tag],
                test_id: record.test_id.clone(),
                // Semantics read the authored text; transforms and truncation are paint-only.
                text: if matches!(record.tag, tag::BUTTON | tag::CHECKBOX | tag::SWITCH) {
                    record
                        .string(field::ACCESSIBILITY_LABEL)
                        .or_else(|| groups.names.get(&index).map(String::as_str))
                        .unwrap_or(&record.text)
                        .to_owned()
                } else if matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) {
                    texts[index].clone()
                } else if link_group.len() > 1 {
                    link_group
                        .iter()
                        .map(|member| records[*member].text.as_str())
                        .collect()
                } else if let Some(paragraph) = &paragraphs[index] {
                    paragraph
                        .runs
                        .iter()
                        .map(|(run, _)| records[*run].text.as_str())
                        .collect()
                } else {
                    record.text.clone()
                },
                bounds,
                visible_bounds,
                clips: Vec::new(),
                scroll,
                radius,
                on_click: if collapsed[index]
                    || piece
                    || record.number(field::ENABLED) == Some(0.0)
                    || record.parent.is_some_and(|p| {
                        records[p].tag == tag::RADIO
                            && records[p].number(field::ENABLED) == Some(0.0)
                    }) {
                    None
                } else {
                    record
                        .fields
                        .iter()
                        .find(|(id, _)| {
                            *id == if matches!(
                                record.tag,
                                tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION
                            ) {
                                40
                            } else {
                                8
                            }
                        })
                        .and_then(|(_, value)| value.as_u64())
                },
            });
        }
        Ok(RetainedLayout {
            commands,
            nodes,
            ranges,
            order: crate::scene::retained::paint_order(
                &parents,
                &records
                    .iter()
                    .map(|record| {
                        (
                            record.number(field::Z_INDEX).unwrap_or(0.0) as i32,
                            // Stuck boxes lift above the in-flow siblings they pass over.
                            i32::from(record.string(field::POSITION) == Some("sticky")),
                            record.number(field::ORDER).unwrap_or(0.0) as i32,
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            parents,
            stickies,
            transforms: transforms
                .iter()
                .zip(&collapsed)
                .map(|(t, collapsed)| t.filter(|t| t.paints() && !collapsed))
                .collect(),
            backdrops: records
                .iter()
                .map(|r| {
                    (r.number(field::BACKDROP_BLUR).is_some()
                        || r.number(field::BACKDROP_SATURATION).is_some())
                    .then(|| {
                        (
                            r.number(field::BACKDROP_BLUR).unwrap_or(0.0),
                            r.number(field::BACKDROP_SATURATION).unwrap_or(1.0),
                        )
                    })
                })
                .collect(),
            opacities: records
                .iter()
                .map(|record| record.number(field::OPACITY).unwrap_or(1.0))
                .collect(),
            width,
            height,
            scale,
            inputs: input_layouts,
            selectables,
        })
    }
}
