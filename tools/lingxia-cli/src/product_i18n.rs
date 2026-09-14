//! Map `app.productName` locales onto each platform's resource directory layout.

use crate::config::ProductName;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

/// Android `res/` qualifier directory for a product-name locale.
///
/// `default` is `values`. `en-US` stays `values-en-rUS` so it does not
/// overwrite the fallback; `zh-CN` is `values-zh-rCN`.
pub fn android_values_dir(tag: &str) -> String {
    if tag == "default" {
        return "values".to_string();
    }
    let parts: Vec<&str> = tag.split('-').collect();
    match parts.as_slice() {
        [lang] => format!("values-{lang}"),
        [lang, region] if is_region(region) => {
            format!("values-{lang}-r{}", region.to_ascii_uppercase())
        }
        [lang, script] if is_script(script) => format!("values-b+{lang}+{script}"),
        [lang, script, region] if is_script(script) && is_region(region) => {
            format!("values-b+{lang}+{script}+{}", region.to_ascii_uppercase())
        }
        _ => format!("values-b+{}", tag.replace('-', "+")),
    }
}

/// Apple `.lproj` directory for a product-name locale. `default` is written
/// to Info.plist, not a strings file.
pub fn apple_lproj_dir(tag: &str) -> Option<String> {
    match tag {
        "default" => None,
        "en" | "en-US" => Some("en.lproj".to_string()),
        "zh-CN" | "zh-Hans" | "zh-Hans-CN" => Some("zh-Hans.lproj".to_string()),
        "zh-TW" | "zh-Hant" | "zh-Hant-TW" => Some("zh-Hant.lproj".to_string()),
        other => Some(format!("{other}.lproj")),
    }
}

/// Harmony resource-scope directory. `default` is `base`; `zh-CN` is `zh_CN`.
pub fn harmony_locale_dir(tag: &str) -> String {
    if tag == "default" {
        return "base".to_string();
    }
    tag.replace('-', "_")
}

pub fn android_strings_xml(name: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
<resources>\n\
    <string name=\"lx_app_name\">{}</string>\n\
</resources>\n",
        escape_android_string(name)
    )
}

pub fn merge_info_plist_strings(existing: &str, name: &str) -> String {
    let mut content = existing.to_string();
    for key in ["CFBundleDisplayName", "CFBundleName"] {
        let assignment = format!("\"{key}\" = \"{}\";\n", escape_strings_value(name));
        if let Some(updated) = replace_strings_assignment(&content, key, &assignment) {
            content = updated;
        } else {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&assignment);
        }
    }
    content
}

fn is_region(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_script(value: &str) -> bool {
    value.len() == 4 && value.bytes().all(|b| b.is_ascii_alphabetic())
}

fn escape_android_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_strings_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn write_android_overlay(res_dir: &Path, product_name: &ProductName) -> Result<()> {
    for (tag, name) in product_name.locale_entries() {
        let dir = res_dir.join(android_values_dir(tag));
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
        fs::write(dir.join("strings.xml"), android_strings_xml(name)).with_context(|| {
            format!("Failed to write product name strings in {}", dir.display())
        })?;
    }
    Ok(())
}

pub fn write_apple_product_name_strings(
    resources_dir: &Path,
    product_name: &ProductName,
) -> Result<()> {
    for (tag, name) in product_name.locale_entries() {
        let Some(lproj) = apple_lproj_dir(tag) else {
            continue;
        };
        let dir = resources_dir.join(lproj);
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
        let path = dir.join("InfoPlist.strings");
        let existing = if path.exists() {
            fs::read_to_string(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?
        } else {
            String::new()
        };
        fs::write(&path, merge_info_plist_strings(&existing, name))
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}

pub fn write_harmony_product_name_strings(
    staging: &Path,
    product_name: &ProductName,
) -> Result<()> {
    for (tag, name) in product_name.locale_entries() {
        let locale = harmony_locale_dir(tag);
        upsert_harmony_string(
            &staging.join(format!("AppScope/resources/{locale}/element/string.json")),
            "app_name",
            name,
        )?;
        upsert_harmony_string(
            &staging.join(format!(
                "entry/src/main/resources/{locale}/element/string.json"
            )),
            "EntryAbility_label",
            name,
        )?;
    }
    Ok(())
}

fn upsert_harmony_string(path: &Path, name: &str, value: &str) -> Result<()> {
    let mut root = if path.exists() {
        serde_json::from_str(
            &fs::read_to_string(path)
                .with_context(|| format!("Failed to read {}", path.display()))?,
        )
        .with_context(|| format!("Failed to parse {}", path.display()))?
    } else {
        json!({ "string": [] })
    };
    let strings = root
        .get_mut("string")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow::anyhow!("{} is missing a string array", path.display()))?;
    if let Some(item) = strings.iter_mut().find(|item| {
        item.get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n == name)
    }) {
        item["value"] = Value::String(value.to_string());
    } else {
        strings.push(json!({ "name": name, "value": value }));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(&root)?))
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn replace_strings_assignment(content: &str, key: &str, assignment: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = content.find(&needle)?;
    let rest = &content[start..];
    let end = rest.find(';')? + start + 1;
    let mut updated = String::with_capacity(content.len() + assignment.len());
    updated.push_str(&content[..start]);
    updated.push_str(assignment.trim_end());
    if content[end..].starts_with('\n') {
        updated.push_str(&content[end..]);
    } else {
        updated.push('\n');
        updated.push_str(&content[end..]);
    }
    Some(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProductName;

    #[test]
    fn android_dirs_keep_default_separate_from_en_us() {
        assert_eq!(android_values_dir("default"), "values");
        assert_eq!(android_values_dir("en-US"), "values-en-rUS");
        assert_eq!(android_values_dir("zh-CN"), "values-zh-rCN");
        assert_eq!(android_values_dir("ja"), "values-ja");
        assert_eq!(android_values_dir("zh-Hans"), "values-b+zh+Hans");
    }

    #[test]
    fn apple_dirs_map_common_cjk_and_english() {
        assert_eq!(apple_lproj_dir("default"), None);
        assert_eq!(apple_lproj_dir("en-US").as_deref(), Some("en.lproj"));
        assert_eq!(apple_lproj_dir("zh-CN").as_deref(), Some("zh-Hans.lproj"));
        assert_eq!(apple_lproj_dir("ja").as_deref(), Some("ja.lproj"));
    }

    #[test]
    fn harmony_dirs_use_underscore() {
        assert_eq!(harmony_locale_dir("default"), "base");
        assert_eq!(harmony_locale_dir("zh-CN"), "zh_CN");
        assert_eq!(harmony_locale_dir("en-US"), "en_US");
    }

    #[test]
    fn merge_overwrites_existing_bundle_display_name() {
        let existing =
            "\"NSCameraUsageDescription\" = \"Camera\";\n\"CFBundleDisplayName\" = \"Old\";\n";
        let merged = merge_info_plist_strings(existing, "我的应用");
        assert!(merged.contains("\"CFBundleDisplayName\" = \"我的应用\";"));
        assert!(merged.contains("\"CFBundleName\" = \"我的应用\";"));
        assert!(merged.contains("\"NSCameraUsageDescription\" = \"Camera\";"));
        assert!(!merged.contains("\"Old\""));
    }

    #[test]
    fn locale_entries_include_default_then_translations() {
        let name = ProductName::with_translations(
            "My App",
            std::collections::BTreeMap::from([("zh-CN".to_string(), "我的应用".to_string())]),
        );
        let entries: Vec<_> = name.locale_entries().collect();
        assert_eq!(entries[0], ("default", "My App"));
        assert!(entries.contains(&("zh-CN", "我的应用")));
    }
}
