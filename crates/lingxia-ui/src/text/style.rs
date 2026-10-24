//! Computed, inherited text style per record, in author units.
use crate::protocol::{Record, field, tag};
use parley::Alignment;

#[derive(Clone)]
pub(crate) struct TextStyle {
    pub(crate) size: f32,
    pub(crate) mono: bool,
    pub(crate) italic: bool,
    pub(crate) weight: f32,
    pub(crate) line_height: Option<f32>,
    pub(crate) letter_spacing: f32,
    pub(crate) color: String,
    pub(crate) align: Alignment,
    pub(crate) word_break: u8,
    pub(crate) overflow_wrap: u8,
    /// The ancestor whose decoration this record's glyphs carry.
    pub(crate) strike_owner: Option<usize>,
}

/// Text painted for each record: editor values, placeholders or masked secrets.
pub(crate) fn display_texts(records: &[Record]) -> Vec<String> {
    records
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
        .collect()
}

pub(crate) fn computed(records: &[Record]) -> Vec<TextStyle> {
    let mut styles: Vec<TextStyle> = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let parent = record.parent.map(|p| styles[p].clone());
        let font_size = record
            .number(field::FONT_SIZE)
            .unwrap_or_else(|| parent.as_ref().map(|p| p.size).unwrap_or(14.0));
        // Decorations propagate through layout descendants; `none` only
        // removes a node's own decoration, not one drawn by its ancestor.
        let strike_owner = parent.as_ref().and_then(|p| p.strike_owner).or_else(|| {
            (record.string(field::TEXT_DECORATION_LINE) == Some("line-through")).then_some(index)
        });
        let align = match record.string(field::TEXT_ALIGN) {
            Some("left") => Alignment::Left,
            Some("right") => Alignment::Right,
            Some("center") => Alignment::Center,
            Some("justify") => Alignment::Justify,
            Some("end") => Alignment::End,
            Some("start") => Alignment::Start,
            _ if record.tag == tag::BUTTON => Alignment::Center,
            _ => parent.as_ref().map(|p| p.align).unwrap_or(Alignment::Start),
        };
        let word_break = match record.string(field::WORD_BREAK) {
            Some("break-all") => 1,
            Some("keep-all") => 2,
            Some("normal") => 0,
            _ => parent.as_ref().map(|p| p.word_break).unwrap_or(0),
        };
        let overflow_wrap = match record.string(field::OVERFLOW_WRAP) {
            Some("break-word") => 1,
            Some("anywhere") => 2,
            Some("normal") => 0,
            _ => parent.as_ref().map(|p| p.overflow_wrap).unwrap_or(0),
        };
        let mono = record
            .string(field::FONT_FAMILY)
            .map(|family| family == "mono")
            .unwrap_or_else(|| parent.as_ref().is_some_and(|p| p.mono));
        let italic = record
            .string(field::FONT_STYLE)
            .map(|style| style == "italic")
            .unwrap_or_else(|| parent.as_ref().is_some_and(|p| p.italic));
        let weight = record
            .number(field::FONT_WEIGHT)
            .unwrap_or_else(|| parent.as_ref().map(|p| p.weight).unwrap_or(400.0));
        let line_height = record
            .number(field::LINE_HEIGHT)
            .or_else(|| parent.as_ref().and_then(|p| p.line_height));
        let letter_spacing = record
            .number(field::LETTER_SPACING)
            .unwrap_or_else(|| parent.as_ref().map(|p| p.letter_spacing).unwrap_or(0.0));
        let color = record
            .string(field::COLOR)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if record.tag == tag::BUTTON {
                    "primaryText".into()
                } else {
                    parent
                        .as_ref()
                        .map(|p| p.color.clone())
                        .unwrap_or("text".into())
                }
            });
        styles.push(TextStyle {
            size: font_size,
            mono,
            italic,
            weight,
            line_height,
            letter_spacing,
            color,
            align,
            word_break,
            overflow_wrap,
            strike_owner,
        });
    }
    styles
}
