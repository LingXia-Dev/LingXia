//! Host package / bundle id and product display-name helpers.

use anyhow::{Result, anyhow};
use std::collections::BTreeMap;

/// Default launcher name plus optional BCP-47 translations from `productNames:`.
#[derive(Debug, Clone, Copy)]
pub struct ProductName<'a> {
    pub default: &'a str,
    pub translations: &'a BTreeMap<String, String>,
}

impl<'a> ProductName<'a> {
    pub fn locale_entries(self) -> impl Iterator<Item = (&'a str, &'a str)> {
        std::iter::once(("default", self.default)).chain(
            self.translations
                .iter()
                .map(|(tag, name)| (tag.as_str(), name.as_str())),
        )
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

pub fn validate_product_name(name: &str) -> Result<()> {
    validate_display_name("app.productName", name)
}

pub fn validate_product_names(names: &BTreeMap<String, String>) -> Result<()> {
    for (locale, value) in names {
        if locale == "default" {
            return Err(anyhow!(
                "app.productNames must not include `default`; that name is `app.productName`"
            ));
        }
        validate_product_name_locale(locale)?;
        validate_display_name(&format!("app.productNames.{locale}"), value)?;
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

fn validate_display_name(field: &str, value: &str) -> Result<()> {
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
            "app.productNames locale 'auto' is reserved; use a BCP-47 tag such as zh-CN"
        ));
    }
    let parsed = language_tags::LanguageTag::parse(tag).map_err(|error| {
        anyhow!("app.productNames locale '{tag}' is not a valid BCP-47 tag: {error}")
    })?;
    parsed.validate().map_err(|error| {
        anyhow!("app.productNames locale '{tag}' is not a valid BCP-47 tag: {error}")
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_names_reject_default_key() {
        let mut names = BTreeMap::new();
        names.insert("default".to_string(), "Nope".to_string());
        let err = validate_product_names(&names).unwrap_err().to_string();
        assert!(err.contains("productName"), "{err}");
    }

    #[test]
    fn product_names_reject_invalid_locale() {
        let mut names = BTreeMap::new();
        names.insert("zh_CN".to_string(), "我的应用".to_string());
        let err = validate_product_names(&names).unwrap_err().to_string();
        assert!(err.contains("BCP-47"), "{err}");
    }
}
