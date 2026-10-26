//! What the platform keyboard does for one editor. Core decides; adapters map
//! each field onto their IME, return key, autofill and picker APIs.
use crate::scene::node::InputSnapshot;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Keyboard {
    #[default]
    Text,
    /// Signed decimal.
    Number,
    Email,
    Phone,
    Search,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capitalize {
    #[default]
    None,
    Sentences,
    Words,
    Characters,
}

/// The return key's label and meaning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnKey {
    /// Plain return: inserts a newline.
    #[default]
    Newline,
    Done,
    Go,
    Next,
    Previous,
    Search,
    Send,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PickerKind {
    /// `YYYY-MM-DD`.
    Date,
    /// `HH:MM`.
    Time,
}

/// A date or time editor takes its value from a platform picker, never a keyboard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Picker {
    pub kind: PickerKind,
    /// Inclusive bounds in the kind's format.
    pub min: Option<String>,
    pub max: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ImeConfig {
    pub keyboard: Keyboard,
    /// Masked entry; also implied by password autocomplete tokens.
    pub secure: bool,
    pub multiline: bool,
    pub capitalize: Capitalize,
    /// Spelling suggestions and personalized learning.
    pub suggestions: bool,
    pub return_key: ReturnKey,
    /// Return inserts a newline; shift+return always does in a multiline editor.
    pub return_inserts_newline: bool,
    /// Dismiss the keyboard after a submit.
    pub hide_on_submit: bool,
    pub picker: Option<Picker>,
    /// HTML autocomplete token; adapters map it to their autofill hint.
    pub auto_complete: Option<String>,
    /// UTF-16 units; core cuts insertions, a mirror may cut early to avoid flicker.
    pub max_length: Option<u32>,
}

impl ImeConfig {
    /// Configurations compare equal exactly when the platform need not restart input.
    pub fn new(input: &InputSnapshot, multiline: bool) -> Self {
        let attributes = &input.attributes;
        let keyboard = match input.keyboard.as_str() {
            "number" => Keyboard::Number,
            "email" => Keyboard::Email,
            "phone" => Keyboard::Phone,
            "search" => Keyboard::Search,
            _ => Keyboard::Text,
        };
        let auto_complete = attributes.auto_complete.clone();
        let secure = input.secure
            || matches!(
                auto_complete.as_deref(),
                Some("current-password" | "new-password")
            );
        let capitalize = match attributes.auto_capitalize.as_deref() {
            _ if matches!(keyboard, Keyboard::Number | Keyboard::Phone) => Capitalize::None,
            Some("sentences" | "on") => Capitalize::Sentences,
            Some("words") => Capitalize::Words,
            Some("characters") => Capitalize::Characters,
            _ => Capitalize::None,
        };
        let newline = multiline && !input.submit_on_enter;
        let hinted = match attributes.enter_key_hint.as_deref() {
            Some("done") => Some(ReturnKey::Done),
            Some("go") => Some(ReturnKey::Go),
            Some("next") => Some(ReturnKey::Next),
            Some("previous") => Some(ReturnKey::Previous),
            Some("search") => Some(ReturnKey::Search),
            Some("send") => Some(ReturnKey::Send),
            _ => None,
        };
        let return_key = match hinted {
            Some(hinted) if !newline => hinted,
            _ if newline => ReturnKey::Newline,
            _ if multiline => ReturnKey::Send,
            _ if keyboard == Keyboard::Search => ReturnKey::Search,
            _ => ReturnKey::Done,
        };
        let bound = |value: &Option<serde_json::Value>| value.as_ref()?.as_str().map(str::to_owned);
        let picker = attributes.picker.as_deref().and_then(|kind| {
            let kind = match kind {
                "date" => PickerKind::Date,
                "time" => PickerKind::Time,
                _ => return None,
            };
            let valid = |value: Option<String>| value.filter(|value| parse(kind, value).is_some());
            Some(Picker {
                kind,
                min: valid(bound(&attributes.min)),
                max: valid(bound(&attributes.max)),
            })
        });
        Self {
            keyboard,
            secure,
            multiline,
            capitalize,
            suggestions: attributes.spell_check.unwrap_or(true) && !secure,
            return_key,
            return_inserts_newline: newline,
            hide_on_submit: !multiline && keyboard != Keyboard::Search,
            picker,
            auto_complete,
            max_length: attributes.max_length,
        }
    }

    /// The picker's initial value: the editor's value when it parses.
    pub fn picker_seed(&self, value: &str) -> Option<String> {
        let picker = self.picker.as_ref()?;
        parse(picker.kind, value).map(|_| value[..len(picker.kind)].to_owned())
    }

    /// A picked value normalized and clamped to the bounds, or `None` when invalid.
    pub fn picked(&self, value: &str) -> Option<String> {
        let picker = self.picker.as_ref()?;
        parse(picker.kind, value)?;
        let mut value = value[..len(picker.kind)].to_owned();
        // Zero-padded ISO forms order lexically.
        if let Some(min) = &picker.min
            && value.as_str() < &min[..len(picker.kind)]
        {
            value = min[..len(picker.kind)].to_owned();
        }
        if let Some(max) = &picker.max
            && value.as_str() > &max[..len(picker.kind)]
        {
            value = max[..len(picker.kind)].to_owned();
        }
        Some(value)
    }
}

fn len(kind: PickerKind) -> usize {
    match kind {
        PickerKind::Date => 10,
        PickerKind::Time => 5,
    }
}

/// `[year, month, day]` or `[hour, minute, 0]`; time accepts trailing `:SS`.
fn parse(kind: PickerKind, value: &str) -> Option<[u32; 3]> {
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = value.get(range)?;
        part.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let bytes = value.as_bytes();
    match kind {
        PickerKind::Date => {
            if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
                return None;
            }
            let date = [digits(0..4)?, digits(5..7)?, digits(8..10)?];
            ((1..=12).contains(&date[1]) && (1..=31).contains(&date[2])).then_some(date)
        }
        PickerKind::Time => {
            if !(bytes.len() == 5 || bytes.len() == 8) || bytes[2] != b':' {
                return None;
            }
            if bytes.len() == 8 && (bytes[5] != b':' || digits(6..8)? > 59) {
                return None;
            }
            let time = [digits(0..2)?, digits(3..5)?, 0];
            (time[0] < 24 && time[1] < 60).then_some(time)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::node::EditorAttributes;

    fn input(keyboard: &str, attributes: EditorAttributes) -> InputSnapshot {
        InputSnapshot {
            value: String::new(),
            placeholder: String::new(),
            secure: false,
            keyboard: keyboard.into(),
            auto_focus: false,
            submit_on_enter: false,
            enabled: true,
            on_input: None,
            on_submit: None,
            attributes,
            edit: None,
            edit_token: None,
            geometry: None,
            scroll_offset: [0.0; 2],
            ime: Default::default(),
        }
    }

    #[test]
    fn return_key_follows_the_hint_unless_return_must_insert_a_newline() {
        let hint = |hint: &str| EditorAttributes {
            enter_key_hint: Some(hint.into()),
            ..Default::default()
        };
        assert_eq!(
            ImeConfig::new(&input("text", hint("go")), false).return_key,
            ReturnKey::Go
        );
        assert_eq!(
            ImeConfig::new(&input("text", hint("enter")), false).return_key,
            ReturnKey::Done
        );
        assert_eq!(
            ImeConfig::new(&input("search", Default::default()), false).return_key,
            ReturnKey::Search
        );
        let notes = ImeConfig::new(&input("text", hint("send")), true);
        assert_eq!(notes.return_key, ReturnKey::Newline);
        assert!(notes.return_inserts_newline && !notes.hide_on_submit);
        let mut chat = input("text", hint("send"));
        chat.submit_on_enter = true;
        let chat = ImeConfig::new(&chat, true);
        assert_eq!(chat.return_key, ReturnKey::Send);
        assert!(!chat.return_inserts_newline);
        let mut plain = input("text", Default::default());
        plain.submit_on_enter = true;
        assert_eq!(ImeConfig::new(&plain, true).return_key, ReturnKey::Send);
    }

    #[test]
    fn single_line_submit_hides_the_keyboard_except_for_search() {
        assert!(ImeConfig::new(&input("text", Default::default()), false).hide_on_submit);
        assert!(!ImeConfig::new(&input("search", Default::default()), false).hide_on_submit);
    }

    #[test]
    fn password_tokens_mask_and_suppress_suggestions() {
        let config = ImeConfig::new(
            &input(
                "text",
                EditorAttributes {
                    auto_complete: Some("new-password".into()),
                    auto_capitalize: Some("words".into()),
                    ..Default::default()
                },
            ),
            false,
        );
        assert!(config.secure && !config.suggestions);
        assert_eq!(config.capitalize, Capitalize::Words);
        let off = ImeConfig::new(
            &input(
                "text",
                EditorAttributes {
                    spell_check: Some(false),
                    ..Default::default()
                },
            ),
            false,
        );
        assert!(!off.secure && !off.suggestions);
        let number = ImeConfig::new(
            &input(
                "number",
                EditorAttributes {
                    auto_capitalize: Some("characters".into()),
                    ..Default::default()
                },
            ),
            false,
        );
        assert_eq!(number.capitalize, Capitalize::None);
        assert_eq!(number.keyboard, Keyboard::Number);
    }

    #[test]
    fn pickers_seed_from_valid_values_and_clamp_picks_to_their_bounds() {
        let time = ImeConfig::new(
            &input(
                "time",
                EditorAttributes {
                    picker: Some("time".into()),
                    min: Some("09:30".into()),
                    max: Some("18:00:00".into()),
                    ..Default::default()
                },
            ),
            false,
        );
        let picker = time.picker.as_ref().unwrap();
        assert_eq!(picker.kind, PickerKind::Time);
        assert_eq!(time.picked("08:00").as_deref(), Some("09:30"));
        assert_eq!(time.picked("19:15").as_deref(), Some("18:00"));
        assert_eq!(time.picked("12:05:59").as_deref(), Some("12:05"));
        assert_eq!(time.picked("24:00"), None);
        assert_eq!(time.picker_seed("7:00"), None);
        let date = ImeConfig::new(
            &input(
                "date",
                EditorAttributes {
                    picker: Some("date".into()),
                    min: Some(serde_json::json!(5)),
                    max: Some("2026-12-31".into()),
                    ..Default::default()
                },
            ),
            false,
        );
        let picker = date.picker.as_ref().unwrap();
        assert_eq!(picker.min, None, "a non-date bound is ignored");
        assert_eq!(date.picked("2027-01-02").as_deref(), Some("2026-12-31"));
        assert_eq!(
            date.picker_seed("2026-02-03").as_deref(),
            Some("2026-02-03")
        );
        assert_eq!(date.picked("2026-13-01"), None);
        assert!(
            ImeConfig::new(&input("text", Default::default()), false)
                .picker
                .is_none()
        );
    }
}
