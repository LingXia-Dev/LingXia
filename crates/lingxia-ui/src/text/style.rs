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
    /// The ancestor whose decoration this record's glyphs carry, and its line.
    pub(crate) decoration: Option<(usize, Decoration)>,
    pub(crate) transform: Transform,
    pub(crate) nowrap: bool,
    pub(crate) collapse: Collapse,
    /// The paragraph base direction; `start`/`end` alignment follows it.
    pub(crate) rtl: bool,
    /// OpenType feature settings in CSS source form; empty for `normal`.
    pub(crate) features: String,
}

/// How CSS `white-space` folds authored whitespace before shaping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Collapse {
    /// `normal`, `nowrap`: every whitespace run is one space.
    All,
    /// `pre-line`: spaces and tabs collapse; newlines break.
    Spaces,
    /// `pre`, `pre-wrap`: authored whitespace is kept.
    None,
}

impl Collapse {
    pub(crate) fn apply(self, text: &str) -> String {
        if self == Self::None {
            return text.to_owned();
        }
        let mut out = String::with_capacity(text.len());
        let mut pending = false;
        for c in text.chars() {
            let newline = matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
            if newline && self == Self::Spaces {
                // Spaces around a preserved break are dropped, as in CSS.
                pending = false;
                out.truncate(out.trim_end_matches([' ', '\t']).len());
                out.push('\n');
            } else if c.is_whitespace() {
                pending = !out.is_empty() && !out.ends_with('\n');
            } else {
                if pending {
                    out.push(' ');
                }
                pending = false;
                out.push(c);
            }
        }
        out
    }
}

/// OpenType features for CSS font-variant-numeric keywords.
fn numeric_features(value: &str) -> String {
    value
        .split_whitespace()
        .filter_map(|keyword| {
            Some(match keyword {
                "ordinal" => "ordn",
                "slashed-zero" => "zero",
                "lining-nums" => "lnum",
                "oldstyle-nums" => "onum",
                "proportional-nums" => "pnum",
                "tabular-nums" => "tnum",
                "diagonal-fractions" => "frac",
                "stacked-fractions" => "afrc",
                _ => return None,
            })
        })
        .map(|tag| format!("\"{tag}\" 1"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decoration {
    LineThrough,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

impl Transform {
    pub(crate) fn apply(self, text: &str) -> String {
        match self {
            Self::None => text.to_owned(),
            Self::Uppercase => text.to_uppercase(),
            Self::Lowercase => text.to_lowercase(),
            Self::Capitalize => {
                let mut start = true;
                text.chars()
                    .flat_map(|c| {
                        let upper = start && c.is_alphanumeric();
                        start = c.is_whitespace();
                        if upper {
                            c.to_uppercase().collect::<Vec<_>>()
                        } else {
                            vec![c]
                        }
                    })
                    .collect()
            }
        }
    }
}

/// Text painted for each record: transformed labels, editor values, placeholders or masked secrets.
pub(crate) fn display_texts(records: &[Record], styles: &[TextStyle]) -> Vec<String> {
    records
        .iter()
        .zip(styles)
        .map(|(record, style)| {
            if !matches!(record.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) {
                return style.transform.apply(&style.collapse.apply(&record.text));
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

pub(crate) fn computed(records: &[Record], directions: &[bool]) -> Vec<TextStyle> {
    let mut styles: Vec<TextStyle> = Vec::with_capacity(records.len());
    for (index, record) in records.iter().enumerate() {
        let parent = record.parent.map(|p| styles[p].clone());
        let font_size = record
            .number(field::FONT_SIZE)
            .unwrap_or_else(|| parent.as_ref().map(|p| p.size).unwrap_or(14.0));
        // Decorations propagate through layout descendants; `none` only
        // removes a node's own decoration, not one drawn by its ancestor.
        let decoration = parent.as_ref().and_then(|p| p.decoration).or_else(|| {
            match record.string(field::TEXT_DECORATION_LINE) {
                Some("line-through") => Some((index, Decoration::LineThrough)),
                Some("underline") => Some((index, Decoration::Underline)),
                _ => None,
            }
        });
        let transform = match record.string(field::TEXT_TRANSFORM) {
            Some("uppercase") => Transform::Uppercase,
            Some("lowercase") => Transform::Lowercase,
            Some("capitalize") => Transform::Capitalize,
            Some(_) => Transform::None,
            None => parent
                .as_ref()
                .map(|p| p.transform)
                .unwrap_or(Transform::None),
        };
        let (nowrap, collapse) = match record.string(field::WHITE_SPACE) {
            Some("nowrap") => (true, Collapse::All),
            Some("pre") => (true, Collapse::None),
            Some("pre-wrap") => (false, Collapse::None),
            Some("pre-line") => (false, Collapse::Spaces),
            Some(_) => (false, Collapse::All),
            None => parent
                .as_ref()
                .map(|p| (p.nowrap, p.collapse))
                .unwrap_or((false, Collapse::All)),
        };
        let features = match record.string(field::FONT_VARIANT_NUMERIC) {
            Some(value) => numeric_features(value),
            None => parent
                .as_ref()
                .map(|p| p.features.clone())
                .unwrap_or_default(),
        };
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
            decoration,
            transform,
            nowrap,
            collapse,
            features,
            rtl: directions[index],
        });
    }
    styles
}
