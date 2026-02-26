use anyhow::{Result, anyhow};
use std::collections::{BTreeSet, HashMap};
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
    let source_locale = normalize_permission_locale(locale)?;
    let catalog = catalog()?;
    let locale_entries = catalog
        .get(source_locale)
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
    let en = parse_locale_file(
        "en-US",
        include_str!("../../../i18n/permission/cli/en-US.yaml"),
    )?;
    let zh_cn = parse_locale_file(
        "zh-CN",
        include_str!("../../../i18n/permission/cli/zh-CN.yaml"),
    )?;
    validate_permission_key_consistency("en-US", &en, "zh-CN", &zh_cn)?;

    out.insert("en-US".to_string(), en);
    out.insert("zh-CN".to_string(), zh_cn);
    Ok(out)
}

fn parse_locale_file(
    locale: &str,
    content: &str,
) -> std::result::Result<HashMap<String, String>, String> {
    let parsed: HashMap<String, String> = serde_yaml_ng::from_str(content)
        .map_err(|err| format!("Invalid `{locale}` permissions YAML: {err}"))?;
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

fn validate_permission_key_consistency(
    left_locale: &str,
    left: &HashMap<String, String>,
    right_locale: &str,
    right: &HashMap<String, String>,
) -> std::result::Result<(), String> {
    let left_keys = left.keys().cloned().collect::<BTreeSet<_>>();
    let right_keys = right.keys().cloned().collect::<BTreeSet<_>>();

    let missing_in_right = left_keys
        .difference(&right_keys)
        .cloned()
        .collect::<Vec<_>>();
    let missing_in_left = right_keys
        .difference(&left_keys)
        .cloned()
        .collect::<Vec<_>>();

    if !missing_in_right.is_empty() || !missing_in_left.is_empty() {
        let mut details = Vec::new();
        if !missing_in_right.is_empty() {
            details.push(format!(
                "`{right_locale}` missing keys: {}",
                missing_in_right.join(", ")
            ));
        }
        if !missing_in_left.is_empty() {
            details.push(format!(
                "`{left_locale}` missing keys: {}",
                missing_in_left.join(", ")
            ));
        }
        return Err(format!(
            "Permission i18n locale key mismatch detected: {}",
            details.join("; ")
        ));
    }

    Ok(())
}

fn normalize_permission_locale(locale: &str) -> Result<&'static str> {
    let normalized = locale.trim();
    if normalized.eq_ignore_ascii_case("en") || normalized.eq_ignore_ascii_case("en-us") {
        return Ok("en-US");
    }
    if normalized.eq_ignore_ascii_case("zh")
        || normalized.eq_ignore_ascii_case("zh-cn")
        || normalized.eq_ignore_ascii_case("zh-hans")
    {
        return Ok("zh-CN");
    }

    Err(anyhow!("Unsupported permission locale `{locale}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_supported_locales() {
        assert_eq!(
            normalize_permission_locale("en").expect("normalize en"),
            "en-US"
        );
        assert_eq!(
            normalize_permission_locale("EN-us").expect("normalize en-us"),
            "en-US"
        );
        assert_eq!(
            normalize_permission_locale("zh").expect("normalize zh"),
            "zh-CN"
        );
        assert_eq!(
            normalize_permission_locale("zh-Hans").expect("normalize zh-Hans"),
            "zh-CN"
        );
    }

    #[test]
    fn rejects_unsupported_locale() {
        let error = normalize_permission_locale("fr-FR").expect_err("expected unsupported locale");
        assert!(error.to_string().contains("Unsupported permission locale"));
    }

    #[test]
    fn detects_permission_key_mismatch() {
        let left = HashMap::from([
            (
                "apple.info_plist.NSCameraUsageDescription".to_string(),
                "A".to_string(),
            ),
            (
                "apple.info_plist.NSMicrophoneUsageDescription".to_string(),
                "B".to_string(),
            ),
        ]);
        let right = HashMap::from([(
            "apple.info_plist.NSCameraUsageDescription".to_string(),
            "A".to_string(),
        )]);

        let error = validate_permission_key_consistency("en-US", &left, "zh-CN", &right)
            .expect_err("expected key mismatch");
        assert!(error.contains("zh-CN"));
        assert!(error.contains("NSMicrophoneUsageDescription"));
    }
}
