//! One language's catalogue file and the checks between a translation and its source.
//!
//! ```json
//! { "cart.items": { "defaultMessage": "{count, plural, one {# item} other {# items}}",
//!                   "description": "Under the list; count = items in the cart." } }
//! ```

use crate::message::{self, Message, Signature};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Entry {
    pub source: String,
    pub message: Message,
    pub signature: Signature,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub key: Option<String>,
    pub reason: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.key {
            Some(key) => write!(f, "{key}: {}", self.reason),
            None => f.write_str(&self.reason),
        }
    }
}

fn problem(key: &str, reason: impl Into<String>) -> Problem {
    Problem {
        key: Some(key.to_string()),
        reason: reason.into(),
    }
}

/// Dot-separated camelCase segments, optionally under a package namespace: `cart.title`,
/// `@view.picker.done`.
pub fn valid_key(key: &str) -> bool {
    let body = key.strip_prefix('@').unwrap_or(key);
    let mut segments = body.split('.');
    let count = body.split('.').count();
    // A namespace alone names no message.
    if key.starts_with('@') && count < 2 {
        return false;
    }
    segments.all(|segment| {
        let mut chars = segment.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_alphanumeric())
    })
}

impl Catalogue {
    /// The source catalogue documents every key for its translators; a translation need not.
    pub fn parse(json: &str, is_source: bool) -> Result<Catalogue, Vec<Problem>> {
        let value: serde_json::Value = serde_json::from_str(json).map_err(|error| {
            vec![Problem {
                key: None,
                reason: format!("not JSON: {error}"),
            }]
        })?;
        let serde_json::Value::Object(object) = value else {
            return Err(vec![Problem {
                key: None,
                reason: "a catalogue is an object of keys".into(),
            }]);
        };
        let mut catalogue = Catalogue::default();
        let mut problems = Vec::new();
        for (key, value) in object {
            match entry(&key, &value, is_source) {
                Ok(entry) => {
                    catalogue.entries.insert(key, entry);
                }
                Err(reason) => problems.push(problem(&key, reason)),
            }
        }
        if problems.is_empty() {
            Ok(catalogue)
        } else {
            Err(problems)
        }
    }
}

fn entry(key: &str, value: &serde_json::Value, is_source: bool) -> Result<Entry, String> {
    if !valid_key(key) {
        return Err("a key is dot-separated camelCase segments, as `cart.itemCount`".into());
    }
    let object = value
        .as_object()
        .ok_or("an entry is `{ \"defaultMessage\": …, \"description\": … }`")?;
    if let Some(field) = object
        .keys()
        .find(|field| *field != "defaultMessage" && *field != "description")
    {
        return Err(format!("unknown field `{field}`"));
    }
    let source = object
        .get("defaultMessage")
        .and_then(|v| v.as_str())
        .ok_or("`defaultMessage` must be a string")?;
    let description = match object.get("description") {
        None => None,
        Some(serde_json::Value::String(text)) if !text.trim().is_empty() => Some(text.clone()),
        Some(_) => return Err("`description` must be a non-empty string".into()),
    };
    if is_source && description.is_none() {
        return Err("the source language describes every key for its translators".into());
    }
    let message = message::parse(source).map_err(|error| format!("{error}"))?;
    let signature = message.signature()?;
    Ok(Entry {
        source: source.to_string(),
        message,
        signature,
        description,
    })
}

/// What a translation gets wrong against its source, and the source keys it has yet to translate.
/// `@` keys absent from the source override a package's and are checked against that package.
pub fn check(source: &Catalogue, translation: &Catalogue) -> (Vec<Problem>, Vec<String>) {
    let mut problems = Vec::new();
    for (key, entry) in &translation.entries {
        match source.entries.get(key) {
            Some(original) => {
                for drift in message::signature_drift(&original.signature, &entry.signature) {
                    problems.push(problem(key, drift));
                }
            }
            None if key.starts_with('@') => {}
            None => problems.push(problem(key, "not in the source language")),
        }
    }
    let missing = source
        .entries
        .keys()
        .filter(|key| !translation.entries.contains_key(*key))
        .cloned()
        .collect();
    (problems, missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_dotted_camel_case() {
        for good in ["cart.title", "cart.itemCount", "@view.picker.done", "title"] {
            assert!(valid_key(good), "{good}");
        }
        for bad in [
            "Cart.title",
            "cart..title",
            "cart.item-count",
            "@view",
            "cart.",
            "cart_title",
        ] {
            assert!(!valid_key(bad), "{bad}");
        }
    }

    #[test]
    fn only_the_source_must_describe_its_keys() {
        let bare = r#"{ "cart.title": { "defaultMessage": "Cart" } }"#;
        assert!(Catalogue::parse(bare, true).is_err());
        assert!(Catalogue::parse(bare, false).is_ok());
    }

    #[test]
    fn a_translation_is_checked_key_by_key() {
        let source = Catalogue::parse(
            r#"{ "a.greet": { "defaultMessage": "Hi {name}", "description": "d" },
                 "a.left": { "defaultMessage": "{n, plural, one {# left} other {# left}}", "description": "d" } }"#,
            true,
        )
        .unwrap();
        let translation = Catalogue::parse(
            r#"{ "a.greet": { "defaultMessage": "你好" },
                 "a.stale": { "defaultMessage": "旧" },
                 "@view.picker.done": { "defaultMessage": "好" } }"#,
            false,
        )
        .unwrap();
        let (problems, missing) = check(&source, &translation);
        let reasons: Vec<String> = problems.iter().map(ToString::to_string).collect();
        assert_eq!(
            reasons,
            [
                "a.greet: drops `name`",
                "a.stale: not in the source language"
            ]
        );
        assert_eq!(missing, ["a.left"]);
    }
}
