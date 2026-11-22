//! A message formatted in Rust, for text a host or Logic shows natively (toasts, dialogs).
//! Tags keep only their contents. Plural rules, number, date and time shapes match the JS formatter the
//! build emits (`js::FORMATTER`), so a key reads the same on a page and in a toast.

use crate::message::{Arg, DateStyle, Message, NumberStyle, Part, PluralCategory, Selector};
use chrono::{DateTime, Datelike, Local, Timelike};
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
            Part::Arg(Arg::Date { name, style }) => out.push_str(
                &instant(&args[name])
                    .map_or_else(|| plain(&args[name]), |at| date(&at, language, *style)),
            ),
            Part::Arg(Arg::Time { name, style }) => out.push_str(
                &instant(&args[name]).map_or_else(|| plain(&args[name]), |at| time(&at, *style)),
            ),
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

/// A date argument: milliseconds since the epoch, or an RFC 3339 string (how a JS `Date` crosses
/// into Rust), in the device's time zone as the page's `Date` would show it.
fn instant(value: &Value) -> Option<DateTime<Local>> {
    match value {
        Value::Number(n) => DateTime::from_timestamp_millis(n.as_f64()? as i64),
        Value::String(text) => DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|at| at.to_utc()),
        _ => None,
    }
    .map(|at| at.with_timezone(&Local))
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The JS formatter's date shapes (`js::FORMATTER`), so a toast and a page agree.
fn date(at: &DateTime<Local>, language: &str, style: DateStyle) -> String {
    let (y, m, d) = (at.year(), at.month(), at.day());
    if language == "zh" || language == "ja" {
        return match style {
            DateStyle::Short => format!("{y}/{m}/{d}"),
            _ => format!("{y}年{m}月{d}日"),
        };
    }
    let month = MONTHS[m as usize - 1];
    match style {
        DateStyle::Short => format!("{m}/{d}/{:02}", y.rem_euclid(100)),
        DateStyle::Long => format!("{month} {d}, {y}"),
        DateStyle::Medium => format!("{} {d}, {y}", &month[..3]),
    }
}

fn time(at: &DateTime<Local>, style: DateStyle) -> String {
    let clock = format!("{:02}:{:02}", at.hour(), at.minute());
    match style {
        DateStyle::Short => clock,
        _ => format!("{clock}:{:02}", at.second()),
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
    use chrono::TimeZone;
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

    #[test]
    fn dates_take_milliseconds_or_rfc3339_in_the_local_zone() {
        let at = Local.with_ymd_and_hms(2026, 3, 5, 9, 7, 30).unwrap();
        let ms = json!({ "at": at.timestamp_millis() });
        let iso = json!({ "at": at.to_utc().to_rfc3339() });
        let message =
            crate::parse("{at, date} {at, date, short} {at, time, short} {at, time}").unwrap();
        assert_eq!(
            format(&message, "en-US", &ms),
            "Mar 5, 2026 3/5/26 09:07 09:07:30"
        );
        assert_eq!(
            format(&message, "en-US", &iso),
            format(&message, "en-US", &ms)
        );
        assert_eq!(
            format(&message, "zh-CN", &ms),
            "2026年3月5日 2026/3/5 09:07 09:07:30"
        );
        assert_eq!(
            format(&message, "en", &json!({ "at": "soon" })),
            "soon soon soon soon"
        );
    }
}
