//! Box geometry, virtual-list window reconciliation and the layout pass.
use crate::protocol::{field, tag};
pub(crate) mod grid;
pub mod list;
pub(crate) mod list_layout;
pub(crate) mod style;
#[cfg(test)]
mod tests;

use crate::layout::style::*;
use crate::protocol::Record;
use crate::scene::display::*;
use crate::scene::node::*;
use crate::text::shape::*;
use parley::{FontContext, Layout, LayoutContext};
use std::{collections::HashMap, sync::Arc};
use taffy::prelude::*;
use taffy::util::ResolveOrZero;

pub struct Renderer {
    pub(crate) text_scale: crate::text::scale::TextScale,
    pub(crate) fonts: FontContext,
    pub(crate) locale: Option<parley::fontique::Language>,
    pub(crate) text: LayoutContext<crate::text::RunBrush>,
    pub(crate) text_cache: crate::text::cache::TextCache,
    pub(crate) shadow_cache: crate::scene::shadow::MaskCache,
    pub(crate) text_time: std::time::Duration,
    pub(crate) safe_area: style::SafeArea,
    /// The environment's inline direction; `dir` overrides it per subtree.
    pub(crate) rtl: bool,
    /// The environment's appearance; colours come from `palette`.
    pub(crate) dark: bool,
    pub(crate) palette: crate::theme::Palette,
}

#[cfg(test)]
#[path = "tests/text_scale.rs"]
mod text_scale_layout_tests;

impl Default for Renderer {
    fn default() -> Self {
        let fonts = crate::text::fonts::context();
        Self {
            text_scale: Default::default(),
            fonts,
            locale: None,
            text: LayoutContext::new(),
            text_cache: Default::default(),
            shadow_cache: Default::default(),
            text_time: std::time::Duration::ZERO,
            safe_area: Default::default(),
            rtl: false,
            dark: false,
            palette: crate::theme::Palette::builtin(false),
        }
    }
}

impl Renderer {
    pub(crate) fn clear_text_cache(&mut self) {
        self.text_cache.clear();
        self.shadow_cache.clear();
    }
    pub(crate) fn edit_text(
        &mut self,
        editor: &mut crate::text::input::Editor,
        edits: &[crate::text::input::Edit],
    ) -> Result<crate::text::input::Change, String> {
        editor
            .apply_batch(edits, &mut self.fonts, &mut self.text)
            .map_err(|e| format!("UI_INPUT: {e:?}"))
    }

    pub fn render(
        &mut self,
        records: &[Record],
        width: u16,
        height: u16,
        scale: f32,
    ) -> Result<Frame, String> {
        self.layout(records, width, height, scale)?.rasterize()
    }

    pub fn layout(
        &mut self,
        records: &[Record],
        width: u16,
        height: u16,
        scale: f32,
    ) -> Result<DisplayList, String> {
        self.layout_scrolled(records, width, height, scale, &HashMap::new())
    }

    /// Offsets belong to the native presenter; scrolling does not require a View render call.
    pub fn layout_scrolled(
        &mut self,
        records: &[Record],
        width: u16,
        height: u16,
        scale: f32,
        offsets: &HashMap<String, f32>,
    ) -> Result<DisplayList, String> {
        if records.iter().any(|record| {
            record
                .fields
                .iter()
                .any(|(field, _)| *field == field::ANIMATION)
        }) {
            return Err("UI_ANIMATION: page requires animation presentation scheduling".into());
        }
        self.text_time = std::time::Duration::ZERO;
        if offsets.values().any(|value| !value.is_finite()) {
            return Err("UI_SCROLL: non-finite offset".into());
        }
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || !scale.is_finite()
            || scale <= 0.0
        {
            return Err("UI_VIEWPORT: invalid dimensions".into());
        }
        let groups = crate::semantics::groups::control_groups(records)?;
        let directions = style::directions(records, self.rtl);
        let styles = crate::text::style::computed(records, &directions);
        let mut texts = crate::text::style::display_texts(records, &styles);
        let paragraphs = crate::text::runs::paragraphs(records, &styles);
        let mut layouts: Vec<Option<Layout<crate::text::RunBrush>>> =
            Vec::with_capacity(records.len());
        for (index, record) in records.iter().enumerate() {
            let runs: Vec<_> = match &paragraphs[index] {
                Some(paragraph) => {
                    texts[index] = paragraph.text.clone();
                    paragraph
                        .runs
                        .iter()
                        .map(|(run, range)| (range.clone(), &styles[*run]))
                        .collect()
                }
                None => Vec::new(),
            };
            layouts.push(self.shape(record, &styles[index], &texts[index], &runs));
        }
        let parents = style::layout_parents(records);
        let mut tree: TaffyTree<usize> = TaffyTree::new();
        let mut ids = Vec::new();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); records.len()];
        for (index, record) in records.iter().enumerate() {
            let mut style = style::node_style(
                records,
                index,
                (width, height, scale),
                self.safe_area,
                directions[index],
            )?;
            // Runs are shaped inside their paragraph and take no box of their own.
            let run = matches!(record.tag, tag::SPAN | tag::LINK);
            if run {
                style.display = Display::None;
            }
            if let Some(ch) = record.number(field::MAX_WIDTH_CH) {
                style.max_size.width = length(ch * self.zero_advance(&styles[index]));
            }
            let id = tree
                .new_leaf_with_context(style, index)
                .map_err(|e| e.to_string())?;
            if let Some(parent) = parents[index].filter(|_| !run) {
                if records[parent].tag == tag::LIST {
                    tree.add_child(ids[parent], id).map_err(|e| e.to_string())?;
                } else {
                    children[parent].push(index);
                }
            }
            if record.tag == tag::LIST {
                // Zero-width spacers do not contribute to Taffy's scroll overflow.
                let spacer = tree
                    .new_leaf(Style {
                        size: Size {
                            width: Dimension::AUTO,
                            height: length(record.number(field::LIST_LEADING).unwrap_or(0.0)),
                        },
                        flex_shrink: 0.0,
                        ..Default::default()
                    })
                    .map_err(|e| e.to_string())?;
                tree.add_child(id, spacer).map_err(|e| e.to_string())?;
            }
            ids.push(id);
        }
        // CSS `order` reorders flex and grid items for layout; reading order keeps the source.
        for (parent, mut items) in children.into_iter().enumerate() {
            if items.is_empty() {
                continue;
            }
            items.sort_by_key(|item| records[*item].number(field::ORDER).unwrap_or(0.0) as i32);
            let items: Vec<_> = items.into_iter().map(|item| ids[item]).collect();
            tree.set_children(ids[parent], &items)
                .map_err(|e| e.to_string())?;
        }
        for (index, record) in records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.tag == tag::LIST)
        {
            let spacer = tree
                .new_leaf(Style {
                    size: Size {
                        width: Dimension::AUTO,
                        height: length(record.number(field::LIST_TRAILING).unwrap_or(0.0)),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                })
                .map_err(|e| e.to_string())?;
            tree.add_child(ids[index], spacer)
                .map_err(|e| e.to_string())?;
        }
        let root = *ids.first().ok_or("UI_ROOT: missing")?;
        let needs_baselines = records.iter().any(|record| {
            record.string(field::ALIGN) == Some("baseline")
                || record.string(field::ALIGN_SELF) == Some("baseline")
        });
        tree.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::Definite(width as f32 / scale),
                height: AvailableSpace::Definite(height as f32 / scale),
            },
            |mut inputs, _, context, style| {
                let index = context.as_ref().map(|index| **index);
                // Taffy applies `aspect-ratio` to styled sizes only; a size the parent
                // fixes (a flexed main size, a stretched cross size) must derive the other axis.
                if let Some(ratio) = style.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0) {
                    let known = &mut inputs.known_dimensions;
                    match (known.width, known.height) {
                        (Some(width), None) => known.height = Some(width / ratio),
                        (None, Some(height)) => known.width = Some(height * ratio),
                        _ => {}
                    }
                }
                let mut output = taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        let tag = context.as_ref().map(|i| records[**i].tag).unwrap_or(0);
                        let single_line = matches!(tag, tag::INPUT | tag::SEARCH | tag::SELECT)
                            || context.as_ref().is_some_and(|i| styles[**i].nowrap);
                        let limit = context
                            .as_ref()
                            .and_then(|i| records[**i].number(field::TEXT_LINES))
                            .map(|n| n as usize);
                        let rows = context.as_ref().and_then(|i| {
                            let record = &records[**i];
                            record
                                .number(field::ROWS)
                                .map(|rows| (rows, record.number(field::MAX_ROWS)))
                        });
                        let Some(layout) = context.and_then(|i| layouts[*i].as_mut()) else {
                            return Size::ZERO;
                        };
                        // Taffy supplies border-box known dimensions but content-box
                        // available space. Wrapping must match the painted content box.
                        let max_width = match available.width {
                            AvailableSpace::Definite(w) => Some(w),
                            _ => None,
                        };
                        let started = std::time::Instant::now();
                        layout.break_all_lines(if single_line {
                            None
                        } else {
                            max_width.map(|w| (w - check_reserve(tag)).max(0.0))
                        });
                        self.text_time += started.elapsed();
                        let (width, mut height) = text_extent(layout, limit);
                        // An autosizing textarea grows with its lines between `rows` and `maxRows`.
                        if let Some((rows, max_rows)) = rows {
                            let line = layout
                                .lines()
                                .next()
                                .map(|line| line.metrics().line_height)
                                .filter(|height| *height > 0.0)
                                .unwrap_or(16.0 * 1.25);
                            height = height.max(rows * line);
                            if let Some(max) = max_rows {
                                height = height.min(max * line);
                            }
                        }
                        Size {
                            width: known.width.unwrap_or(width.ceil() + check_reserve(tag)),
                            height: known.height.unwrap_or(height.ceil().max(check_height(tag))),
                        }
                    },
                );
                if let Some(index) = index.filter(|_| needs_baselines)
                    && let Some(text) = layouts[index].as_mut()
                {
                    let record = &records[index];
                    let padding = style
                        .padding
                        .resolve_or_zero(inputs.parent_size.width, |_, _| 0.0);
                    let border = style
                        .border
                        .resolve_or_zero(inputs.parent_size.width, |_, _| 0.0);
                    let top = padding.top + border.top;
                    let bottom = padding.bottom + border.bottom;
                    // Known dimensions can bypass measurement. Reflow against
                    // the final content width so the reported baseline matches paint.
                    let started = std::time::Instant::now();
                    text.break_all_lines(if matches!(record.tag, tag::INPUT | tag::SEARCH) {
                        None
                    } else {
                        Some(
                            (output.size.width
                                - padding.left
                                - padding.right
                                - border.left
                                - border.right
                                - check_reserve(record.tag))
                            .max(0.0),
                        )
                    });
                    self.text_time += started.elapsed();
                    let label_y = if matches!(
                        record.tag,
                        tag::BUTTON
                            | tag::INPUT
                            | tag::CHECKBOX
                            | tag::SWITCH
                            | tag::RADIO_OPTION
                            | tag::SEARCH
                            | tag::SELECT
                    ) {
                        ((output.size.height - top - bottom - text.height()) / 2.0).max(0.0)
                    } else {
                        0.0
                    };
                    output.baselines.first = text
                        .lines()
                        .next()
                        .map(|line| top + label_y + line.metrics().baseline);
                }
                output
            },
        )
        .map_err(|e| e.to_string())?;
        let retained = self.build_scene(
            records,
            &tree,
            &ids,
            (&styles, &texts, &paragraphs),
            &mut layouts,
            &groups,
            (width, height, scale),
        )?;
        Arc::new(retained).compose(offsets)
    }
}
