//! `en-XA`, a pseudo-language generated from the source catalogue to find text the layout clips or
//! leaves untranslated: accented, about 40% longer, and bracketed so a cut end shows. Never shipped.

use crate::catalogue::{Catalogue, Entry};
use crate::message::{Arg, DateStyle, Message, NumberStyle, Part, PluralCategory, Selector};

pub const TAG: &str = "en-XA";

pub fn catalogue(source: &Catalogue) -> Catalogue {
    let entries = source
        .entries
        .iter()
        .map(|(key, entry)| {
            let message = message(&entry.message);
            let entry = Entry {
                source: icu(&message),
                message,
                signature: entry.signature.clone(),
                description: None,
            };
            (key.clone(), entry)
        })
        .collect();
    Catalogue { entries }
}

fn message(source: &Message) -> Message {
    let length = text_length(&source.parts);
    let mut parts = vec![Part::Text("[".into())];
    parts.extend(accent(&source.parts));
    // Two in five more characters, as German or Finnish run longer than English.
    let pad = "~".repeat(length.div_ceil(5) * 2);
    parts.push(Part::Text(format!("{pad}]")));
    Message {
        parts: merged(parts),
    }
}

/// Adjacent text as one part, as `parse` reads it.
fn merged(parts: Vec<Part>) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::with_capacity(parts.len());
    for part in parts {
        match (out.last_mut(), part) {
            (Some(Part::Text(last)), Part::Text(text)) => last.push_str(&text),
            (_, part) => out.push(part),
        }
    }
    out
}

fn text_length(parts: &[Part]) -> usize {
    parts
        .iter()
        .map(|part| match part {
            Part::Text(text) => text.chars().count(),
            Part::Tag { children, .. } => text_length(children),
            // A case's text is one of several; the longest is what must fit.
            Part::Plural { cases, .. } => cases
                .iter()
                .map(|(_, parts)| text_length(parts))
                .max()
                .unwrap_or(0),
            Part::Select { cases, .. } => cases
                .iter()
                .map(|(_, parts)| text_length(parts))
                .max()
                .unwrap_or(0),
            Part::Arg(_) | Part::Pound => 0,
        })
        .sum()
}

fn accent(parts: &[Part]) -> Vec<Part> {
    parts
        .iter()
        .map(|part| match part {
            Part::Text(text) => Part::Text(text.chars().map(accented).collect()),
            Part::Tag { name, children } => Part::Tag {
                name: name.clone(),
                children: accent(children),
            },
            Part::Plural {
                name,
                ordinal,
                offset,
                cases,
            } => Part::Plural {
                name: name.clone(),
                ordinal: *ordinal,
                offset: *offset,
                cases: cases
                    .iter()
                    .map(|(selector, parts)| (*selector, accent(parts)))
                    .collect(),
            },
            Part::Select { name, cases } => Part::Select {
                name: name.clone(),
                cases: cases
                    .iter()
                    .map(|(key, parts)| (key.clone(), accent(parts)))
                    .collect(),
            },
            other => other.clone(),
        })
        .collect()
}

fn accented(c: char) -> char {
    const FROM: &str = "aceinostuyACEINOSTUYz";
    const TO: &str = "åçéîñöšţûýÅÇÉÎÑÖŠŢÛÝž";
    FROM.chars()
        .position(|from| from == c)
        .and_then(|at| TO.chars().nth(at))
        .unwrap_or(c)
}

/// The ICU source of `message`, which `parse` reads back to the same parts.
pub fn icu(message: &Message) -> String {
    let mut out = String::new();
    write(&message.parts, false, &mut out);
    out
}

fn write(parts: &[Part], in_plural: bool, out: &mut String) {
    for part in parts {
        match part {
            Part::Text(text) => {
                for c in text.chars() {
                    match c {
                        '\'' => out.push_str("''"),
                        '{' | '}' | '<' | '>' => out.extend(['\'', c, '\'']),
                        '#' if in_plural => out.push_str("'#'"),
                        c => out.push(c),
                    }
                }
            }
            Part::Pound => out.push('#'),
            Part::Arg(Arg::Text(name)) => out.push_str(&format!("{{{name}}}")),
            Part::Arg(Arg::Number { name, style }) => out.push_str(&match style {
                NumberStyle::Plain => format!("{{{name}, number}}"),
                NumberStyle::Integer => format!("{{{name}, number, integer}}"),
                NumberStyle::Percent => format!("{{{name}, number, percent}}"),
            }),
            Part::Arg(Arg::Date { name, style }) => {
                out.push_str(&format!("{{{name}, date{}}}", date_style(*style)))
            }
            Part::Arg(Arg::Time { name, style }) => {
                out.push_str(&format!("{{{name}, time{}}}", date_style(*style)))
            }
            Part::Plural {
                name,
                ordinal,
                offset,
                cases,
            } => {
                out.push_str(&format!(
                    "{{{name}, {}, ",
                    if *ordinal { "selectordinal" } else { "plural" }
                ));
                if *offset > 0 {
                    out.push_str(&format!("offset:{offset} "));
                }
                for (selector, parts) in cases {
                    match selector {
                        Selector::Exact(n) => out.push_str(&format!("={n} {{")),
                        Selector::Category(category) => {
                            out.push_str(&format!("{} {{", category_name(*category)))
                        }
                    }
                    write(parts, true, out);
                    out.push_str("} ");
                }
                out.pop();
                out.push('}');
            }
            Part::Select { name, cases } => {
                out.push_str(&format!("{{{name}, select, "));
                for (key, parts) in cases {
                    out.push_str(&format!("{key} {{"));
                    write(parts, in_plural, out);
                    out.push_str("} ");
                }
                out.pop();
                out.push('}');
            }
            Part::Tag { name, children } => {
                out.push_str(&format!("<{name}>"));
                write(children, in_plural, out);
                out.push_str(&format!("</{name}>"));
            }
        }
    }
}

fn date_style(style: DateStyle) -> &'static str {
    match style {
        DateStyle::Short => ", short",
        DateStyle::Medium => "",
        DateStyle::Long => ", long",
    }
}

fn category_name(category: PluralCategory) -> &'static str {
    match category {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pseudo_message_keeps_its_arguments_and_reads_back_as_written() {
        let source = crate::parse(
            "{n, plural, =0 {No items} one {# item} other {# items}} in <b>{shop}</b>, '{'braces'}' it''s",
        )
        .unwrap();
        let pseudo = message(&source);
        let text = icu(&pseudo);
        assert!(text.starts_with('[') && text.ends_with("~]"), "{text}");
        assert!(text.contains("# îţémš"), "{text}");
        assert_eq!(crate::parse(&text).unwrap(), pseudo, "{text}");
        assert_eq!(crate::parse(&icu(&source)).unwrap(), source);
    }
}
