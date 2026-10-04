//! Manifest text that may be declared once per display language.
//!
//! `tabBar.items[].text` and a page's `navigationBar.title` accept either a
//! plain string — as before — or a map keyed by BCP-47 tag:
//!
//! ```json
//! { "text": { "en-US": "Profiles", "zh-CN": "节点" } }
//! ```
//!
//! The host resolves the declaration against the effective display language
//! whenever it renders, so the native chrome follows a language switch the
//! way page content does. Without this, an lxapp with no Logic had no way to
//! localize its tab bar or titles: the runtime update APIs are Logic-only.

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A plain string, or the same text per language in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalizedText {
    Plain(String),
    PerLanguage(Vec<(String, String)>),
}

impl Default for LocalizedText {
    fn default() -> Self {
        Self::Plain(String::new())
    }
}

impl From<&str> for LocalizedText {
    fn from(value: &str) -> Self {
        Self::Plain(value.to_string())
    }
}

impl From<String> for LocalizedText {
    fn from(value: String) -> Self {
        Self::Plain(value)
    }
}

impl LocalizedText {
    /// The text to show for `language` (a BCP-47 tag).
    ///
    /// Exact tag first; otherwise the declared tag sharing the most leading
    /// subtags with it, as long as the language subtag matches (`zh-CN` reads
    /// `zh`, `zh-Hans-CN` reads `zh-Hans`); otherwise the first declared
    /// entry. Ties keep declaration order. Tags compare case-insensitively and
    /// treat `_` as `-`.
    pub fn resolve(&self, language: &str) -> &str {
        let entries = match self {
            Self::Plain(text) => return text,
            Self::PerLanguage(entries) => entries,
        };
        let wanted = subtags(language);
        let mut best: Option<(usize, &str)> = None;
        for (tag, text) in entries {
            let shared = shared_prefix(&wanted, &subtags(tag));
            if shared == 0 {
                continue;
            }
            if shared == wanted.len() && shared == subtags(tag).len() {
                return text;
            }
            if best.is_none_or(|(score, _)| shared > score) {
                best = Some((shared, text));
            }
        }
        best.map(|(_, text)| text)
            .or_else(|| entries.first().map(|(_, text)| text.as_str()))
            .unwrap_or("")
    }

    /// Whether the declaration carries no text in any language.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Plain(text) => text.is_empty(),
            Self::PerLanguage(entries) => entries.iter().all(|(_, text)| text.is_empty()),
        }
    }
}

fn subtags(tag: &str) -> Vec<String> {
    tag.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn shared_prefix(a: &[String], b: &[String]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

impl Serialize for LocalizedText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Plain(text) => serializer.serialize_str(text),
            Self::PerLanguage(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (tag, text) in entries {
                    map.serialize_entry(tag, text)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for LocalizedText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct LocalizedTextVisitor;

        impl<'de> Visitor<'de> for LocalizedTextVisitor {
            type Value = LocalizedText;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string, or an object mapping language tags to strings")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(LocalizedText::Plain(value.to_string()))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(LocalizedText::Plain(value))
            }

            // A map keeps its source order, so "first entry" is the one the
            // author wrote first, not an alphabetical accident.
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries: Vec<(String, String)> = Vec::new();
                while let Some((tag, text)) = map.next_entry::<String, String>()? {
                    let tag = tag.trim().to_string();
                    if tag.is_empty() {
                        return Err(de::Error::custom("language tag must not be empty"));
                    }
                    if entries
                        .iter()
                        .any(|(seen, _)| seen.eq_ignore_ascii_case(&tag))
                    {
                        return Err(de::Error::custom(format!("duplicate language tag '{tag}'")));
                    }
                    entries.push((tag, text));
                }
                if entries.is_empty() {
                    return Err(de::Error::custom("expected at least one language"));
                }
                Ok(LocalizedText::PerLanguage(entries))
            }
        }

        deserializer.deserialize_any(LocalizedTextVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(value: serde_json::Value) -> LocalizedText {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn a_plain_string_is_every_language() {
        let text = map(serde_json::json!("Home"));
        assert_eq!(text.resolve("zh-CN"), "Home");
        assert_eq!(
            serde_json::to_value(&text).unwrap(),
            serde_json::json!("Home")
        );
    }

    #[test]
    fn resolves_exact_then_closest_then_first() {
        let text = map(serde_json::json!({
            "en-US": "Profiles",
            "zh-Hans": "节点",
            "zh-Hant-TW": "節點"
        }));
        assert_eq!(text.resolve("en-US"), "Profiles");
        assert_eq!(text.resolve("EN_us"), "Profiles");
        // Same language, closest region/script.
        assert_eq!(text.resolve("zh-Hans-CN"), "节点");
        assert_eq!(text.resolve("zh-Hant-TW"), "節點");
        assert_eq!(text.resolve("en-GB"), "Profiles");
        // No shared language: the first declared entry.
        assert_eq!(text.resolve("fr-FR"), "Profiles");
    }

    #[test]
    fn keeps_declaration_order_when_serialized() {
        let text = map(serde_json::json!({ "zh-CN": "节点", "en-US": "Profiles" }));
        assert_eq!(text.resolve("de"), "节点");
        assert_eq!(
            serde_json::to_string(&text).unwrap(),
            r#"{"zh-CN":"节点","en-US":"Profiles"}"#
        );
    }

    #[test]
    fn rejects_empty_and_duplicate_maps() {
        assert!(serde_json::from_value::<LocalizedText>(serde_json::json!({})).is_err());
        assert!(serde_json::from_str::<LocalizedText>(r#"{"en":"a","EN":"b"}"#).is_err());
        assert!(serde_json::from_value::<LocalizedText>(serde_json::json!({ "en": 1 })).is_err());
        assert!(serde_json::from_value::<LocalizedText>(serde_json::json!(3)).is_err());
    }
}
