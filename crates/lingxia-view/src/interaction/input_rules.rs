//! Editor attributes the session enforces natively: `maxLength`, numeric
//! and date/time bounds, and picker value formats.
use crate::protocol::{Record, field};
use crate::text::input::Snapshot;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Rules {
    pub max_length: Option<usize>,
    pub keyboard: String,
    min: Option<serde_json::Value>,
    max: Option<serde_json::Value>,
    step: Option<f64>,
}

impl Rules {
    /// What the View's picker needs for a `date` or `time` editor: kind, min, max and step.
    pub fn picker_spec(&self) -> Option<serde_json::Value> {
        self.picker().then(|| {
            serde_json::json!({
                "kind": self.keyboard, "min": self.min, "max": self.max, "step": self.step,
            })
        })
    }
}

/// What a `keyboard` editor keeps of `text` typed between `before` and `after`, as its own
/// filter does; `None` when it drops a character.
pub(crate) fn typed(keyboard: &str, before: &str, after: &str, text: &str) -> Option<String> {
    let rules = Rules {
        keyboard: keyboard.to_owned(),
        ..Rules::default()
    };
    let caret = units(before) as u32;
    let snapshot = Snapshot {
        text: format!("{before}{after}"),
        selection: [caret, caret],
        composition: None,
        focused: true,
        can_undo: false,
        can_redo: false,
    };
    let kept = rules.filter(&snapshot, text);
    (kept.chars().count() == text.chars().count()).then_some(kept)
}

/// Whether `text` is an ISO 8601 `date` (YYYY-MM-DD) or `time` (HH:MM[:SS]).
pub(crate) fn iso(text: &str, kind: &str) -> bool {
    let digits = |part: &str, width: usize, max: u32| {
        part.len() == width
            && part.bytes().all(|b| b.is_ascii_digit())
            && part.parse::<u32>().is_ok_and(|v| v <= max)
    };
    let parts: Vec<&str> = text.split(if kind == "date" { '-' } else { ':' }).collect();
    match (kind, parts.as_slice()) {
        ("date", [year, month, day]) => {
            digits(year, 4, 9999)
                && digits(month, 2, 12)
                && digits(day, 2, 31)
                && month != &"00"
                && day != &"00"
        }
        ("time", [hours, minutes]) => digits(hours, 2, 23) && digits(minutes, 2, 59),
        ("time", [hours, minutes, seconds]) => {
            digits(hours, 2, 23) && digits(minutes, 2, 59) && digits(seconds, 2, 59)
        }
        _ => false,
    }
}

/// The bound an ISO `text` lies outside, as "from {min}" or "up to {max}": a picker cannot
/// choose it, and a commit would clamp it there.
pub(crate) fn outside(
    text: &str,
    min: Option<&serde_json::Value>,
    max: Option<&serde_json::Value>,
) -> Option<String> {
    // ISO dates and times order like their strings.
    if let Some(min) = min.and_then(|v| v.as_str()).filter(|min| text < *min) {
        return Some(format!("from {min}"));
    }
    max.and_then(|v| v.as_str())
        .filter(|max| text > *max)
        .map(|max| format!("up to {max}"))
}

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `text` before UTF-16 offset `start` and after `end`.
fn split_utf16(text: &str, start: u32, end: u32) -> (&str, &str) {
    let byte = |unit: u32| {
        let mut seen = 0;
        for (i, c) in text.char_indices() {
            if seen >= unit {
                return i;
            }
            seen += c.len_utf16() as u32;
        }
        text.len()
    };
    (&text[..byte(start)], &text[byte(end)..])
}

impl Rules {
    pub fn from_record(record: &Record) -> Self {
        let value = |id| {
            record
                .fields
                .iter()
                .find(|(field, _)| *field == id)
                .map(|(_, value)| value.clone())
        };
        Self {
            max_length: record.number(field::MAX_LENGTH).map(|n| n as usize),
            keyboard: record.string(field::KEYBOARD).unwrap_or("text").to_owned(),
            min: value(field::INPUT_MIN),
            max: value(field::INPUT_MAX),
            step: record.number(field::INPUT_STEP).map(f64::from),
        }
    }

    pub fn picker(&self) -> bool {
        matches!(self.keyboard.as_str(), "date" | "time")
    }

    /// Insertion text cut so the buffer never grows past `maxLength`; deletions always pass.
    pub fn fit(&self, before: &Snapshot, text: &str) -> String {
        let Some(max) = self.max_length else {
            return text.to_owned();
        };
        let [a, b] = before.composition.unwrap_or(before.selection);
        let replaced = a.abs_diff(b) as usize;
        let room = max.saturating_sub(units(&before.text).saturating_sub(replaced));
        let mut taken = 0;
        text.chars()
            .take_while(|c| {
                taken += c.len_utf16();
                taken <= room
            })
            .collect()
    }

    /// Inserted text a `number` or `decimal` editor keeps, as the platform's own numeric
    /// field filters it: digits; for `decimal` also one `.` (a `,` is taken as one) and a
    /// leading `-`. Full-width digits become ASCII. Other editors keep everything.
    pub fn filter(&self, before: &Snapshot, text: &str) -> String {
        let decimal = match self.keyboard.as_str() {
            "number" => false,
            "decimal" => true,
            _ => return text.to_owned(),
        };
        let [a, b] = before.composition.unwrap_or(before.selection);
        let (head, tail) = split_utf16(&before.text, a.min(b), a.max(b));
        let mut separator = head.contains('.') || tail.contains('.');
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            let c = match c {
                '\u{FF10}'..='\u{FF19}' => char::from(b'0' + (c as u32 - 0xFF10) as u8),
                ',' | '\u{066B}' => '.',
                '\u{2212}' => '-',
                c => c,
            };
            match c {
                '0'..='9' => out.push(c),
                '.' if decimal && !separator => {
                    separator = true;
                    out.push(c);
                }
                '-' if decimal && head.is_empty() && out.is_empty() && !tail.starts_with('-') => {
                    out.push(c)
                }
                _ => {}
            }
        }
        out
    }

    /// Picker editors hold an ISO value or nothing.
    pub fn accepts(&self, text: &str) -> bool {
        !self.picker() || text.is_empty() || iso(text, &self.keyboard)
    }

    /// The committed value after bounds and step, when it differs from `text`.
    pub fn commit(&self, text: &str) -> Option<String> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        if self.picker() {
            // ISO dates and times order like their strings.
            let mut value = text;
            if let Some(min) = self.min.as_ref().and_then(|v| v.as_str()) {
                value = value.max(min);
            }
            if let Some(max) = self.max.as_ref().and_then(|v| v.as_str()) {
                value = value.min(max);
            }
            return (value != text).then(|| value.to_owned());
        }
        if !matches!(self.keyboard.as_str(), "number" | "decimal") {
            return None;
        }
        let parsed: f64 = text.parse().ok().filter(|v: &f64| v.is_finite())?;
        let min = self.min.as_ref().and_then(|v| v.as_f64());
        let max = self.max.as_ref().and_then(|v| v.as_f64());
        let mut value = parsed;
        if let Some(step) = self.step {
            // The step base is `min`, as in HTML; values snap to the nearest step.
            let base = min.unwrap_or(0.0);
            value = base + ((value - base) / step).round() * step;
        }
        if let Some(min) = min {
            value = value.max(min);
        }
        if let Some(max) = max {
            value = value.min(max);
        }
        let decimals = [self.step, min, max]
            .iter()
            .flatten()
            .chain([parsed].iter())
            .map(|v| {
                let text = format!("{v}");
                text.split_once('.').map(|(_, f)| f.len()).unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
            .min(12);
        let formatted = format!("{value:.decimals$}");
        let formatted = if formatted.contains('.') {
            formatted
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        } else {
            formatted
        };
        // The typed text stands unless the number itself changes: "7.0" stays "7.0".
        (formatted.parse::<f64>().ok() != Some(parsed))
            .then_some(formatted)
            .filter(|formatted| formatted != text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules(
        keyboard: &str,
        min: serde_json::Value,
        max: serde_json::Value,
        step: Option<f64>,
    ) -> Rules {
        Rules {
            max_length: None,
            keyboard: keyboard.into(),
            min: (!min.is_null()).then_some(min),
            max: (!max.is_null()).then_some(max),
            step,
        }
    }

    fn at(text: &str, caret: u32) -> Snapshot {
        Snapshot {
            text: text.into(),
            selection: [caret, caret],
            composition: None,
            focused: true,
            can_undo: false,
            can_redo: false,
        }
    }

    #[test]
    fn numeric_editors_keep_only_what_their_keyboard_types() {
        let number = rules("number", json!(null), json!(null), None);
        assert_eq!(number.filter(&at("", 0), ".0"), "0");
        assert_eq!(number.filter(&at("1", 1), "-2,5"), "25");
        assert_eq!(number.filter(&at("", 0), "１２"), "12");
        let decimal = rules("decimal", json!(null), json!(null), None);
        assert_eq!(decimal.filter(&at("", 0), "-1.5.2"), "-1.52");
        assert_eq!(decimal.filter(&at("2", 1), ",5"), ".5");
        assert_eq!(decimal.filter(&at("2.5", 3), ".1"), "1");
        assert_eq!(decimal.filter(&at("2", 1), "-"), "");
        assert_eq!(decimal.filter(&at("-2", 0), "-"), "");
        let text = rules("text", json!(null), json!(null), None);
        assert_eq!(text.filter(&at("", 0), "a.-"), "a.-");
    }

    #[test]
    fn numbers_clamp_and_snap_on_commit() {
        let r = rules("number", json!(1), json!(10), Some(0.5));
        assert_eq!(r.commit("12"), Some("10".into()));
        assert_eq!(r.commit("0"), Some("1".into()));
        assert_eq!(r.commit("3.3"), Some("3.5".into()));
        assert_eq!(r.commit("3.5"), None);
        assert_eq!(r.commit("abc"), None);
        assert_eq!(r.commit("7.0"), None);
        assert_eq!(
            rules("number", json!(null), json!(null), Some(0.1)).commit("0.30000000000000004"),
            Some("0.3".into())
        );
    }

    #[test]
    fn dates_and_times_validate_and_clamp_as_iso_strings() {
        let r = rules("date", json!("2024-01-01"), json!("2024-12-31"), None);
        assert!(
            r.accepts("2024-02-29")
                && r.accepts("")
                && !r.accepts("2024-2-29")
                && !r.accepts("02/29/2024")
        );
        assert_eq!(r.commit("2025-03-01"), Some("2024-12-31".into()));
        let (min, max) = (json!("2024-01-01"), json!("2024-12-31"));
        assert_eq!(
            outside("2023-12-31", Some(&min), Some(&max)).as_deref(),
            Some("from 2024-01-01")
        );
        assert_eq!(
            outside("2025-01-01", Some(&min), Some(&max)).as_deref(),
            Some("up to 2024-12-31")
        );
        assert_eq!(outside("2024-01-01", Some(&min), Some(&max)), None);
        assert_eq!(outside("2024-12-31", None, Some(&json!(null))), None);
        assert!(iso("23:59:59", "time") && !iso("24:00", "time") && !iso("2024-13-01", "date"));
    }

    #[test]
    fn max_length_cuts_insertions_in_utf16_units() {
        let r = Rules {
            max_length: Some(5),
            ..Rules::default()
        };
        let before = Snapshot {
            text: "abc".into(),
            selection: [3, 3],
            composition: None,
            focused: true,
            can_undo: false,
            can_redo: false,
        };
        assert_eq!(r.fit(&before, "de😀"), "de");
        let replacing = Snapshot {
            selection: [0, 3],
            ..before.clone()
        };
        assert_eq!(r.fit(&replacing, "12345678"), "12345");
    }
}
