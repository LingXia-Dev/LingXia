//! Box geometry, virtual-list window reconciliation and the layout pass.
use crate::protocol::{field, tag};
pub mod list;
pub(crate) mod list_layout;
pub(crate) mod style;
#[cfg(test)]
mod tests;

use crate::layout::style::*;
use crate::protocol::{Record, color};
use crate::scene::display::*;
use crate::scene::node::*;
use crate::scene::paint::*;
use crate::scene::retained::*;
use crate::text::shape::*;
use parley::style::{FontStyle, FontWeight, GenericFamily, LineHeight};
use parley::{
    Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, PositionedLayoutItem,
    StyleProperty,
};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};
use taffy::prelude::*;
use taffy::util::ResolveOrZero;
use taffy::{Overflow, Point};
use vello_cpu::Glyph;

pub struct Renderer {
    pub(crate) text_scale: crate::text::scale::TextScale,
    pub(crate) fonts: FontContext,
    pub(crate) text: LayoutContext<()>,
    pub(crate) text_cache: crate::text::cache::TextCache,
    pub(crate) shadow_cache: crate::scene::shadow::MaskCache,
    pub(crate) text_time: std::time::Duration,
}

#[cfg(test)]
#[path = "tests/text_scale.rs"]
mod text_scale_layout_tests;

impl Default for Renderer {
    fn default() -> Self {
        let fonts = FontContext::new();
        #[cfg(target_os = "android")]
        let fonts = {
            let mut fonts = fonts;
            // Fontique does not yet resolve Android's fonts.xml family aliases.
            // Preserve an existing generic mapping; otherwise use a scanned mono face.
            if fonts
                .collection
                .generic_families(GenericFamily::Monospace)
                .next()
                .is_none()
            {
                for name in ["Droid Sans Mono", "Noto Sans Mono", "Roboto Mono"] {
                    if let Some(family) = fonts.collection.family_by_name(name) {
                        fonts.collection.set_generic_families(
                            GenericFamily::Monospace,
                            core::iter::once(family.id()),
                        );
                        break;
                    }
                }
            }
            fonts
        };
        Self {
            text_scale: Default::default(),
            fonts,
            text: LayoutContext::new(),
            text_cache: Default::default(),
            shadow_cache: Default::default(),
            text_time: std::time::Duration::ZERO,
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
        let mut overflow_wraps: Vec<u8> = Vec::with_capacity(records.len());
        let mut word_breaks: Vec<u8> = Vec::with_capacity(records.len());
        let mut strike_owners: Vec<Option<usize>> = Vec::with_capacity(records.len());
        let mut text_alignments: Vec<Alignment> = Vec::with_capacity(records.len());
        let mut button_owners: Vec<Option<usize>> = Vec::with_capacity(records.len());
        let mut button_hashes: HashMap<usize, std::collections::hash_map::DefaultHasher> =
            HashMap::new();
        for record in records {
            let owner = record.parent.and_then(|parent| {
                if matches!(records[parent].tag, tag::BUTTON | tag::CHECKBOX) {
                    Some(parent)
                } else {
                    button_owners[parent]
                }
            });
            button_owners.push(owner);
            if let Some(owner) = owner {
                // A gesture cannot survive a changed child label or image merely
                // because the outer button's identity and bounds stayed the same.
                let hash = button_hashes.entry(owner).or_default();
                record.tag.hash(hash);
                record.key.hash(hash);
                record.text.hash(hash);
                record.parent.hash(hash);
                serde_json::to_string(&record.fields)
                    .map_err(|error| error.to_string())?
                    .hash(hash);
            }
        }
        let mut tree: TaffyTree<usize> = TaffyTree::new();
        let mut ids = Vec::new();
        let mut layouts: Vec<Option<Layout<()>>> = Vec::new();
        let mut font_sizes = Vec::new();
        let mut font_families = Vec::new();
        let mut font_italics = Vec::new();
        let mut font_weights = Vec::new();
        let mut line_heights = Vec::new();
        let mut letter_spacings = Vec::new();
        let mut colors: Vec<String> = Vec::new();
        let texts: Vec<String> = records
            .iter()
            .map(|record| {
                if !matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) {
                    return record.text.clone();
                }
                let value = record.string(field::INPUT_VALUE).unwrap_or("");
                let text = if value.is_empty() {
                    record.string(field::PLACEHOLDER).unwrap_or("").to_owned()
                } else if record.number(field::SECURE) == Some(1.0) {
                    "\u{2022}".repeat(value.chars().count())
                } else {
                    value.to_owned()
                };
                if matches!(record.tag, tag::INPUT | tag::SEARCH) {
                    text.replace(['\r', '\n', '\u{2028}', '\u{2029}'], " ")
                } else {
                    text
                }
            })
            .collect();
        for (index, record) in records.iter().enumerate() {
            let font_size = record
                .number(field::FONT_SIZE)
                .unwrap_or_else(|| record.parent.map(|p| font_sizes[p]).unwrap_or(14.0));
            font_sizes.push(font_size);
            // Decorations propagate through layout descendants; `none` only
            // removes a node's own decoration, not one drawn by its ancestor.
            strike_owners.push(record.parent.and_then(|p| strike_owners[p]).or_else(|| {
                (record.string(field::TEXT_DECORATION_LINE) == Some("line-through"))
                    .then_some(index)
            }));
            text_alignments.push(match record.string(field::TEXT_ALIGN) {
                Some("left") => Alignment::Left,
                Some("right") => Alignment::Right,
                Some("center") => Alignment::Center,
                Some("justify") => Alignment::Justify,
                Some("end") => Alignment::End,
                Some("start") => Alignment::Start,
                _ if record.tag == tag::BUTTON => Alignment::Center,
                _ => record
                    .parent
                    .map(|parent| text_alignments[parent])
                    .unwrap_or(Alignment::Start),
            });
            let word_break = match record.string(field::WORD_BREAK) {
                Some("break-all") => 1,
                Some("keep-all") => 2,
                Some("normal") => 0,
                _ => record.parent.map(|parent| word_breaks[parent]).unwrap_or(0),
            };
            word_breaks.push(word_break);
            let overflow_wrap = match record.string(field::OVERFLOW_WRAP) {
                Some("break-word") => 1,
                Some("anywhere") => 2,
                Some("normal") => 0,
                _ => record
                    .parent
                    .map(|parent| overflow_wraps[parent])
                    .unwrap_or(0),
            };
            overflow_wraps.push(overflow_wrap);
            let family = record
                .string(field::FONT_FAMILY)
                .unwrap_or_else(|| record.parent.map(|p| font_families[p]).unwrap_or("sans"));
            font_families.push(family);
            let italic = record
                .string(field::FONT_STYLE)
                .map(|style| style == "italic")
                .unwrap_or_else(|| record.parent.map(|p| font_italics[p]).unwrap_or(false));
            font_italics.push(italic);
            let weight = record
                .number(field::FONT_WEIGHT)
                .unwrap_or_else(|| record.parent.map(|p| font_weights[p]).unwrap_or(400.0));
            font_weights.push(weight);
            let line_height = record
                .number(field::LINE_HEIGHT)
                .or_else(|| record.parent.and_then(|p| line_heights[p]));
            line_heights.push(line_height);
            let letter_spacing = record
                .number(field::LETTER_SPACING)
                .unwrap_or_else(|| record.parent.map(|p| letter_spacings[p]).unwrap_or(0.0));
            letter_spacings.push(letter_spacing);
            // Keep inherited values in author units; scaling inherited output again
            // would compound with every nested layout container.
            let scaled_font_size = self.text_scale.map(font_size);
            let text_ratio = if font_size > 0.0 {
                scaled_font_size / font_size
            } else {
                1.0
            };
            let line_height = line_height.map(|height| height * text_ratio);
            let letter_spacing = letter_spacing * text_ratio;
            let font_size = scaled_font_size;
            let color_name = record
                .string(field::COLOR)
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if record.tag == tag::BUTTON {
                        "primaryText".into()
                    } else {
                        record
                            .parent
                            .map(|p| colors[p].clone())
                            .unwrap_or("text".into())
                    }
                });
            colors.push(color_name);
            let text = if matches!(
                record.tag,
                tag::TEXT
                    | tag::BUTTON
                    | tag::INPUT
                    | tag::TEXTAREA
                    | tag::CHECKBOX
                    | tag::SWITCH
                    | tag::RADIO_OPTION
                    | tag::SEARCH
            ) {
                let started = std::time::Instant::now();
                let key = (!matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH))
                    .then(|| {
                        crate::text::cache::Key::new(
                            &texts[index],
                            (font_size, letter_spacing),
                            weight,
                            line_height,
                            (family == "mono", italic),
                            word_break,
                            overflow_wrap,
                        )
                    })
                    .flatten();
                let cached = key.as_ref().and_then(|key| self.text_cache.get(key));
                let mut layout = if let Some(layout) = cached {
                    layout
                } else {
                    let mut builder =
                        self.text
                            .ranged_builder(&mut self.fonts, &texts[index], 1.0, true);
                    builder.push_default(StyleProperty::FontSize(font_size));
                    builder.push_default(StyleProperty::LetterSpacing(letter_spacing));
                    builder.push_default(StyleProperty::FontStyle(if italic {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    }));
                    if matches!(record.tag, tag::INPUT | tag::SEARCH) {
                        builder.push_default(StyleProperty::TextWrapMode(
                            parley::TextWrapMode::NoWrap,
                        ));
                    }
                    builder.push_default(StyleProperty::OverflowWrap(match overflow_wrap {
                        1 => parley::OverflowWrap::BreakWord,
                        2 => parley::OverflowWrap::Anywhere,
                        _ => parley::OverflowWrap::Normal,
                    }));
                    builder.push_default(StyleProperty::WordBreak(match word_break {
                        1 => parley::WordBreak::BreakAll,
                        2 => parley::WordBreak::KeepAll,
                        _ => parley::WordBreak::Normal,
                    }));
                    builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
                    builder.push_default(if family == "mono" {
                        GenericFamily::Monospace
                    } else {
                        GenericFamily::SansSerif
                    });
                    if let Some(height) = line_height {
                        builder
                            .push_default(StyleProperty::LineHeight(LineHeight::Absolute(height)));
                    }
                    let layout = builder.build(&texts[index]);
                    if let Some(key) = key {
                        self.text_cache.insert(key, &layout);
                    }
                    layout
                };
                layout.break_all_lines(None);
                self.text_time += started.elapsed();
                Some(layout)
            } else {
                None
            };
            layouts.push(text);
            let padding = record.number(field::PADDING).unwrap_or(
                if matches!(
                    record.tag,
                    tag::BUTTON
                        | tag::INPUT
                        | tag::TEXTAREA
                        | tag::CHECKBOX
                        | tag::SWITCH
                        | tag::RADIO_OPTION
                        | tag::SEARCH
                        | tag::SLIDER
                ) {
                    12.0
                } else {
                    0.0
                },
            );
            let [padding_top, padding_right, padding_bottom, padding_left] =
                crate::protocol::PADDING_EDGES.map(|field| record.number(field).unwrap_or(padding));
            let border = record.number(field::BORDER_WIDTH).unwrap_or(0.0);
            let [border_top, border_right, border_bottom, border_left] =
                crate::protocol::BORDER_EDGES.map(|id| record.number(id).unwrap_or(border));
            let style = Style {
                display: if matches!(record.tag, tag::GRID | tag::STACK) {
                    Display::Grid
                } else {
                    Display::Flex
                },
                grid_column: if record.parent.is_some_and(|p| records[p].tag == tag::STACK) {
                    Line {
                        start: line(1),
                        end: line(2),
                    }
                } else {
                    Default::default()
                },
                grid_row: if record.parent.is_some_and(|p| records[p].tag == tag::STACK) {
                    Line {
                        start: line(1),
                        end: line(2),
                    }
                } else {
                    Default::default()
                },
                inset: Rect {
                    left: record.number(field::OFFSET_X).map(length).unwrap_or(auto()),
                    top: record.number(field::OFFSET_Y).map(length).unwrap_or(auto()),
                    right: auto(),
                    bottom: auto(),
                },
                grid_template_columns: if record.tag == tag::GRID {
                    vec![flex(1.0); record.number(field::GRID_COLUMNS).unwrap_or(1.0) as usize]
                } else {
                    Vec::new()
                },
                flex_direction: match record.string(field::FLEX_DIRECTION) {
                    Some("row") => FlexDirection::Row,
                    Some("row-reverse") => FlexDirection::RowReverse,
                    Some("column") => FlexDirection::Column,
                    Some("column-reverse") => FlexDirection::ColumnReverse,
                    _ if record.tag == tag::ROW => FlexDirection::Row,
                    _ => FlexDirection::Column,
                },
                flex_wrap: match record.string(field::FLEX_WRAP) {
                    Some("wrap") => FlexWrap::Wrap,
                    Some("wrap-reverse") => FlexWrap::WrapReverse,
                    _ => FlexWrap::NoWrap,
                },
                flex_grow: record.number(field::GROW).unwrap_or(0.0),
                flex_basis: match record
                    .fields
                    .iter()
                    .find(|(id, _)| *id == field::FLEX_BASIS)
                    .map(|(_, value)| crate::protocol::flex_basis(value))
                    .transpose()?
                    .flatten()
                {
                    Some((value, true)) => percent(value / 100.0),
                    Some((value, false)) => length(value),
                    None => auto(),
                },
                flex_shrink: record.number(field::SHRINK).unwrap_or(
                    if (matches!(record.tag, tag::SCROLL | tag::LIST)
                        || record.number(field::GROW).unwrap_or(0.0) > 0.0)
                        && dimension::<Dimension>(record, field::HEIGHT).is_none()
                    {
                        1.0
                    } else {
                        0.0
                    },
                ),
                align_items: record.string(field::ALIGN).map(cross_alignment),
                align_self: record.string(field::ALIGN_SELF).map(cross_alignment),
                justify_content: record.string(field::JUSTIFY).map(|value| match value {
                    "center" => JustifyContent::SAFE_CENTER,
                    "end" => JustifyContent::SAFE_FLEX_END,
                    "between" => JustifyContent::SPACE_BETWEEN,
                    _ => JustifyContent::FLEX_START,
                }),
                overflow: Point {
                    x: if record.string(field::SCROLL_AXIS) == Some("horizontal") {
                        Overflow::Scroll
                    } else if matches!(record.tag, tag::SCROLL | tag::LIST) {
                        Overflow::Hidden
                    } else {
                        Overflow::Visible
                    },
                    y: if record.string(field::SCROLL_AXIS) == Some("horizontal") {
                        Overflow::Hidden
                    } else if matches!(record.tag, tag::SCROLL | tag::LIST) {
                        Overflow::Scroll
                    } else {
                        Overflow::Visible
                    },
                },
                min_size: Size {
                    width: dimension(record, field::MIN_WIDTH).unwrap_or(
                        if record.tag == tag::SLIDER {
                            length(120.0)
                        } else if matches!(
                            record.tag,
                            tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION
                        ) {
                            length(
                                check_reserve(record.tag)
                                    + padding_left
                                    + padding_right
                                    + border_left
                                    + border_right,
                            )
                        } else if matches!(record.tag, tag::SCROLL | tag::LIST)
                            || record.number(field::GROW).unwrap_or(0.0) > 0.0
                        {
                            length(0.0)
                        } else {
                            auto()
                        },
                    ),
                    height: dimension(record, field::MIN_HEIGHT).unwrap_or(
                        if matches!(record.tag, tag::INPUT | tag::SEARCH)
                            && dimension::<taffy::Dimension>(record, field::HEIGHT).is_none()
                            && record.number(field::HEIGHT_AUTO) != Some(1.0)
                        {
                            length(48.0)
                        } else if matches!(
                            record.tag,
                            tag::CHECKBOX
                                | tag::SWITCH
                                | tag::RADIO_OPTION
                                | tag::SEARCH
                                | tag::SLIDER
                        ) {
                            length(44.0)
                        } else if matches!(record.tag, tag::SCROLL | tag::LIST)
                            || record.number(field::GROW).unwrap_or(0.0) > 0.0
                        {
                            length(0.0)
                        } else {
                            auto()
                        },
                    ),
                },
                max_size: Size {
                    width: dimension(record, field::MAX_WIDTH).unwrap_or(auto()),
                    height: dimension(record, field::MAX_HEIGHT).unwrap_or(auto()),
                },
                size: Size {
                    width: dimension(record, field::WIDTH).unwrap_or(if index == 0 {
                        length(width as f32 / scale)
                    } else if record.tag == tag::VECTOR {
                        length(24.0)
                    } else if record.tag == tag::IMAGE {
                        length(64.0)
                    } else {
                        auto()
                    }),
                    height: dimension(record, field::HEIGHT).unwrap_or(
                        if record.number(field::HEIGHT_AUTO) == Some(1.0)
                            || matches!(record.tag, tag::INPUT | tag::SEARCH)
                        {
                            auto()
                        } else if record.tag == tag::SLIDER {
                            length(48.0)
                        } else if record.tag == tag::TEXTAREA {
                            length(120.0)
                        } else if index == 0 {
                            length(height as f32 / scale)
                        } else if record.tag == tag::VECTOR {
                            length(24.0)
                        } else if record.tag == tag::IMAGE {
                            length(64.0)
                        } else {
                            auto()
                        },
                    ),
                },
                gap: Size {
                    width: length(if record.tag == tag::LIST {
                        0.0
                    } else {
                        record.number(field::GAP).unwrap_or(0.0)
                    }),
                    height: length(if record.tag == tag::LIST {
                        0.0
                    } else {
                        record.number(field::GAP).unwrap_or(0.0)
                    }),
                },
                border: Rect {
                    left: length(border_left),
                    right: length(border_right),
                    top: length(border_top),
                    bottom: length(border_bottom),
                },
                padding: Rect {
                    left: length(padding_left + if record.tag == tag::SEARCH { 32.0 } else { 0.0 }),
                    right: length(padding_right),
                    top: length(padding_top),
                    bottom: length(padding_bottom),
                },
                margin: Rect {
                    bottom: length(
                        record
                            .parent
                            .filter(|p| records[*p].tag == tag::LIST)
                            .filter(|p| {
                                record.number(field::LIST_ROW_INDEX).unwrap_or(0.0) + 1.0
                                    < records[*p].number(field::LIST_COUNT).unwrap_or(0.0)
                            })
                            .and_then(|p| records[p].number(field::GAP))
                            .unwrap_or(0.0),
                    ),
                    left: length(0.0),
                    right: length(0.0),
                    top: length(0.0),
                },
                ..Default::default()
            };
            let id = tree
                .new_leaf_with_context(style, index)
                .map_err(|e| e.to_string())?;
            if let Some(parent) = record.parent {
                tree.add_child(ids[parent], id).map_err(|e| e.to_string())?;
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
            |inputs, _, context, style| {
                let index = context.as_ref().map(|index| **index);
                let mut output = taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        let tag = context.as_ref().map(|i| records[**i].tag).unwrap_or(0);
                        let single_line = matches!(tag, tag::INPUT | tag::SEARCH);
                        let limit = context
                            .as_ref()
                            .and_then(|i| records[**i].number(field::TEXT_LINES))
                            .map(|n| n as usize);
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
                        let (width, height) = text_extent(layout, limit);
                        Size {
                            width: known.width.unwrap_or(width.ceil() + check_reserve(tag)),
                            height: known.height.unwrap_or(height.ceil().max(check_height(tag))),
                        }
                    },
                );
                if let Some(index) = index.filter(|_| needs_baselines) {
                    if let Some(text) = layouts[index].as_mut() {
                        let record = &records[index];
                        let padding = style
                            .padding
                            .resolve_or_zero(inputs.parent_size.width, &|_, _| 0.0);
                        let border = style
                            .border
                            .resolve_or_zero(inputs.parent_size.width, &|_, _| 0.0);
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
                }
                output
            },
        )
        .map_err(|e| e.to_string())?;
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
        for (index, record) in records.iter().enumerate() {
            let mut layout = *tree.layout(ids[index]).map_err(|e| e.to_string())?;
            layout.padding.left += layout.border.left;
            layout.padding.right += layout.border.right;
            layout.padding.top += layout.border.top;
            layout.padding.bottom += layout.border.bottom;
            let (px, py) = record.parent.map(|p| origins[p]).unwrap_or((0.0, 0.0));
            let x = px + layout.location.x;
            let y = py + layout.location.y;
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
            let authored = crate::protocol::RADIUS_CORNERS
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
                });
            let control_bounds = content_clip.map(|clip| clip.bounds).unwrap_or(bounds);
            let visible_bounds = intersect(bounds, viewport);
            let scroll_parent = record.parent.and_then(|p| scroll_parents[p].clone());
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
            let paint_start = commands.len();
            if let Some(source) = record.string(field::BOX_SHADOW) {
                for shadow in crate::scene::shadow::BoxShadow::parse(source)?
                    .into_iter()
                    .rev()
                {
                    let tint = if shadow.color == "currentColor" {
                        &colors[index]
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
                let [r, g, b, a] = color(background)?;
                commands.push(Paint::Rect {
                    corners,
                    color: [r, g, b, a],
                    bounds: [x, y, layout.size.width, layout.size.height],
                    radius,
                });
            }
            if let Some(source) = record.string(field::BACKGROUND_GRADIENT) {
                if let Some(gradient) = crate::scene::gradient::LinearGradient::parse(source)? {
                    commands.push(Paint::Gradient {
                        corners,
                        gradient: Arc::new(gradient),
                        bounds: [x, y, layout.size.width, layout.size.height],
                        radius,
                    });
                }
            }
            if border.is_some() {
                let tint = record
                    .string(field::BORDER_COLOR)
                    .filter(|value| *value != "currentColor")
                    .unwrap_or(&colors[index]);
                commands.push(Paint::Border {
                    corners,
                    color: color(tint)?,
                    bounds,
                    widths: border_widths,
                    radius,
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
                    commands.push(Paint::Vector {
                        path: Arc::new(path),
                        color: color(if tint == "currentColor" {
                            &colors[index]
                        } else {
                            &tint
                        })?,
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
                    color(&colors[index])?
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
                text.align(text_alignments[index], AlignmentOptions::default());
                let placeholder = matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH)
                    && record.string(field::INPUT_VALUE).unwrap_or("").is_empty();
                let tint = if placeholder {
                    record
                        .string(field::PLACEHOLDER_COLOR)
                        .unwrap_or(&colors[index])
                } else {
                    &colors[index]
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
                    for item in line.items() {
                        if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
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
                            if let Some(owner) = strike_owners[index] {
                                let metrics = run.font_metrics();
                                commands.push(Paint::Rect {
                                    corners: None,
                                    color: color(&colors[owner])?,
                                    bounds: [
                                        x + layout.padding.left + label_x + glyph_run.offset(),
                                        y + layout.padding.top + label_y + baseline
                                            - metrics.strikethrough_offset,
                                        glyph_run.advance(),
                                        metrics.strikethrough_size.max(1.0 / scale),
                                    ],
                                    radius: 0.0,
                                });
                            }
                        }
                    }
                }
            }
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
                        color: color(&colors[index])?,
                    })
                } else {
                    None
                },
            );
            ranges.push(paint_start..commands.len());
            nodes.push(NodeSnapshot {
                plot_local: None,
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
                accessibility_hidden: (button_owners[index].is_some()
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
                    }
                }),
                tag: crate::protocol::TAGS[record.tag],
                test_id: record.test_id.clone(),
                text: if matches!(record.tag, tag::BUTTON | tag::CHECKBOX) {
                    record
                        .string(field::ACCESSIBILITY_LABEL)
                        .unwrap_or(&texts[index])
                        .to_owned()
                } else {
                    texts[index].clone()
                },
                bounds,
                visible_bounds,
                clips: Vec::new(),
                scroll,
                radius,
                on_click: if record.number(field::ENABLED) == Some(0.0)
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
        Arc::new(RetainedLayout {
            commands,
            nodes,
            ranges,
            parents: records.iter().map(|record| record.parent).collect(),
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
        })
        .compose(offsets)
    }
}
