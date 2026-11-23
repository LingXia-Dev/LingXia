//! A catalogue compiled to a JS module: one function per message, `(args, f) => text`.
//!
//! The functions do no formatting of their own. `f` is the runtime's, so a GPU page backs it with
//! the Rust formatter and a WebView page with `Intl`:
//! - `f.j(parts)` joins parts: a string, or an array when a tag returned something else;
//! - `f.n(value, style)`, `f.d(value, style)`, `f.t(value, style)` format a number, date, time;
//! - `f.p(value, offset, ordinal, cases)` calls the case for `=value`, else for the plural
//!   category of `value - offset`, else `other`, passing it `value - offset` for `#`;
//! - `f.s(value, cases)` calls `cases[value]`, else `cases.other`;
//! - `f.tag(fn, contents)` hands a tag's joined contents to the caller's function;
//! - `f.i(value)` is a text argument as parts, in FSI…PDI when it needs isolating (`format::format`);
//!   `f.n`, `f.d` and `f.t` isolate their own output in LRI…PDI.

use crate::catalogue::Catalogue;
use crate::message::{Arg, DateStyle, NumberStyle, Part, PluralCategory, Selector};
use std::fmt::Write;

/// `formatter(tag)`: the `f` each compiled message is called with, as the compiled index exports it,
/// with the CLDR plural rules of `languages` (the catalogue's tags). Number and date shapes are plain
/// until the runtime has `Intl`. `format::format` matches it.
pub fn formatter(languages: &[&str]) -> String {
    format!("{}{}{FORMATTER}", crate::plural::js(languages), direction())
}

/// `rightToLeft(tag)` and `RTL_TEXT`, from the same tables as `direction`.
fn direction() -> String {
    use crate::direction::{LANGUAGES, RANGES, SCRIPTS};
    let set = |items: &[&str]| {
        items
            .iter()
            .map(|item| format!("'{item}'"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let ranges: String = RANGES
        .iter()
        .map(|(from, to)| format!("\\u{{{from:X}}}-\\u{{{to:X}}}"))
        .collect();
    format!(
        "\nconst RTL_LANGUAGES = new Set([{}]);\nconst RTL_SCRIPTS = new Set([{}]);\nconst RTL_TEXT = /[{ranges}]/u;\n\
         function rightToLeft(tag) {{\n  const [language, ...rest] = tag.toLowerCase().split(/[-_]/);\n  \
         const script = rest.find(part => part.length === 4);\n  \
         return script ? RTL_SCRIPTS.has(script) : RTL_LANGUAGES.has(language);\n}}\n",
        set(LANGUAGES),
        set(SCRIPTS)
    )
}

const FORMATTER: &str = r#"

function group(n, digits) {
  const fixed = digits === undefined ? String(Math.abs(n)) : Math.abs(n).toFixed(digits);
  const [whole, fraction] = fixed.split('.');
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  return (n < 0 ? '-' : '') + grouped + (fraction ? `.${fraction}` : '');
}

const pad = n => String(n).padStart(2, '0');

export function formatter(tag) {
  const rtl = rightToLeft(tag);
  const isolate = (text, open) => (rtl || RTL_TEXT.test(text) ? `${open}${text}\u2069` : text);
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
    n: (value, style) => isolate(style === 'percent' ? `${group(value * 100, 0)}%` : group(value, style === 'integer' ? 0 : undefined), '\u2066'),
    d: (value, style) => isolate(date(value, style), '\u2066'),
    t: (value, style) => isolate(time(value, style), '\u2066'),
    i: value => (value !== null && typeof value === 'object'
      ? (rtl ? ['\u2068', value, '\u2069'] : [value])
      : [isolate(value == null ? '' : String(value), '\u2068')]),
    p: (value, offset, isOrdinal, cases) => {
      const exact = cases[`=${value}`];
      if (exact) return exact(value - offset);
      return (cases[plural(tag, isOrdinal, value - offset)] ?? cases.other)(value - offset);
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
        Part::Arg(Arg::Text(name)) => format!("...f.i({})", arg(name)),
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

    /// Runs a compiled catalogue and `formatter` in Node, the path a bundle takes: an Arabic message
    /// with all six categories, and `f.p` over the shared plural table.
    #[test]
    fn a_compiled_catalogue_picks_cldr_plural_cases_in_js() {
        let catalogue = Catalogue::parse(
            r#"{ "a.count": { "defaultMessage": "{n, plural, zero {zero} one {one} two {two} few {few} many {many} other {other}}", "description": "d" } }"#,
            true,
        )
        .unwrap();
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../tests/plural-cases.json")).unwrap();
        let mut languages: Vec<&str> = cases["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| case["language"].as_str().unwrap())
            .collect();
        languages.sort_unstable();
        languages.dedup();
        let dir = std::env::temp_dir().join(format!("lingxia-i18n-plural-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ar.mjs"), module(&catalogue)).unwrap();
        std::fs::write(dir.join("formatter.mjs"), formatter(&languages)).unwrap();
        std::fs::write(dir.join("cases.json"), cases.to_string()).unwrap();
        std::fs::write(
            dir.join("check.mjs"),
            r#"import ar from './ar.mjs';
import { formatter } from './formatter.mjs';
import fs from 'node:fs';
const failures = [];
const cases0 = () => Object.fromEntries(['zero', 'one', 'two', 'few', 'many', 'other'].map((c) => [c, () => c]));
const arabic = [[0, 'zero'], [1, 'one'], [2, 'two'], [3, 'few'], [11, 'many'], [100, 'other']];
for (const [n, want] of arabic) {
  const got = ar['a.count']({ n }, formatter('ar'));
  if (got !== want) failures.push(`ar message n=${n}: ${got}, want ${want}`);
}
// A tag with a region reads its language's rules: en-US is en, ar-EG is ar.
if (ar['a.count']({ n: 3 }, formatter('ar-EG')) !== 'few') failures.push('ar-EG reads ar');
if (formatter('en-US').p(2, 0, true, cases0()) !== 'two') failures.push('en-US ordinal reads en');
const cases = Object.fromEntries(['zero', 'one', 'two', 'few', 'many', 'other'].map((c) => [c, () => c]));
for (const c of JSON.parse(fs.readFileSync(new URL('./cases.json', import.meta.url))).cases) {
  const got = formatter(c.language).p(c.value, 0, c.type === 'ordinal', cases);
  if (got !== c.category) failures.push(`${c.language} ${c.type} ${c.value}: ${got}, want ${c.category}`);
}
if (failures.length) { console.error(failures.join('\n')); process.exit(1); }
"#,
        )
        .unwrap();
        let output = match std::process::Command::new("node")
            .arg(dir.join("check.mjs"))
            .output()
        {
            Ok(output) => output,
            // CI always has Node; a bare local checkout may not.
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && std::env::var_os("CI").is_none() =>
            {
                eprintln!("skipping the JS plural check: node is not on PATH");
                return;
            }
            Err(error) => panic!("the JS plural check needs Node.js: {error}"),
        };
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The same messages through `format::format` and through a compiled module in Node.
    #[test]
    fn isolates_read_the_same_in_js_and_rust() {
        let catalogue = Catalogue::parse(
            r#"{ "a.price": { "defaultMessage": "{price}/חודש", "description": "d" },
                 "a.hello": { "defaultMessage": "Hello, {name}! You have {n, number} new.", "description": "d" },
                 "a.files": { "defaultMessage": "{n, plural, one {# ملف} other {# ملفات}}", "description": "d" },
                 "a.pick": { "defaultMessage": "{kind, select, a {أ} other {ب}}", "description": "d" } }"#,
            true,
        )
        .unwrap();
        let cases = serde_json::json!([
            ["a.price", "he", { "price": "$4" }],
            ["a.hello", "en-US", { "name": "דוד", "n": 3 }],
            ["a.hello", "en-US", { "name": "Ann", "n": 1234 }],
            ["a.hello", "ar", { "name": "Ann", "n": 3 }],
            ["a.files", "ar", { "n": 5 }],
            ["a.pick", "ar", { "kind": "a" }],
        ]);
        let rust: Vec<String> = cases
            .as_array()
            .unwrap()
            .iter()
            .map(|case| {
                let entry = &catalogue.entries[case[0].as_str().unwrap()];
                crate::format::format(&entry.message, case[1].as_str().unwrap(), &case[2])
            })
            .collect();
        let dir = std::env::temp_dir().join(format!("lingxia-i18n-bidi-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.mjs"), module(&catalogue)).unwrap();
        std::fs::write(dir.join("formatter.mjs"), formatter(&["ar", "en-US", "he"])).unwrap();
        std::fs::write(
            dir.join("check.mjs"),
            format!(
                "import m from './m.mjs';\nimport {{ formatter }} from './formatter.mjs';\n\
                 const out = {cases}.map(([key, tag, args]) => {{ const text = m[key](args, formatter(tag)); \
                 return Array.isArray(text) ? text.join('') : text; }});\n\
                 // A node argument is isolated by the message's language; its text is the caller's.\n\
                 const node = {{ node: true }};\n\
                 const rich = m['a.price']({{ price: node }}, formatter('he'));\n\
                 if (rich.length !== 4 || rich[0] !== '\\u2068' || rich[1] !== node || rich[2] !== '\\u2069') out.push('rich: ' + JSON.stringify(rich));\n\
                 console.log(JSON.stringify(out));\n"
            ),
        )
        .unwrap();
        let output = match std::process::Command::new("node")
            .arg(dir.join("check.mjs"))
            .output()
        {
            Ok(output) => output,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && std::env::var_os("CI").is_none() =>
            {
                eprintln!("skipping the JS isolate check: node is not on PATH");
                return;
            }
            Err(error) => panic!("the JS isolate check needs Node.js: {error}"),
        };
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let js: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(js, rust);
    }

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
