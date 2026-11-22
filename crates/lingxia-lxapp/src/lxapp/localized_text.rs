//! Manifest text: `tabBar.items[].text` and a page's `navigationBar.title`.
//!
//! A plain string, or a key in the lxapp's `i18n/` catalogues:
//!
//! ```json
//! { "text": { "message": "tab.profiles" } }
//! ```
//!
//! The lxapp binds a key to its text in every language it declares when it loads the manifest,
//! so native chrome follows a language switch without Logic, in the language its pages use.

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

const MESSAGE_KEY: &str = "message";

/// A plain string, or a catalogue key, before or after the lxapp binds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalizedText {
    Plain(String),
    Message(String),
    /// Each declared language's text, the source first.
    Bound {
        key: String,
        entries: Vec<(String, String)>,
    },
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
    /// The text to show for `language` (a BCP-47 tag), narrowed to the declared languages as a
    /// page narrows it: the exact tag, then the closest of the same language, then the source.
    /// An unbound key shows as itself.
    pub fn resolve(&self, language: &str) -> &str {
        let entries = match self {
            Self::Plain(text) | Self::Message(text) => return text,
            Self::Bound { entries, .. } => entries,
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
            .unwrap_or_default()
    }

    /// Binds a key to `text(tag)` for each of `languages`, the first being the source.
    pub fn bind(&mut self, languages: &[String], text: impl Fn(&str) -> Option<String>) {
        let Self::Message(key) = self else {
            return;
        };
        let Some(source) = languages.first() else {
            return;
        };
        if text(source).is_none() {
            return;
        }
        let entries = languages
            .iter()
            .filter_map(|tag| text(tag).map(|text| (tag.clone(), text)))
            .collect();
        *self = Self::Bound {
            key: std::mem::take(key),
            entries,
        };
    }

    /// Whether the declaration carries no text.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Plain(text) | Self::Message(text) => text.is_empty(),
            Self::Bound { key, .. } => key.is_empty(),
        }
    }
}

/// The entry of `languages` a reader of `language` reads best: the exact tag,
/// then the closest of the same language, then the first. `None` for an empty list.
pub fn narrow<'a>(language: &str, languages: &'a [String]) -> Option<&'a str> {
    let wanted = Tag::parse(language);
    let mut best: Option<(i32, &str)> = None;
    for tag in languages {
        let candidate = Tag::parse(tag);
        if candidate == wanted {
            return Some(tag);
        }
        if let Some(score) = wanted.affinity(&candidate)
            && best.is_none_or(|(top, _)| score > top)
        {
            best = Some((score, tag));
        }
    }
    best.map(|(_, tag)| tag)
        .or_else(|| languages.first().map(String::as_str))
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
            Self::Message(key) | Self::Bound { key, .. } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry(MESSAGE_KEY, key)?;
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
                formatter.write_str("a string, or `{ \"message\": \"<catalogue key>\" }`")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(LocalizedText::Plain(value.to_string()))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(LocalizedText::Plain(value))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut key: Option<String> = None;
                while let Some((field, value)) = map.next_entry::<String, String>()? {
                    if field != MESSAGE_KEY || key.is_some() {
                        return Err(de::Error::custom(format!(
                            "unexpected `{field}`: name a catalogue key as {{ \"message\": \"tab.home\" }}"
                        )));
                    }
                    key = Some(value);
                }
                key.filter(|key| lingxia_i18n::valid_key(key))
                    .map(LocalizedText::Message)
                    .ok_or_else(|| {
                        de::Error::custom("`message` must be a catalogue key like \"tab.home\"")
                    })
            }
        }

        deserializer.deserialize_any(LocalizedTextVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrows_the_display_language_to_the_languages_an_app_is_written_in() {
        let languages: Vec<String> = ["en", "zh-Hans", "zh-Hant"].map(String::from).into();
        assert_eq!(narrow("EN_us", &languages), Some("en"));
        assert_eq!(narrow("zh-CN", &languages), Some("zh-Hans"));
        assert_eq!(narrow("zh-TW", &languages), Some("zh-Hant"));
        assert_eq!(
            narrow("ja-JP", &languages),
            Some("en"),
            "the first is the fallback"
        );
        assert_eq!(narrow("ja-JP", &[]), None);
    }

    #[test]
    fn a_bound_key_reads_as_a_page_in_that_language_would() {
        let mut text: LocalizedText =
            serde_json::from_value(serde_json::json!({ "message": "tab.nodes" })).unwrap();
        let languages: Vec<String> = ["en-US", "zh-CN", "zh-TW"].map(String::from).into();
        text.bind(&languages, |tag| {
            Some(
                match tag {
                    "en-US" => "Nodes",
                    "zh-CN" => "节点",
                    _ => "節點",
                }
                .to_string(),
            )
        });
        assert_eq!(
            text.resolve("zh-Hant-HK"),
            "節點",
            "a region implies the script"
        );
        assert_eq!(text.resolve("zh-Hans-SG"), "节点");
        assert_eq!(text.resolve("ja-JP"), "Nodes", "else the source");

        let mut text: LocalizedText =
            serde_json::from_value(serde_json::json!({ "message": "tab.nodes" })).unwrap();
        let languages: Vec<String> = ["en-US", "zh-CN", "en-XA"].map(String::from).into();
        text.bind(&languages, |tag| Some(format!("{tag} text")));
        assert_eq!(
            text.resolve("en-US"),
            "en-US text",
            "the source matches exactly, ahead of another English"
        );
        assert_eq!(
            serde_json::to_value(&text).unwrap(),
            serde_json::json!({ "message": "tab.nodes" })
        );
    }

    #[test]
    fn a_language_map_is_no_longer_manifest_text() {
        let reject = |json: &str| assert!(serde_json::from_str::<LocalizedText>(json).is_err());
        reject(r#"{"default":"Profiles","zh-CN":"节点"}"#);
        reject(r#"{"message":"Not a key"}"#);
        reject("3");
    }
}
