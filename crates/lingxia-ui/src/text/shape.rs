//! Text shaping, shaped extents and glyph ink bounds.
use crate::layout::Renderer;
use crate::protocol::{Record, tag};
use crate::text::style::TextStyle;
use parley::Layout;
use parley::StyleProperty;
use parley::style::{FontStyle, FontWeight, GenericFamily, LineHeight};
use vello_cpu::Glyph;

pub(crate) fn text_extent(layout: &Layout<()>, lines: Option<usize>) -> (f32, f32) {
    let Some(limit) = lines else {
        return (layout.width(), layout.height());
    };
    let mut width: f32 = 0.0;
    let mut height = 0.0;
    for line in layout.lines().take(limit) {
        let metrics = line.metrics();
        width = width.max(metrics.advance - metrics.hanging_advance);
        height = metrics.block_max_coord;
    }
    (width, height)
}

pub(crate) fn glyph_ink(
    font: &parley::FontData,
    size: f32,
    coordinates: &[i16],
    glyphs: &[Glyph],
    skew: f32,
    fringe: f32,
) -> Option<[f32; 4]> {
    use skrifa::{
        FontRef, GlyphId, MetadataProvider,
        instance::{NormalizedCoord, Size},
    };
    let font = FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let coordinates: Vec<_> = coordinates
        .iter()
        .map(|value| NormalizedCoord::from_bits(*value))
        .collect();
    let metrics = font.glyph_metrics(Size::new(size), coordinates.as_slice());
    let mut bounds: Option<[f32; 4]> = None;
    for glyph in glyphs {
        let ink = metrics.bounds(GlyphId::new(glyph.id))?;
        if ink.x_min == ink.x_max || ink.y_min == ink.y_max {
            continue;
        }
        let next = [
            glyph.x + ink.x_min + (skew * ink.y_min).min(skew * ink.y_max),
            glyph.y - ink.y_max,
            glyph.x + ink.x_max + (skew * ink.y_min).max(skew * ink.y_max),
            glyph.y - ink.y_min,
        ];
        bounds = Some(match bounds {
            Some(old) => [
                old[0].min(next[0]),
                old[1].min(next[1]),
                old[2].max(next[2]),
                old[3].max(next[3]),
            ],
            None => next,
        });
    }
    // Keep an antialiasing fringe; missing font metrics conservatively disable culling.
    Some(
        bounds
            .map(|b| {
                [
                    b[0] - fringe,
                    b[1] - fringe,
                    b[2] - b[0] + 2.0 * fringe,
                    b[3] - b[1] + 2.0 * fringe,
                ]
            })
            .unwrap_or([0.0; 4]),
    )
}

impl Renderer {
    /// Shape one record's text at the system text scale; the cache keys every shaping input.
    pub(crate) fn shape(
        &mut self,
        record: &Record,
        style: &TextStyle,
        text: &str,
    ) -> Option<Layout<()>> {
        if !matches!(
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
            return None;
        }
        // Keep inherited values in author units; scaling inherited output again
        // would compound with every nested layout container.
        let scaled_font_size = self.text_scale.map(style.size);
        let text_ratio = if style.size > 0.0 {
            scaled_font_size / style.size
        } else {
            1.0
        };
        let line_height = style.line_height.map(|height| height * text_ratio);
        let letter_spacing = style.letter_spacing * text_ratio;
        let font_size = scaled_font_size;
        let (weight, italic, word_break, overflow_wrap) = (
            style.weight,
            style.italic,
            style.word_break,
            style.overflow_wrap,
        );

        let started = std::time::Instant::now();
        let key = (!matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH))
            .then(|| {
                crate::text::cache::Key::new(
                    text,
                    (font_size, letter_spacing),
                    weight,
                    line_height,
                    (style.mono, italic),
                    (word_break, overflow_wrap, style.nowrap),
                )
            })
            .flatten();
        let cached = key.as_ref().and_then(|key| self.text_cache.get(key));
        let mut layout = if let Some(layout) = cached {
            layout
        } else {
            let mut builder = self.text.ranged_builder(&mut self.fonts, text, 1.0, true);
            builder.push_default(StyleProperty::FontSize(font_size));
            builder.push_default(StyleProperty::LetterSpacing(letter_spacing));
            builder.push_default(StyleProperty::FontStyle(if italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            }));
            if matches!(record.tag, tag::INPUT | tag::SEARCH) || style.nowrap {
                builder.push_default(StyleProperty::TextWrapMode(parley::TextWrapMode::NoWrap));
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
            builder.push_default(if style.mono {
                GenericFamily::Monospace
            } else {
                GenericFamily::SansSerif
            });
            if let Some(height) = line_height {
                builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(height)));
            }
            let layout = builder.build(text);
            if let Some(key) = key {
                self.text_cache.insert(key, &layout);
            }
            layout
        };
        layout.break_all_lines(None);
        self.text_time += started.elapsed();
        Some(layout)
    }

    /// The longest prefix plus an ellipsis that fits `lines` lines of `width`, or `None` if all fits.
    pub(crate) fn ellipsize(
        &mut self,
        record: &Record,
        style: &TextStyle,
        text: &str,
        width: f32,
        lines: Option<usize>,
    ) -> Option<Layout<()>> {
        let fits = |layout: &mut Layout<()>| {
            layout.break_all_lines(if style.nowrap { None } else { Some(width) });
            lines.is_none_or(|limit| layout.len() <= limit)
                && (!style.nowrap || layout.width() <= width + 0.5)
        };
        if fits(&mut self.shape(record, style, text)?) {
            return None;
        }
        let ends: Vec<usize> = text.char_indices().map(|(at, _)| at).collect();
        // Prefixes of `k` characters for k < count; the whole text is known not to fit.
        let (mut low, mut high) = (0, ends.len().saturating_sub(1));
        let mut best = self.shape(record, style, "\u{2026}")?;
        while low < high {
            let middle = (low + high).div_ceil(2);
            let candidate = format!("{}\u{2026}", text[..ends[middle]].trim_end());
            let mut layout = self.shape(record, style, &candidate)?;
            if fits(&mut layout) {
                best = layout;
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        Some(best)
    }
}
