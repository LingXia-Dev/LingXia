//! A catalogue compiled to a JS module: one function per message, `(args, f) => text`.
//!
//! The functions do no formatting of their own. `f` is the runtime's, so a GPU page backs it with
//! the Rust formatter and a WebView page with `Intl`:
//! - `f.j(parts)` joins parts: a string, or an array when a tag returned something else;
//! - `f.n(value, style)`, `f.d(value, style)`, `f.t(value, style)` format a number, date, time;
//! - `f.p(value, offset, ordinal, cases)` calls the case for `=value`, else for the plural
//!   category of `value - offset`, else `other`, passing it `value - offset` for `#`;
//! - `f.s(value, cases)` calls `cases[value]`, else `cases.other`;
//! - `f.tag(fn, contents)` hands a tag's joined contents to the caller's function.

use crate::catalogue::Catalogue;
use crate::message::{Arg, DateStyle, NumberStyle, Part, PluralCategory, Selector};
use std::fmt::Write;

/// `formatter(tag)`: the `f` each compiled message is called with, as the compiled index exports it.
/// Until the host formatter serves pages, plural rules and number shapes cover the languages the
/// SDK ships; any other language takes `other` and plain grouping. `format::format` matches it.
pub const FORMATTER: &str = r#"const cardinal = {
  en: n => (n === 1 ? 'one' : 'other'),
  zh: () => 'other',
  ja: () => 'other',
  ko: () => 'other',
};
const ordinal = {
  en: n => {
    const ten = n % 10, hundred = n % 100;
    if (ten === 1 && hundred !== 11) return 'one';
    if (ten === 2 && hundred !== 12) return 'two';
    if (ten === 3 && hundred !== 13) return 'few';
    return 'other';
  },
};

function group(n, digits) {
  const fixed = digits === undefined ? String(Math.abs(n)) : Math.abs(n).toFixed(digits);
  const [whole, fraction] = fixed.split('.');
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  return (n < 0 ? '-' : '') + grouped + (fraction ? `.${fraction}` : '');
}

const pad = n => String(n).padStart(2, '0');

export function formatter(tag) {
  const language = tag.toLowerCase().split('-')[0];
  const zh = language === 'zh' || language === 'ja';
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const date = (value, style) => {
    const d = value instanceof Date ? value : new Date(value);
    const [y, m, day] = [d.getFullYear(), d.getMonth() + 1, d.getDate()];
    if (zh) return style === 'short' ? `${y}/${m}/${day}` : `${y}年${m}月${day}日`;
    return style === 'short' ? `${m}/${day}/${String(y).slice(2)}` : style === 'long'
      ? `${['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'][m - 1]} ${day}, ${y}`
      : `${months[m - 1]} ${day}, ${y}`;
  };
  const time = (value, style) => {
    const d = value instanceof Date ? value : new Date(value);
    const clock = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
    return style === 'short' ? clock : `${clock}:${pad(d.getSeconds())}`;
  };
  return {
    j: parts => (parts.every(part => typeof part === 'string' || typeof part === 'number') ? parts.join('') : parts),
    n: (value, style) => (style === 'percent' ? `${group(value * 100, 0)}%` : group(value, style === 'integer' ? 0 : undefined)),
    d: date,
    t: time,
    p: (value, offset, isOrdinal, cases) => {
      const exact = cases[`=${value}`];
      if (exact) return exact(value - offset);
      const rule = (isOrdinal ? ordinal : cardinal)[language];
      return (cases[rule ? rule(value - offset) : 'other'] ?? cases.other)(value - offset);
    },
    s: (value, cases) => (cases[value] ?? cases.other)(),
    tag: (fn, contents) => (typeof fn === 'function' ? fn(contents) : contents),
  };
}
"#;

pub fn module(catalogue: &Catalogue) -> String {
    let mut out = String::from("export default {\n");
    for (key, entry) in &catalogue.entries {
        let body = match entry.message.parts.as_slice() {
            [] => "\"\"".to_string(),
            [Part::Text(text)] => string(text),
            parts => format!("f.j({})", list(parts, None)),
        };
        let _ = writeln!(out, "  {}: (a, f) => {body},", string(key));
    }
    out.push_str("};\n");
    out
}

/// `pound` names the variable `#` reads inside a plural case.
fn list(parts: &[Part], pound: Option<&str>) -> String {
    let items: Vec<String> = parts.iter().map(|part| expression(part, pound)).collect();
    format!("[{}]", items.join(", "))
}

fn expression(part: &Part, pound: Option<&str>) -> String {
    match part {
        Part::Text(text) => string(text),
        Part::Pound => format!("f.n({}, \"\")", pound.unwrap_or("0")),
        Part::Arg(Arg::Text(name)) => arg(name),
        Part::Arg(Arg::Number { name, style }) => {
            let style = match style {
                NumberStyle::Plain => "",
                NumberStyle::Integer => "integer",
                NumberStyle::Percent => "percent",
            };
            format!("f.n({}, \"{style}\")", arg(name))
        }
        Part::Arg(Arg::Date { name, style }) => format!("f.d({}, \"{}\")", arg(name), date(*style)),
        Part::Arg(Arg::Time { name, style }) => format!("f.t({}, \"{}\")", arg(name), date(*style)),
        Part::Plural {
            name,
            ordinal,
            offset,
            cases,
        } => {
            let cases: Vec<String> = cases
                .iter()
                .map(|(selector, parts)| {
                    let label = match selector {
                        Selector::Exact(n) => format!("={n}"),
                        Selector::Category(category) => category_name(*category).to_string(),
                    };
                    format!("{}: (h) => f.j({})", string(&label), list(parts, Some("h")))
                })
                .collect();
            format!(
                "f.p({}, {offset}, {ordinal}, {{ {} }})",
                arg(name),
                cases.join(", ")
            )
        }
        Part::Select { name, cases } => {
            let cases: Vec<String> = cases
                .iter()
                .map(|(key, parts)| format!("{}: () => f.j({})", string(key), list(parts, pound)))
                .collect();
            format!("f.s({}, {{ {} }})", arg(name), cases.join(", "))
        }
        Part::Tag { name, children } => {
            format!("f.tag({}, f.j({}))", arg(name), list(children, pound))
        }
    }
}

fn arg(name: &str) -> String {
    format!("a[{}]", string(name))
}

fn date(style: DateStyle) -> &'static str {
    match style {
        DateStyle::Short => "short",
        DateStyle::Medium => "medium",
        DateStyle::Long => "long",
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

/// A JSON string is a JS string literal, except that U+2028/2029 end a line in older engines.
fn string(text: &str) -> String {
    serde_json::to_string(text)
        .unwrap_or_default()
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plural_hands_its_case_the_number_less_the_offset() {
        let catalogue = Catalogue::parse(
            r#"{ "a.others": { "defaultMessage": "{n, plural, offset:1 =0 {nobody} other {<b>#</b> more}}", "description": "d" } }"#,
            true,
        )
        .unwrap();
        assert_eq!(
            module(&catalogue),
            "export default {\n  \"a.others\": (a, f) => f.j([f.p(a[\"n\"], 1, false, { \"=0\": (h) => f.j([\"nobody\"]), \"other\": (h) => f.j([f.tag(a[\"b\"], f.j([f.n(h, \"\")])), \" more\"]) })]),\n};\n"
        );
    }
}
