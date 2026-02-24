use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::OnceLock;

const DEFAULT_LOCALE: &str = "en";
const SUPPORTED_LOCALES: &[&str] = &["en", "zh-Hans"];

static CATALOG: OnceLock<std::result::Result<HashMap<String, HashMap<String, String>>, String>> =
    OnceLock::new();

pub fn default_locale() -> &'static str {
    DEFAULT_LOCALE
}

pub fn supported_locales() -> &'static [&'static str] {
    SUPPORTED_LOCALES
}

pub fn permission_text(locale: &str, key: &str) -> Result<String> {
    let catalog = catalog()?;
    let locale_entries = catalog
        .get(locale)
        .ok_or_else(|| anyhow!("Unsupported permission locale `{locale}`"))?;
    let value = locale_entries
        .get(key)
        .ok_or_else(|| anyhow!("Missing permission text for locale `{locale}` and key `{key}`"))?;
    if value.trim().is_empty() {
        return Err(anyhow!(
            "Permission text is empty for locale `{locale}` and key `{key}`"
        ));
    }
    Ok(value.clone())
}

fn catalog() -> Result<&'static HashMap<String, HashMap<String, String>>> {
    match CATALOG.get_or_init(build_catalog) {
        Ok(catalog) => Ok(catalog),
        Err(message) => Err(anyhow!("{message}")),
    }
}

fn build_catalog() -> std::result::Result<HashMap<String, HashMap<String, String>>, String> {
    let mut out = HashMap::new();
    let en = parse_locale_file("en", include_str!("../i18n/permissions/en.json"))?;
    let zh_hans = parse_locale_file("zh-Hans", include_str!("../i18n/permissions/zh-Hans.json"))?;

    out.insert("en".to_string(), en);
    out.insert("zh-Hans".to_string(), zh_hans);
    Ok(out)
}

fn parse_locale_file(
    locale: &str,
    content: &str,
) -> std::result::Result<HashMap<String, String>, String> {
    let parsed: HashMap<String, String> = serde_json::from_str(content)
        .map_err(|err| format!("Invalid `{locale}` permissions JSON: {err}"))?;
    if parsed.is_empty() {
        return Err(format!(
            "Permissions catalog for locale `{locale}` is empty"
        ));
    }
    for (key, value) in &parsed {
        if key.trim().is_empty() {
            return Err(format!(
                "Permissions catalog for locale `{locale}` contains empty key"
            ));
        }
        if value.trim().is_empty() {
            return Err(format!(
                "Permissions catalog for locale `{locale}` contains empty text for key `{key}`"
            ));
        }
    }
    Ok(parsed)
}
