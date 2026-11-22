//! The ICU MessageFormat 1 subset a catalogue may use: `{arg}`, `number`, `date`, `time`,
//! `plural`, `selectordinal`, `select` and paired tags. `choice` and skeletons are refused, so a
//! message means the same on every target the generator writes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Text(String),
    Arg(Arg),
    Plural {
        name: String,
        ordinal: bool,
        offset: u32,
        cases: Vec<(Selector, Vec<Part>)>,
    },
    Select {
        name: String,
        cases: Vec<(String, Vec<Part>)>,
    },
    /// `#` in a plural case: the plural's number less its offset.
    Pound,
    Tag {
        name: String,
        children: Vec<Part>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    Text(String),
    Number { name: String, style: NumberStyle },
    Date { name: String, style: DateStyle },
    Time { name: String, style: DateStyle },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberStyle {
    Plain,
    Integer,
    Percent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    Short,
    Medium,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "zero" => Self::Zero,
            "one" => Self::One,
            "two" => Self::Two,
            "few" => Self::Few,
            "many" => Self::Many,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Selector {
    Exact(u32),
    Category(PluralCategory),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub reason: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {}: {}", self.offset, self.reason)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(source: &str) -> Result<Message, ParseError> {
    let mut parser = Parser {
        chars: source.char_indices().collect(),
        at: 0,
        len: source.len(),
    };
    let parts = parser.parts(false, None)?;
    if parser.at < parser.chars.len() {
        return Err(parser.error("unmatched `}`"));
    }
    Ok(Message { parts })
}

struct Parser {
    chars: Vec<(usize, char)>,
    at: usize,
    len: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).map(|&(_, c)| c)
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).map(|&(_, c)| c)
    }

    fn offset(&self) -> usize {
        self.chars.get(self.at).map_or(self.len, |&(i, _)| i)
    }

    fn error(&self, reason: impl Into<String>) -> ParseError {
        ParseError {
            offset: self.offset(),
            reason: reason.into(),
        }
    }

    /// Parts up to an unconsumed `}` or a closing tag; `tag` is the one that must close them.
    fn parts(&mut self, in_plural: bool, tag: Option<&str>) -> Result<Vec<Part>, ParseError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let flush = |text: &mut String, parts: &mut Vec<Part>| {
            if !text.is_empty() {
                parts.push(Part::Text(std::mem::take(text)));
            }
        };
        while let Some(c) = self.peek() {
            match c {
                '}' => break,
                '{' => {
                    flush(&mut text, &mut parts);
                    parts.push(self.placeholder()?);
                }
                '#' if in_plural => {
                    flush(&mut text, &mut parts);
                    self.at += 1;
                    parts.push(Part::Pound);
                }
                '\'' => self.apostrophe(in_plural, &mut text),
                '<' if self.peek_at(1) == Some('/') => {
                    let Some(open) = tag else {
                        return Err(self.error("closing tag without an opening one"));
                    };
                    self.at += 2;
                    let name = self.tag_name()?;
                    if name != open {
                        return Err(self.error(format!("`</{name}>` closes `<{open}>`")));
                    }
                    self.expect('>')?;
                    flush(&mut text, &mut parts);
                    return Ok(parts);
                }
                '<' if self.peek_at(1).is_some_and(|c| c.is_ascii_alphabetic()) => {
                    flush(&mut text, &mut parts);
                    self.at += 1;
                    let name = self.tag_name()?;
                    if self.peek() == Some('/') {
                        return Err(self.error("a tag wraps text: write `<name>…</name>`"));
                    }
                    self.expect('>')?;
                    let children = self.parts(in_plural, Some(&name))?;
                    parts.push(Part::Tag { name, children });
                }
                _ => {
                    text.push(c);
                    self.at += 1;
                }
            }
        }
        if let Some(open) = tag {
            return Err(self.error(format!("`<{open}>` is never closed")));
        }
        flush(&mut text, &mut parts);
        Ok(parts)
    }

    /// `''` is one apostrophe; one before a syntax character quotes up to the next lone `'`.
    fn apostrophe(&mut self, in_plural: bool, text: &mut String) {
        self.at += 1;
        match self.peek() {
            Some('\'') => {
                text.push('\'');
                self.at += 1;
            }
            Some('{' | '}' | '<' | '>') => self.quoted(text),
            Some('#') if in_plural => self.quoted(text),
            _ => text.push('\''),
        }
    }

    fn quoted(&mut self, text: &mut String) {
        while let Some(c) = self.peek() {
            self.at += 1;
            if c != '\'' {
                text.push(c);
            } else if self.peek() == Some('\'') {
                text.push('\'');
                self.at += 1;
            } else {
                return;
            }
        }
    }

    fn placeholder(&mut self) -> Result<Part, ParseError> {
        self.expect('{')?;
        self.space();
        let name = self.identifier("an argument name")?;
        self.space();
        if self.eat('}') {
            return Ok(Part::Arg(Arg::Text(name)));
        }
        self.expect(',')?;
        self.space();
        let kind = self.identifier("an argument type")?;
        self.space();
        let part = match kind.as_str() {
            "number" => {
                let style = match self.style()?.as_deref() {
                    None => NumberStyle::Plain,
                    Some("integer") => NumberStyle::Integer,
                    Some("percent") => NumberStyle::Percent,
                    Some(other) => {
                        return Err(self.error(format!("unknown number style `{other}`")));
                    }
                };
                Part::Arg(Arg::Number { name, style })
            }
            "date" | "time" => {
                let style = match self.style()?.as_deref() {
                    None | Some("medium") => DateStyle::Medium,
                    Some("short") => DateStyle::Short,
                    Some("long") => DateStyle::Long,
                    Some(other) => {
                        return Err(self.error(format!("unknown {kind} style `{other}`")));
                    }
                };
                Part::Arg(if kind == "date" {
                    Arg::Date { name, style }
                } else {
                    Arg::Time { name, style }
                })
            }
            "plural" | "selectordinal" => {
                self.expect(',')?;
                return self.plural(name, kind == "selectordinal");
            }
            "select" => {
                self.expect(',')?;
                return self.select(name);
            }
            other => return Err(self.error(format!("unsupported argument type `{other}`"))),
        };
        self.expect('}')?;
        Ok(part)
    }

    fn style(&mut self) -> Result<Option<String>, ParseError> {
        if !self.eat(',') {
            return Ok(None);
        }
        self.space();
        if self.peek() == Some(':') {
            return Err(self.error("skeletons are not supported"));
        }
        let style = self.identifier("a style")?;
        self.space();
        Ok(Some(style))
    }

    fn plural(&mut self, name: String, ordinal: bool) -> Result<Part, ParseError> {
        self.space();
        let mut offset = 0;
        if self.chars[self.at..]
            .iter()
            .map(|&(_, c)| c)
            .take(7)
            .eq("offset:".chars())
        {
            self.at += 7;
            self.space();
            offset = self.number()?;
        }
        let mut cases: Vec<(Selector, Vec<Part>)> = Vec::new();
        loop {
            self.space();
            if self.eat('}') {
                break;
            }
            let selector = if self.eat('=') {
                Selector::Exact(self.number()?)
            } else {
                let word = self.identifier("a plural category")?;
                Selector::Category(
                    PluralCategory::named(&word)
                        .ok_or_else(|| self.error(format!("`{word}` is not a plural category")))?,
                )
            };
            if cases.iter().any(|(s, _)| *s == selector) {
                return Err(self.error("a plural case appears twice"));
            }
            cases.push((selector, self.case(true)?));
        }
        if !cases
            .iter()
            .any(|(s, _)| *s == Selector::Category(PluralCategory::Other))
        {
            return Err(self.error(format!("`{name}` has no `other` case")));
        }
        Ok(Part::Plural {
            name,
            ordinal,
            offset,
            cases,
        })
    }

    fn select(&mut self, name: String) -> Result<Part, ParseError> {
        let mut cases: Vec<(String, Vec<Part>)> = Vec::new();
        loop {
            self.space();
            if self.eat('}') {
                break;
            }
            let key = self.identifier("a select key")?;
            if cases.iter().any(|(k, _)| *k == key) {
                return Err(self.error(format!("select case `{key}` appears twice")));
            }
            cases.push((key, self.case(false)?));
        }
        if !cases.iter().any(|(k, _)| k == "other") {
            return Err(self.error(format!("`{name}` has no `other` case")));
        }
        Ok(Part::Select { name, cases })
    }

    fn case(&mut self, in_plural: bool) -> Result<Vec<Part>, ParseError> {
        self.space();
        self.expect('{')?;
        let parts = self.parts(in_plural, None)?;
        self.expect('}')?;
        Ok(parts)
    }

    fn identifier(&mut self, what: &str) -> Result<String, ParseError> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.at += 1;
        }
        let word: String = self.chars[start..self.at].iter().map(|&(_, c)| c).collect();
        if word.is_empty() || word.starts_with(|c: char| c.is_ascii_digit()) {
            self.at = start;
            return Err(self.error(format!("expected {what}")));
        }
        Ok(word)
    }

    fn tag_name(&mut self) -> Result<String, ParseError> {
        let start = self.at;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            self.at += 1;
        }
        if start == self.at {
            return Err(self.error("expected a tag name"));
        }
        Ok(self.chars[start..self.at].iter().map(|&(_, c)| c).collect())
    }

    fn number(&mut self) -> Result<u32, ParseError> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        let digits: String = self.chars[start..self.at].iter().map(|&(_, c)| c).collect();
        digits.parse().map_err(|_| {
            self.at = start;
            self.error("expected a number")
        })
    }

    fn space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        let hit = self.peek() == Some(c);
        if hit {
            self.at += 1;
        }
        hit
    }

    fn expect(&mut self, c: char) -> Result<(), ParseError> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(self.error(format!("expected `{c}`")))
        }
    }
}

/// How a message uses one argument; a caller passes a value of that kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Use {
    Text,
    Number,
    Date,
    /// The keys the message names besides `other`.
    Select(BTreeSet<String>),
    Tag,
}

impl Use {
    fn kind(&self) -> &'static str {
        match self {
            Use::Text => "text",
            Use::Number => "a number",
            Use::Date => "a date",
            Use::Select(_) => "a select",
            Use::Tag => "a tag",
        }
    }
}

/// Every argument and tag a message takes. Tags are keyed `<name>` so they never meet an argument.
pub type Signature = BTreeMap<String, Use>;

impl Message {
    pub fn signature(&self) -> Result<Signature, String> {
        let mut signature = Signature::new();
        collect(&self.parts, &mut signature)?;
        Ok(signature)
    }
}

fn collect(parts: &[Part], signature: &mut Signature) -> Result<(), String> {
    for part in parts {
        match part {
            Part::Text(_) | Part::Pound => {}
            Part::Arg(Arg::Text(name)) => add(signature, name, Use::Text)?,
            Part::Arg(Arg::Number { name, .. }) => add(signature, name, Use::Number)?,
            Part::Arg(Arg::Date { name, .. } | Arg::Time { name, .. }) => {
                add(signature, name, Use::Date)?
            }
            Part::Plural { name, cases, .. } => {
                add(signature, name, Use::Number)?;
                for (_, case) in cases {
                    collect(case, signature)?;
                }
            }
            Part::Select { name, cases } => {
                let keys = cases
                    .iter()
                    .map(|(key, _)| key.clone())
                    .filter(|key| key != "other")
                    .collect();
                add(signature, name, Use::Select(keys))?;
                for (_, case) in cases {
                    collect(case, signature)?;
                }
            }
            Part::Tag { name, children } => {
                add(signature, &format!("<{name}>"), Use::Tag)?;
                collect(children, signature)?;
            }
        }
    }
    Ok(())
}

fn add(signature: &mut Signature, name: &str, next: Use) -> Result<(), String> {
    let Some(known) = signature.get_mut(name) else {
        signature.insert(name.to_string(), next);
        return Ok(());
    };
    match (known, next) {
        (Use::Select(keys), Use::Select(more)) => keys.extend(more),
        // A plain `{n}` prints whatever it is given, so the stricter use decides.
        (known, next) if *known == Use::Text && next != Use::Tag => *known = next,
        (known, Use::Text) if *known != Use::Tag => {}
        // A tag may wrap text twice; the caller still passes one function.
        (known, next) if *known == next => {}
        (known, next) => {
            return Err(format!(
                "`{name}` is used as {} and as {}",
                known.kind(),
                next.kind()
            ));
        }
    }
    Ok(())
}

/// What a translation changes about the arguments its source takes; empty when it keeps them.
pub fn signature_drift(source: &Signature, translation: &Signature) -> Vec<String> {
    let mut drift = Vec::new();
    for (name, wanted) in source {
        match translation.get(name) {
            None => drift.push(format!("drops `{name}`")),
            Some(Use::Select(keys)) => {
                if let Use::Select(known) = wanted {
                    for key in keys.difference(known) {
                        drift.push(format!(
                            "`{name}` selects `{key}`, which the source never does"
                        ));
                    }
                } else {
                    drift.push(format!(
                        "uses `{name}` as a select, the source as {}",
                        wanted.kind()
                    ));
                }
            }
            // A language without plural forms may print the count, or a select's value, as is.
            Some(Use::Text) if matches!(wanted, Use::Number | Use::Select(_)) => {}
            Some(used) if std::mem::discriminant(used) != std::mem::discriminant(wanted) => drift
                .push(format!(
                    "uses `{name}` as {}, the source as {}",
                    used.kind(),
                    wanted.kind()
                )),
            Some(_) => {}
        }
    }
    for name in translation
        .keys()
        .filter(|name| !source.contains_key(*name))
    {
        drift.push(format!("adds `{name}`, which the source does not take"));
    }
    drift
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(source: &str) -> Signature {
        parse(source).unwrap().signature().unwrap()
    }

    #[test]
    fn an_apostrophe_quotes_only_before_syntax() {
        let text = |source| match parse(source).unwrap().parts.as_slice() {
            [Part::Text(text)] => text.clone(),
            parts => panic!("{parts:?}"),
        };
        assert_eq!(text("It's '{literal}'"), "It's {literal}");
        assert_eq!(text("a''b"), "a'b");
    }

    #[test]
    fn pound_is_text_outside_a_plural() {
        assert_eq!(parse("#1").unwrap().parts, vec![Part::Text("#1".into())]);
        let parts = parse("{n, plural, other {# left}}").unwrap().parts;
        let [Part::Plural { cases, .. }] = parts.as_slice() else {
            panic!()
        };
        assert_eq!(cases[0].1, vec![Part::Pound, Part::Text(" left".into())]);
    }

    #[test]
    fn a_plural_or_select_needs_other() {
        assert!(parse("{n, plural, one {#}}").is_err());
        assert!(parse("{s, select, a {A}}").is_err());
        assert!(parse("{n, plural, offset:1 =0 {none} one {#} other {#}}").is_ok());
    }

    #[test]
    fn only_the_portable_subset_parses() {
        for refused in [
            "{n, choice, 0#none}",
            "{n, number, ::currency/EUR}",
            "{d, date, full}",
            "<br/>",
            "<b>open",
            "a</b>",
            "{n",
        ] {
            assert!(parse(refused).is_err(), "{refused}");
        }
        // `<` before a non-letter is text.
        assert_eq!(
            parse("1 < 2").unwrap().parts,
            vec![Part::Text("1 < 2".into())]
        );
    }

    #[test]
    fn an_argument_keeps_one_kind() {
        assert_eq!(signature("{n} of {n, plural, other {#}}").len(), 1);
        assert!(
            parse("{n, number} on {n, date}")
                .unwrap()
                .signature()
                .is_err()
        );
        assert!(parse("<n>x</n> {n}").unwrap().signature().is_ok());
        assert_eq!(signature("{d} {d, date}")["d"], Use::Date);
        let both = signature("{s, select, a {A} other {}} {s, select, b {B} other {}}");
        assert_eq!(both["s"], Use::Select(["a".into(), "b".into()].into()));
    }

    #[test]
    fn a_translation_keeps_its_source_arguments() {
        let source =
            signature("<link>Terms</link> for {name}: {state, select, open {Open} other {?}}");
        assert!(
            signature_drift(
                &source,
                &signature("{name}: <link>条款</link> {state, select, open {开} other {?}}")
            )
            .is_empty()
        );
        let drift = signature_drift(
            &source,
            &signature("{who} {state, select, shut {关} other {?}}"),
        );
        assert_eq!(drift.len(), 4, "{drift:?}");
    }
}
