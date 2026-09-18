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

/// Which characters an id may carry. The platforms do not agree, so one rule
/// for all of them is wrong in one direction or the other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IdStyle {
    /// A Java package name. Android's package id is one, so `app.packageId` —
    /// which every platform falls back to — has to be one too. `-` is not a
    /// Java identifier character, and a dashed id there yields source
    /// directories and `package` declarations that do not compile.
    JavaPackage,
    /// An Apple bundle id. `CFBundleIdentifier` also allows `-`, and a listing
    /// that already owns a dashed id cannot be renamed.
    AppleBundle,
}

impl IdStyle {
    fn allows(self, ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_' || (self == IdStyle::AppleBundle && ch == '-')
    }

    fn charset_error(self, field: &str) -> anyhow::Error {
        match self {
            IdStyle::JavaPackage => anyhow!(
                "{field} can only contain alphanumeric characters, underscores, and dots. \
                 An Android package id is a Java package name, so `-` cannot appear in the \
                 id every platform shares; write it with `_`. An Apple listing that owns a \
                 dashed id keeps it on `ios.bundleId` / `macos.bundleId` instead."
            ),
            IdStyle::AppleBundle => anyhow!(
                "{field} can only contain alphanumeric characters, hyphens, underscores, and dots"
            ),
        }
    }
}

/// The style a platform's own override is read in. Everything that is not
/// Apple is a Java package name (Harmony's `bundleName` and the Windows app id
/// follow the same shape).
pub fn platform_id_style(platform: &str) -> IdStyle {
    match platform {
        "ios" | "macos" => IdStyle::AppleBundle,
        _ => IdStyle::JavaPackage,
    }
}

pub fn validate_package_id(package_id: &str) -> Result<()> {
    validate_package_id_field("Package ID", package_id)
}

/// The shared id, and any non-Apple override: a Java package name.
pub fn validate_package_id_field(field: &str, package_id: &str) -> Result<()> {
    validate_package_id_styled(field, package_id, IdStyle::JavaPackage)
}

pub fn validate_package_id_styled(field: &str, package_id: &str, style: IdStyle) -> Result<()> {
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
        if !part.chars().all(|c| style.allows(c)) {
            return Err(style.charset_error(field));
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
    fn the_shared_id_stays_a_java_package_name() {
        let err = validate_package_id_field("app.packageId", "com.heights-t.h-app")
            .unwrap_err()
            .to_string();
        assert!(err.contains("Java package name"), "{err}");
        assert!(validate_package_id_field("app.packageId", "com.heights_t.h_app").is_ok());
    }

    #[test]
    fn an_apple_override_keeps_its_dashes() {
        assert!(
            validate_package_id_styled("ios.bundleId", "com.heights-t.h-app", IdStyle::AppleBundle)
                .is_ok()
        );
        // Everything else is still a Java package name.
        assert_eq!(platform_id_style("ios"), IdStyle::AppleBundle);
        assert_eq!(platform_id_style("macos"), IdStyle::AppleBundle);
        assert_eq!(platform_id_style("android"), IdStyle::JavaPackage);
        assert_eq!(platform_id_style("harmony"), IdStyle::JavaPackage);
        assert!(
            validate_package_id_styled("android.packageId", "com.a-b.c", IdStyle::JavaPackage)
                .is_err()
        );
    }

    #[test]
    fn a_dot_is_never_part_of_a_segment() {
        for style in [IdStyle::JavaPackage, IdStyle::AppleBundle] {
            assert!(validate_package_id_styled("id", "com..app", style).is_err());
            assert!(validate_package_id_styled("id", "single", style).is_err());
        }
    }

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
