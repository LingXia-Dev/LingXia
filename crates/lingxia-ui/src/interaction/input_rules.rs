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

fn units(text: &str) -> usize {
    text.encode_utf16().count()
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
        if self.keyboard != "number" {
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
        (value != parsed || formatted != text)
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

    #[test]
    fn numbers_clamp_and_snap_on_commit() {
        let r = rules("number", json!(1), json!(10), Some(0.5));
        assert_eq!(r.commit("12"), Some("10".into()));
        assert_eq!(r.commit("0"), Some("1".into()));
        assert_eq!(r.commit("3.3"), Some("3.5".into()));
        assert_eq!(r.commit("3.5"), None);
        assert_eq!(r.commit("abc"), None);
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
