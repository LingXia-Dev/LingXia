//! Host package id and product display name.

use anyhow::{Result, anyhow};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// User-facing product display name.
///
/// YAML may be a bare string or a map with `default` plus BCP-47 keys
/// (`zh-CN`, `ja`, …). The map form is stored as a default plus translations;
/// `default` never appears in [`Self::translations`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductName {
    default: String,
    translations: BTreeMap<String, String>,
}

impl ProductName {
    pub fn new(default: impl Into<String>) -> Self {
        Self {
            default: default.into(),
            translations: BTreeMap::new(),
        }
    }

    pub fn with_translations(
        default: impl Into<String>,
        translations: BTreeMap<String, String>,
    ) -> Self {
        Self {
            default: default.into(),
            translations,
        }
    }

    pub fn default_name(&self) -> &str {
        &self.default
    }

    pub fn translations(&self) -> &BTreeMap<String, String> {
        &self.translations
    }

    pub fn locale_entries(&self) -> impl Iterator<Item = (&str, &str)> {
        std::iter::once(("default", self.default.as_str())).chain(
            self.translations
                .iter()
                .map(|(tag, name)| (tag.as_str(), name.as_str())),
        )
    }

    pub fn validate(&self) -> Result<()> {
        validate_product_name_value("app.productName", &self.default)?;
        for (locale, value) in &self.translations {
            validate_product_name_locale(locale)?;
            validate_product_name_value(&format!("app.productName.{locale}"), value)?;
        }
        Ok(())
    }
}

impl Serialize for ProductName {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        if self.translations.is_empty() {
            return self.default.serialize(serializer);
        }
        let mut map = BTreeMap::new();
        map.insert("default", self.default.as_str());
        for (tag, name) in &self.translations {
            map.insert(tag, name.as_str());
        }
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProductName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Single(String),
            Map(BTreeMap<String, String>),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Single(default) => Ok(Self::new(default)),
            Raw::Map(mut map) => {
                let default = map.remove("default").ok_or_else(|| {
                    D::Error::custom("app.productName map must include a `default` name")
                })?;
                Ok(Self {
                    default,
                    translations: map,
                })
            }
        }
    }
}

pub fn validate_package_id(package_id: &str) -> Result<()> {
    validate_package_id_field("Package ID", package_id)
}

pub fn validate_package_id_field(field: &str, package_id: &str) -> Result<()> {
    let package_id = package_id.trim();
    if package_id.is_empty() {
        return Err(anyhow!("{field} must not be empty"));
    }
    let parts: Vec<&str> = package_id.split('.').collect();
    if parts.len() < 2 {
        return Err(anyhow!(
            "{field} must have at least 2 parts (e.g., com.example.app)"
        ));
    }
    for part in parts {
        if part.is_empty() {
            return Err(anyhow!(
                "{field} parts cannot be empty (no leading, trailing, or doubled dots)"
            ));
        }
        if !part.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(anyhow!(
                "{field} can only contain alphanumeric characters, underscores, and dots"
            ));
        }
    }
    Ok(())
}

pub fn missing_app_package_id_error() -> anyhow::Error {
    anyhow!(
        "app.packageId is required. Package ids no longer live on each \
         platform block (`android.packageId`, `ios.bundleId`, \
         `macos.bundleId`, `windows.appId`, `harmony.bundleName`). \
         Put the common reverse-DNS id on `app.packageId`; keep a \
         platform override only when that store listing is a different id."
    )
}

pub fn platform_package_id_field(platform: &str) -> &'static str {
    match platform {
        "android" => "android.packageId",
        "ios" => "ios.bundleId",
        "macos" => "macos.bundleId",
        "windows" => "windows.appId",
        "harmony" => "harmony.bundleName",
        _ => "app.packageId",
    }
}

pub fn non_empty_opt(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn validate_product_name_value(field: &str, value: &str) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("{field} must not be empty"));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(anyhow!("{field} must be a single line"));
    }
    Ok(())
}

fn validate_product_name_locale(tag: &str) -> Result<()> {
    if tag.eq_ignore_ascii_case("auto") {
        return Err(anyhow!(
            "app.productName locale 'auto' is reserved; use a BCP-47 tag such as zh-CN"
        ));
    }
    let parsed = language_tags::LanguageTag::parse(tag).map_err(|error| {
        anyhow!("app.productName locale '{tag}' is not a valid BCP-47 tag: {error}")
    })?;
    parsed.validate().map_err(|error| {
        anyhow!("app.productName locale '{tag}' is not a valid BCP-47 tag: {error}")
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_form_round_trips() {
        let name: ProductName = serde_yaml_ng::from_str("My App").unwrap();
        assert_eq!(name.default_name(), "My App");
        assert!(name.translations().is_empty());
        assert_eq!(serde_yaml_ng::to_string(&name).unwrap().trim(), "My App");
    }

    #[test]
    fn map_form_keeps_default_out_of_translations() {
        let name: ProductName = serde_yaml_ng::from_str(
            r#"
default: My App
zh-CN: 我的应用
"#,
        )
        .unwrap();
        assert_eq!(name.default_name(), "My App");
        assert_eq!(
            name.translations().get("zh-CN").map(String::as_str),
            Some("我的应用")
        );
        assert!(!name.translations().contains_key("default"));
        name.validate().unwrap();
    }

    #[test]
    fn map_form_requires_default() {
        let err = serde_yaml_ng::from_str::<ProductName>("zh-CN: 我的应用")
            .unwrap_err()
            .to_string();
        assert!(err.contains("default"), "{err}");
    }
}
