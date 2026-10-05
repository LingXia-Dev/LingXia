//! Manifest text that may be declared once per display language.
//!
//! `tabBar.items[].text` and a page's `navigationBar.title` accept either a
//! plain string — as before — or a map keyed by BCP-47 tag:
//!
//! ```json
//! { "text": { "en-US": "Profiles", "zh-CN": "节点" } }
//! ```
//!
//! The host resolves it against the effective display language on render,
//! so native chrome follows a language switch without Logic.

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
    /// The exact tag wins, then same-language entries ranked by script,
    /// then region (a region implies a script: `zh-TW` serves `zh-Hant-TW`),
    /// then the first declared entry. Case-insensitive; `_` reads as `-`.
    pub fn resolve(&self, language: &str) -> &str {
        let entries = match self {
            Self::Plain(text) => return text,
            Self::PerLanguage(entries) => entries,
        };
        let wanted = Tag::parse(language);
        let mut best: Option<(i32, &str)> = None;
        for (tag, text) in entries {
            let candidate = Tag::parse(tag);
            if candidate == wanted {
                return text;
            }
            let Some(score) = wanted.affinity(&candidate) else {
                continue;
            };
            if best.is_none_or(|(top, _)| score > top) {
                best = Some((score, text));
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

/// The parts of a tag that decide which text a reader can read.
#[derive(Debug, PartialEq, Eq)]
struct Tag {
    language: String,
    script: Option<String>,
    region: Option<String>,
}

impl Tag {
    fn parse(tag: &str) -> Self {
        let mut parts = tag
            .split(['-', '_'])
            .filter(|part| !part.is_empty())
            .map(str::to_ascii_lowercase);
        let language = parts.next().unwrap_or_default();
        let mut script = None;
        let mut region = None;
        for part in parts {
            let alpha = part.chars().all(|c| c.is_ascii_alphabetic());
            if script.is_none() && region.is_none() && part.len() == 4 && alpha {
                script = Some(part);
            } else if region.is_none()
                && ((part.len() == 2 && alpha)
                    || (part.len() == 3 && part.chars().all(|c| c.is_ascii_digit())))
            {
                region = Some(part);
            }
        }
        Self {
            language,
            script,
            region,
        }
    }

    /// The written script: declared, or implied by the region.
    fn effective_script(&self) -> Option<&str> {
        if let Some(script) = &self.script {
            return Some(script);
        }
        match (self.language.as_str(), self.region.as_deref()?) {
            ("zh", "tw" | "hk" | "mo") => Some("hant"),
            ("zh", "cn" | "sg" | "my") => Some("hans"),
            _ => None,
        }
    }

    /// How well `candidate` serves a reader of `self`; `None` for another
    /// language. A script both sides name decides first: the same one is
    /// best, the other one is worse than none at all.
    fn affinity(&self, candidate: &Tag) -> Option<i32> {
        if self.language != candidate.language {
            return None;
        }
        let mut score = 1;
        match (self.effective_script(), candidate.effective_script()) {
            (Some(a), Some(b)) if a == b => score += 4,
            (Some(_), Some(_)) => score -= 4,
            _ => {}
        }
        if self.region.is_some() && self.region == candidate.region {
            score += 2;
        }
        Some(score)
    }
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

            // Entries are collected in the order the deserializer yields them.
            // From a `serde_json::Value` that is source order because this
            // crate enables `preserve_order`; without it a JSON object is a
            // `BTreeMap` and "first entry" would be alphabetical.
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
        assert_eq!(text.resolve("zh-Hans-CN"), "节点");
        assert_eq!(text.resolve("zh-Hant-TW"), "節點");
        assert_eq!(text.resolve("en-GB"), "Profiles");
        // No entry in the language: the first declared one.
        assert_eq!(text.resolve("fr-FR"), "Profiles");
    }

    #[test]
    fn a_region_implies_the_chinese_script() {
        // Apple reports the script; authors usually write the region.
        let simplified_first = map(serde_json::json!({ "zh-CN": "节点", "zh-TW": "節點" }));
        assert_eq!(simplified_first.resolve("zh-Hant-TW"), "節點");
        assert_eq!(simplified_first.resolve("zh-Hant-HK"), "節點");
        assert_eq!(simplified_first.resolve("zh-Hans-CN"), "节点");

        let traditional_first = map(serde_json::json!({ "zh-TW": "節點", "zh-CN": "节点" }));
        assert_eq!(traditional_first.resolve("zh-Hans-CN"), "节点");
        assert_eq!(traditional_first.resolve("zh-SG"), "节点");

        // The other script ranks below a bare language entry.
        let generic = map(serde_json::json!({ "zh-TW": "節點", "zh": "节点" }));
        assert_eq!(generic.resolve("zh-Hans-CN"), "节点");
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
