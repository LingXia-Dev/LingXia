//! A message formatted in Rust, for text a host or Logic shows natively (toasts, dialogs).
//! Tags keep only their contents. Plural rules and number shapes match the JS formatter the
//! build emits (`js::FORMATTER`), so a key reads the same on a page and in a toast.

use crate::message::{Arg, Message, NumberStyle, Part, PluralCategory, Selector};
use serde_json::Value;

pub fn format(message: &Message, tag: &str, args: &Value) -> String {
    let mut out = String::new();
    let language = tag.split('-').next().unwrap_or(tag).to_ascii_lowercase();
    write(&message.parts, &language, args, None, &mut out);
    out
}

fn write(parts: &[Part], language: &str, args: &Value, pound: Option<f64>, out: &mut String) {
    for part in parts {
        match part {
            Part::Text(text) => out.push_str(text),
            Part::Pound => out.push_str(&number(pound.unwrap_or(0.0), NumberStyle::Plain)),
            Part::Arg(Arg::Text(name)) => out.push_str(&plain(&args[name])),
            Part::Arg(Arg::Number { name, style }) => out.push_str(
                &args[name]
                    .as_f64()
                    .map_or_else(|| plain(&args[name]), |n| number(n, *style)),
            ),
            // Logic passes dates as text it formatted itself; a number is shown as given.
            Part::Arg(Arg::Date { name, .. } | Arg::Time { name, .. }) => {
                out.push_str(&plain(&args[name]))
            }
            Part::Plural {
                name,
                ordinal,
                offset,
                cases,
            } => {
                let value = args[name].as_f64().unwrap_or(0.0);
                let shifted = value - f64::from(*offset);
                let exact = cases.iter().find(|(selector, _)| {
                    matches!(selector, Selector::Exact(n) if f64::from(*n) == value)
                });
                let category = if *ordinal {
                    ordinal_category(language, shifted)
                } else {
                    cardinal_category(language, shifted)
                };
                let case = exact
                    .or_else(|| {
                        cases
                            .iter()
                            .find(|(s, _)| *s == Selector::Category(category))
                    })
                    .or_else(|| {
                        cases
                            .iter()
                            .find(|(s, _)| *s == Selector::Category(PluralCategory::Other))
                    });
                if let Some((_, parts)) = case {
                    write(parts, language, args, Some(shifted), out);
                }
            }
            Part::Select { name, cases } => {
                let value = plain(&args[name]);
                let case = cases
                    .iter()
                    .find(|(key, _)| *key == value)
                    .or_else(|| cases.iter().find(|(key, _)| key == "other"));
                if let Some((_, parts)) = case {
                    write(parts, language, args, pound, out);
                }
            }
            Part::Tag { children, .. } => write(children, language, args, pound, out),
        }
    }
}

fn plain(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Number(n) => n
            .as_f64()
            .map_or_else(|| n.to_string(), |n| number(n, NumberStyle::Plain)),
        other => other.to_string(),
    }
}

fn cardinal_category(language: &str, n: f64) -> PluralCategory {
    match language {
        "en" if n == 1.0 => PluralCategory::One,
        _ => PluralCategory::Other,
    }
}

fn ordinal_category(language: &str, n: f64) -> PluralCategory {
    if language != "en" || n.fract() != 0.0 {
        return PluralCategory::Other;
    }
    let n = n.abs() as u64;
    match (n % 10, n % 100) {
        (1, h) if h != 11 => PluralCategory::One,
        (2, h) if h != 12 => PluralCategory::Two,
        (3, h) if h != 13 => PluralCategory::Few,
        _ => PluralCategory::Other,
    }
}

fn number(n: f64, style: NumberStyle) -> String {
    let (n, suffix, digits) = match style {
        NumberStyle::Percent => (n * 100.0, "%", Some(0)),
        NumberStyle::Integer => (n, "", Some(0)),
        NumberStyle::Plain => (n, "", None),
    };
    let fixed = match digits {
        Some(digits) => format!("{:.*}", digits, n.abs()),
        None => {
            let text = format!("{}", n.abs());
            text
        }
    };
    let (whole, fraction) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let mut grouped = String::new();
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let sign = if n < 0.0 { "-" } else { "" };
    let fraction = if fraction.is_empty() {
        String::new()
    } else {
        format!(".{fraction}")
    };
    format!("{sign}{grouped}{fraction}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_toast_reads_as_the_page_would() {
        let items = crate::parse(
            "{n, plural, =0 {No items} one {# item} other {# items}} in <b>{shop}</b>",
        )
        .unwrap();
        assert_eq!(
            format(&items, "en-US", &json!({ "n": 1234, "shop": "Acme" })),
            "1,234 items in Acme"
        );
        assert_eq!(
            format(&items, "en-US", &json!({ "n": 1, "shop": "Acme" })),
            "1 item in Acme"
        );
        assert_eq!(
            format(&items, "zh-CN", &json!({ "n": 0, "shop": "店" })),
            "No items in 店"
        );
        let place =
            crate::parse("{n, selectordinal, one {#st} two {#nd} few {#rd} other {#th}}").unwrap();
        let places: Vec<String> = [1, 2, 3, 11, 22]
            .iter()
            .map(|n| format(&place, "en", &json!({ "n": n })))
            .collect();
        assert_eq!(places, ["1st", "2nd", "3rd", "11th", "22nd"]);
    }
}
