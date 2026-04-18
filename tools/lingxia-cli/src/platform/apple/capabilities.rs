use crate::i18n;
use anyhow::{Context, Result, anyhow};
use plist::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

pub struct AppleInfoPlistRequirement {
    pub key: &'static str,
    pub permission_key: &'static str,
}

const REQUIRED_APPLE_INFO_PLIST_KEYS: &[AppleInfoPlistRequirement] = &[
    AppleInfoPlistRequirement {
        key: "NSCameraUsageDescription",
        permission_key: "apple.info_plist.NSCameraUsageDescription",
    },
    AppleInfoPlistRequirement {
        key: "NSMicrophoneUsageDescription",
        permission_key: "apple.info_plist.NSMicrophoneUsageDescription",
    },
    AppleInfoPlistRequirement {
        key: "NSLocationWhenInUseUsageDescription",
        permission_key: "apple.info_plist.NSLocationWhenInUseUsageDescription",
    },
    AppleInfoPlistRequirement {
        key: "NSPhotoLibraryUsageDescription",
        permission_key: "apple.info_plist.NSPhotoLibraryUsageDescription",
    },
    AppleInfoPlistRequirement {
        key: "NSPhotoLibraryAddUsageDescription",
        permission_key: "apple.info_plist.NSPhotoLibraryAddUsageDescription",
    },
];

const RESTRICTED_APPLE_ENTITLEMENTS: &[&str] = &[
    "com.apple.developer.networking.wifi-info",
    "com.apple.developer.networking.HotspotConfiguration",
];
const ASSOCIATED_DOMAINS_ENTITLEMENT: &str = "com.apple.developer.associated-domains";

const IOS_INFO_PLIST_FILE: &str = "Info.plist";
const IOS_APP_ENTITLEMENTS_FILE: &str = "App.entitlements";
const MACOS_INFO_PLIST_FILE: &str = "Info.plist";
const MACOS_APP_ENTITLEMENTS_FILE: &str = "App.entitlements";
const INFO_PLIST_STRINGS_FILE: &str = "InfoPlist.strings";

pub fn controlled_apple_entitlements() -> impl Iterator<Item = &'static str> {
    RESTRICTED_APPLE_ENTITLEMENTS.iter().copied()
}

pub fn sync_ios_capability_files(
    ios_dir: &Path,
    granted_entitlements: &[String],
    app_link_hosts: &[String],
) -> Result<bool> {
    let info_plist_changed = sync_info_plist(&ios_dir.join(IOS_INFO_PLIST_FILE))?;
    let localization_changed = sync_info_plist_localizations(ios_dir)?;
    let entitlements_changed = sync_app_entitlements(
        &ios_dir.join(IOS_APP_ENTITLEMENTS_FILE),
        granted_entitlements,
        app_link_hosts,
    )?;
    Ok(info_plist_changed || localization_changed || entitlements_changed)
}

pub fn sync_macos_capability_files(
    macos_dir: &Path,
    granted_entitlements: &[String],
    app_link_hosts: &[String],
) -> Result<bool> {
    let info_plist_changed = sync_info_plist(&macos_dir.join(MACOS_INFO_PLIST_FILE))?;
    let localization_changed = sync_info_plist_localizations(macos_dir)?;
    let entitlements_changed = sync_app_entitlements(
        &macos_dir.join(MACOS_APP_ENTITLEMENTS_FILE),
        granted_entitlements,
        app_link_hosts,
    )?;
    Ok(info_plist_changed || localization_changed || entitlements_changed)
}

pub fn validate_built_app_info_plist(app_path: &Path) -> Result<()> {
    let info_path = app_path.join(IOS_INFO_PLIST_FILE);
    let dict = load_plist_dictionary(&info_path).with_context(|| {
        format!(
            "Failed to validate iOS capability Info.plist in {}",
            info_path.display()
        )
    })?;

    let mut missing = Vec::new();
    for requirement in required_info_plist_requirements() {
        let exists = dict
            .get(requirement.key)
            .and_then(Value::as_string)
            .is_some_and(|value| !value.trim().is_empty());
        if !exists {
            missing.push(requirement.key.to_string());
        }
    }

    if missing.is_empty() {
        return Ok(());
    }

    Err(anyhow!(
        "Missing SDK-required iOS Info.plist usage descriptions: {}.\n\
Run `lingxia build --platform ios` to regenerate iOS capability metadata.",
        missing.join(", ")
    ))
}

pub fn missing_restricted_apple_entitlements(granted_entitlements: &[String]) -> Vec<String> {
    let granted = granted_entitlements
        .iter()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .collect::<HashSet<_>>();

    controlled_apple_entitlements()
        .filter(|entitlement| !granted.contains(*entitlement))
        .map(|entry| entry.to_string())
        .collect()
}

fn sync_info_plist(info_plist_path: &Path) -> Result<bool> {
    let mut dict = if info_plist_path.exists() {
        load_plist_dictionary(info_plist_path)?
    } else {
        plist::Dictionary::new()
    };

    let mut changed = false;
    let desired = desired_info_plist_entries()?;
    let managed = managed_info_plist_keys();

    for key in managed {
        if desired.contains_key(&key) {
            continue;
        }
        if dict.remove(&key).is_some() {
            changed = true;
        }
    }

    for (key, value) in desired {
        let needs_update = match dict.get(&key) {
            Some(Value::String(existing)) => existing.trim().is_empty(),
            Some(_) => true,
            None => true,
        };
        if needs_update {
            dict.insert(key, Value::String(value));
            changed = true;
        }
    }

    if changed || !info_plist_path.exists() {
        plist::to_file_xml(info_plist_path, &dict)
            .with_context(|| format!("Failed to write {}", info_plist_path.display()))?;
    }

    Ok(changed)
}

fn sync_info_plist_localizations(project_dir: &Path) -> Result<bool> {
    let mut changed = false;
    for locale in i18n::supported_locales() {
        let mut locale_changed = false;
        let locale_dir = project_dir.join(format!("{locale}.lproj"));
        fs::create_dir_all(&locale_dir)
            .with_context(|| format!("Failed to create {}", locale_dir.display()))?;

        let strings_path = locale_dir.join(INFO_PLIST_STRINGS_FILE);
        let mut content = if strings_path.exists() {
            fs::read_to_string(&strings_path)
                .with_context(|| format!("Failed to read {}", strings_path.display()))?
        } else {
            String::new()
        };

        for requirement in required_info_plist_requirements() {
            if content.contains(&format!("\"{}\"", requirement.key)) {
                continue;
            }
            let value = localized_requirement_value(requirement, locale)?;
            content.push_str(&format!(
                "\"{}\" = \"{}\";\n",
                requirement.key,
                escape_strings_value(&value)
            ));
            changed = true;
            locale_changed = true;
        }

        if locale_changed || !strings_path.exists() {
            fs::write(&strings_path, content)
                .with_context(|| format!("Failed to write {}", strings_path.display()))?;
        }
    }
    Ok(changed)
}

fn sync_app_entitlements(
    entitlements_path: &Path,
    granted_entitlements: &[String],
    app_link_hosts: &[String],
) -> Result<bool> {
    let mut dict = if entitlements_path.exists() {
        load_plist_dictionary(entitlements_path)?
    } else {
        plist::Dictionary::new()
    };

    let mut changed = false;
    let desired = desired_apple_entitlements(granted_entitlements);
    let managed = controlled_apple_entitlements()
        .map(str::to_string)
        .collect::<HashSet<_>>();

    let existing_keys = dict.keys().cloned().collect::<Vec<_>>();
    for key in existing_keys {
        if !managed.contains(&key) {
            continue;
        }
        if desired.contains_key(&key) {
            continue;
        }
        if dict.remove(&key).is_some() {
            changed = true;
        }
    }

    for (key, value) in desired {
        let needs_update = !matches!(dict.get(&key), Some(existing) if *existing == value);
        if needs_update {
            dict.insert(key, value);
            changed = true;
        }
    }
    changed |= merge_associated_domains(&mut dict, app_link_hosts)?;

    if changed || !entitlements_path.exists() {
        plist::to_file_xml(entitlements_path, &dict)
            .with_context(|| format!("Failed to write {}", entitlements_path.display()))?;
    }

    Ok(changed)
}

fn required_info_plist_requirements() -> Vec<&'static AppleInfoPlistRequirement> {
    REQUIRED_APPLE_INFO_PLIST_KEYS.iter().collect()
}

fn localized_requirement_value(
    requirement: &AppleInfoPlistRequirement,
    locale: &str,
) -> Result<String> {
    i18n::permission_text(locale, requirement.permission_key).with_context(|| {
        format!(
            "Failed to load permission text for Info.plist key `{}`",
            requirement.key
        )
    })
}

fn desired_info_plist_entries() -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for requirement in required_info_plist_requirements() {
        let value = localized_requirement_value(requirement, i18n::default_locale())?;
        out.insert(requirement.key.to_string(), value.to_string());
    }
    Ok(out)
}

fn managed_info_plist_keys() -> HashSet<String> {
    required_info_plist_requirements()
        .into_iter()
        .map(|requirement| requirement.key.to_string())
        .collect()
}

fn desired_apple_entitlements(granted_entitlements: &[String]) -> BTreeMap<String, Value> {
    let granted = granted_entitlements
        .iter()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .collect::<HashSet<_>>();

    let mut out = BTreeMap::new();
    for entitlement in controlled_apple_entitlements() {
        if granted.contains(entitlement) {
            out.insert(entitlement.to_string(), Value::Boolean(true));
        }
    }
    out
}

fn desired_applink_domains(app_link_hosts: &[String]) -> Vec<String> {
    app_link_hosts
        .iter()
        .map(|host| host.trim())
        .filter(|host| !host.is_empty())
        .map(|host| format!("applinks:{host}"))
        .collect()
}

fn merge_associated_domains(
    dict: &mut plist::Dictionary,
    app_link_hosts: &[String],
) -> Result<bool> {
    let desired = desired_applink_domains(app_link_hosts);
    if desired.is_empty() {
        return Ok(false);
    }

    let mut domains = match dict.remove(ASSOCIATED_DOMAINS_ENTITLEMENT) {
        Some(Value::Array(values)) => values,
        Some(_) => {
            return Err(anyhow!(
                "{} must be an array in App.entitlements",
                ASSOCIATED_DOMAINS_ENTITLEMENT
            ));
        }
        None => Vec::new(),
    };

    let mut existing = domains
        .iter()
        .filter_map(Value::as_string)
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();
    let mut changed = false;
    for domain in desired {
        if existing.insert(domain.clone()) {
            domains.push(Value::String(domain));
            changed = true;
        }
    }

    dict.insert(
        ASSOCIATED_DOMAINS_ENTITLEMENT.to_string(),
        Value::Array(domains),
    );
    Ok(changed)
}

fn load_plist_dictionary(path: &Path) -> Result<plist::Dictionary> {
    let value: Value =
        plist::from_file(path).with_context(|| format!("Failed to parse {}", path.display()))?;
    value
        .into_dictionary()
        .ok_or_else(|| anyhow!("{} must contain a plist dictionary", path.display()))
}

fn escape_strings_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{ASSOCIATED_DOMAINS_ENTITLEMENT, Value, merge_associated_domains};

    fn associated_domains(dict: &plist::Dictionary) -> Vec<String> {
        dict.get(ASSOCIATED_DOMAINS_ENTITLEMENT)
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .filter_map(Value::as_string)
            .map(ToOwned::to_owned)
            .collect()
    }

    #[test]
    fn associated_domain_sync_preserves_existing_domains() {
        let mut dict = plist::Dictionary::new();
        dict.insert(
            ASSOCIATED_DOMAINS_ENTITLEMENT.to_string(),
            Value::Array(vec![
                Value::String("webcredentials:example.com".to_string()),
                Value::String("applinks:old.example.com".to_string()),
            ]),
        );

        let changed = merge_associated_domains(&mut dict, &["new.example.com".to_string()])
            .expect("merge associated domains");

        assert!(changed);
        assert_eq!(
            associated_domains(&dict),
            vec![
                "webcredentials:example.com",
                "applinks:old.example.com",
                "applinks:new.example.com",
            ]
        );
    }

    #[test]
    fn associated_domain_sync_is_noop_without_hosts() {
        let mut dict = plist::Dictionary::new();
        dict.insert(
            ASSOCIATED_DOMAINS_ENTITLEMENT.to_string(),
            Value::Array(vec![Value::String(
                "webcredentials:example.com".to_string(),
            )]),
        );

        let changed = merge_associated_domains(&mut dict, &[]).expect("merge associated domains");

        assert!(!changed);
        assert_eq!(
            associated_domains(&dict),
            vec!["webcredentials:example.com"]
        );
    }
}
